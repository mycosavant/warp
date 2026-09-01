//! Implementations for user-facing `warpctrl` command groups.
use std::path::Path;

use local_control::discovery::InstanceRecord;
use local_control::protocol::{
    Action, ActionKind, ActionNameParams, AgentApprovalsResult, AgentApproveParams,
    AgentCancelParams, AgentPromptParams, AgentReadParams, AgentRevealParams, AgentRevealTarget,
    AgentSettleParams, AgentSpawnParams, BindingNameParams, BooleanValueParams, ColorValueParams,
    ControlError, DirectionParams, DriveObjectCreateParams, DriveObjectGetParams,
    DriveObjectListParams, DriveObjectTrashParams, EmptyParams, ErrorCode, EventStreamResult,
    FileOpenParams, KeyParams, KeyValueParams, PageQueryParams, QueryParams,
    RemoteWslConnectParams, RenameParams, RequestEnvelope, ResizeParams, SettingListParams,
    SlashRunParams, TabActivateParams, TabActivationMode, TabCloseMode, TabCloseParams,
    TabCreateParams, TextParams, ThemeNameParams,
};
use local_control::selection::select_instance;
use serde::Serialize;
use warp_core::channel::ChannelState;

use crate::agent::OutputFormat;
use crate::local_control::output::{write_json, write_json_line};
use crate::local_control::selectors::{instance_selector, target_selector};
use crate::local_control::{
    ActionCatalogCommand, AgentCommand, AppCommand, AppearanceCommand, CapabilityCommand,
    CliRevealTarget, DriveCommand, DriveObjectCommand, EventsCommand, FileCommand, InputCommand,
    InstanceCommand, KeybindingCommand, PairCommand, PaneCommand, PaneMainCommand, RemoteCommand,
    RemoteWslCommand, SessionCommand, SettingCommand, SlashCommand, SurfaceCommand,
    SurfaceOpenCommand, SurfaceOpenToggleCommand, SurfaceQueryCommand, SurfaceSettingsCommand,
    SurfaceToggleCommand, TabActivateArgs, TabCloseArgs, TabColorCommand, TabCommand, TargetArgs,
    ThemeCommand, WindowCommand, WindowVisorCommand,
};

pub(super) fn run_surface_command(
    command: SurfaceCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        SurfaceCommand::List(args) => {
            run_action_with_params(args, ActionKind::SurfaceList, EmptyParams {}, output_format)
        }
        SurfaceCommand::Settings(command) => match command {
            SurfaceSettingsCommand::Open(args) => run_action_with_params(
                args.target,
                ActionKind::SurfaceSettingsOpen,
                PageQueryParams {
                    page: args.page,
                    query: args.query,
                },
                output_format,
            ),
        },
        SurfaceCommand::CommandPalette(command) => run_surface_query_command(
            command,
            ActionKind::SurfaceCommandPaletteOpen,
            output_format,
        ),
        SurfaceCommand::CommandSearch(command) => {
            run_surface_query_command(command, ActionKind::SurfaceCommandSearchOpen, output_format)
        }
        SurfaceCommand::ThemePicker(command) => {
            run_surface_open_command(command, ActionKind::SurfaceThemePickerOpen, output_format)
        }
        SurfaceCommand::Keybindings(command) => {
            run_surface_open_command(command, ActionKind::SurfaceKeybindingsOpen, output_format)
        }
        SurfaceCommand::WarpDrive(command) => match command {
            SurfaceOpenToggleCommand::Open(args) => run_action_with_params(
                args,
                ActionKind::SurfaceWarpDriveOpen,
                EmptyParams {},
                output_format,
            ),
            SurfaceOpenToggleCommand::Toggle(args) => run_action_with_params(
                args,
                ActionKind::SurfaceWarpDriveToggle,
                EmptyParams {},
                output_format,
            ),
        },
        SurfaceCommand::ResourceCenter(command) => run_surface_toggle_command(
            command,
            ActionKind::SurfaceResourceCenterToggle,
            output_format,
        ),
        SurfaceCommand::AiAssistant(command) => {
            run_surface_toggle_command(command, ActionKind::SurfaceAiAssistantToggle, output_format)
        }
        SurfaceCommand::CodeReview(command) => match command {
            SurfaceOpenToggleCommand::Open(args) => run_action_with_params(
                args,
                ActionKind::SurfaceCodeReviewOpen,
                EmptyParams {},
                output_format,
            ),
            SurfaceOpenToggleCommand::Toggle(args) => run_action_with_params(
                args,
                ActionKind::SurfaceCodeReviewToggle,
                EmptyParams {},
                output_format,
            ),
        },
        SurfaceCommand::ProjectExplorer(command) => run_surface_open_command(
            command,
            ActionKind::SurfaceProjectExplorerOpen,
            output_format,
        ),
        SurfaceCommand::GlobalSearch(command) => {
            run_surface_open_command(command, ActionKind::SurfaceGlobalSearchOpen, output_format)
        }
        SurfaceCommand::ConversationList(command) => run_surface_open_command(
            command,
            ActionKind::SurfaceConversationListOpen,
            output_format,
        ),
        SurfaceCommand::LeftPanel(command) => {
            run_surface_toggle_command(command, ActionKind::SurfaceLeftPanelToggle, output_format)
        }
        SurfaceCommand::RightPanel(command) => {
            run_surface_toggle_command(command, ActionKind::SurfaceRightPanelToggle, output_format)
        }
        SurfaceCommand::VerticalTabs(command) => match command {
            SurfaceOpenToggleCommand::Open(args) => run_action_with_params(
                args,
                ActionKind::SurfaceVerticalTabsOpen,
                EmptyParams {},
                output_format,
            ),
            SurfaceOpenToggleCommand::Toggle(args) => run_action_with_params(
                args,
                ActionKind::SurfaceVerticalTabsToggle,
                EmptyParams {},
                output_format,
            ),
        },
        SurfaceCommand::AgentManagement(command) => run_surface_open_command(
            command,
            ActionKind::SurfaceAgentManagementOpen,
            output_format,
        ),
    }
}

fn render_human_readable(action: ActionKind, data: &serde_json::Value) -> String {
    match action {
        ActionKind::AppPing => format!(
            "Warp instance {} is reachable (protocol version {})",
            value_or_unknown(data, "instance_id"),
            value_or_unknown(data, "protocol_version")
        ),
        ActionKind::AppVersion => format!(
            "Warp instance {}\nchannel: {}\napp_id: {}\nprotocol_version: {}",
            value_or_unknown(data, "instance_id"),
            value_or_unknown(data, "channel"),
            value_or_unknown(data, "app_id"),
            value_or_unknown(data, "protocol_version")
        ),
        ActionKind::TabCreate => format!(
            "Created tab {} in window {} (active index {}, tab count {})",
            nested_value_or_unknown(data, &["tab", "id"]),
            nested_value_or_unknown(data, &["window", "id"]),
            nested_value_or_unknown(data, &["tab", "active_index"]),
            nested_value_or_unknown(data, &["tab", "count"])
        ),
        ActionKind::PaneSplit => format!(
            "Split created pane {}",
            nested_value_or_unknown(data, &["pane", "id"])
        ),
        ActionKind::AgentApprovals => render_approvals(data),
        ActionKind::ControlPair => render_pairing(data),
        _ => serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string()),
    }
}

/// One block per waiting request, ending in the command that answers it.
///
/// **The command is printed because transcription was the measured cost.**
/// T14.9 answered about thirty-five requests in a working session, each by
/// copying an `approval_id` and a 64-character digest out of pretty-printed
/// JSON and into a shell. That is the whole of the friction for the answerable
/// ones — not the modality — so printing the line a person would have typed
/// removes it without adding a surface.
///
/// **And it removes nothing from the binding.** The digest still travels, and it
/// is still the digest of what was displayed *in this listing*: if the agent
/// moved on between the reading and the answer, the hash no longer fits and the
/// answer is refused. Copying a printed line and typing the same line are the
/// same act to the server, which is the property that makes this a convenience
/// rather than a loosening. The alternative that would have loosened it — a
/// `--latest` that addresses whatever is pending with no digest at all — is the
/// one this is written to make unnecessary.
///
/// Falls back to the raw JSON if the payload does not parse, because a renderer
/// is not a place to lose data a person asked for.
/// What an empty approvals listing says, **in the payload and not only in a
/// comment above it.**
///
/// The parenthetical is the load-bearing half. T14.19 measured a poll reporting
/// zero parked approvals while a request was genuinely waiting, and that phantom
/// zero was one inference away from a security investigation into an
/// auto-approval hole that does not exist. The distinction between "nobody is
/// asking" and "nothing is running" is the one a person reads straight past, and
/// `agent list` is what actually answers the second.
///
/// A constant because the sentence has a test asserting it, in another crate. It
/// was edited here and left red there for several hours on 2026-08-31 --
/// `-p local_control` and `-p warp --lib` were run, `-p warp_cli` was not. One
/// home for the string means the next edit cannot repeat that.
pub(crate) const NOTHING_IS_WAITING: &str = "Nothing is waiting on you right now. (An agent is free to ask nothing at all, so this is \
     not evidence that nothing is running — `agent list` answers that.)";

fn render_approvals(data: &serde_json::Value) -> String {
    let Ok(result) = serde_json::from_value::<AgentApprovalsResult>(data.clone()) else {
        return serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string());
    };
    if result.approvals.is_empty() {
        // Says what empty means, in the payload and not only in a comment above
        // it: the distinction between "nobody is asking" and "nothing is
        // running" is exactly the one a person reads past at 2am.
        //
        // Returned from the constant rather than spelled again here. It was
        // spelled again here until 2026-09-01, which made the constant -- added
        // so that the string would have *one* home -- dead code that the test
        // asserted against while this function kept its own copy. The two
        // happened to be identical, so nothing was red; `dead_code` was the only
        // thing that noticed, and it is the whole reason the warning was worth
        // chasing rather than silencing.
        return NOTHING_IS_WAITING.to_owned();
    }

    let mut out = String::new();
    for (index, approval) in result.approvals.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&format!(
            "[{}] {} — {}\n",
            approval.source,
            approval.agent,
            approval.summary.as_deref().unwrap_or("(no summary given)")
        ));
        // **Labelled `approval_id`, and the label is the point.** The id was
        // already here, but only inside the runnable `agent approve '<id>'`
        // line — so a script grepping the pretty output for `approval_id`
        // found nothing while a request sat parked, and reported a confident
        // zero. That phantom zero was one inference away from a security
        // investigation into an auto-approval hole that does not exist.
        //
        // The documented remedy is `--output-format json` for anything a
        // script decides on, and it still is. This is the cheaper half: make
        // the human format contain the token a person would reach for anyway,
        // so the trap needs the documentation rather than depending on it.
        out.push_str(&format!("  approval_id {}\n", approval.approval_id));
        if let Some(tool) = &approval.tool_name {
            out.push_str(&format!("  tool      {tool}\n"));
        }
        if let Some(input) = &approval.tool_input {
            out.push_str(&format!("  the call  {input}\n"));
        }
        // "not stated" rather than falling back to `cwd`: presenting Warp's own
        // session directory as the call's is the certainty this fork does not
        // have, and `acts_on` exists precisely to keep them apart.
        out.push_str(&format!(
            "  acts on   {}\n",
            if approval.acts_on.is_empty() {
                "not stated by the agent".to_owned()
            } else {
                approval.acts_on.join(", ")
            }
        ));
        if !approval.options_offered.is_empty() {
            out.push_str(&format!(
                "  offered   {}\n",
                approval.options_offered.join(", ")
            ));
        }
        // **Gated on `can_approve`, not on whether a reason came with it** —
        // T14.6's finding, which cost a phone a *Yes* button on rows that could
        // never work. The reason is an explanation attached to that fact, not
        // the fact itself, so an entry refused without one must still not be
        // offered a yes. It is only ever missing if a server neglected to write
        // one, which is why there is a sentence to fall back to.
        if approval.can_approve {
            out.push_str(&format!(
                "  yes       warpctrl agent approve '{}' --digest {}\n",
                approval.approval_id, approval.digest
            ));
        } else {
            out.push_str(&format!(
                "  no yes    {}\n",
                approval.approve_refused_because.as_deref().unwrap_or(
                    "Warp will not say yes to this request, and did not say why. Denying works, \
                     and so does cancelling the turn."
                )
            ));
        }
        // Always offered, and last, because a no is the answer that can only
        // ever make less happen — the asymmetry `agent.deny` is built on.
        out.push_str(&format!(
            "  no        warpctrl agent deny '{}' --digest {}\n",
            approval.approval_id, approval.digest
        ));
    }
    out.pop();
    out
}

/// Draws the pairing code as something a phone can be pointed at.
///
/// **The QR is why this exists, and it was generated but never printed.**
/// `PairingResult::qr` carries the code already rendered as text — its own doc
/// says "so a terminal client can show one without an image viewer" — and the
/// only human-facing output format dumped the escaped JSON string instead. So
/// the way to pair was to copy a URL with a secret in its fragment across
/// devices, or type the hash, which is exactly what a QR exists to avoid.
///
/// The grants are printed under it rather than beside it, because *"which of
/// these actions does my phone get"* is the question `PairingResult` says
/// should be answerable before anyone scans, and a list that scrolls off the
/// top of a QR is not an answer.
fn render_pairing(data: &serde_json::Value) -> String {
    let mut out = String::new();
    if let Some(qr) = data.get("qr").and_then(serde_json::Value::as_str) {
        out.push_str(qr);
        if !qr.ends_with('\n') {
            out.push('\n');
        }
    }
    out.push_str(&format!("{}\n", value_or_unknown(data, "url")));
    // The expiry is not decoration: the code is spendable for two minutes and
    // once, so a person reading this needs to know whether it is already dead
    // before they wonder why their phone timed out.
    out.push_str(&format!(
        "expires {} (two minutes, single use)\n",
        value_or_unknown(data, "expires_at")
    ));
    let actions = data
        .get("actions")
        .and_then(serde_json::Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "<unknown>".to_owned());
    out.push_str(&format!("a device that scans this may: {actions}"));
    out
}

fn value_or_unknown(data: &serde_json::Value, key: &str) -> String {
    nested_value_or_unknown(data, &[key])
}

fn nested_value_or_unknown(data: &serde_json::Value, path: &[&str]) -> String {
    let value = path
        .iter()
        .try_fold(data, |value, key| value.get(*key))
        .unwrap_or(&serde_json::Value::Null);
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Null => "<unknown>".to_owned(),
        value => value.to_string(),
    }
}

#[cfg(test)]
pub(crate) fn render_human_readable_for_test(
    action: ActionKind,
    data: &serde_json::Value,
) -> String {
    render_human_readable(action, data)
}

pub(super) fn run_instance_command(
    command: InstanceCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        InstanceCommand::List => render_instance_list(
            local_control::discovery::list_instances(&ChannelState::channel().to_string()),
            output_format,
        ),
        InstanceCommand::Inspect(args) => run_action_with_params(
            args,
            ActionKind::InstanceInspect,
            EmptyParams {},
            output_format,
        ),
    }
}

/// JSON payload for `warpctrl instance list`.
#[derive(Serialize)]
pub(super) struct InstanceListOutput {
    instances: Vec<InstanceSummary>,
}

#[derive(Serialize)]
struct InstanceSummary {
    instance_id: String,
    pid: u32,
    channel: String,
    app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_version: Option<String>,
    protocol_version: u32,
}

/// Builds the list payload from probed discovery records.
pub(super) fn instance_list_output(records: Vec<InstanceRecord>) -> InstanceListOutput {
    InstanceListOutput {
        instances: records
            .into_iter()
            .map(|record| InstanceSummary {
                instance_id: record.instance_id.0,
                pid: record.pid,
                channel: record.channel,
                app_id: record.app_id,
                app_version: record.app_version,
                protocol_version: record.protocol_version,
            })
            .collect(),
    }
}

/// Lists every reachable instance without selecting one. Zero reachable
/// instances is a successful empty list, never an error.
fn render_instance_list(
    records: Vec<InstanceRecord>,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    let output = instance_list_output(records);
    match output_format {
        OutputFormat::Json => write_json(&output),
        OutputFormat::Ndjson => write_json_line(&output),
        OutputFormat::Pretty | OutputFormat::Text => {
            if output.instances.is_empty() {
                println!("No running Warp instances with local control were found.");
                return Ok(());
            }
            for instance in &output.instances {
                println!(
                    "{} (pid {}, channel {}, app {}, protocol {})",
                    instance.instance_id,
                    instance.pid,
                    instance.channel,
                    instance.app_id,
                    instance.protocol_version
                );
            }
            Ok(())
        }
    }
}

pub(super) fn run_app_command(
    command: AppCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        AppCommand::Ping(args) => run_action(args, ActionKind::AppPing, output_format),
        AppCommand::Version(args) => run_action(args, ActionKind::AppVersion, output_format),
        AppCommand::Active(args) => {
            run_action_with_params(args, ActionKind::AppActive, EmptyParams {}, output_format)
        }
        AppCommand::Focus(args) => run_action(args, ActionKind::AppFocus, output_format),
    }
}

pub(super) fn run_action_catalog_command(
    command: ActionCatalogCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        ActionCatalogCommand::List => run_action_with_params(
            TargetArgs::default(),
            ActionKind::ActionList,
            EmptyParams {},
            output_format,
        ),
        ActionCatalogCommand::Inspect { action } => run_action_with_params(
            TargetArgs::default(),
            ActionKind::ActionInspect,
            ActionNameParams { action },
            output_format,
        ),
    }
}

pub(super) fn run_capability_command(
    command: CapabilityCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        CapabilityCommand::List => run_action_with_params(
            TargetArgs::default(),
            ActionKind::CapabilityList,
            EmptyParams {},
            output_format,
        ),
        CapabilityCommand::Inspect { action } => run_action_with_params(
            TargetArgs::default(),
            ActionKind::CapabilityInspect,
            ActionNameParams { action },
            output_format,
        ),
    }
}

pub(super) fn run_window_command(
    command: WindowCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        WindowCommand::List(args) => {
            run_action_with_params(args, ActionKind::WindowList, EmptyParams {}, output_format)
        }
        WindowCommand::Inspect(args) => run_action_with_params(
            args,
            ActionKind::WindowInspect,
            EmptyParams {},
            output_format,
        ),
        WindowCommand::Create(args) => run_action_with_params(
            args.target,
            ActionKind::WindowCreate,
            TabCreateParams {
                tab_type: args.tab_type.map(Into::into),
            },
            output_format,
        ),
        WindowCommand::Focus(args) => {
            run_action_with_params(args, ActionKind::WindowFocus, EmptyParams {}, output_format)
        }
        WindowCommand::Close(args) => {
            run_action_with_params(args, ActionKind::WindowClose, EmptyParams {}, output_format)
        }
        WindowCommand::Visor(command) => match command {
            WindowVisorCommand::Toggle(args) => {
                run_action(args, ActionKind::WindowVisorToggle, output_format)
            }
            WindowVisorCommand::Status(args) => {
                run_action(args, ActionKind::WindowVisorStatus, output_format)
            }
        },
    }
}

pub(super) fn run_tab_command(
    command: TabCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        TabCommand::List(args) => {
            run_action_with_params(args, ActionKind::TabList, EmptyParams {}, output_format)
        }
        TabCommand::Inspect(args) => {
            run_action_with_params(args, ActionKind::TabInspect, EmptyParams {}, output_format)
        }
        TabCommand::Create(args) => run_action_with_params(
            args.target,
            ActionKind::TabCreate,
            TabCreateParams {
                tab_type: args.tab_type.map(Into::into),
            },
            output_format,
        ),
        TabCommand::Activate(args) => {
            let mode = tab_activation_mode(&args);
            run_action_with_params(
                args.target,
                ActionKind::TabActivate,
                TabActivateParams { mode },
                output_format,
            )
        }
        TabCommand::Move(args) => run_action_with_params(
            args.target,
            ActionKind::TabMove,
            DirectionParams {
                direction: args.direction.into(),
            },
            output_format,
        ),
        TabCommand::Merge(args) => run_action_with_params(
            args.target,
            ActionKind::TabMerge,
            DirectionParams {
                direction: args.direction.into(),
            },
            output_format,
        ),
        TabCommand::Close(args) => {
            let mode = tab_close_mode(&args);
            run_action_with_params(
                args.target,
                ActionKind::TabClose,
                TabCloseParams { mode },
                output_format,
            )
        }
        TabCommand::Rename(args) => run_action_with_params(
            args.target,
            ActionKind::TabRename,
            RenameParams { title: args.title },
            output_format,
        ),
        TabCommand::ResetName(args) => run_action(args, ActionKind::TabResetName, output_format),
        TabCommand::Color(command) => match command {
            TabColorCommand::Set(args) => run_action_with_params(
                args.target,
                ActionKind::TabColorSet,
                ColorValueParams { color: args.color },
                output_format,
            ),
            TabColorCommand::Clear(args) => {
                run_action(args, ActionKind::TabColorClear, output_format)
            }
        },
    }
}

pub(super) fn run_pane_command(
    command: PaneCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        PaneCommand::List(args) => {
            run_action_with_params(args, ActionKind::PaneList, EmptyParams {}, output_format)
        }
        PaneCommand::Inspect(args) => {
            run_action_with_params(args, ActionKind::PaneInspect, EmptyParams {}, output_format)
        }
        PaneCommand::Split(args) => run_action_with_params(
            args.target,
            ActionKind::PaneSplit,
            DirectionParams {
                direction: args.direction.into(),
            },
            output_format,
        ),
        PaneCommand::Focus(args) => {
            run_action_with_params(args, ActionKind::PaneFocus, EmptyParams {}, output_format)
        }
        PaneCommand::Navigate(args) => run_action_with_params(
            args.target,
            ActionKind::PaneNavigate,
            DirectionParams {
                direction: args.direction.into(),
            },
            output_format,
        ),
        PaneCommand::Resize(args) => run_action_with_params(
            args.target,
            ActionKind::PaneResize,
            ResizeParams {
                direction: args.direction.into(),
                amount: args.amount,
            },
            output_format,
        ),
        PaneCommand::Maximize(args) => run_action_with_params(
            args,
            ActionKind::PaneMaximize,
            EmptyParams {},
            output_format,
        ),
        PaneCommand::Unmaximize(args) => run_action_with_params(
            args,
            ActionKind::PaneUnmaximize,
            EmptyParams {},
            output_format,
        ),
        PaneCommand::Close(args) => {
            run_action_with_params(args, ActionKind::PaneClose, EmptyParams {}, output_format)
        }
        PaneCommand::Rename(args) => run_action_with_params(
            args.target,
            ActionKind::PaneRename,
            RenameParams { title: args.title },
            output_format,
        ),
        PaneCommand::ResetName(args) => run_action(args, ActionKind::PaneResetName, output_format),
        PaneCommand::Main(command) => match command {
            PaneMainCommand::Get(args) => run_action(args, ActionKind::PaneMainGet, output_format),
            PaneMainCommand::Set(args) => run_action(args, ActionKind::PaneMainSet, output_format),
            PaneMainCommand::Clear(args) => {
                run_action(args, ActionKind::PaneMainClear, output_format)
            }
        },
    }
}

pub(super) fn run_session_command(
    command: SessionCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        SessionCommand::List(args) => {
            run_action_with_params(args, ActionKind::SessionList, EmptyParams {}, output_format)
        }
        SessionCommand::Inspect(args) => run_action_with_params(
            args,
            ActionKind::SessionInspect,
            EmptyParams {},
            output_format,
        ),
        SessionCommand::Activate(args) => run_action_with_params(
            args,
            ActionKind::SessionActivate,
            EmptyParams {},
            output_format,
        ),
        SessionCommand::Previous(args) => run_action_with_params(
            args,
            ActionKind::SessionPrevious,
            EmptyParams {},
            output_format,
        ),
        SessionCommand::Next(args) => {
            run_action_with_params(args, ActionKind::SessionNext, EmptyParams {}, output_format)
        }
        SessionCommand::ReopenClosed(args) => run_action_with_params(
            args,
            ActionKind::SessionReopenClosed,
            EmptyParams {},
            output_format,
        ),
    }
}

pub(super) fn run_input_command(
    command: InputCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        InputCommand::Insert(args) => run_action_with_params(
            args.target,
            ActionKind::InputInsert,
            TextParams { text: args.text },
            output_format,
        ),
        InputCommand::Replace(args) => run_action_with_params(
            args.target,
            ActionKind::InputReplace,
            TextParams { text: args.text },
            output_format,
        ),
        InputCommand::Submit(args) => run_action_with_params(
            args.target,
            ActionKind::InputSubmit,
            TextParams { text: args.text },
            output_format,
        ),
    }
}

pub(super) fn run_theme_command(
    command: ThemeCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        ThemeCommand::List(args) => {
            run_action_with_params(args, ActionKind::ThemeList, EmptyParams {}, output_format)
        }
        ThemeCommand::Get(args) => {
            run_action_with_params(args, ActionKind::ThemeGet, EmptyParams {}, output_format)
        }
        ThemeCommand::Set(args) => run_action_with_params(
            args.target,
            ActionKind::ThemeSet,
            ThemeNameParams {
                theme_name: args.name,
            },
            output_format,
        ),
        ThemeCommand::SystemSet(args) => run_action_with_params(
            args.target,
            ActionKind::ThemeSystemSet,
            BooleanValueParams {
                value: args.enabled,
            },
            output_format,
        ),
        ThemeCommand::LightSet(args) => run_action_with_params(
            args.target,
            ActionKind::ThemeLightSet,
            ThemeNameParams {
                theme_name: args.name,
            },
            output_format,
        ),
        ThemeCommand::DarkSet(args) => run_action_with_params(
            args.target,
            ActionKind::ThemeDarkSet,
            ThemeNameParams {
                theme_name: args.name,
            },
            output_format,
        ),
    }
}

pub(super) fn run_appearance_command(
    command: AppearanceCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        AppearanceCommand::Get(args) => run_action_with_params(
            args,
            ActionKind::AppearanceGet,
            EmptyParams {},
            output_format,
        ),
        AppearanceCommand::FontSizeIncrease(args) => {
            run_action(args, ActionKind::AppearanceFontSizeIncrease, output_format)
        }
        AppearanceCommand::FontSizeDecrease(args) => {
            run_action(args, ActionKind::AppearanceFontSizeDecrease, output_format)
        }
        AppearanceCommand::FontSizeReset(args) => {
            run_action(args, ActionKind::AppearanceFontSizeReset, output_format)
        }
        AppearanceCommand::ZoomIncrease(args) => {
            run_action(args, ActionKind::AppearanceZoomIncrease, output_format)
        }
        AppearanceCommand::ZoomDecrease(args) => {
            run_action(args, ActionKind::AppearanceZoomDecrease, output_format)
        }
        AppearanceCommand::ZoomReset(args) => {
            run_action(args, ActionKind::AppearanceZoomReset, output_format)
        }
    }
}

pub(super) fn run_setting_command(
    command: SettingCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        SettingCommand::List(args) => run_action_with_params(
            args.target,
            ActionKind::SettingList,
            SettingListParams {
                namespace: args.namespace,
            },
            output_format,
        ),
        SettingCommand::Get(args) => run_action_with_params(
            args.target,
            ActionKind::SettingGet,
            KeyParams { key: args.key },
            output_format,
        ),
        SettingCommand::Set(args) => run_action_with_params(
            args.target,
            ActionKind::SettingSet,
            KeyValueParams {
                key: args.key,
                value: parse_json_value_or_string(args.value),
            },
            output_format,
        ),
        SettingCommand::Toggle(args) => run_action_with_params(
            args.target,
            ActionKind::SettingToggle,
            KeyParams { key: args.key },
            output_format,
        ),
    }
}

pub(super) fn run_keybinding_command(
    command: KeybindingCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        KeybindingCommand::List(args) => run_action_with_params(
            args,
            ActionKind::KeybindingList,
            EmptyParams {},
            output_format,
        ),
        KeybindingCommand::Get(args) => run_action_with_params(
            args.target,
            ActionKind::KeybindingGet,
            BindingNameParams {
                binding_name: args.name,
            },
            output_format,
        ),
    }
}

pub(super) fn run_file_command(
    command: FileCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        FileCommand::Open(args) => run_action_with_params(
            args.target,
            ActionKind::FileOpen,
            FileOpenParams {
                path: args.path,
                line: args.line,
                column: args.column,
                new_tab: args.new_tab,
            },
            output_format,
        ),
    }
}

/// `warpctrl agent …` — the actions that let an agent drive an agent.
pub(super) fn run_agent_command(
    command: AgentCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        AgentCommand::List(args) => {
            run_action_with_params(args, ActionKind::AgentList, EmptyParams {}, output_format)
        }
        AgentCommand::Prompt(args) => run_action_with_params(
            args.target,
            ActionKind::AgentPrompt,
            AgentPromptParams {
                prompt: args.prompt,
                conversation_id: args.conversation,
            },
            output_format,
        ),
        AgentCommand::Read(args) => run_action_with_params(
            args.target,
            ActionKind::AgentRead,
            AgentReadParams {
                conversation_id: args.conversation,
                last: args.last,
                include_tool_results: args.include_tool_results,
            },
            output_format,
        ),
        AgentCommand::Spawn(args) => run_action_with_params(
            args.target,
            ActionKind::AgentSpawn,
            AgentSpawnParams {
                prompt: args.prompt,
                name: args.name,
                parent_conversation_id: args.parent,
                allow_tools: args.allow_tools,
            },
            output_format,
        ),
        AgentCommand::Cancel(args) => run_action_with_params(
            args.target,
            ActionKind::AgentCancel,
            AgentCancelParams {
                conversation_id: args.conversation,
            },
            output_format,
        ),
        AgentCommand::Settle(args) => run_action_with_params(
            args.target,
            ActionKind::AgentSettle,
            AgentSettleParams {
                conversation_id: args.conversation,
                settled: !args.undo,
            },
            output_format,
        ),
        AgentCommand::Reveal(args) => run_action_with_params(
            args.target,
            ActionKind::AgentReveal,
            AgentRevealParams {
                conversation_id: args.conversation,
                target: match args.reveal_target {
                    CliRevealTarget::Pane => AgentRevealTarget::Pane,
                    CliRevealTarget::Tab => AgentRevealTarget::Tab,
                    CliRevealTarget::Swap => AgentRevealTarget::Swap,
                },
            },
            output_format,
        ),
        AgentCommand::Approvals(args) => run_action_with_params(
            args,
            ActionKind::AgentApprovals,
            EmptyParams {},
            output_format,
        ),
        AgentCommand::Approve(args) => run_action_with_params(
            args.target,
            ActionKind::AgentApprove,
            AgentApproveParams {
                approval_id: args.approval,
                digest: args.digest,
            },
            output_format,
        ),
        AgentCommand::Deny(args) => run_action_with_params(
            args.target,
            ActionKind::AgentDeny,
            AgentApproveParams {
                approval_id: args.approval,
                digest: args.digest,
            },
            output_format,
        ),
    }
}

/// `warpctrl slash …` — Warp's slash-command registry, behind the allowlist.
/// `remote wsl list` — the data source a WSL picker needs, and the answer to
/// "is this machine even a candidate" before one is built.
pub(super) fn run_remote_command(
    command: RemoteCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        RemoteCommand::Wsl(RemoteWslCommand::List(args)) => run_action_with_params(
            args,
            ActionKind::RemoteWslList,
            EmptyParams {},
            output_format,
        ),
        RemoteCommand::Wsl(RemoteWslCommand::Connect(args)) => run_action_with_params(
            args.target,
            ActionKind::RemoteWslConnect,
            RemoteWslConnectParams {
                distro: args.distro,
            },
            output_format,
        ),
    }
}

pub(super) fn run_pair_command(
    command: PairCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        PairCommand::Show(args) => {
            run_action_with_params(args, ActionKind::ControlPair, EmptyParams {}, output_format)
        }
    }
}

pub(super) fn run_events_command(
    command: EventsCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        EventsCommand::Subscribe(args) => run_action_with_params(
            args,
            ActionKind::EventsSubscribe,
            EmptyParams {},
            output_format,
        ),
        EventsCommand::Tail(args) => run_events_tail(args, output_format),
    }
}

/// Follows the live stream, printing one line per event.
///
/// The URL comes from `events.subscribe` rather than being assembled here, so
/// the CLI discovers the endpoint exactly the way any other client would — which
/// is also what keeps this honest as a demonstration that the advertised URL
/// works.
fn run_events_tail(args: TargetArgs, output_format: OutputFormat) -> Result<(), ControlError> {
    let data = send_action(&args, ActionKind::EventsSubscribe, EmptyParams {})?;
    let stream: EventStreamResult = serde_json::from_value(data).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "events.subscribe returned an unexpected result",
            err.to_string(),
        )
    })?;
    let records = local_control::discovery::list_instances(&ChannelState::channel().to_string());
    let instance = select_instance(&records, &instance_selector(&args))?;
    local_control::client::stream_events(&instance, &stream.url, |event| {
        match event.name {
            // Stream-level frames go to stderr, so `warpctrl events tail | jq`
            // sees only event lines and a lag notice cannot be mistaken for one.
            Some("expired") => eprintln!("credential expired; re-run to reconnect"),
            Some("lagged") => {
                eprintln!(
                    "warning: dropped {} events; this reader fell behind",
                    event.data
                )
            }
            Some(other) => eprintln!("warning: unrecognized stream event {other:?}"),
            None => match output_format {
                OutputFormat::Pretty | OutputFormat::Text => {
                    println!("{}", render_event_line(event.data))
                }
                // Already one JSON object per line — the format this asks for.
                OutputFormat::Json | OutputFormat::Ndjson => println!("{}", event.data),
            },
        }
        true
    })
}

/// Renders one event line for a person rather than for `jq`.
///
/// Falls back to the raw line when it does not parse, because a line this does
/// not understand is exactly the one worth seeing verbatim.
fn render_event_line(data: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
        return data.to_owned();
    };
    let field = |key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("-")
            .to_owned()
    };
    let mut line = format!(
        "{}  {:<12} {:<10} {}",
        field("ts"),
        field("source"),
        field("agent"),
        field("event")
    );
    if let Some(tool) = value.get("tool_name").and_then(serde_json::Value::as_str) {
        line.push_str(&format!("  {tool}"));
    }
    if let Some(preview) = value
        .get("tool_input_preview")
        .and_then(serde_json::Value::as_str)
    {
        line.push_str(&format!("  {preview}"));
    }
    line
}

pub(super) fn run_slash_command(
    command: SlashCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        SlashCommand::List(args) => {
            run_action_with_params(args, ActionKind::SlashList, EmptyParams {}, output_format)
        }
        SlashCommand::Run(args) => run_action_with_params(
            args.target,
            ActionKind::SlashRun,
            SlashRunParams {
                command: args.command,
                argument: args.argument,
                force: args.force,
            },
            output_format,
        ),
    }
}

pub(super) fn run_drive_command(
    command: DriveCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        DriveCommand::Status(args) => run_action_with_params(
            args,
            ActionKind::DriveSyncStatus,
            EmptyParams {},
            output_format,
        ),
        DriveCommand::Export(args) => run_action_with_params(
            args,
            ActionKind::DriveSyncExport,
            EmptyParams {},
            output_format,
        ),
        DriveCommand::Import(args) => run_action_with_params(
            args,
            ActionKind::DriveSyncImport,
            EmptyParams {},
            output_format,
        ),
        DriveCommand::Object(command) => run_drive_object_command(command, output_format),
    }
}

fn run_drive_object_command(
    command: DriveObjectCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        DriveObjectCommand::List(args) => run_action_with_params(
            args.target,
            ActionKind::DriveObjectList,
            DriveObjectListParams {
                include_trashed: args.include_trashed,
                object_type: args.object_type,
            },
            output_format,
        ),
        DriveObjectCommand::Get(args) => run_action_with_params(
            args.target,
            ActionKind::DriveObjectGet,
            DriveObjectGetParams { id: args.id },
            output_format,
        ),
        DriveObjectCommand::Create(args) => {
            let body = match (&args.body, &args.body_file) {
                (Some(body), _) => Some(body.clone()),
                (None, Some(path)) => Some(read_body(path)?),
                (None, None) => None,
            };
            run_action_with_params(
                args.target,
                ActionKind::DriveObjectCreate,
                DriveObjectCreateParams {
                    object_type: args.object_type,
                    name: args.name,
                    body,
                    folder: args.folder,
                },
                output_format,
            )
        }
        DriveObjectCommand::Trash(args) => run_action_with_params(
            args.target,
            ActionKind::DriveObjectTrash,
            DriveObjectTrashParams { id: args.id },
            output_format,
        ),
    }
}

/// Reads `--body-file`, or stdin when it is `-`.
///
/// Stdin matters more than it looks: a workflow's JSON is the output of
/// `drive object get` piped through `jq`, and a notebook is a markdown file
/// that already exists. Neither belongs on a command line.
fn read_body(path: &Path) -> Result<String, ControlError> {
    if path == Path::new("-") {
        let mut body = String::new();
        return std::io::Read::read_to_string(&mut std::io::stdin(), &mut body)
            .map(|_| body)
            .map_err(|err| {
                ControlError::new(
                    ErrorCode::InvalidParams,
                    format!("could not read the body from stdin: {err}"),
                )
            });
    }

    std::fs::read_to_string(path).map_err(|err| {
        ControlError::new(
            ErrorCode::InvalidParams,
            format!("could not read {}: {err}", path.display()),
        )
    })
}

fn tab_activation_mode(args: &TabActivateArgs) -> TabActivationMode {
    if args.previous {
        TabActivationMode::Previous
    } else if args.next {
        TabActivationMode::Next
    } else if args.last {
        TabActivationMode::Last
    } else {
        TabActivationMode::Target
    }
}

fn tab_close_mode(args: &TabCloseArgs) -> TabCloseMode {
    if args.others {
        TabCloseMode::Others
    } else if args.right_of {
        TabCloseMode::RightOf
    } else if args.active {
        TabCloseMode::Active
    } else {
        TabCloseMode::Target
    }
}

fn run_surface_query_command(
    command: SurfaceQueryCommand,
    action: ActionKind,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        SurfaceQueryCommand::Open(args) => run_action_with_params(
            args.target,
            action,
            QueryParams { query: args.query },
            output_format,
        ),
    }
}

fn run_surface_open_command(
    command: SurfaceOpenCommand,
    action: ActionKind,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        SurfaceOpenCommand::Open(args) => {
            run_action_with_params(args, action, EmptyParams {}, output_format)
        }
    }
}
fn run_surface_toggle_command(
    command: SurfaceToggleCommand,
    action: ActionKind,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        SurfaceToggleCommand::Toggle(args) => {
            run_action_with_params(args, action, EmptyParams {}, output_format)
        }
    }
}

fn run_action(
    args: TargetArgs,
    action: ActionKind,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    run_action_with_params(args, action, EmptyParams {}, output_format)
}

/// Sends one action and hands back what it answered.
///
/// Split out of [`run_action_with_params`] for callers that act on the result
/// rather than print it — `graph run` polls `agent.read` and decides what to
/// spawn next from the answer.
pub(super) fn send_action<T: Serialize>(
    args: &TargetArgs,
    action: ActionKind,
    params: T,
) -> Result<serde_json::Value, ControlError> {
    let selector = instance_selector(args);
    let records = local_control::discovery::list_instances(&ChannelState::channel().to_string());
    let target = target_selector(args)?;
    let instance = select_instance(&records, &selector)?;
    let mut request = RequestEnvelope::new(Action::with_params(action, params)?);
    request.target = target;
    let response = local_control::client::send_request(&instance, &request)?;
    let local_control::protocol::ControlResponse::Ok { data } = response.response else {
        return Err(ControlError::new(
            ErrorCode::Internal,
            "local-control request failed without an error payload",
        ));
    };
    Ok(data)
}

fn run_action_with_params<T: Serialize>(
    args: TargetArgs,
    action: ActionKind,
    params: T,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    let data = send_action(&args, action, params)?;
    match output_format {
        OutputFormat::Json => write_json(&data),
        OutputFormat::Ndjson => write_json_line(&data),
        OutputFormat::Pretty | OutputFormat::Text => {
            println!("{}", render_human_readable(action, &data));
            Ok(())
        }
    }
}

fn parse_json_value_or_string(value: String) -> serde_json::Value {
    serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value))
}
