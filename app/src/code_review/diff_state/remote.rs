//! Remote diff state model.
//!
//! Client-side model for a single remote repository diff state subscription
//! received from the remote server. Presents the same read API as
//! `LocalDiffStateModel` and emits the same `DiffStateModelEvent` variants.
//!
//! The active [`DiffMode`] can change; the model handles this by unsubscribing
//! from the old `(repo_path, mode)` subscription and re-subscribing with the
//! new mode.

use std::sync::Arc;

use instant::Instant;
use remote_server::manager::{
    CommitMessageOutcome, CreatePrOutcome, RemoteServerManager, RemoteServerManagerEvent,
};
use warp_core::{HostId, SessionId, send_telemetry_from_ctx};
use warp_util::remote_path::RemotePath;
use warp_util::standardized_path::StandardizedPath;
use warpui::{ModelContext, SingletonEntity};

use super::{
    BackendOrigin, CommitChainMode, DiffMetadata, DiffMode, DiffOperation, DiffState,
    DiffStateError, DiffStateModelEvent, DiffStats, FileDiffAndContent, GitDiffData,
    GitDiffWithBaseContent,
};
use crate::code_review::git_actions;
use crate::code_review::telemetry_event::CodeReviewTelemetryEvent;
use crate::fork;
use crate::remote_server::diff_state_proto::{try_decode_file_delta, try_decode_snapshot};
use crate::remote_server::proto;
use crate::server::server_api::ServerApiProvider;
use crate::util::git::{BranchEntry, Commit, FileChangeEntry, PrInfo};

// ── Internal state ────────────────────────────────────────────────

#[derive(Default)]
enum InternalRemoteDiffState {
    #[default]
    Loading,
    NotInRepository,
    Loaded(GitDiffData),
    Error(String),
    /// The remote connection was lost. Preserves stale data until the model
    /// can re-establish the server-side subscription.
    Disconnected,
}

// ── Model ────────────────────────────────────────────────────────────────────

pub struct RemoteDiffStateModel {
    remote_path: RemotePath,
    mode: DiffMode,
    state: InternalRemoteDiffState,
    metadata: Option<DiffMetadata>,
    /// Start time for the latest caller-tracked full diff snapshot request.
    tracked_diff_load_start_time: Option<Instant>,
    /// Set while this side is finishing a PR the daemon deliberately did not
    /// create. See [`PendingPr`].
    pending_pr: Option<PendingPr>,
}

/// A PR the client is completing on the daemon's behalf.
///
/// Under fork policy the daemon returns the *inputs* for a PR title and body
/// rather than generating them, because the model configuration lives in this
/// process. That turns one round trip into two, and this is what has to be
/// remembered across the gap: nothing in the second response says which
/// branch it was for, or which completion event the dialog is waiting on.
struct PendingPr {
    /// Passed to the model as context, and back to the daemon on the create.
    branch: String,
    /// `true` when a commit chain asked for the inputs. The chain's commit and
    /// push have already happened; the dialog is waiting for
    /// [`GitOpResult::CommitChain`], not [`GitOpResult::PrCreated`], and
    /// emitting the wrong one leaves it open.
    from_commit_chain: bool,
}

impl warpui::Entity for RemoteDiffStateModel {
    type Event = DiffStateModelEvent;
}

impl RemoteDiffStateModel {
    /// Creates a new remote diff state model.
    ///
    /// Identity is `(host_id, repo_path, mode)`. The model is session-agnostic:
    /// the manager resolves a connected session for the host on every outbound
    /// RPC, and host-level connect/disconnect events drive subscription
    /// lifecycle.
    ///
    /// `preferred_session` is the session that opened this review (the
    /// triggering callsite). It is used only for the *initial* `GetDiffState`
    /// dispatch and is deliberately not stored: a shared, long-lived model
    /// must not pin a session, and later re-triggers supply their own session
    /// (or `None`) rather than reusing a stale one.
    ///
    /// A session for this host is required at construction time. The model starts in `Loading` and
    /// issues the initial `GetDiffState` request. Runtime disconnects transition the model through
    /// `mark_disconnected`; subsequent reconnects re-subscribe via the `HostConnected` event handler.
    pub fn new(
        remote_path: RemotePath,
        mode: DiffMode,
        preferred_session: Option<SessionId>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        // Subscribe to RemoteServerManager push events and filter by remote_path and diff_mode
        let mgr_handle = RemoteServerManager::handle(ctx);
        ctx.subscribe_to_model(&mgr_handle, |me, _, event, ctx| {
            me.handle_manager_event(event, ctx)
        });

        let host_id = remote_path.host_id.clone();
        let repo_path = remote_path.path.clone();
        let mode_clone = mode.clone();
        mgr_handle.update(ctx, |mgr, ctx| {
            mgr.get_diff_state(
                host_id,
                repo_path,
                proto::DiffMode::from(&mode_clone),
                preferred_session,
                ctx,
            );
        });

        Self {
            remote_path,
            mode,
            state: InternalRemoteDiffState::Loading,
            metadata: None,
            tracked_diff_load_start_time: None,
            pending_pr: None,
        }
    }

    // ── Event handler ───────────────────────────────────────────

    fn matches_remote_path_and_mode(
        &self,
        host_id: &HostId,
        repo_path: &StandardizedPath,
        mode: &proto::DiffMode,
    ) -> bool {
        let remote_mode = proto::DiffMode::from(&self.mode);
        self.remote_path.matches(host_id, repo_path) && mode == &remote_mode
    }

    fn handle_manager_event(
        &mut self,
        event: &RemoteServerManagerEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        match event {
            RemoteServerManagerEvent::DiffStateSnapshotReceived {
                host_id,
                repo_path,
                mode,
                snapshot,
            } => {
                if !self.matches_remote_path_and_mode(host_id, repo_path, mode) {
                    return;
                }
                self.handle_snapshot_received(snapshot, ctx);
            }
            RemoteServerManagerEvent::DiffStateMetadataUpdateReceived {
                host_id,
                repo_path,
                mode,
                update,
            } => {
                if !self.matches_remote_path_and_mode(host_id, repo_path, mode) {
                    return;
                }
                self.handle_metadata_update_received(update, ctx);
            }
            RemoteServerManagerEvent::DiffStateFileDeltaReceived {
                host_id,
                repo_path,
                mode,
                delta,
            } => {
                if !self.matches_remote_path_and_mode(host_id, repo_path, mode) {
                    return;
                }
                self.handle_file_delta_received(delta, ctx);
            }
            RemoteServerManagerEvent::GetBranchesResponse {
                repo_path, result, ..
            } if repo_path == &self.remote_path.path => {
                let branches = match result {
                    Ok(branch_infos) => branch_infos
                        .iter()
                        .map(|info| BranchEntry {
                            name: info.name.clone(),
                            is_main: info.is_main,
                        })
                        .collect(),
                    Err(err) => {
                        log::warn!("RemoteDiffStateModel: GetBranches failed: {err}");
                        vec![]
                    }
                };
                ctx.emit(DiffStateModelEvent::BranchesReceived(branches));
            }
            RemoteServerManagerEvent::CommitChainResponse {
                host_id,
                repo_path,
                result,
            } if self.remote_path.matches(host_id, repo_path) => {
                self.handle_git_commit_chain_response(result, ctx);
            }
            RemoteServerManagerEvent::GitPushResponse {
                host_id,
                repo_path,
                result,
            } if self.remote_path.matches(host_id, repo_path) => {
                self.handle_git_push_response(result, ctx);
            }
            RemoteServerManagerEvent::CreatePrResponse {
                host_id,
                repo_path,
                result,
                branch,
            } if self.remote_path.matches(host_id, repo_path) => {
                let _ = branch;
                self.handle_create_pr_response(result, ctx);
            }
            RemoteServerManagerEvent::GenerateCommitMessageResponse {
                host_id,
                repo_path,
                result,
                branch_name,
            } if self.remote_path.matches(host_id, repo_path) => match result {
                // The daemon ran the model; just relay the result to the dialog.
                Ok(CommitMessageOutcome::Message(message)) => {
                    ctx.emit(DiffStateModelEvent::CommitMessageGenerated(Ok(
                        message.clone()
                    )));
                }
                // We asked for the diff, so the model call is ours to make.
                Ok(CommitMessageOutcome::Diff(diff)) => {
                    self.generate_commit_message_from_remote_diff(
                        diff.clone(),
                        branch_name.clone(),
                        ctx,
                    );
                }
                Err(e) => {
                    ctx.emit(DiffStateModelEvent::CommitMessageGenerated(Err(e.clone())));
                }
            },
            RemoteServerManagerEvent::GetCommittedBranchFilesResponse {
                host_id,
                repo_path,
                result,
            } if self.remote_path.matches(host_id, repo_path) => {
                self.handle_get_committed_branch_files_response(result, ctx);
            }
            RemoteServerManagerEvent::HostDisconnected { host_id }
                if host_id == &self.remote_path.host_id =>
            {
                self.mark_disconnected(ctx);
            }
            RemoteServerManagerEvent::HostConnected { host_id }
                if host_id == &self.remote_path.host_id
                    && matches!(self.state, InternalRemoteDiffState::Disconnected) =>
            {
                // Reconnect is event-driven with no viewing-session in scope
                // (and the prior session may be gone), so re-subscribe over
                // any connected session for the host.
                self.resubscribe(false, None, ctx);
            }
            _ => {}
        }
    }

    /// Marks the model as disconnected, preserving any stale data and
    /// emitting `ConnectionLost`.
    fn mark_disconnected(&mut self, ctx: &mut ModelContext<Self>) {
        if matches!(self.state, InternalRemoteDiffState::Disconnected) {
            return;
        }
        self.tracked_diff_load_start_time = None;
        self.state = InternalRemoteDiffState::Disconnected;
        ctx.emit(DiffStateModelEvent::ConnectionLost);
    }

    /// Re-sends `GetDiffState` for this model's `(host_id, repo, mode)` and
    /// transitions to `Loading` while waiting for a fresh snapshot.
    ///
    /// `preferred_session` is supplied by the triggering callsite (the
    /// session-scoped view) so the request rides the connection that needs the
    /// result; `None` (e.g. reconnect) falls back to any connected session.
    fn resubscribe(
        &mut self,
        track_load_duration: bool,
        preferred_session: Option<SessionId>,
        ctx: &mut ModelContext<Self>,
    ) {
        // Always overwrite to avoid carrying a stale `Instant` from a prior
        // tracked load that was interrupted by a session blip.
        self.tracked_diff_load_start_time = track_load_duration.then(Instant::now);
        let host_id = self.remote_path.host_id.clone();
        let repo_path = self.remote_path.path.clone();
        let mode = self.mode.clone();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.get_diff_state(
                host_id,
                repo_path,
                proto::DiffMode::from(&mode),
                preferred_session,
                ctx,
            );
        });
        self.state = InternalRemoteDiffState::Loading;
        ctx.emit(DiffStateModelEvent::NewDiffsComputed {
            diffs: None,
            load_duration: None,
        });
    }

    // ── Proto → state conversion helpers ────────────────────────────────────────────────

    fn handle_snapshot_received(
        &mut self,
        snapshot: &proto::DiffStateSnapshot,
        ctx: &mut ModelContext<Self>,
    ) {
        match try_decode_snapshot(snapshot) {
            Ok((metadata, state, diffs)) => self.apply_snapshot(metadata, state, diffs, ctx),
            Err(error) => {
                self.tracked_diff_load_start_time = None;
                warp_core::safe_error!(
                    safe: ("RemoteDiffStateModel: failed to decode diff state snapshot"),
                    full: ("RemoteDiffStateModel: failed to decode diff state snapshot: {error}")
                );
            }
        }
    }

    fn handle_metadata_update_received(
        &mut self,
        update: &proto::DiffStateMetadataUpdate,
        ctx: &mut ModelContext<Self>,
    ) {
        match update
            .metadata
            .as_ref()
            .map(DiffMetadata::try_from)
            .transpose()
        {
            Ok(Some(metadata)) => self.apply_metadata_update(&metadata, ctx),
            Ok(None) => {}
            Err(error) => {
                warp_core::safe_error!(
                    safe: ("RemoteDiffStateModel: failed to decode diff state metadata update"),
                    full: ("RemoteDiffStateModel: failed to decode diff state metadata update: {error}")
                );
            }
        }
    }

    fn handle_file_delta_received(
        &mut self,
        delta: &proto::DiffStateFileDelta,
        ctx: &mut ModelContext<Self>,
    ) {
        match try_decode_file_delta(delta) {
            Ok((file_path, diff, metadata)) => {
                self.apply_file_delta(file_path, diff, metadata, ctx)
            }
            Err(error) => {
                warp_core::safe_error!(
                    safe: ("RemoteDiffStateModel: failed to decode diff state file delta"),
                    full: ("RemoteDiffStateModel: failed to decode diff state file delta: {error}")
                );
            }
        }
    }

    // ── Apply methods ──────────────────────────────────────────────────────

    /// Requests a fresh diff snapshot from the remote server, including file
    /// content. Unlike the former `replay_latest_diffs` (which reconstructed
    /// data from cached `GitDiffData` and lost `content_at_head`), this sends
    /// an actual `GetDiffState` RPC so the server can reload content from disk.
    ///
    /// Does NOT transition to `Loading` or emit `NewDiffsComputed(None)` first,
    /// so existing views subscribed to this model won't flash a loading state.
    /// The server response arrives as a `DiffStateSnapshotReceived` event and
    /// flows through `apply_snapshot` normally.
    pub(crate) fn fetch_fresh_snapshot(
        &mut self,
        track_load_duration: bool,
        preferred_session: Option<SessionId>,
        ctx: &mut ModelContext<Self>,
    ) {
        if track_load_duration {
            self.tracked_diff_load_start_time = Some(Instant::now());
        }
        let host_id = self.remote_path.host_id.clone();
        let repo_path = self.remote_path.path.clone();
        let mode = self.mode.clone();
        // `preferred_session` is supplied per-call by the triggering view (the
        // session showing the review); `None` falls back to any connected
        // session for the host. Never cached on this shared model.
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.get_diff_state(
                host_id,
                repo_path,
                proto::DiffMode::from(&mode),
                preferred_session,
                ctx,
            );
        });
    }

    fn apply_snapshot(
        &mut self,
        metadata: Option<DiffMetadata>,
        state: DiffState,
        diffs: Option<GitDiffWithBaseContent>,
        ctx: &mut ModelContext<Self>,
    ) {
        // Update metadata, detecting branch changes.
        if let Some(metadata) = &metadata {
            self.apply_metadata_update(metadata, ctx);
        }

        // Update state.
        match state {
            // Disconnected is never produced by proto deserialization.
            DiffState::Disconnected => {}
            DiffState::NotInRepository => {
                self.tracked_diff_load_start_time = None;
                self.state = InternalRemoteDiffState::NotInRepository;
                ctx.emit(DiffStateModelEvent::NewDiffsComputed {
                    diffs: None,
                    load_duration: None,
                });
            }
            DiffState::Loading => {
                self.state = InternalRemoteDiffState::Loading;
                ctx.emit(DiffStateModelEvent::NewDiffsComputed {
                    diffs: None,
                    load_duration: None,
                });
            }
            DiffState::Error(msg) => {
                let load_duration = self
                    .tracked_diff_load_start_time
                    .take()
                    .map(|start| start.elapsed());
                let err = DiffStateError::from_message(&msg);
                err.report_and_log();
                send_telemetry_from_ctx!(
                    CodeReviewTelemetryEvent::LoadDiffFailed {
                        backend_origin: BackendOrigin::ClientRemote,
                        operation: DiffOperation::RemoteDiff,
                        mode: self.mode.clone(),
                        error: err.to_string(),
                        load_duration,
                    },
                    ctx
                );
                self.state = InternalRemoteDiffState::Error(msg);
                ctx.emit(DiffStateModelEvent::NewDiffsComputed {
                    diffs: None,
                    load_duration: None,
                });
            }
            DiffState::Loaded => {
                let Some(base_content) = diffs else {
                    let load_duration = self
                        .tracked_diff_load_start_time
                        .take()
                        .map(|start| start.elapsed());
                    let err = DiffStateError::empty_diff_data();
                    err.report_and_log();
                    send_telemetry_from_ctx!(
                        CodeReviewTelemetryEvent::LoadDiffFailed {
                            backend_origin: BackendOrigin::ClientRemote,
                            operation: DiffOperation::RemoteDiff,
                            mode: self.mode.clone(),
                            error: err.to_string(),
                            load_duration,
                        },
                        ctx
                    );
                    self.state = InternalRemoteDiffState::Error(err.to_string());
                    ctx.emit(DiffStateModelEvent::NewDiffsComputed {
                        diffs: None,
                        load_duration: None,
                    });
                    return;
                };
                let diffs = GitDiffData::from(&base_content);
                let load_duration = self
                    .tracked_diff_load_start_time
                    .take()
                    .map(|start| start.elapsed());
                self.state = InternalRemoteDiffState::Loaded(diffs);
                ctx.emit(DiffStateModelEvent::NewDiffsComputed {
                    diffs: Some(Arc::new(base_content)),
                    load_duration,
                });
            }
        }
    }

    fn apply_metadata_update(&mut self, metadata: &DiffMetadata, ctx: &mut ModelContext<Self>) {
        let previous_branch = self
            .metadata
            .as_ref()
            .map(|m| m.current_branch_name.as_str());
        let branch_changed =
            previous_branch.is_some_and(|prev| prev != metadata.current_branch_name.as_str());

        let metadata = metadata.clone();
        self.metadata = Some(metadata.clone());

        // Only emit CurrentBranchChanged when there was a previous branch to
        // compare against. On the first metadata update (initial snapshot)
        // previous_branch is None — that's initial population, not a switch.
        if branch_changed {
            ctx.emit(DiffStateModelEvent::CurrentBranchChanged);
        }
        ctx.emit(DiffStateModelEvent::MetadataRefreshed(Box::new(metadata)));
    }

    fn apply_file_delta(
        &mut self,
        file_path: String,
        diff: Option<FileDiffAndContent>,
        metadata: Option<DiffMetadata>,
        ctx: &mut ModelContext<Self>,
    ) {
        if let Some(metadata) = &metadata {
            self.apply_metadata_update(metadata, ctx);
        }

        let InternalRemoteDiffState::Loaded(ref mut diffs) = self.state else {
            // Ignore file deltas until the initial snapshot has loaded.
            return;
        };

        if let Some(ref new_diff) = diff {
            if let Some(pos) = diffs.files.iter().position(|f| f.file_path == file_path) {
                diffs.files[pos] = new_diff.file_diff.clone();
            } else {
                diffs.files.push(new_diff.file_diff.clone());
            }
        } else {
            diffs.files.retain(|f| f.file_path != file_path);
        }
        diffs.total_additions = diffs.files.iter().map(|f| f.additions()).sum();
        diffs.total_deletions = diffs.files.iter().map(|f| f.deletions()).sum();
        diffs.files_changed = diffs.files.len();
        ctx.emit(DiffStateModelEvent::SingleFileUpdated {
            path: file_path,
            diff: diff.map(Arc::new),
        });
    }

    // ── Cleanup ──────────────────────────────────────────────────────

    /// Sends `UnsubscribeDiffState` to the server. Call before dropping the
    /// model (the wrapper calls it during mode switch / pane close).
    pub fn unsubscribe(&self, ctx: &mut ModelContext<Self>) {
        RemoteServerManager::handle(ctx)
            .as_ref(ctx)
            .unsubscribe_diff_state(
                self.remote_path.host_id.clone(),
                &self.remote_path.path,
                proto::DiffMode::from(&self.mode),
            );
    }

    // ── Read API (matching LocalDiffStateModel interface) ────────────

    pub fn get(&self) -> DiffState {
        match &self.state {
            InternalRemoteDiffState::NotInRepository => DiffState::NotInRepository,
            InternalRemoteDiffState::Loading => DiffState::Loading,
            InternalRemoteDiffState::Loaded(_) => DiffState::Loaded,
            InternalRemoteDiffState::Error(msg) => DiffState::Error(msg.clone()),
            InternalRemoteDiffState::Disconnected => DiffState::Disconnected,
        }
    }

    pub fn diff_mode(&self) -> DiffMode {
        self.mode.clone()
    }

    pub fn get_uncommitted_stats(&self) -> Option<DiffStats> {
        self.metadata
            .as_ref()
            .map(|m| m.against_head.aggregate_stats)
    }

    /// Per-file entries for uncommitted-vs-HEAD changes, from synced metadata.
    pub fn uncommitted_file_entries(&self) -> &[FileChangeEntry] {
        self.metadata
            .as_ref()
            .map(|m| m.against_head.files.as_slice())
            .unwrap_or(&[])
    }

    pub fn get_main_branch_name(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .map(|m| m.main_branch_name.clone())
            .filter(|s| !s.is_empty())
    }

    pub fn get_current_branch_name(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .map(|m| m.current_branch_name.clone())
            .filter(|s| !s.is_empty())
    }

    pub fn is_on_main_branch(&self) -> bool {
        self.metadata.as_ref().is_some_and(|m| {
            !m.current_branch_name.is_empty() && m.current_branch_name == m.main_branch_name
        })
    }

    pub fn unpushed_commits(&self) -> &[Commit] {
        self.metadata
            .as_ref()
            .map(|m| m.unpushed_commits.as_slice())
            .unwrap_or(&[])
    }

    pub fn upstream_ref(&self) -> Option<&str> {
        self.metadata
            .as_ref()
            .and_then(|m| m.upstream_ref.as_deref())
    }

    pub fn upstream_differs_from_main(&self) -> bool {
        match (self.upstream_ref(), self.get_main_branch_name().as_deref()) {
            (Some(upstream), Some(main)) => upstream != main,
            _ => false,
        }
    }

    pub fn has_head(&self) -> bool {
        self.metadata.as_ref().is_some_and(|m| m.has_head_commit)
    }

    pub fn remote_path(&self) -> RemotePath {
        self.remote_path.clone()
    }

    // ── Git operation event handlers ─────────────────────────────────

    /// Converts a proto `GitOpDelta` to domain types and applies it through
    /// the shared `apply_git_op_delta` (the single delta-application path), so
    /// the proto-driven and domain-driven callers stay in sync.
    fn apply_delta_from_proto(
        &mut self,
        delta: &remote_server::proto::GitOpDelta,
        ctx: &mut ModelContext<Self>,
    ) {
        let commits = delta.unpushed_commits.iter().map(Commit::from).collect();
        self.apply_git_op_delta(commits, delta.upstream_ref.clone(), ctx);
    }

    fn handle_git_commit_chain_response(
        &mut self,
        result: &Result<remote_server::manager::CommitChainSuccess, String>,
        ctx: &mut ModelContext<Self>,
    ) {
        let domain_result = match result {
            Ok(success) => {
                // Apply the delta before emitting the completion event so the
                // header updates immediately. PR info is returned in the event
                // result and refreshed through the shared `GitHubRepoModel`.
                let commits = success
                    .delta
                    .unpushed_commits
                    .iter()
                    .map(Commit::from)
                    .collect();
                let pr_info = success.pr_info.as_ref().map(PrInfo::from);
                let metadata = self.metadata.get_or_insert_with(DiffMetadata::default);
                metadata.unpushed_commits = commits;
                metadata.upstream_ref = success.delta.upstream_ref.clone();
                ctx.emit(DiffStateModelEvent::MetadataRefreshed(Box::new(
                    metadata.clone(),
                )));
                // The commit and the push landed; the PR did not, because we
                // asked for its inputs instead. Refresh the header — that part
                // is real — but hold the completion event until the PR exists.
                if let Some(inputs) = success.pr_inputs.as_ref() {
                    let Some(branch) = self
                        .pending_pr
                        .as_ref()
                        .map(|pending| pending.branch.clone())
                    else {
                        log::error!(
                            "RemoteDiffStateModel: chain returned PR inputs with no pending PR"
                        );
                        ctx.emit(DiffStateModelEvent::GitOpCompleted(
                            super::GitOpResult::CommitChainCompleted(Err(
                                "committed and pushed, but the PR request was lost".to_string(),
                            )),
                        ));
                        return;
                    };
                    self.generate_pr_content_and_create(
                        git_actions::PrContentInputs {
                            diff: inputs.diff.clone(),
                            commit_messages: inputs.commit_messages.clone(),
                        },
                        branch,
                        ctx,
                    );
                    return;
                }
                Ok(pr_info)
            }
            Err(msg) => Err(msg.clone()),
        };
        ctx.emit(DiffStateModelEvent::GitOpCompleted(
            super::GitOpResult::CommitChainCompleted(domain_result),
        ));
    }

    fn handle_git_push_response(
        &mut self,
        result: &Result<remote_server::proto::GitOpDelta, String>,
        ctx: &mut ModelContext<Self>,
    ) {
        let domain_result = match result {
            Ok(delta) => {
                self.apply_delta_from_proto(delta, ctx);
                Ok(())
            }
            Err(msg) => Err(msg.clone()),
        };
        ctx.emit(DiffStateModelEvent::GitOpCompleted(
            super::GitOpResult::PushCompleted(domain_result),
        ));
    }

    fn handle_create_pr_response(
        &mut self,
        result: &Result<CreatePrOutcome, String>,
        ctx: &mut ModelContext<Self>,
    ) {
        // The daemon returned the inputs instead of a PR: generate here and
        // come back. Nothing is emitted yet — the dialog is still waiting.
        if let Ok(CreatePrOutcome::Inputs(inputs)) = result {
            let Some(pending) = self.pending_pr.as_ref() else {
                // Inputs nobody asked for. Emitting an error is better than
                // silence, which would leave the dialog spinning forever.
                log::error!("RemoteDiffStateModel: PR inputs arrived with no pending PR");
                ctx.emit(DiffStateModelEvent::GitOpCompleted(
                    super::GitOpResult::PrCreated(Err(
                        "received PR content with no request outstanding".to_string(),
                    )),
                ));
                return;
            };
            let branch = pending.branch.clone();
            self.generate_pr_content_and_create(
                git_actions::PrContentInputs {
                    diff: inputs.diff.clone(),
                    commit_messages: inputs.commit_messages.clone(),
                },
                branch,
                ctx,
            );
            return;
        }

        let domain_result = match result {
            Ok(CreatePrOutcome::Created(proto_pr)) => Ok(PrInfo::from(proto_pr)),
            Ok(CreatePrOutcome::Inputs(_)) => unreachable!("handled above"),
            Err(msg) => Err(msg.clone()),
        };
        // A PR that finishes a commit chain has to complete the *chain*, or
        // the dialog keeps waiting for an event that never comes.
        let from_commit_chain = self
            .pending_pr
            .take()
            .is_some_and(|pending| pending.from_commit_chain);
        if from_commit_chain {
            ctx.emit(DiffStateModelEvent::GitOpCompleted(
                super::GitOpResult::CommitChainCompleted(domain_result.map(Some)),
            ));
            return;
        }
        ctx.emit(DiffStateModelEvent::GitOpCompleted(
            super::GitOpResult::PrCreated(domain_result),
        ));
    }

    /// Handles a `GetCommittedBranchFilesResponse`: converts the proto entries
    /// to domain types and emits `BranchCommittedFilesReceived` for the Create
    /// PR dialog's Changes box. On error, logs and emits an empty list so the
    /// dialog renders an empty box rather than showing stale data.
    fn handle_get_committed_branch_files_response(
        &self,
        result: &Result<Vec<remote_server::proto::FileChangeEntry>, String>,
        ctx: &mut ModelContext<Self>,
    ) {
        let files = match result {
            Ok(files) => files.iter().map(FileChangeEntry::from).collect(),
            Err(msg) => {
                log::warn!("RemoteDiffStateModel: GetCommittedBranchFiles failed: {msg}");
                Vec::new()
            }
        };
        ctx.emit(DiffStateModelEvent::BranchCommittedFilesReceived(files));
    }

    // ── Remote git operations (async; results arrive via manager events) ──
    //
    // Each dispatches via `RemoteServerManager` and returns immediately; the
    // response lands as a manager event in `handle_manager_event`, which
    // converts it into the corresponding `DiffStateModelEvent`.

    /// Runs a commit chain via the remote server manager. The result
    /// arrives as a `CommitChainResponse` manager event, handled above.
    #[allow(clippy::too_many_arguments)]
    pub fn git_commit_chain(
        &mut self,
        mode: CommitChainMode,
        message: String,
        include_unstaged: bool,
        branch: String,
        autogenerate_pr_content: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        // The chain's PR stage has the same problem as the standalone create,
        // plus an ordering constraint: the PR diff is taken against
        // `origin/<branch>`, so the inputs cannot be computed until the
        // chain's own commit and push have landed. The daemon therefore runs
        // the chain, stops before the PR, and hands the inputs back.
        let generate_here = matches!(mode, CommitChainMode::CommitAndCreatePr)
            && autogenerate_pr_content
            && fork::remote_pr_content_generated_locally();
        if generate_here {
            self.pending_pr = Some(PendingPr {
                branch: branch.clone(),
                from_commit_chain: true,
            });
        }
        let host_id = self.remote_path.host_id.clone();
        let repo_path = self.remote_path.path.clone();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.git_commit_chain(
                host_id,
                repo_path,
                proto::GitCommitChainMode::from(&mode),
                message,
                include_unstaged,
                branch,
                autogenerate_pr_content,
                generate_here,
                ctx,
            );
        });
    }

    /// Issues an AI commit-message generation request via the remote server
    /// manager. The result arrives as a `GenerateCommitMessageResponse`
    /// manager event, handled in `handle_manager_event`.
    ///
    /// Under fork policy it asks for the *diff* and generates here — see
    /// [`fork::remote_commit_message_generated_locally`], which carries the
    /// reason.
    pub fn generate_commit_message(
        &self,
        include_unstaged: bool,
        branch_name: String,
        ctx: &mut ModelContext<Self>,
    ) {
        let host_id = self.remote_path.host_id.clone();
        let repo_path = self.remote_path.path.clone();
        let return_diff_only = fork::remote_commit_message_generated_locally();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.git_generate_commit_message(
                host_id,
                repo_path,
                include_unstaged,
                branch_name,
                return_diff_only,
                ctx,
            );
        });
    }

    /// Generates the commit message here, from a diff the daemon computed.
    ///
    /// Reached only when the request set `return_diff_only`. The AI client is
    /// this process's, so `ai::local_completion` resolves the user's endpoint
    /// and model — the configuration the daemon does not have.
    fn generate_commit_message_from_remote_diff(
        &self,
        diff: String,
        branch_name: String,
        ctx: &mut ModelContext<Self>,
    ) {
        let ai_client = ServerApiProvider::handle(ctx).as_ref(ctx).get_ai_client();
        ctx.spawn(
            async move {
                git_actions::generate_commit_message_from_diff(
                    diff,
                    &branch_name,
                    ai_client.as_ref(),
                )
                .await
            },
            |_me, result, ctx| {
                ctx.emit(DiffStateModelEvent::CommitMessageGenerated(
                    result.map_err(|e| e.to_string()),
                ));
            },
        );
    }

    /// Fetches the committed branch files (`merge_base(HEAD, main)..HEAD`) for
    /// the current branch via the remote `GitGetCommittedBranchFiles` RPC. The
    /// result arrives as a `GetCommittedBranchFilesResponse` manager event,
    /// handled in `handle_manager_event`, which emits
    /// `BranchCommittedFilesReceived` for the Create PR dialog.
    pub fn fetch_committed_branch_files(&self, ctx: &mut ModelContext<Self>) {
        let host_id = self.remote_path.host_id.clone();
        let repo_path = self.remote_path.path.clone();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.git_get_committed_branch_files(host_id, repo_path, ctx);
        });
    }

    /// Pushes the branch via the remote server manager.
    pub fn git_push(&self, branch: String, ctx: &mut ModelContext<Self>) {
        let host_id = self.remote_path.host_id.clone();
        let repo_path = self.remote_path.path.clone();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.git_push_branch(host_id, repo_path, branch, ctx);
        });
    }

    /// Creates a PR via the remote server manager. When `autogenerate_content`
    /// is set, the daemon AI-generates the PR title/body (falling back to
    /// `gh pr create --fill`); `branch` is passed as context for that generation.
    #[allow(clippy::too_many_arguments)]
    pub fn create_pr(
        &mut self,
        branch: String,
        autogenerate_content: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        // Under fork policy, ask for the inputs and generate here. Only when
        // the caller wanted generated content at all: a plain create still
        // goes in one trip and still gets `gh pr create --fill`.
        let generate_here = autogenerate_content && fork::remote_pr_content_generated_locally();
        if generate_here {
            self.pending_pr = Some(PendingPr {
                branch: branch.clone(),
                from_commit_chain: false,
            });
        }
        let host_id = self.remote_path.host_id.clone();
        let repo_path = self.remote_path.path.clone();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.git_create_pr(
                host_id,
                repo_path,
                branch,
                autogenerate_content,
                None,
                generate_here,
                ctx,
            );
        });
    }

    /// Asks the daemon to create the PR, with content this side generated.
    /// The second of the two trips; `autogenerate_content` is false because
    /// the generating is done.
    fn create_pr_with_content(
        &self,
        branch: String,
        content: Option<(String, String)>,
        ctx: &mut ModelContext<Self>,
    ) {
        let host_id = self.remote_path.host_id.clone();
        let repo_path = self.remote_path.path.clone();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.git_create_pr(host_id, repo_path, branch, false, content, false, ctx);
        });
    }

    /// Generates a PR title and body here, from inputs the daemon computed,
    /// then asks it to create the PR.
    ///
    /// A generation failure is **not** an error: it sends the create with no
    /// content, which is `gh pr create --fill`. That is the same fallback the
    /// daemon-side generator has always had, kept because losing the PR
    /// entirely because a model was unreachable would be a worse trade than
    /// a title git wrote.
    fn generate_pr_content_and_create(
        &self,
        inputs: git_actions::PrContentInputs,
        branch: String,
        ctx: &mut ModelContext<Self>,
    ) {
        let ai_client = ServerApiProvider::handle(ctx).as_ref(ctx).get_ai_client();
        let branch_for_generate = branch.clone();
        ctx.spawn(
            async move {
                git_actions::generate_pr_content(inputs, &branch_for_generate, ai_client.as_ref())
                    .await
            },
            move |me, result, ctx| {
                let content = match result {
                    Ok(content) => Some((content.title, content.body)),
                    Err(err) => {
                        log::warn!(
                            "RemoteDiffStateModel: PR content generation failed, \
                             falling back to --fill: {err}"
                        );
                        None
                    }
                };
                me.create_pr_with_content(branch, content, ctx);
            },
        );
    }

    // ── Write API ────────────────────────────────────────────────────

    pub fn set_diff_mode(
        &mut self,
        mode: DiffMode,
        track_load_duration: bool,
        preferred_session: Option<SessionId>,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.mode == mode {
            return;
        }

        // Unsubscribe from the old mode before switching, then re-send
        // GetDiffState for the new mode over `preferred_session` (the
        // triggering view's session) when provided, else any connected
        // session for the host.
        self.unsubscribe(ctx);
        self.mode = mode;
        self.resubscribe(track_load_duration, preferred_session, ctx);
    }

    /// Fetches branches for the remote repository via the `GetBranches` RPC.
    /// The response is handled in `handle_manager_event` which emits
    /// `DiffStateModelEvent::BranchesReceived`.
    pub fn fetch_branches(&self, ctx: &mut ModelContext<Self>) {
        let host_id = self.remote_path.host_id.clone();
        let repo_path = self.remote_path.path.clone();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.get_branches(host_id, repo_path, None, false, ctx);
        });
    }

    /// Sends a `DiscardFiles` request to the remote server.
    /// The server's watcher will push updated diff snapshots on success.
    pub fn discard_files(
        &self,
        file_infos: Vec<super::FileStatusInfo>,
        should_stash: bool,
        branch_name: Option<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        let host_id = self.remote_path.host_id.clone();
        let repo_path = self.remote_path.path.clone();
        let mode = self.mode.clone();
        let proto_files = file_infos.iter().map(proto::FileStatusInfo::from).collect();
        RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
            mgr.discard_files(
                host_id,
                repo_path,
                proto_files,
                should_stash,
                branch_name,
                proto::DiffMode::from(&mode),
                ctx,
            );
        });
    }

    /// Applies a post-git-operation delta (refreshed unpushed commits +
    /// upstream ref returned by the daemon) to the cached metadata and emits
    /// `MetadataRefreshed`, so the code review header updates immediately
    /// rather than waiting for the next server-pushed snapshot.
    pub fn apply_git_op_delta(
        &mut self,
        unpushed_commits: Vec<Commit>,
        upstream_ref: Option<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        let metadata = self.metadata.get_or_insert_with(DiffMetadata::default);
        metadata.unpushed_commits = unpushed_commits;
        metadata.upstream_ref = upstream_ref;
        ctx.emit(DiffStateModelEvent::MetadataRefreshed(Box::new(
            metadata.clone(),
        )));
    }
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod remote_tests;
