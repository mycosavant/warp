//! Bridge between protocol-level control requests and Warp application models.
//!
//! The bridge validates protocol version, selectors, credentials, and settings
//! before routing each supported action to an app-side handler.

use ::local_control::auth::CredentialGrant;
use ::local_control::{
    Action, ActionKind, ControlError, ErrorCode, InstanceId, RequestEnvelope, ResponseEnvelope,
};
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::local_control::handlers::{
    agent, app_state, approvals, close, drive_objects, drive_sync, events, main_pane, metadata,
    metadata_config, pairing, remote_wsl, settings_surfaces, visor,
};
use crate::local_control::permissions::{
    ensure_action_allowed, ensure_feature_enabled, ensure_protocol_version,
};
use crate::local_control::resolver::{validate_action_params, validate_action_target};

/// WarpUI model that executes already-authenticated local-control actions.
pub struct LocalControlBridge {
    instance_id: Option<InstanceId>,
    /// `host:port` this instance's control server is answering on.
    ///
    /// Held so `events.subscribe` can hand a client an absolute URL rather than
    /// have it reassemble one from a discovery record (T11.2). Set from the same
    /// place as `instance_id`, where both are already known.
    control_origin: Option<String>,
    /// Where `control.pair` mints into, and the address it tells a device to
    /// come back to (T11.4).
    ///
    /// `None` covers two different situations that want the same answer: the
    /// server has not started yet, and the server started with no wide listener.
    /// Both mean "there is nothing to pair with", which is what the handler
    /// says.
    pairing: Option<PairingContext>,
}

/// What `control.pair` needs that only the server knows.
#[derive(Clone)]
pub(super) struct PairingContext {
    pub(super) pairings: std::sync::Arc<std::sync::Mutex<super::pairing::Pairings>>,
    /// `host:port` of the *wide* listener, not the loopback one. A pairing URL
    /// pointing at `127.0.0.1` would be a QR that only works on the machine
    /// displaying it.
    pub(super) origin: String,
}

impl Entity for LocalControlBridge {
    type Event = ();
}

impl SingletonEntity for LocalControlBridge {}

impl LocalControlBridge {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self {
            instance_id: None,
            control_origin: None,
            pairing: None,
        }
    }

    pub(super) fn set_instance_id(&mut self, instance_id: InstanceId) {
        self.instance_id = Some(instance_id);
    }

    pub(super) fn set_control_origin(&mut self, origin: String) {
        self.control_origin = Some(origin);
    }

    /// Installs the pairing state and wide address, or clears both (T11.4).
    ///
    /// Takes both halves at once because neither is usable alone: a map with no
    /// address mints codes pointing nowhere, and an address with no map has
    /// nothing to check what arrives.
    pub(super) fn set_pairing(
        &mut self,
        pairings: Option<std::sync::Arc<std::sync::Mutex<super::pairing::Pairings>>>,
        origin: Option<String>,
    ) {
        self.pairing = pairings
            .zip(origin)
            .map(|(pairings, origin)| PairingContext { pairings, origin });
    }

    pub(super) fn handle_request(
        &mut self,
        request: RequestEnvelope,
        grant: CredentialGrant,
        ctx: &mut ModelContext<Self>,
    ) -> ResponseEnvelope {
        if let Err(error) = ensure_feature_enabled() {
            return ResponseEnvelope::error(request.request_id, error);
        }
        if let Err(error) = ensure_protocol_version(request.protocol_version) {
            return ResponseEnvelope::error(request.request_id, error);
        }
        let Some(instance_id) = &self.instance_id else {
            return ResponseEnvelope::error(
                request.request_id,
                ControlError::new(
                    ErrorCode::BridgeUnavailable,
                    "local-control bridge has no active instance identity",
                ),
            );
        };
        if let Err(error) = validate_request_authority(instance_id, &request.action, &grant) {
            return ResponseEnvelope::error(request.request_id, error);
        }
        if let Err(error) = ensure_action_allowed(request.action.kind, ctx) {
            return ResponseEnvelope::error(request.request_id, error);
        }
        if let Err(error) = validate_action_target(request.action.kind, &request.target) {
            return ResponseEnvelope::error(request.request_id, error);
        }
        let result = match request.action.kind {
            ActionKind::InstanceList => metadata::instance(&self.instance_id),
            ActionKind::InstanceInspect => metadata::inspect(&self.instance_id, ctx),
            ActionKind::AppPing => metadata::ping(&self.instance_id),
            ActionKind::AppVersion => metadata::version(&self.instance_id),
            ActionKind::AppActive => metadata::active(&self.instance_id, ctx),
            ActionKind::CapabilityList => Ok(metadata::capability_list()),
            ActionKind::CapabilityInspect => metadata::capability_inspect(&request.action),
            ActionKind::ActionList => Ok(metadata::action_list()),
            ActionKind::ActionInspect => metadata::action_inspect(&request.action),
            ActionKind::SurfaceList => metadata::surface_list(ctx),
            ActionKind::DriveSyncStatus => drive_sync::status(ctx),
            ActionKind::DriveSyncExport => drive_sync::export(ctx),
            ActionKind::DriveSyncImport => drive_sync::import(ctx),
            ActionKind::DriveObjectList => drive_objects::list(&request.action.params, ctx),
            ActionKind::DriveObjectGet => drive_objects::get(&request.action.params, ctx),
            ActionKind::DriveObjectCreate => drive_objects::create(&request.action.params, ctx),
            ActionKind::DriveObjectTrash => drive_objects::trash(&request.action.params, ctx),
            ActionKind::PaneMainGet => main_pane::get(&request.target, ctx),
            ActionKind::PaneMainSet => main_pane::set(&request.target, ctx),
            ActionKind::PaneMainClear => main_pane::clear(&request.target, ctx),
            ActionKind::RemoteWslList => remote_wsl::list(ctx),
            ActionKind::RemoteWslConnect => {
                remote_wsl::connect(&request.action.params, &request.target, ctx)
            }
            ActionKind::WindowVisorToggle => visor::toggle(&self.instance_id, &request.target, ctx),
            ActionKind::WindowVisorStatus => visor::status(&request.target, ctx),
            ActionKind::WindowList => metadata::window_list(&request.target, ctx),
            ActionKind::WindowInspect => metadata::window_inspect(&request.target, ctx),
            ActionKind::TabList => metadata::tab_list(&request.target, ctx),
            ActionKind::TabInspect => metadata::tab_inspect(&request.target, ctx),
            ActionKind::AppFocus
            | ActionKind::WindowCreate
            | ActionKind::WindowFocus
            | ActionKind::TabCreate
            | ActionKind::TabActivate
            | ActionKind::TabMove
            | ActionKind::TabMerge
            | ActionKind::PaneSplit
            | ActionKind::PaneFocus
            | ActionKind::PaneNavigate
            | ActionKind::PaneResize
            | ActionKind::PaneMaximize
            | ActionKind::PaneUnmaximize
            | ActionKind::SessionActivate
            | ActionKind::SessionPrevious
            | ActionKind::SessionNext
            | ActionKind::SessionReopenClosed
            | ActionKind::InputInsert
            | ActionKind::InputReplace
            | ActionKind::InputSubmit
            | ActionKind::SurfaceSettingsOpen
            | ActionKind::SurfaceCommandPaletteOpen
            | ActionKind::SurfaceCommandSearchOpen
            | ActionKind::SurfaceThemePickerOpen
            | ActionKind::SurfaceKeybindingsOpen
            | ActionKind::SurfaceWarpDriveOpen
            | ActionKind::SurfaceWarpDriveToggle
            | ActionKind::SurfaceResourceCenterToggle
            | ActionKind::SurfaceAiAssistantToggle
            | ActionKind::SurfaceCodeReviewOpen
            | ActionKind::SurfaceCodeReviewToggle
            | ActionKind::SurfaceProjectExplorerOpen
            | ActionKind::SurfaceGlobalSearchOpen
            | ActionKind::SurfaceConversationListOpen
            | ActionKind::SurfaceLeftPanelToggle
            | ActionKind::SurfaceRightPanelToggle
            | ActionKind::SurfaceVerticalTabsOpen
            | ActionKind::SurfaceVerticalTabsToggle
            | ActionKind::SurfaceAgentManagementOpen
            | ActionKind::FileOpen => app_state::handle(
                &self.instance_id,
                request.action.kind,
                &request.action.params,
                &request.target,
                ctx,
            ),
            // Fork-local: the agent and slash surfaces (`.fork/TASKS.md` T6.5).
            ActionKind::AgentList => agent::agent_list(&self.instance_id, ctx),
            ActionKind::AgentPrompt => agent::agent_prompt(
                &self.instance_id,
                &request.action.params,
                &request.target,
                ctx,
            ),
            ActionKind::AgentRead => {
                agent::agent_read(&self.instance_id, &request.action.params, ctx)
            }
            ActionKind::AgentSpawn => agent::agent_spawn(
                &self.instance_id,
                &request.action.params,
                &request.target,
                ctx,
            ),
            ActionKind::AgentCancel => {
                agent::agent_cancel(&self.instance_id, &request.action.params, ctx)
            }
            ActionKind::AgentSettle => {
                agent::agent_settle(&self.instance_id, &request.action.params, ctx)
            }
            ActionKind::AgentReveal => agent::agent_reveal(
                &self.instance_id,
                &request.action.params,
                &request.target,
                ctx,
            ),
            // Fork-local: the CLI agents in panes, which `agent.list` has never
            // been able to see (`.fork/TASKS.md` T11.5).
            ActionKind::AgentApprovals => approvals::agent_approvals(&self.instance_id, ctx),
            ActionKind::AgentApprove => approvals::agent_answer(
                &self.instance_id,
                approvals::Decision::Allow,
                &request.action.params,
                ctx,
            ),
            ActionKind::AgentDeny => approvals::agent_answer(
                &self.instance_id,
                approvals::Decision::Deny,
                &request.action.params,
                ctx,
            ),
            // Fork-local: the read surface (`.fork/TASKS.md` T11.2). Answers
            // *where* the stream is; the stream itself is a GET route, because
            // SSE is not a request/response shape and this envelope is.
            ActionKind::EventsSubscribe => {
                events::events_subscribe(self.control_origin.as_deref(), &grant)
            }
            // Fork-local: the wide bind's front door (`.fork/TASKS.md` T11.4).
            // Answers with a code to *show*; redeeming it is a route, because a
            // device that has not paired yet cannot invoke an action.
            ActionKind::ControlPair => pairing::control_pair(self.pairing.as_ref()),
            ActionKind::SlashList => agent::slash_list(&self.instance_id, &request.target, ctx),
            ActionKind::SlashRun => agent::slash_run(
                &self.instance_id,
                &request.action.params,
                &request.target,
                ctx,
            ),
            ActionKind::TabRename => metadata_config::tab_rename(
                &self.instance_id,
                &request.target,
                &request.action,
                ctx,
            ),
            ActionKind::TabResetName => {
                metadata_config::tab_reset_name(&self.instance_id, &request.target, ctx)
            }
            ActionKind::TabColorSet => metadata_config::tab_color_set(
                &self.instance_id,
                &request.target,
                &request.action,
                ctx,
            ),
            ActionKind::TabColorClear => {
                metadata_config::tab_color_clear(&self.instance_id, &request.target, ctx)
            }
            ActionKind::PaneList => metadata::pane_list(&request.target, ctx),
            ActionKind::PaneInspect => metadata::pane_inspect(&request.target, ctx),
            ActionKind::PaneRename => metadata_config::pane_rename(
                &self.instance_id,
                &request.target,
                &request.action,
                ctx,
            ),
            ActionKind::PaneResetName => {
                metadata_config::pane_reset_name(&self.instance_id, &request.target, ctx)
            }
            ActionKind::SessionList => metadata::session_list(&request.target, ctx),
            ActionKind::SessionInspect => metadata::session_inspect(&request.target, ctx),
            ActionKind::ThemeList => settings_surfaces::theme_list(ctx),
            ActionKind::ThemeGet => settings_surfaces::theme_get(ctx),
            ActionKind::ThemeSet
            | ActionKind::ThemeSystemSet
            | ActionKind::ThemeLightSet
            | ActionKind::ThemeDarkSet => metadata_config::theme_set(
                &self.instance_id,
                request.action.kind,
                &request.action,
                ctx,
            ),
            ActionKind::AppearanceGet => settings_surfaces::appearance_get(ctx),
            ActionKind::AppearanceFontSizeIncrease
            | ActionKind::AppearanceFontSizeDecrease
            | ActionKind::AppearanceFontSizeReset
            | ActionKind::AppearanceZoomIncrease
            | ActionKind::AppearanceZoomDecrease
            | ActionKind::AppearanceZoomReset => {
                metadata_config::appearance_mutation(&self.instance_id, request.action.kind, ctx)
            }
            ActionKind::SettingList => settings_surfaces::setting_list(&request.action, ctx),
            ActionKind::SettingGet => settings_surfaces::setting_get(&request.action, ctx),
            ActionKind::SettingSet => metadata_config::setting_set(&request.action, ctx),
            ActionKind::SettingToggle => metadata_config::setting_toggle(&request.action, ctx),
            ActionKind::KeybindingList => settings_surfaces::keybinding_list(ctx),
            ActionKind::KeybindingGet => settings_surfaces::keybinding_get(&request.action, ctx),
            ActionKind::WindowClose => close::window_close(&self.instance_id, &request, ctx),
            ActionKind::TabClose => close::tab_close(&self.instance_id, &request, ctx),
            ActionKind::PaneClose => close::pane_close(&self.instance_id, &request, ctx),
        };
        match result {
            Ok(data) => ResponseEnvelope::ok(request.request_id, data),
            Err(error) => ResponseEnvelope::error(request.request_id, error),
        }
    }
}

pub(crate) fn validate_request_authority(
    instance_id: &InstanceId,
    action: &Action,
    grant: &CredentialGrant,
) -> Result<(), ControlError> {
    grant.verify_for_action(instance_id, action.kind)?;
    if !action.kind.is_implemented() {
        return Err(ControlError::new(
            ErrorCode::UnsupportedAction,
            format!(
                "{} is not implemented by this local-control bridge",
                action.kind.as_str()
            ),
        ));
    }
    validate_action_params(action)
}
