use std::collections::HashSet;

use clap_complete::aot::Shell;
use local_control::protocol::{ActionKind, ControlError, ErrorCode};
use serde_json::json;

use super::*;

#[test]
fn parses_typed_create_and_setting_list_params() {
    let args = ControlArgs::try_parse_from([
        "warpctrl",
        "tab",
        "create",
        "--type",
        "agent",
        "--session",
        "session_1",
    ])
    .expect("tab create parses");
    let ControlCommand::Tab(TabCommand::Create(args)) = args.command else {
        panic!("expected tab create command");
    };
    assert_eq!(args.tab_type, Some(CliTabType::Agent));
    assert_eq!(args.target.session.as_deref(), Some("session_1"));

    let err = ControlArgs::try_parse_from(["warpctrl", "tab", "create", "--shell", "zsh"])
        .expect_err("shell is not an accepted tab create flag");
    assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);

    let args =
        ControlArgs::try_parse_from(["warpctrl", "setting", "list", "--namespace", "editor"])
            .expect("setting list parses");
    let ControlCommand::Setting(SettingCommand::List(args)) = args.command else {
        panic!("expected setting list command");
    };
    assert_eq!(args.namespace.as_deref(), Some("editor"));
}

#[test]
fn rejects_conflicting_instance_selectors() {
    let err = ControlArgs::try_parse_from([
        "warpctrl",
        "tab",
        "create",
        "--instance",
        "inst_123",
        "--pid",
        "123",
    ])
    .expect_err("instance and pid conflict");
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn parses_instance_and_pid_selectors() {
    let args = ControlArgs::try_parse_from(["warpctrl", "tab", "create", "--instance", "inst_123"])
        .expect("instance selector parses");
    let ControlCommand::Tab(TabCommand::Create(create)) = args.command else {
        panic!("expected tab create command");
    };
    assert_eq!(create.target.instance.as_deref(), Some("inst_123"));

    let args = ControlArgs::try_parse_from(["warpctrl", "app", "ping", "--pid", "123"])
        .expect("pid selector parses");
    let ControlCommand::App(AppCommand::Ping(target)) = args.command else {
        panic!("expected app ping command");
    };
    assert_eq!(target.pid, Some(123));
}

#[test]
fn surface_list_accepts_instance_selection() {
    let args =
        ControlArgs::try_parse_from(["warpctrl", "surface", "list", "--instance", "inst_123"])
            .expect("surface list instance selector parses");
    let ControlCommand::Surface(SurfaceCommand::List(target)) = args.command else {
        panic!("expected surface list command");
    };
    assert_eq!(target.instance.as_deref(), Some("inst_123"));
}

#[test]
fn rejects_excluded_command_routes() {
    for args in [
        vec!["warpctrl", "history", "list"],
        vec!["warpctrl", "block", "list"],
        vec!["warpctrl", "block", "inspect", "block_1"],
        vec!["warpctrl", "block", "output", "block_1"],
        vec!["warpctrl", "input", "get"],
        vec!["warpctrl", "input", "clear"],
        vec!["warpctrl", "input", "mode", "set", "agent"],
        vec!["warpctrl", "input", "run", "pwd"],
        vec!["warpctrl", "file", "list"],
        vec!["warpctrl", "drive", "list"],
        vec!["warpctrl", "auth", "status"],
    ] {
        assert!(ControlArgs::try_parse_from(args).is_err());
    }
}

#[test]
fn parses_first_slice_instance_list() {
    let args = ControlArgs::try_parse_from(["warpctrl", "instance", "list"])
        .expect("instance list parses");
    assert!(matches!(
        args.command,
        ControlCommand::Instance(InstanceCommand::List)
    ));
}

#[test]
fn parses_first_slice_app_smoke_metadata_commands() {
    assert!(ControlArgs::try_parse_from(["warpctrl", "app", "ping"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "app", "version"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "app", "active"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "app", "focus"]).is_ok());
}

#[test]
fn parses_catalog_metadata_commands() {
    let args =
        ControlArgs::try_parse_from(["warpctrl", "action", "inspect", "surface.settings.open"])
            .expect("action inspect parses");
    let ControlCommand::Action(ActionCatalogCommand::Inspect { action }) = args.command else {
        panic!("expected action inspect command");
    };
    assert_eq!(action, "surface.settings.open");
    assert!(ControlArgs::try_parse_from(["warpctrl", "action", "list"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "capability", "list"]).is_ok());
    assert!(
        ControlArgs::try_parse_from(["warpctrl", "capability", "inspect", "tab.create"]).is_ok()
    );
}

#[test]
fn parses_control_mode_args_after_hidden_flag() {
    let args = ControlArgs::try_parse_control_mode_from(["warp", "--warpctrl", "tab", "create"])
        .expect("control mode flag is present")
        .expect("control mode args parse");
    assert!(matches!(
        args.command,
        ControlCommand::Tab(TabCommand::Create(_))
    ));
}

#[test]
fn ignores_args_without_control_mode_flag() {
    assert!(ControlArgs::try_parse_control_mode_from(["warp", "tab", "create"]).is_none());
}

#[test]
fn parses_completion_generation_command() {
    let args = ControlArgs::try_parse_from(["warpctrl", "completions", "bash"])
        .expect("completions parses");
    assert!(matches!(
        args.command,
        ControlCommand::Completions {
            shell: Some(Shell::Bash)
        }
    ));
}

#[test]
fn parses_exact_window_tab_pane_and_session_selectors() {
    let args = ControlArgs::try_parse_from([
        "warpctrl",
        "session",
        "inspect",
        "--window-title",
        "docs",
        "--tab-index",
        "2",
        "--pane",
        "pane_1",
        "--session",
        "session_1",
    ])
    .expect("exact target selectors parse");
    let ControlCommand::Session(SessionCommand::Inspect(target)) = args.command else {
        panic!("expected session inspect command");
    };
    assert_eq!(target.window_title.as_deref(), Some("docs"));
    assert_eq!(target.tab_index, Some(2));
    assert_eq!(target.pane.as_deref(), Some("pane_1"));
    assert_eq!(target.session.as_deref(), Some("session_1"));
}

#[test]
fn instance_list_output_serializes_empty_and_populated_lists() {
    let empty = serde_json::to_value(commands::instance_list_output(Vec::new()))
        .expect("empty list serializes");
    assert_eq!(empty, json!({ "instances": [] }));

    let record = local_control::discovery::InstanceRecord::for_current_process(
        None,
        "dev",
        "dev.warp.Warp",
        Some("v0.1.0".to_owned()),
        Vec::new(),
    );
    let instance_id = record.instance_id.0.clone();
    let populated = serde_json::to_value(commands::instance_list_output(vec![record]))
        .expect("populated list serializes");
    assert_eq!(populated["instances"][0]["instance_id"], json!(instance_id));
    assert_eq!(populated["instances"][0]["channel"], json!("dev"));
    assert_eq!(populated["instances"][0]["app_id"], json!("dev.warp.Warp"));
    assert_eq!(populated["instances"][0]["app_version"], json!("v0.1.0"));
}

#[test]
fn excluded_actions_are_not_allowlisted_catalog_entries() {
    for excluded in ["auth.api_key.set", "file.write", "block.list"] {
        assert!(
            ActionKind::ALL
                .iter()
                .all(|action| action.as_str() != excluded)
        );
    }
}

#[test]
fn generated_bash_completions_include_readonly_commands() {
    let completions =
        generate_completion_string(Shell::Bash).expect("bash completions render to UTF-8");
    assert!(completions.contains("instance"));
    assert!(completions.contains("action"));
    assert!(completions.contains("capability"));
    assert!(!completions.contains("stubs-only"));
    assert!(completions.contains("window"));
    assert!(completions.contains("input"));
    assert!(completions.contains("completions"));
    assert!(!completions.contains("block"));
}

#[test]
fn every_retained_catalog_action_has_a_parseable_cli_example() {
    let mut covered = HashSet::new();
    for (kind, argv) in retained_action_examples() {
        let args = ControlArgs::try_parse_from(argv)
            .unwrap_or_else(|err| panic!("{} parses: {err}", kind.as_str()));
        assert_eq!(parsed_action_kind(&args.command), Some(kind));
        covered.insert(kind);
    }
    let expected = ActionKind::ALL.iter().copied().collect::<HashSet<_>>();
    let missing = expected
        .difference(&covered)
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "retained catalog actions missing parser examples: {missing:?}"
    );
}

#[test]
fn generated_bash_completions_include_mutating_command_groups() {
    let completions =
        generate_completion_string(Shell::Bash).expect("bash completions render to UTF-8");
    assert!(completions.contains("surface"));
    assert!(completions.contains("command-palette"));
    assert!(completions.contains("warp-drive"));
    assert!(completions.contains("resource-center"));
    assert!(completions.contains("activate"));
    assert!(completions.contains("split"));
    assert!(!completions.contains("history"));
    assert!(!completions.contains("share-to-team"));
}

#[test]
fn structured_error_output_uses_stable_error_code() {
    let error = ControlError::new(ErrorCode::NoInstance, "no local Warp control instances");
    let value = serde_json::to_value(ErrorSummary {
        ok: false,
        error: &error,
    })
    .expect("error summary serializes");
    assert_eq!(value["ok"], json!(false));
    assert_eq!(value["error"]["code"], json!("no_instance"));
    assert_eq!(
        value["error"]["message"],
        json!("no local Warp control instances")
    );
}

#[test]
fn renders_human_readable_tab_create_output() {
    let rendered = render_human_readable_for_test(
        local_control::protocol::ActionKind::TabCreate,
        &json!({
            "tab": {
                "id": "tab_123",
                "active_index": 2,
                "count": 3
            },
            "window": {
                "id": "window_123"
            }
        }),
    );
    assert_eq!(
        rendered,
        "Created tab tab_123 in window window_123 (active index 2, tab count 3)"
    );
}

/// The listing ends in the command that answers it (T14.8).
///
/// Pinned because the digest is the part that is easy to drop while "making it
/// friendlier": a listing that names the request without carrying its digest
/// forward would push a person toward an addressing scheme that has none, which
/// is exactly the loosening the digest exists to prevent.
#[test]
fn renders_an_approvable_request_with_the_command_that_answers_it() {
    let rendered = render_human_readable_for_test(
        local_control::protocol::ActionKind::AgentApprovals,
        &json!({
            "approvals": [{
                "approval_id": "turn-1:7",
                "agent": "opencode",
                "source": "acp",
                "kind": "permission",
                "summary": "git status --short",
                "tool_name": "execute",
                "tool_input": "{\"command\":\"git status --short\"}",
                "acts_on": [],
                "options_offered": [
                    {"name": "Allow once", "warp_can_select": true},
                    {"name": "Always allow", "warp_can_select": false},
                    {"name": "Reject", "warp_can_select": true}
                ],
                "digest": "abc123",
                "can_approve": true,
                "approve_selects": "once"
            }]
        }),
    );

    assert!(
        rendered.contains("warpctrl agent approve 'turn-1:7' --digest abc123"),
        "a yes should be one paste, got:\n{rendered}"
    );
    assert!(
        rendered.contains("warpctrl agent deny 'turn-1:7' --digest abc123"),
        "a no is always offered, got:\n{rendered}"
    );
    // **T20.2: the list is a record, and it has to read like one.** This
    // rendered "Allow once, Always allow, Reject" beside a single approve
    // command, so a person read a menu whose middle item had simply lost its
    // control. The middle item can never be selected -- `acp_permission::choose`
    // refuses any option that would set a session policy -- and nothing said so.
    assert!(
        rendered.contains("Always allow (Warp never selects this)"),
        "an option Warp will never send back has to say so, got:\n{rendered}"
    );
    // The other two are untouched, which is the calibration: a renderer that
    // annotated everything would be as wrong as one that annotated nothing, and
    // would pass the assertion above.
    assert!(
        rendered.contains("Allow once | Always allow (Warp never selects this) | Reject\n"),
        "only the unselectable option is annotated, got:\n{rendered}"
    );
    assert!(
        rendered.contains("not stated by the agent"),
        "an empty acts_on says so rather than borrowing cwd, got:\n{rendered}"
    );
}

/// A request Warp will not say yes to shows the reason and no yes line — and
/// still shows the no. The measured failure it guards is a turn that parked
/// while its operator worked out that denying was the only move left.
#[test]
fn renders_an_unapprovable_request_with_its_reason_and_only_a_no() {
    let rendered = render_human_readable_for_test(
        local_control::protocol::ActionKind::AgentApprovals,
        &json!({
            "approvals": [{
                "approval_id": "turn-1:9",
                "agent": "opencode",
                "source": "acp",
                "kind": "permission",
                "summary": "cat /etc/hostname",
                "tool_name": "other",
                "tool_input": "{\"command\":\"cat /etc/hostname\"}",
                "acts_on": ["/etc"],
                "options_offered": [
                    {"name": "Allow once", "warp_can_select": true},
                    {"name": "Always allow", "warp_can_select": false},
                    {"name": "Reject", "warp_can_select": true}
                ],
                "digest": "def456",
                "can_approve": false,
                "approve_refused_because": "the call's kind is `other`, so Warp cannot tell."
            }]
        }),
    );

    assert!(
        !rendered.contains("agent approve"),
        "no yes may be offered for an entry that has none, got:\n{rendered}"
    );
    assert!(
        rendered.contains("warpctrl agent deny 'turn-1:9' --digest def456"),
        "a no must still be one paste, got:\n{rendered}"
    );
    assert!(
        rendered.contains("Warp cannot tell"),
        "the reason reaches the person, got:\n{rendered}"
    );
}

/// The yes is gated on `can_approve`, not on a reason being present.
///
/// T14.6's bug, one surface over: the console drew its *Yes* from a per-device
/// fact with no per-entry check, so a phone showed a button on rows that could
/// never work. An entry refused without an explanation is still refused, and a
/// renderer that keyed on the explanation would reintroduce exactly that.
#[test]
fn offers_no_yes_for_an_unapprovable_request_that_gave_no_reason() {
    let rendered = render_human_readable_for_test(
        local_control::protocol::ActionKind::AgentApprovals,
        &json!({
            "approvals": [{
                "approval_id": "turn-1:11",
                "agent": "codex",
                "source": "pane",
                "kind": "permission",
                "summary": "rm -rf /",
                "digest": "aaa",
                "can_approve": false
            }]
        }),
    );

    assert!(
        !rendered.contains("agent approve"),
        "can_approve false is the gate, with or without a reason, got:\n{rendered}"
    );
    assert!(
        rendered.contains("did not say why"),
        "an unexplained refusal still says something, got:\n{rendered}"
    );
}

/// Empty says what empty means, **in the payload and not only in a comment above
/// it.**
///
/// The caveat is the assertion. T14.19 measured a poll reporting zero parked
/// approvals while a request was genuinely waiting, and that phantom zero was one
/// inference away from a security investigation into an auto-approval hole that
/// does not exist. An empty list is not evidence that nothing is running, and the
/// person reading it is the one who needs told.
///
/// **This test was left red for several hours on 2026-08-31**, by the same commit
/// that added the caveat: `cargo test -p local_control` and `-p warp --lib` were
/// run, `-p warp_cli` was not, and the string lives here. Exactly the pattern this
/// repo already documents from T8.6 — one pin updated, its twin shipped red —
/// committed by someone who had read that warning the same day. Assert against the
/// constant rather than a copy of the sentence, so the next edit cannot do it
/// again.
#[test]
fn renders_an_empty_approval_list_as_a_sentence() {
    let rendered = render_human_readable_for_test(
        local_control::protocol::ActionKind::AgentApprovals,
        &json!({ "approvals": [] }),
    );

    assert_eq!(rendered, commands::NOTHING_IS_WAITING);
    assert!(
        rendered.contains("not evidence that nothing is running"),
        "the empty case must carry its own caveat, not rely on a doc comment: {rendered}"
    );
}

fn retained_action_examples() -> Vec<(ActionKind, Vec<&'static str>)> {
    vec![
        (
            ActionKind::InstanceList,
            vec!["warpctrl", "instance", "list"],
        ),
        (
            ActionKind::InstanceInspect,
            vec!["warpctrl", "instance", "inspect"],
        ),
        (ActionKind::AppPing, vec!["warpctrl", "app", "ping"]),
        (ActionKind::AppVersion, vec!["warpctrl", "app", "version"]),
        (ActionKind::AppActive, vec!["warpctrl", "app", "active"]),
        (ActionKind::AppFocus, vec!["warpctrl", "app", "focus"]),
        (
            ActionKind::CapabilityList,
            vec!["warpctrl", "capability", "list"],
        ),
        (
            ActionKind::CapabilityInspect,
            vec!["warpctrl", "capability", "inspect", "tab.create"],
        ),
        (ActionKind::WindowList, vec!["warpctrl", "window", "list"]),
        (
            ActionKind::WindowInspect,
            vec!["warpctrl", "window", "inspect"],
        ),
        (
            ActionKind::WindowCreate,
            vec!["warpctrl", "window", "create"],
        ),
        (ActionKind::WindowFocus, vec!["warpctrl", "window", "focus"]),
        (ActionKind::WindowClose, vec!["warpctrl", "window", "close"]),
        (ActionKind::TabList, vec!["warpctrl", "tab", "list"]),
        (ActionKind::TabInspect, vec!["warpctrl", "tab", "inspect"]),
        (ActionKind::TabCreate, vec!["warpctrl", "tab", "create"]),
        (ActionKind::TabActivate, vec!["warpctrl", "tab", "activate"]),
        (
            ActionKind::TabMove,
            vec!["warpctrl", "tab", "move", "--direction", "next"],
        ),
        (ActionKind::TabClose, vec!["warpctrl", "tab", "close"]),
        (
            ActionKind::TabRename,
            vec!["warpctrl", "tab", "rename", "docs"],
        ),
        (
            ActionKind::TabResetName,
            vec!["warpctrl", "tab", "reset-name"],
        ),
        (
            ActionKind::TabColorSet,
            vec!["warpctrl", "tab", "color", "set", "red"],
        ),
        (
            ActionKind::TabColorClear,
            vec!["warpctrl", "tab", "color", "clear"],
        ),
        (ActionKind::PaneList, vec!["warpctrl", "pane", "list"]),
        (ActionKind::PaneInspect, vec!["warpctrl", "pane", "inspect"]),
        (
            ActionKind::PaneSplit,
            vec!["warpctrl", "pane", "split", "--direction", "right"],
        ),
        (ActionKind::PaneFocus, vec!["warpctrl", "pane", "focus"]),
        (
            ActionKind::PaneNavigate,
            vec!["warpctrl", "pane", "navigate", "--direction", "next"],
        ),
        (
            ActionKind::PaneResize,
            vec![
                "warpctrl",
                "pane",
                "resize",
                "--direction",
                "right",
                "--amount",
                "4",
            ],
        ),
        (
            ActionKind::PaneMaximize,
            vec!["warpctrl", "pane", "maximize"],
        ),
        (
            ActionKind::PaneUnmaximize,
            vec!["warpctrl", "pane", "unmaximize"],
        ),
        (ActionKind::PaneClose, vec!["warpctrl", "pane", "close"]),
        (
            ActionKind::PaneRename,
            vec!["warpctrl", "pane", "rename", "server"],
        ),
        (
            ActionKind::PaneResetName,
            vec!["warpctrl", "pane", "reset-name"],
        ),
        (ActionKind::SessionList, vec!["warpctrl", "session", "list"]),
        (
            ActionKind::SessionInspect,
            vec!["warpctrl", "session", "inspect"],
        ),
        (
            ActionKind::SessionActivate,
            vec!["warpctrl", "session", "activate"],
        ),
        (
            ActionKind::SessionPrevious,
            vec!["warpctrl", "session", "previous"],
        ),
        (ActionKind::SessionNext, vec!["warpctrl", "session", "next"]),
        (
            ActionKind::SessionReopenClosed,
            vec!["warpctrl", "session", "reopen-closed"],
        ),
        (
            ActionKind::InputInsert,
            vec!["warpctrl", "input", "insert", "hello"],
        ),
        (
            ActionKind::InputReplace,
            vec!["warpctrl", "input", "replace", "hello"],
        ),
        (
            ActionKind::InputSubmit,
            vec!["warpctrl", "input", "submit", "pwd"],
        ),
        (ActionKind::ThemeList, vec!["warpctrl", "theme", "list"]),
        (ActionKind::ThemeGet, vec!["warpctrl", "theme", "get"]),
        (
            ActionKind::ThemeSet,
            vec!["warpctrl", "theme", "set", "Dracula"],
        ),
        (
            ActionKind::ThemeSystemSet,
            vec!["warpctrl", "theme", "system-set", "true"],
        ),
        (
            ActionKind::ThemeLightSet,
            vec!["warpctrl", "theme", "light-set", "Light"],
        ),
        (
            ActionKind::ThemeDarkSet,
            vec!["warpctrl", "theme", "dark-set", "Dark"],
        ),
        (
            ActionKind::AppearanceGet,
            vec!["warpctrl", "appearance", "get"],
        ),
        (
            ActionKind::AppearanceFontSizeIncrease,
            vec!["warpctrl", "appearance", "font-size-increase"],
        ),
        (
            ActionKind::AppearanceFontSizeDecrease,
            vec!["warpctrl", "appearance", "font-size-decrease"],
        ),
        (
            ActionKind::AppearanceFontSizeReset,
            vec!["warpctrl", "appearance", "font-size-reset"],
        ),
        (
            ActionKind::AppearanceZoomIncrease,
            vec!["warpctrl", "appearance", "zoom-increase"],
        ),
        (
            ActionKind::AppearanceZoomDecrease,
            vec!["warpctrl", "appearance", "zoom-decrease"],
        ),
        (
            ActionKind::AppearanceZoomReset,
            vec!["warpctrl", "appearance", "zoom-reset"],
        ),
        (ActionKind::SettingList, vec!["warpctrl", "setting", "list"]),
        (
            ActionKind::SettingGet,
            vec!["warpctrl", "setting", "get", "font_size"],
        ),
        (
            ActionKind::SettingSet,
            vec!["warpctrl", "setting", "set", "font_size", "14"],
        ),
        (
            ActionKind::SettingToggle,
            vec!["warpctrl", "setting", "toggle", "autosuggestions"],
        ),
        (
            ActionKind::KeybindingList,
            vec!["warpctrl", "keybinding", "list"],
        ),
        (
            ActionKind::KeybindingGet,
            vec!["warpctrl", "keybinding", "get", "copy"],
        ),
        (ActionKind::ActionList, vec!["warpctrl", "action", "list"]),
        (
            ActionKind::ActionInspect,
            vec!["warpctrl", "action", "inspect", "tab.create"],
        ),
        (ActionKind::SurfaceList, vec!["warpctrl", "surface", "list"]),
        (
            ActionKind::SurfaceSettingsOpen,
            vec!["warpctrl", "surface", "settings", "open"],
        ),
        (
            ActionKind::SurfaceCommandPaletteOpen,
            vec!["warpctrl", "surface", "command-palette", "open"],
        ),
        (
            ActionKind::SurfaceCommandSearchOpen,
            vec!["warpctrl", "surface", "command-search", "open"],
        ),
        (
            ActionKind::SurfaceThemePickerOpen,
            vec!["warpctrl", "surface", "theme-picker", "open"],
        ),
        (
            ActionKind::SurfaceKeybindingsOpen,
            vec!["warpctrl", "surface", "keybindings", "open"],
        ),
        (
            ActionKind::SurfaceWarpDriveOpen,
            vec!["warpctrl", "surface", "warp-drive", "open"],
        ),
        (
            ActionKind::SurfaceWarpDriveToggle,
            vec!["warpctrl", "surface", "warp-drive", "toggle"],
        ),
        (
            ActionKind::SurfaceResourceCenterToggle,
            vec!["warpctrl", "surface", "resource-center", "toggle"],
        ),
        (
            ActionKind::SurfaceAiAssistantToggle,
            vec!["warpctrl", "surface", "ai-assistant", "toggle"],
        ),
        (
            ActionKind::SurfaceCodeReviewOpen,
            vec!["warpctrl", "surface", "code-review", "open"],
        ),
        (
            ActionKind::SurfaceCodeReviewToggle,
            vec!["warpctrl", "surface", "code-review", "toggle"],
        ),
        (
            ActionKind::SurfaceProjectExplorerOpen,
            vec!["warpctrl", "surface", "project-explorer", "open"],
        ),
        (
            ActionKind::SurfaceGlobalSearchOpen,
            vec!["warpctrl", "surface", "global-search", "open"],
        ),
        (
            ActionKind::SurfaceConversationListOpen,
            vec!["warpctrl", "surface", "conversation-list", "open"],
        ),
        (
            ActionKind::SurfaceLeftPanelToggle,
            vec!["warpctrl", "surface", "left-panel", "toggle"],
        ),
        (
            ActionKind::SurfaceRightPanelToggle,
            vec!["warpctrl", "surface", "right-panel", "toggle"],
        ),
        (
            ActionKind::SurfaceVerticalTabsOpen,
            vec!["warpctrl", "surface", "vertical-tabs", "open"],
        ),
        (
            ActionKind::SurfaceVerticalTabsToggle,
            vec!["warpctrl", "surface", "vertical-tabs", "toggle"],
        ),
        (
            ActionKind::SurfaceAgentManagementOpen,
            vec!["warpctrl", "surface", "agent-management", "open"],
        ),
        (
            ActionKind::FileOpen,
            vec!["warpctrl", "file", "open", "/tmp/example.txt"],
        ),
        (
            ActionKind::DriveSyncStatus,
            vec!["warpctrl", "drive", "status"],
        ),
        (
            ActionKind::DriveSyncExport,
            vec!["warpctrl", "drive", "export"],
        ),
        (
            ActionKind::DriveSyncImport,
            vec!["warpctrl", "drive", "import"],
        ),
        (
            ActionKind::DriveObjectList,
            vec!["warpctrl", "drive", "object", "list"],
        ),
        (
            ActionKind::DriveObjectGet,
            vec!["warpctrl", "drive", "object", "get", "Client-example"],
        ),
        (
            ActionKind::DriveObjectCreate,
            vec![
                "warpctrl",
                "drive",
                "object",
                "create",
                "--type",
                "workflow",
                "--name",
                "ship",
                "--body",
                "{\"name\":\"ship\",\"command\":\"echo ship\"}",
            ],
        ),
        (
            ActionKind::DriveObjectTrash,
            vec!["warpctrl", "drive", "object", "trash", "Client-example"],
        ),
        (ActionKind::AgentList, vec!["warpctrl", "agent", "list"]),
        (
            ActionKind::AgentPrompt,
            vec!["warpctrl", "agent", "prompt", "summarise the diff"],
        ),
        (
            ActionKind::AgentRead,
            vec![
                "warpctrl",
                "agent",
                "read",
                "3f2f0e6a-0000-4000-8000-000000000000",
                "--last",
                "1",
            ],
        ),
        (
            ActionKind::AgentSpawn,
            vec![
                "warpctrl",
                "agent",
                "spawn",
                "review the diff",
                "--name",
                "reviewer",
                "--allow-tools",
                "read-only",
            ],
        ),
        (
            ActionKind::AgentCancel,
            vec![
                "warpctrl",
                "agent",
                "cancel",
                "3f2f0e6a-0000-4000-8000-000000000000",
            ],
        ),
        (
            ActionKind::AgentSettle,
            vec![
                "warpctrl",
                "agent",
                "settle",
                "3f2f0e6a-0000-4000-8000-000000000000",
            ],
        ),
        (
            ActionKind::AgentReveal,
            vec![
                "warpctrl",
                "agent",
                "reveal",
                "3f2f0e6a-0000-4000-8000-000000000000",
                "--as",
                "tab",
            ],
        ),
        (
            ActionKind::RemoteWslList,
            vec!["warpctrl", "remote", "wsl", "list"],
        ),
        (
            ActionKind::RemoteWslConnect,
            vec!["warpctrl", "remote", "wsl", "connect", "--distro", "Ubuntu"],
        ),
        (
            ActionKind::PaneMainGet,
            vec!["warpctrl", "pane", "main", "get"],
        ),
        (
            ActionKind::PaneMainSet,
            vec!["warpctrl", "pane", "main", "set"],
        ),
        (
            ActionKind::PaneMainClear,
            vec!["warpctrl", "pane", "main", "clear"],
        ),
        (
            ActionKind::TabMerge,
            vec![
                "warpctrl",
                "tab",
                "merge",
                "--tab-index",
                "1",
                "--direction",
                "right",
            ],
        ),
        (
            ActionKind::WindowVisorToggle,
            vec!["warpctrl", "window", "visor", "toggle"],
        ),
        (
            ActionKind::WindowVisorStatus,
            vec!["warpctrl", "window", "visor", "status"],
        ),
        (ActionKind::SlashList, vec!["warpctrl", "slash", "list"]),
        (
            ActionKind::EventsSubscribe,
            vec!["warpctrl", "events", "subscribe"],
        ),
        (ActionKind::ControlPair, vec!["warpctrl", "pair", "show"]),
        (
            ActionKind::AgentApprovals,
            vec!["warpctrl", "agent", "approvals"],
        ),
        (
            ActionKind::AgentApprove,
            vec!["warpctrl", "agent", "approve", "7", "--digest", "abc"],
        ),
        (
            ActionKind::AgentDeny,
            vec!["warpctrl", "agent", "deny", "7", "--digest", "abc"],
        ),
        (
            ActionKind::SlashRun,
            vec!["warpctrl", "slash", "run", "compact"],
        ),
    ]
}

fn parsed_action_kind(command: &ControlCommand) -> Option<ActionKind> {
    match command {
        ControlCommand::Instance(command) => match command {
            InstanceCommand::List => Some(ActionKind::InstanceList),
            InstanceCommand::Inspect(_) => Some(ActionKind::InstanceInspect),
        },
        ControlCommand::App(command) => match command {
            AppCommand::Ping(_) => Some(ActionKind::AppPing),
            AppCommand::Version(_) => Some(ActionKind::AppVersion),
            AppCommand::Active(_) => Some(ActionKind::AppActive),
            AppCommand::Focus(_) => Some(ActionKind::AppFocus),
        },
        ControlCommand::Capability(command) => match command {
            CapabilityCommand::List => Some(ActionKind::CapabilityList),
            CapabilityCommand::Inspect { .. } => Some(ActionKind::CapabilityInspect),
        },
        ControlCommand::Action(command) => match command {
            ActionCatalogCommand::List => Some(ActionKind::ActionList),
            ActionCatalogCommand::Inspect { .. } => Some(ActionKind::ActionInspect),
        },
        ControlCommand::Window(command) => match command {
            WindowCommand::List(_) => Some(ActionKind::WindowList),
            WindowCommand::Inspect(_) => Some(ActionKind::WindowInspect),
            WindowCommand::Create(_) => Some(ActionKind::WindowCreate),
            WindowCommand::Focus(_) => Some(ActionKind::WindowFocus),
            WindowCommand::Close(_) => Some(ActionKind::WindowClose),
            WindowCommand::Visor(command) => match command {
                WindowVisorCommand::Toggle(_) => Some(ActionKind::WindowVisorToggle),
                WindowVisorCommand::Status(_) => Some(ActionKind::WindowVisorStatus),
            },
        },
        ControlCommand::Tab(command) => match command {
            TabCommand::List(_) => Some(ActionKind::TabList),
            TabCommand::Inspect(_) => Some(ActionKind::TabInspect),
            TabCommand::Create(_) => Some(ActionKind::TabCreate),
            TabCommand::Activate(_) => Some(ActionKind::TabActivate),
            TabCommand::Move(_) => Some(ActionKind::TabMove),
            TabCommand::Merge(_) => Some(ActionKind::TabMerge),
            TabCommand::Close(_) => Some(ActionKind::TabClose),
            TabCommand::Rename(_) => Some(ActionKind::TabRename),
            TabCommand::ResetName(_) => Some(ActionKind::TabResetName),
            TabCommand::Color(command) => match command {
                TabColorCommand::Set(_) => Some(ActionKind::TabColorSet),
                TabColorCommand::Clear(_) => Some(ActionKind::TabColorClear),
            },
        },
        ControlCommand::Pane(command) => match command {
            PaneCommand::List(_) => Some(ActionKind::PaneList),
            PaneCommand::Inspect(_) => Some(ActionKind::PaneInspect),
            PaneCommand::Split(_) => Some(ActionKind::PaneSplit),
            PaneCommand::Focus(_) => Some(ActionKind::PaneFocus),
            PaneCommand::Navigate(_) => Some(ActionKind::PaneNavigate),
            PaneCommand::Resize(_) => Some(ActionKind::PaneResize),
            PaneCommand::Maximize(_) => Some(ActionKind::PaneMaximize),
            PaneCommand::Unmaximize(_) => Some(ActionKind::PaneUnmaximize),
            PaneCommand::Close(_) => Some(ActionKind::PaneClose),
            PaneCommand::Rename(_) => Some(ActionKind::PaneRename),
            PaneCommand::ResetName(_) => Some(ActionKind::PaneResetName),
            PaneCommand::Main(command) => match command {
                PaneMainCommand::Get(_) => Some(ActionKind::PaneMainGet),
                PaneMainCommand::Set(_) => Some(ActionKind::PaneMainSet),
                PaneMainCommand::Clear(_) => Some(ActionKind::PaneMainClear),
            },
        },
        ControlCommand::Session(command) => match command {
            SessionCommand::List(_) => Some(ActionKind::SessionList),
            SessionCommand::Inspect(_) => Some(ActionKind::SessionInspect),
            SessionCommand::Activate(_) => Some(ActionKind::SessionActivate),
            SessionCommand::Previous(_) => Some(ActionKind::SessionPrevious),
            SessionCommand::Next(_) => Some(ActionKind::SessionNext),
            SessionCommand::ReopenClosed(_) => Some(ActionKind::SessionReopenClosed),
        },
        ControlCommand::Input(command) => match command {
            InputCommand::Insert(_) => Some(ActionKind::InputInsert),
            InputCommand::Replace(_) => Some(ActionKind::InputReplace),
            InputCommand::Submit(_) => Some(ActionKind::InputSubmit),
        },
        ControlCommand::Theme(command) => match command {
            ThemeCommand::List(_) => Some(ActionKind::ThemeList),
            ThemeCommand::Get(_) => Some(ActionKind::ThemeGet),
            ThemeCommand::Set(_) => Some(ActionKind::ThemeSet),
            ThemeCommand::SystemSet(_) => Some(ActionKind::ThemeSystemSet),
            ThemeCommand::LightSet(_) => Some(ActionKind::ThemeLightSet),
            ThemeCommand::DarkSet(_) => Some(ActionKind::ThemeDarkSet),
        },
        ControlCommand::Appearance(command) => match command {
            AppearanceCommand::Get(_) => Some(ActionKind::AppearanceGet),
            AppearanceCommand::FontSizeIncrease(_) => Some(ActionKind::AppearanceFontSizeIncrease),
            AppearanceCommand::FontSizeDecrease(_) => Some(ActionKind::AppearanceFontSizeDecrease),
            AppearanceCommand::FontSizeReset(_) => Some(ActionKind::AppearanceFontSizeReset),
            AppearanceCommand::ZoomIncrease(_) => Some(ActionKind::AppearanceZoomIncrease),
            AppearanceCommand::ZoomDecrease(_) => Some(ActionKind::AppearanceZoomDecrease),
            AppearanceCommand::ZoomReset(_) => Some(ActionKind::AppearanceZoomReset),
        },
        ControlCommand::Setting(command) => match command {
            SettingCommand::List(_) => Some(ActionKind::SettingList),
            SettingCommand::Get(_) => Some(ActionKind::SettingGet),
            SettingCommand::Set(_) => Some(ActionKind::SettingSet),
            SettingCommand::Toggle(_) => Some(ActionKind::SettingToggle),
        },
        ControlCommand::Keybinding(command) => match command {
            KeybindingCommand::List(_) => Some(ActionKind::KeybindingList),
            KeybindingCommand::Get(_) => Some(ActionKind::KeybindingGet),
        },
        ControlCommand::File(command) => match command {
            FileCommand::Open(_) => Some(ActionKind::FileOpen),
        },
        ControlCommand::Surface(command) => match command {
            SurfaceCommand::List(_) => Some(ActionKind::SurfaceList),
            SurfaceCommand::Settings(command) => match command {
                SurfaceSettingsCommand::Open(_) => Some(ActionKind::SurfaceSettingsOpen),
            },
            SurfaceCommand::CommandPalette(command) => match command {
                SurfaceQueryCommand::Open(_) => Some(ActionKind::SurfaceCommandPaletteOpen),
            },
            SurfaceCommand::CommandSearch(command) => match command {
                SurfaceQueryCommand::Open(_) => Some(ActionKind::SurfaceCommandSearchOpen),
            },
            SurfaceCommand::ThemePicker(command) => match command {
                SurfaceOpenCommand::Open(_) => Some(ActionKind::SurfaceThemePickerOpen),
            },
            SurfaceCommand::Keybindings(command) => match command {
                SurfaceOpenCommand::Open(_) => Some(ActionKind::SurfaceKeybindingsOpen),
            },
            SurfaceCommand::WarpDrive(command) => match command {
                SurfaceOpenToggleCommand::Open(_) => Some(ActionKind::SurfaceWarpDriveOpen),
                SurfaceOpenToggleCommand::Toggle(_) => Some(ActionKind::SurfaceWarpDriveToggle),
            },
            SurfaceCommand::ResourceCenter(command) => match command {
                SurfaceToggleCommand::Toggle(_) => Some(ActionKind::SurfaceResourceCenterToggle),
            },
            SurfaceCommand::AiAssistant(command) => match command {
                SurfaceToggleCommand::Toggle(_) => Some(ActionKind::SurfaceAiAssistantToggle),
            },
            SurfaceCommand::CodeReview(command) => match command {
                SurfaceOpenToggleCommand::Open(_) => Some(ActionKind::SurfaceCodeReviewOpen),
                SurfaceOpenToggleCommand::Toggle(_) => Some(ActionKind::SurfaceCodeReviewToggle),
            },
            SurfaceCommand::ProjectExplorer(command) => match command {
                SurfaceOpenCommand::Open(_) => Some(ActionKind::SurfaceProjectExplorerOpen),
            },
            SurfaceCommand::GlobalSearch(command) => match command {
                SurfaceOpenCommand::Open(_) => Some(ActionKind::SurfaceGlobalSearchOpen),
            },
            SurfaceCommand::ConversationList(command) => match command {
                SurfaceOpenCommand::Open(_) => Some(ActionKind::SurfaceConversationListOpen),
            },
            SurfaceCommand::LeftPanel(command) => match command {
                SurfaceToggleCommand::Toggle(_) => Some(ActionKind::SurfaceLeftPanelToggle),
            },
            SurfaceCommand::RightPanel(command) => match command {
                SurfaceToggleCommand::Toggle(_) => Some(ActionKind::SurfaceRightPanelToggle),
            },
            SurfaceCommand::VerticalTabs(command) => match command {
                SurfaceOpenToggleCommand::Open(_) => Some(ActionKind::SurfaceVerticalTabsOpen),
                SurfaceOpenToggleCommand::Toggle(_) => Some(ActionKind::SurfaceVerticalTabsToggle),
            },
            SurfaceCommand::AgentManagement(command) => match command {
                SurfaceOpenCommand::Open(_) => Some(ActionKind::SurfaceAgentManagementOpen),
            },
        },
        ControlCommand::Drive(command) => match command {
            DriveCommand::Status(_) => Some(ActionKind::DriveSyncStatus),
            DriveCommand::Export(_) => Some(ActionKind::DriveSyncExport),
            DriveCommand::Import(_) => Some(ActionKind::DriveSyncImport),
            DriveCommand::Object(command) => match command {
                DriveObjectCommand::List(_) => Some(ActionKind::DriveObjectList),
                DriveObjectCommand::Get(_) => Some(ActionKind::DriveObjectGet),
                DriveObjectCommand::Create(_) => Some(ActionKind::DriveObjectCreate),
                DriveObjectCommand::Trash(_) => Some(ActionKind::DriveObjectTrash),
            },
        },
        ControlCommand::Agent(command) => match command {
            AgentCommand::List(_) => Some(ActionKind::AgentList),
            AgentCommand::Prompt(_) => Some(ActionKind::AgentPrompt),
            AgentCommand::Read(_) => Some(ActionKind::AgentRead),
            AgentCommand::Spawn(_) => Some(ActionKind::AgentSpawn),
            AgentCommand::Cancel(_) => Some(ActionKind::AgentCancel),
            AgentCommand::Settle(_) => Some(ActionKind::AgentSettle),
            AgentCommand::Reveal(_) => Some(ActionKind::AgentReveal),
            AgentCommand::Approvals(_) => Some(ActionKind::AgentApprovals),
            AgentCommand::Approve(_) => Some(ActionKind::AgentApprove),
            AgentCommand::Deny(_) => Some(ActionKind::AgentDeny),
        },
        ControlCommand::Slash(command) => match command {
            SlashCommand::List(_) => Some(ActionKind::SlashList),
            SlashCommand::Run(_) => Some(ActionKind::SlashRun),
        },
        ControlCommand::Pair(command) => match command {
            PairCommand::Show(_) => Some(ActionKind::ControlPair),
        },
        ControlCommand::Events(command) => match command {
            EventsCommand::Subscribe(_) | EventsCommand::Tail(_) => {
                Some(ActionKind::EventsSubscribe)
            }
        },
        ControlCommand::Remote(command) => match command {
            RemoteCommand::Wsl(RemoteWslCommand::List(_)) => Some(ActionKind::RemoteWslList),
            RemoteCommand::Wsl(RemoteWslCommand::Connect(_)) => Some(ActionKind::RemoteWslConnect),
        },
        ControlCommand::Completions { .. } => None,
        // Not a single action: `mcp` serves the whole catalog over stdio.
        ControlCommand::Mcp => None,
        // Also not a single action, and deliberately so — `graph` is a loop
        // over `agent.spawn` and `agent.read`, which is why T7.1 added no
        // actions to the catalog.
        ControlCommand::Graph(_) => None,
        // Not an action either: `acp` talks to a process that is not Warp, so
        // there is nothing in the catalog for it to be. T14.1 keeps it out on
        // purpose — a probe should not pay the four-test pin tax.
        ControlCommand::Acp(_) => None,
    }
}

#[test]
fn renders_the_pairing_qr_rather_than_an_escaped_json_string() {
    // The whole point of `PairingResult::qr` is that a terminal client can show
    // one without an image viewer. The renderer printed the JSON instead, so the
    // only way to pair was to carry a URL with a secret in its fragment across
    // devices by hand.
    let rendered = render_human_readable_for_test(
        local_control::protocol::ActionKind::ControlPair,
        &json!({
            "url": "http://192.168.1.5:41234/#code",
            "qr": "██▀▄█\n█▄▀ █\n",
            "expires_at": "2026-08-30T17:38:57Z",
            "actions": ["app.ping", "agent.approve"]
        }),
    );
    assert!(rendered.starts_with("██▀▄█\n█▄▀ █\n"), "{rendered}");
    assert!(
        !rendered.contains("\\n"),
        "the QR must be drawn, not escaped: {rendered}"
    );
    assert!(
        rendered.contains("http://192.168.1.5:41234/#code"),
        "{rendered}"
    );
}

#[test]
fn the_pairing_render_says_what_a_scan_grants_and_when_it_dies() {
    // `PairingResult` states that "which of these actions does my phone get" is
    // the first question anyone should ask about a QR code, and the code is
    // spendable for two minutes and once — so a stale code should be readable
    // as stale rather than debugged as a network fault.
    let rendered = render_human_readable_for_test(
        local_control::protocol::ActionKind::ControlPair,
        &json!({
            "url": "http://h/#c",
            "qr": "█\n",
            "expires_at": "2026-08-30T17:38:57Z",
            "actions": ["app.ping", "agent.approve"]
        }),
    );
    assert!(rendered.contains("app.ping, agent.approve"), "{rendered}");
    assert!(rendered.contains("2026-08-30T17:38:57Z"), "{rendered}");
    assert!(rendered.contains("single use"), "{rendered}");
}
