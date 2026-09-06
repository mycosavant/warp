//! `agent.trace` — the record of a conversation, served from inside the
//! instance (board item 6, phase 3).
//!
//! `warpctrl agent trace` reads two files and needs no running Warp: Warp's
//! event log for the conversation and the agent's own session file, joined on
//! the `toolu_…` id (`crates/warp_cli/src/local_control/trace.rs`, which owns
//! the merge and is called from here unchanged). A phone has neither file.
//! This action runs the same merge in the process that has both, so the
//! console can draw a run as it happens.
//!
//! # What the instance knows that the CLI has to be told
//!
//! On Windows the agent runs inside a distribution and its session file is at
//! `/home/<user>/.claude/projects/<slug>/<session>.jsonl` *there*. The CLI
//! takes `--harness-dir` because it has no session to ask; this handler has
//! the conversation's pane, so it asks the session for the native spelling of
//! guest paths (`session::filesystem::native_path`, the T20.1 seam) and looks
//! in the guest's home directories. The guest's `$HOME` is not on the wire
//! anywhere, so [`guest_homes`] guesses it from the session directory first
//! (`/home/effatha/git/warp` puts `/home/effatha` at the head of the list)
//! and then lists `/home`; `WARP_FORK_HARNESS_DIR` overrides the search.
//! The header's `harness_file` says which was used and its `note` says where
//! the search looked when nothing was found.
//!
//! # Polling
//!
//! Both files are append-only and both parsers count non-empty lines, so a
//! line count is an exact cursor. `warp_after` and `harness_after` ask for the
//! tail only; the header's `warp_lines` and `harness_lines` are then totals
//! over the file and are the cursors for the next call. A client merging the
//! tails itself has to order by time the way the CLI does, with the same
//! caveat: the two files are stamped by two clocks.
//!
//! # What it needs, and says so
//!
//! Warp's log exists only if `WARP_FORK_EVENT_LOG` was on when the
//! conversation ran, and the log is where the join key comes from. With the
//! log off there is no file and no way to find the harness's, and the
//! refusal names the variable. The launcher's product profile turns the log
//! on since this handler exists, because a log with a reader is no longer a
//! cost with nothing against it.
use std::path::{Path, PathBuf};

use ::local_control::protocol::AgentTraceParams;
use ::local_control::{ActionKind, ControlError, ErrorCode, InstanceId};
use warp_cli::local_control::trace::{self, Trace};
use warpui::{ModelContext, SingletonEntity};

use crate::ai::blocklist::history_model::BlocklistAIHistoryModel;
use crate::local_control::LocalControlBridge;
use crate::local_control::handlers::ack;
use crate::local_control::handlers::agent::{parse_conversation_id, surface_locations};
use crate::terminal::model::session::Session;
use crate::terminal::model::session::filesystem::native_path;

/// Answers `agent.trace`.
pub fn agent_trace(
    instance_id: &Option<InstanceId>,
    params: &serde_json::Value,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: AgentTraceParams = serde_json::from_value(params.clone())
        .map_err(|error| ControlError::new(ErrorCode::InvalidParams, error.to_string()))?;
    // Parsed for its shape only: a conversation that has finished and whose
    // pane is closed still has both files, and the trace is the one read here
    // that does not need the conversation to be live.
    let conversation_id = parse_conversation_id(&params.conversation_id)?;

    let Some(events_dir) = crate::fork::event_log_dir() else {
        return Err(ControlError::new(
            ErrorCode::TargetStateConflict,
            "WARP_FORK_EVENT_LOG is off, so no conversation has a record to trace; set it to \
             `on` and restart Warp",
        ));
    };
    let events_file = events_dir.join(format!("{}.jsonl", params.conversation_id));
    let events = std::fs::read_to_string(&events_file).map_err(|err| {
        ControlError::new(
            ErrorCode::MissingTarget,
            format!(
                "no event log for conversation {} at {}: {err}",
                params.conversation_id,
                events_file.display()
            ),
        )
    })?;

    let (warp_rows, facts) = trace::warp_rows(&trace::tail(&events, params.warp_after));
    // The join key and the cwd are on Warp's first tool line, which a tail
    // may not include; read them off the whole file so a poll for the tail
    // still finds the harness's file.
    let (_, whole_facts) = trace::warp_rows(&events);

    let session = surface_locations(ctx);
    let session = BlocklistAIHistoryModel::as_ref(ctx)
        .terminal_surface_id_for_conversation(&conversation_id)
        .and_then(|id| session.get(&id))
        .and_then(|location| {
            location
                .terminal_view
                .as_ref(ctx)
                .active_session()
                .as_ref(ctx)
                .session(ctx)
        });

    let mut note = None;
    let harness_file = match (&whole_facts.linked_session_id, &whole_facts.cwd) {
        (Some(linked), Some(cwd)) => match locate_harness_file(session.as_deref(), cwd, linked) {
            Ok(path) => Some(path),
            Err(looked) => {
                note = Some(looked);
                None
            }
        },
        (Some(_), None) => {
            note = Some(
                "Warp's log names a linked session but no cwd, so the harness file's directory \
                 cannot be computed."
                    .to_owned(),
            );
            None
        }
        (None, _) => {
            note = Some(
                "Warp's log names no linked session, so there is no harness file to join: this \
                 is Warp's half alone."
                    .to_owned(),
            );
            None
        }
    };
    let harness = match &harness_file {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => Some(trace::tail(&text, params.harness_after)),
            Err(err) => {
                note = Some(format!(
                    "the harness's session file at {} was found and then not readable: {err}",
                    path.display()
                ));
                None
            }
        },
        None => None,
    };

    let built = trace::build(
        params.conversation_id.clone(),
        events_file,
        warp_rows,
        facts,
        harness_file,
        harness.as_deref(),
        note,
    )
    .after(params.warp_after, params.harness_after);

    let mut response = ack(instance_id, ActionKind::AgentTrace);
    merge(&mut response, &built)?;
    Ok(response)
}

fn merge(response: &mut serde_json::Value, trace: &Trace) -> Result<(), ControlError> {
    let extra = serde_json::to_value(trace)
        .map_err(|error| ControlError::new(ErrorCode::Internal, error.to_string()))?;
    if let (Some(response), Some(extra)) = (response.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            response.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}

/// Where the harness's session file is, or where this looked.
///
/// In order: `WARP_FORK_HARNESS_DIR`; this process's own
/// `~/.claude/projects`; and, for a WSL session, the guest's home
/// directories through the session's path conversion. The first file that
/// exists wins. The `Err` is a sentence for the header's `note`, because a
/// live view that draws Warp's half and says why the other half is missing is
/// more use than a refusal.
fn locate_harness_file(
    session: Option<&Session>,
    cwd: &str,
    linked: &str,
) -> Result<PathBuf, String> {
    let relative = Path::new(&trace::slug(cwd)).join(format!("{linked}.jsonl"));
    let mut looked = Vec::new();

    if let Some(dir) = crate::fork::harness_projects_dir() {
        let candidate = dir.join(&relative);
        if candidate.is_file() {
            return Ok(candidate);
        }
        looked.push(candidate);
    }

    if let Some(home) = std::env::home_dir() {
        let candidate = home.join(".claude").join("projects").join(&relative);
        if candidate.is_file() {
            return Ok(candidate);
        }
        looked.push(candidate);
    }

    if let Some(session) = session.filter(|session| session.is_wsl()) {
        let listing = native_path(session, "/home")
            .and_then(|home| std::fs::read_dir(home).ok())
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for home in guest_homes(cwd, &listing) {
            let projects = format!("{home}/.claude/projects");
            let Some(native) = native_path(session, &projects) else {
                continue;
            };
            let candidate = native.join(&relative);
            if candidate.is_file() {
                return Ok(candidate);
            }
            looked.push(candidate);
        }
    }

    Err(format!(
        "the harness's session file was not found; looked at {}. WARP_FORK_HARNESS_DIR names the \
         `.claude/projects` directory to read instead.",
        looked
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// The guest home directories worth trying, most likely first.
///
/// The session directory's own `/home/<user>` prefix leads, because the
/// agent almost always works under its own home; then every entry of `/home`
/// in the order the directory lists them; then `/root`. Deduplicated so the
/// leading guess is not tried twice.
pub(super) fn guest_homes(cwd: &str, home_listing: &[String]) -> Vec<String> {
    let mut homes = Vec::new();
    if let Some(rest) = cwd.strip_prefix("/home/") {
        let user = rest.split('/').next().unwrap_or_default();
        if !user.is_empty() {
            homes.push(format!("/home/{user}"));
        }
    }
    for entry in home_listing {
        let home = format!("/home/{entry}");
        if !homes.contains(&home) {
            homes.push(home);
        }
    }
    homes.push("/root".to_owned());
    homes
}

#[cfg(test)]
#[path = "trace_tests.rs"]
mod tests;
