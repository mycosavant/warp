//! Command-line interface for controlling a running local Warp app.
mod commands;
mod completions;
mod graph;
mod mcp;
mod output;
mod selectors;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use clap_complete::aot::Shell;
use commands::{
    run_action_catalog_command, run_agent_command, run_app_command, run_appearance_command,
    run_capability_command, run_drive_command, run_events_command, run_file_command,
    run_input_command, run_instance_command, run_keybinding_command, run_pane_command,
    run_remote_command, run_session_command, run_setting_command, run_slash_command,
    run_surface_command, run_tab_command, run_theme_command, run_window_command,
};
use completions::generate_completions_to_stdout;
use output::write_control_error;

use crate::agent::OutputFormat;

/// Hidden flag used by the channel-specific Warp app binary to enter `warpctrl` mode.
pub const CONTROL_MODE_FLAG: &str = "--warpctrl";

/// Parsed top-level arguments for `warpctrl`.
#[derive(Debug, Parser)]
#[command(
    name = "warpctrl",
    display_name = "warpctrl",
    about = "Control a running local Warp app instance"
)]
pub struct ControlArgs {
    /// Set the output format.
    #[arg(
        long = "output-format",
        global = true,
        value_enum,
        default_value_t = OutputFormat::Pretty,
        env = "WARP_OUTPUT_FORMAT"
    )]
    pub output_format: OutputFormat,

    #[command(subcommand)]
    pub command: ControlCommand,
}

/// Commands that inspect the public action catalog.
#[derive(Debug, Clone, Subcommand)]
pub enum ActionCatalogCommand {
    /// List allowlisted catalog actions.
    List,

    /// Inspect a single allowlisted catalog action.
    Inspect {
        /// Canonical action name, such as `tab.create` or `surface.settings.open`.
        action: String,
    },
}

impl ControlArgs {
    pub fn from_env() -> Self {
        let bin_name = crate::binary_name().unwrap_or_else(|| "warpctrl".to_owned());
        Self::try_parse_from_args(std::env::args_os(), bin_name).unwrap_or_else(|err| err.exit())
    }

    /// Parse Warp Control arguments only when the wrapper-injected mode flag is present.
    ///
    /// Startup calls this before the normal Warp/Oz parser. Arguments through
    /// `--warpctrl` are removed, and the remaining arguments are parsed as if
    /// the standalone command name were `warpctrl`.
    pub fn from_control_mode_env() -> Option<Self> {
        Self::try_parse_control_mode_from(std::env::args_os())
            .map(|result| result.unwrap_or_else(|err| err.exit()))
    }

    /// Testable implementation of [`Self::from_control_mode_env`].
    pub fn try_parse_control_mode_from<I, T>(args: I) -> Option<Result<Self, clap::Error>>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let mut stripped_args = vec![OsString::from("warpctrl")];
        let mut found_control_mode = false;

        for arg in args {
            let arg = arg.into();
            if !found_control_mode {
                if arg.to_str() == Some(CONTROL_MODE_FLAG) {
                    found_control_mode = true;
                }
                continue;
            }
            stripped_args.push(arg);
        }

        found_control_mode.then(|| Self::try_parse_from_args(stripped_args, "warpctrl"))
    }

    pub fn clap_command() -> clap::Command {
        let bin_name = crate::binary_name().unwrap_or_else(|| "warpctrl".to_owned());
        Self::clap_command_for_bin_name(bin_name)
    }

    fn try_parse_from_args<I, T>(args: I, bin_name: impl Into<String>) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let matches = Self::clap_command_for_bin_name(bin_name).try_get_matches_from(args)?;
        Self::from_arg_matches(&matches)
    }

    fn clap_command_for_bin_name(bin_name: impl Into<String>) -> clap::Command {
        let bin_name = bin_name.into();
        <Self as CommandFactory>::command()
            .version(crate::version_string())
            .bin_name(bin_name.clone())
            .after_help(color_print::cformat!(
                r#"<bold><underline>Examples:</underline></bold>

  <dim>$</dim> <bold>{bin_name} instance list</bold>

  <dim>$</dim> <bold>{bin_name} tab create</bold>
  <dim>$</dim> <bold>{bin_name} action list</bold>

  <dim>$</dim> <bold>{bin_name} action inspect surface.settings.open</bold>

<bold><underline>Learn more:</underline></bold>
* Use <bold>{bin_name} help</bold> to learn more about each command
* Use <bold>{bin_name} action list</bold> to inspect allowlisted actions
"#
            ))
    }
}

/// Top-level `warpctrl` command groups.
#[derive(Debug, Clone, Subcommand)]
pub enum ControlCommand {
    /// Inspect local Warp app instances.
    #[command(subcommand)]
    Instance(InstanceCommand),
    /// Inspect a selected local Warp app.
    #[command(subcommand)]
    App(AppCommand),
    /// Inspect local-control capabilities.
    #[command(subcommand)]
    Capability(CapabilityCommand),
    /// Inspect public action metadata and implementation status.
    #[command(subcommand)]
    Action(ActionCatalogCommand),

    /// Serve the action catalog to an MCP client over stdio.
    ///
    /// Every implemented action becomes one MCP tool, generated from the
    /// catalog rather than hardcoded. Intended to be launched by the client,
    /// not run interactively: stdout carries JSON-RPC, stderr carries
    /// diagnostics.
    ///
    /// Register with Claude Code:
    ///     claude mcp add warp -- <path-to-warp-binary> --warpctrl mcp
    #[command(verbatim_doc_comment)]
    Mcp,

    /// Inspect local Warp windows.
    #[command(subcommand)]
    Window(WindowCommand),

    /// Control local Warp tabs.
    #[command(subcommand)]
    Tab(TabCommand),
    /// Inspect local Warp panes.
    #[command(subcommand)]
    Pane(PaneCommand),

    /// Inspect local Warp sessions.
    #[command(subcommand)]
    Session(SessionCommand),

    /// Inspect terminal input state.
    #[command(subcommand)]
    Input(InputCommand),

    /// Inspect Warp themes.
    #[command(subcommand)]
    Theme(ThemeCommand),

    /// Inspect appearance state.
    #[command(subcommand)]
    Appearance(AppearanceCommand),

    /// Inspect allowlisted settings.
    #[command(subcommand)]
    Setting(SettingCommand),

    /// Inspect keybinding metadata.
    #[command(subcommand)]
    Keybinding(KeybindingCommand),

    /// Inspect open file app-state metadata.
    #[command(subcommand)]
    File(FileCommand),

    /// Mirror Warp Drive into a directory you keep under git.
    #[command(subcommand)]
    Drive(DriveCommand),

    /// Talk to Warp's agent: list conversations, send prompts, hand off work.
    #[command(subcommand)]
    Agent(AgentCommand),

    /// Run a task graph: a plan of agents, in a file, with edges between them.
    #[command(subcommand)]
    Graph(GraphCommand),

    /// Run Warp's slash commands — `/compact`, `/plan`, `/fork-and-compact`.
    #[command(subcommand)]
    Slash(SlashCommand),

    /// Watch what agents are doing, live.
    #[command(subcommand)]
    Events(EventsCommand),

    /// Inspect remote-development targets on this machine.
    #[command(subcommand)]
    Remote(RemoteCommand),

    /// Open or toggle local Warp surfaces.
    #[command(subcommand)]
    Surface(SurfaceCommand),

    /// Generate shell completions for your shell to stdout.
    ///
    /// For bash, add the following to ~/.bashrc:
    ///     source <(path/to/warpctrl completions bash)
    ///
    /// For zsh, add the following to ~/.zshrc:
    ///     source <(path/to/warpctrl completions zsh)
    ///
    /// For fish, add the following to ~/.config/fish/config.fish:
    ///     path/to/warpctrl completions fish | source
    ///
    /// For Powershell, add the following to $PROFILE:
    ///     path\to\warpctrl completions powershell | Out-String | Invoke-Expression
    ///
    /// If no shell is provided, this defaults to the shell that Warp was run from.
    #[command(verbatim_doc_comment)]
    Completions {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: Option<Shell>,
    },
}

/// Commands that inspect locally discoverable Warp instances.
#[derive(Debug, Clone, Subcommand)]
pub enum InstanceCommand {
    /// List locally discoverable Warp instances.
    List,

    /// Print app, protocol, active target, and action metadata for the selected instance.
    Inspect(TargetArgs),
}

/// Commands that inspect the selected Warp app instance.
#[derive(Debug, Clone, Subcommand)]
pub enum AppCommand {
    /// Check that the selected local Warp app responds.
    Ping(TargetArgs),

    /// Print protocol and build identity metadata for the selected local Warp app.
    Version(TargetArgs),

    /// Print the active window/tab/pane/session chain.
    Active(TargetArgs),

    /// Focus the selected local Warp app.
    Focus(TargetArgs),
}

/// Commands that inspect public local-control capabilities.
#[derive(Debug, Clone, Subcommand)]
pub enum CapabilityCommand {
    /// List allowlisted local-control capabilities.
    List,

    /// Inspect a single local-control capability by canonical action name.
    Inspect {
        /// Canonical action name, such as `tab.create` or `surface.settings.open`.
        action: String,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum WindowCommand {
    /// List windows in the selected local Warp app.
    List(TargetArgs),

    /// Inspect one window in the selected local Warp app.
    Inspect(TargetArgs),

    /// Create a new window.
    Create(TabCreateArgs),

    /// Focus a window.
    Focus(TargetArgs),

    /// Close a window.
    Close(TargetArgs),

    /// The dedicated hotkey window — a drop-down agent prompt.
    #[command(subcommand)]
    Visor(WindowVisorCommand),
}

/// Commands that drive the dedicated hotkey window ("quake mode").
///
/// Upstream reaches this window only through a global keyboard shortcut. These
/// dispatch the same action the shortcut does, so they work whether or not one
/// is bound — which is what makes the window testable on a platform whose
/// global grabs do not work, and what lets an agent ask for a scratch window
/// by name.
#[derive(Debug, Clone, Subcommand)]
pub enum WindowVisorCommand {
    /// Show the hotkey window, or hide it if it is already showing.
    ///
    /// Acknowledges immediately and does not report the resulting state: the
    /// toggle runs after this call returns. Follow with `status`.
    Toggle(TargetArgs),

    /// Report whether the hotkey window exists, is showing, and opens as an agent.
    Status(TargetArgs),
}

/// Commands that control tabs in the selected Warp app instance.
#[derive(Debug, Clone, Subcommand)]
pub enum TabCommand {
    /// List tabs in the selected local Warp app.
    List(TargetArgs),

    /// Inspect one tab in the selected local Warp app.
    Inspect(TargetArgs),

    /// Create a new terminal tab in the active window.
    Create(TabCreateArgs),

    /// Activate a tab.
    Activate(TabActivateArgs),

    /// Move the active tab.
    Move(TabMoveArgs),

    /// Fold a tab into the active tab as a split.
    ///
    /// The scriptable form of dragging a pane onto another pane. The tab must
    /// not be the active one and must hold exactly one pane; anything else has
    /// no single meaning and is refused rather than guessed.
    Merge(TabMergeArgs),

    /// Close tabs.
    Close(TabCloseArgs),

    /// Rename a tab.
    Rename(RenameArgs),

    /// Reset a tab name.
    ResetName(TargetArgs),

    /// Set or clear a tab color.
    #[command(subcommand)]
    Color(TabColorCommand),
}

/// Commands that control tab colors.
#[derive(Debug, Clone, Subcommand)]
pub enum TabColorCommand {
    /// Set a tab color.
    Set(ColorSetArgs),

    /// Clear a tab color.
    Clear(TargetArgs),
}

/// Commands that inspect local Warp panes.
#[derive(Debug, Clone, Subcommand)]
pub enum PaneCommand {
    /// List panes in the selected local Warp app.
    List(TargetArgs),

    /// Inspect one pane in the selected local Warp app.
    Inspect(TargetArgs),

    /// Split the active pane.
    Split(PaneSplitArgs),

    /// Focus a pane.
    Focus(TargetArgs),

    /// Navigate between panes.
    Navigate(PaneNavigateArgs),

    /// Resize the active pane.
    Resize(PaneResizeArgs),

    /// Maximize the active pane.
    Maximize(TargetArgs),

    /// Unmaximize the active pane.
    Unmaximize(TargetArgs),

    /// Close the active pane.
    Close(TargetArgs),

    /// Rename a pane.
    Rename(RenameArgs),

    /// Reset a pane name.
    ResetName(TargetArgs),

    /// The tab's main pane — the one its ambient surfaces follow.
    #[command(subcommand)]
    Main(PaneMainCommand),
}

/// Commands that read and change a tab's designated main pane.
///
/// Warp picks one repository per tab for the file tree and code review, and
/// picks it from whichever pane is active — so glancing at a split moves the
/// file tree. Designating a pane pins that choice.
///
/// The command palette has a single toggle for this. These are separate verbs
/// because a toggle cannot express "make it this one" without first reading the
/// current state, which is a race a script should not have to run.
#[derive(Debug, Clone, Subcommand)]
pub enum PaneMainCommand {
    /// Report the tab's main pane without changing it.
    Get(TargetArgs),

    /// Designate the targeted pane as its tab's main pane.
    ///
    /// Defaults to the tab's focused pane. The pane does not have to be a
    /// terminal: a non-terminal main pane simply stops the ambient surfaces
    /// moving rather than anchoring them somewhere new, which `get` reports as
    /// `anchors_working_directory: false`.
    Set(TargetArgs),

    /// Clear the designation, restoring follow-the-active-pane.
    ///
    /// Succeeds when there was nothing designated.
    Clear(TargetArgs),
}

/// Commands that inspect local Warp sessions.
#[derive(Debug, Clone, Subcommand)]
pub enum SessionCommand {
    /// List sessions in the selected local Warp app.
    List(TargetArgs),

    /// Inspect one session in the selected local Warp app.
    Inspect(TargetArgs),

    /// Activate a session.
    Activate(TargetArgs),

    /// Activate the previous session.
    Previous(TargetArgs),

    /// Activate the next session.
    Next(TargetArgs),

    /// Reopen the most recently closed session.
    ReopenClosed(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum InputCommand {
    /// Insert text into the input buffer without submitting it.
    Insert(TextTargetArgs),

    /// Replace the input buffer without submitting it.
    Replace(TextTargetArgs),

    /// Replace the input buffer and run it.
    ///
    /// Unlike `insert` and `replace`, this executes. The text is still rejected
    /// if it contains newlines or control characters, so one call runs exactly
    /// one command.
    ///
    /// Reports `executed` when the command ran immediately and `queued` when
    /// the pane's shell is still starting or busy. A queued command runs as
    /// soon as the pane is ready.
    Submit(TextTargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SurfaceCommand {
    /// List available and unavailable tour surfaces.
    List(TargetArgs),
    /// Open settings surfaces.
    #[command(subcommand)]
    Settings(SurfaceSettingsCommand),

    /// Open the command palette.
    #[command(subcommand)]
    CommandPalette(SurfaceQueryCommand),

    /// Open command search.
    #[command(subcommand)]
    CommandSearch(SurfaceQueryCommand),
    /// Open the theme picker.
    #[command(subcommand)]
    ThemePicker(SurfaceOpenCommand),

    /// Open keybinding settings.
    #[command(subcommand)]
    Keybindings(SurfaceOpenCommand),

    /// Open or toggle Warp Drive.
    #[command(subcommand)]
    WarpDrive(SurfaceOpenToggleCommand),

    /// Toggle the resource center.
    #[command(subcommand)]
    ResourceCenter(SurfaceToggleCommand),

    /// Toggle the AI assistant.
    #[command(subcommand)]
    AiAssistant(SurfaceToggleCommand),

    /// Open or toggle code review.
    #[command(subcommand)]
    CodeReview(SurfaceOpenToggleCommand),

    /// Open the project explorer.
    #[command(subcommand)]
    ProjectExplorer(SurfaceOpenCommand),

    /// Open global search.
    #[command(subcommand)]
    GlobalSearch(SurfaceOpenCommand),

    /// Open the conversation list.
    #[command(subcommand)]
    ConversationList(SurfaceOpenCommand),

    /// Toggle the left panel.
    #[command(subcommand)]
    LeftPanel(SurfaceToggleCommand),

    /// Toggle the right panel.
    #[command(subcommand)]
    RightPanel(SurfaceToggleCommand),

    /// Open or toggle vertical tabs.
    #[command(subcommand)]
    VerticalTabs(SurfaceOpenToggleCommand),

    /// Open agent management.
    #[command(subcommand)]
    AgentManagement(SurfaceOpenCommand),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SurfaceSettingsCommand {
    /// Open Settings, optionally scoped to a page or query.
    Open(PageQueryArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SurfaceQueryCommand {
    /// Open the surface with an optional seeded query.
    Open(QueryArgs),
}
#[derive(Debug, Clone, Subcommand)]
pub enum SurfaceOpenCommand {
    /// Open the surface.
    Open(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SurfaceOpenToggleCommand {
    /// Open the surface.
    Open(TargetArgs),

    /// Toggle the surface.
    Toggle(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SurfaceToggleCommand {
    /// Toggle the surface.
    Toggle(TargetArgs),
}

/// Commands that inspect Warp themes.
#[derive(Debug, Clone, Subcommand)]
pub enum ThemeCommand {
    /// List available themes.
    List(TargetArgs),

    /// Read current theme state.
    Get(TargetArgs),

    /// Set the current theme.
    Set(ThemeSetArgs),

    /// Set whether Warp follows the system theme.
    SystemSet(ThemeSystemSetArgs),

    /// Set the light theme used when following the system theme.
    LightSet(ThemeSetArgs),

    /// Set the dark theme used when following the system theme.
    DarkSet(ThemeSetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum AppearanceCommand {
    /// Read appearance state.
    Get(TargetArgs),

    /// Increase terminal font size.
    FontSizeIncrease(TargetArgs),

    /// Decrease terminal font size.
    FontSizeDecrease(TargetArgs),

    /// Reset terminal font size.
    FontSizeReset(TargetArgs),

    /// Increase UI zoom.
    ZoomIncrease(TargetArgs),

    /// Decrease UI zoom.
    ZoomDecrease(TargetArgs),

    /// Reset UI zoom.
    ZoomReset(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SettingCommand {
    /// List allowlisted settings.
    List(NamespaceTargetArgs),

    /// Read one allowlisted setting.
    Get(SettingGetArgs),

    /// Set one allowlisted setting.
    Set(SettingSetArgs),

    /// Toggle one allowlisted boolean setting.
    Toggle(SettingToggleArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum KeybindingCommand {
    /// List keybindings.
    List(TargetArgs),

    /// Read one keybinding by name.
    Get(KeybindingGetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum FileCommand {
    /// Open a file in Warp.
    Open(FileOpenArgs),
}

/// Commands for the git-backed Warp Drive mirror.
///
/// The destination is `warp_drive.local_sync.path` in settings, deliberately
/// not a flag: an export prunes the directory it writes to, so where it points
/// is a decision the user makes once rather than one a caller passes in.
#[derive(Debug, Clone, Subcommand)]
pub enum DriveCommand {
    /// Report where the drive would be mirrored and what would go there.
    Status(TargetArgs),

    /// Write the drive into the configured directory. Warp does not run git.
    Export(TargetArgs),

    /// Read the configured directory back into the drive, after your `git pull`.
    ///
    /// The files win. An object the tree no longer has is moved to the trash,
    /// not deleted, so a mistake here is recoverable from the Warp Drive panel.
    Import(TargetArgs),

    /// Work on single objects: workflows, notebooks, folders.
    #[command(subcommand)]
    Object(DriveObjectCommand),
}

/// Commands for individual Warp Drive objects.
///
/// `drive status|export|import` move the whole store to and from a directory,
/// which is the right shape for a git mirror and the wrong one for "make me a
/// workflow that does X". These reach one object at a time.
#[derive(Debug, Clone, Subcommand)]
pub enum DriveObjectCommand {
    /// List the objects in your personal drive.
    List(DriveObjectListArgs),

    /// Print one object, as the file an export would write.
    ///
    /// This is also how to learn a body's shape before creating one: run it on
    /// an object of the same type and read the `data` (or the markdown).
    Get(DriveObjectGetArgs),

    /// Create a workflow, notebook or folder.
    ///
    /// The id and owner are not yours to choose — they are minted here. To
    /// write an object with an identity you supply, put its file in the mirror
    /// directory and run `drive import`.
    Create(DriveObjectCreateArgs),

    /// Move an object to the trash. Recoverable from the Warp Drive panel.
    Trash(DriveObjectTrashArgs),
}

#[derive(Debug, Clone, Args)]
pub struct DriveObjectListArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Include objects that are in the trash.
    #[arg(long)]
    pub include_trashed: bool,

    /// Only this type: `workflow`, `notebook`, `folder`, `prompt` or
    /// `env-vars`.
    #[arg(long = "type")]
    pub object_type: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct DriveObjectGetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// The object's id, from `drive object list`.
    pub id: String,
}

#[derive(Debug, Clone, Args)]
pub struct DriveObjectCreateArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// `workflow`, `notebook`, `folder`, `prompt` or `env-vars`.
    #[arg(long = "type")]
    pub object_type: String,

    /// The object's display name.
    #[arg(long)]
    pub name: String,

    /// The body: markdown for a notebook, JSON otherwise, nothing for a folder.
    ///
    /// Mutually exclusive with `--body-file`. A workflow's JSON is easiest
    /// learned from `drive object get` on one you already have.
    #[arg(long, conflicts_with = "body_file")]
    pub body: Option<String>,

    /// Read the body from a file, or from stdin with `-`.
    #[arg(long)]
    pub body_file: Option<PathBuf>,

    /// Create it inside this folder, by id.
    #[arg(long)]
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct DriveObjectTrashArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// The object's id, from `drive object list`.
    pub id: String,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AgentCommand {
    /// List live agent conversations with their status.
    ///
    /// `status` is the field to poll: `in_progress` means still working;
    /// `success`, `error` and `cancelled` are terminal; `waiting_for_events`
    /// means the agent yielded and is listening; `blocked` means it is waiting
    /// on a person, and `blocked_action` says what for.
    List(TargetArgs),

    /// Send a prompt to the agent, starting a conversation or continuing one.
    ///
    /// Unlike `input submit`, this reaches the agent rather than the shell.
    /// `input submit '/agent do the thing'` runs `/agent` as a command and gets
    /// `No such file or directory`; the keyboard equivalent of this is
    /// ctrl+shift+Return.
    ///
    /// Prints the `conversation_id` the prompt went to, which is how a caller
    /// that started several tells them apart afterwards.
    Prompt(AgentPromptArgs),

    /// Read what a conversation said.
    ///
    /// `agent list` reports that a conversation finished; this reports what it
    /// produced. `--last 1` is the usual call: the answer to the prompt that
    /// was just dispatched, without the transcript leading up to it.
    Read(AgentReadArgs),

    /// Spawn a child agent in a hidden pane.
    ///
    /// The background handoff: the child is parented to a conversation,
    /// starts work immediately, and stays off screen until `agent reveal`.
    ///
    /// The other three targets are compositions and need no action of their
    /// own — `pane split` / `tab create` / `window create` followed by
    /// `agent prompt` — but those start a *sibling*, not a child.
    Spawn(AgentSpawnArgs),

    /// Stop the turn a conversation is running.
    ///
    /// Stop, not kill: the conversation survives and `agent read` still works.
    /// Cancelling one that has already finished is not an error — the response
    /// says `was_running: false`.
    Cancel(AgentCancelArgs),

    /// Settle a thread, or bring one back.
    ///
    /// Settling keeps a thread and moves it to the bottom of the inbox; it is
    /// not a delete. Settled threads are also exempt from the
    /// 200-conversation eviction cap, which is what makes settling a promise
    /// rather than a suggestion.
    ///
    /// Works on conversations that are not open — which is the point, since
    /// the threads worth settling are usually the ones nobody has looked at
    /// this session.
    Settle(AgentSettleArgs),

    /// Put a background child agent on screen.
    ///
    /// The other half of spawning one hidden. By default it splits off beside
    /// the pane that spawned it, which is the only target that adds a surface
    /// rather than taking one over.
    Reveal(AgentRevealArgs),
}

#[derive(Debug, Clone, Args)]
pub struct AgentPromptArgs {
    /// The prompt. Newlines are allowed, unlike `input submit`.
    pub prompt: String,

    /// Continue this conversation instead of starting a new one.
    ///
    /// Take the id from `warpctrl agent list`.
    #[arg(long = "conversation")]
    pub conversation: Option<String>,

    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Debug, Clone, Args)]
pub struct AgentReadArgs {
    /// The conversation to read, from `warpctrl agent list`.
    pub conversation: String,

    /// Return only the last N exchanges, newest last.
    #[arg(long = "last")]
    pub last: Option<u32>,

    /// Include tool-call results — every file read and command output.
    ///
    /// Off by default: without it this returns what the agent *said*, which is
    /// what a caller passing the result to another agent wants to pay for.
    #[arg(long = "tools")]
    pub include_tool_results: bool,

    #[command(flatten)]
    pub target: TargetArgs,
}

/// `warpctrl graph …` — run several agents in a declared order.
///
/// The plan is a TOML file of `[[node]]` entries, each one an `agent spawn`,
/// with `needs` between them. A dependency is an edge that carries a payload:
/// `needs = [{ node = "survey", pass = "the list of files" }]` both orders the
/// two nodes *and* hands the first one's answer to the second.
///
/// A plan in a file rather than in an agent's head is the whole feature. The
/// sequencing stops being a decision the model makes in the moment and becomes
/// a declaration made before the run — and it survives `/compact`, which is
/// exactly when a plan held in context is most at risk.
#[derive(Debug, Clone, Subcommand)]
pub enum GraphCommand {
    /// Print the plan format, as a plan.
    ///
    /// What it prints is itself valid and runnable, so the format cannot drift
    /// from its own documentation: `graph schema > plan.toml` is a starting
    /// point, and `graph check` accepts it unchanged.
    ///
    /// Exists so that an agent writing a plan can learn the format from the
    /// tool rather than from a human pasting documentation at it — which is
    /// the whole of T7.2, where the plan comes from an issue tracker.
    Schema,

    /// Check a plan without running it: parse, resolve edges, find cycles.
    ///
    /// Needs no running Warp. Prints the order the nodes would run in.
    Check(GraphCheckArgs),

    /// Run a plan to completion.
    ///
    /// Blocks until every node has settled. Exits non-zero if any node failed
    /// or was skipped, so this can be the last line of a script.
    Run(GraphRunArgs),
}

#[derive(Debug, Clone, Args)]
pub struct GraphCheckArgs {
    /// Path to the plan.
    pub plan: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct GraphRunArgs {
    /// Path to the plan.
    pub plan: PathBuf,

    /// Parent every node to this conversation instead of the targeted pane's.
    #[arg(long = "parent")]
    pub parent: Option<String>,

    /// How many nodes may run at once.
    ///
    /// Every node is a real agent with a real model behind it, so this is a
    /// bound on load rather than on correctness: the graph decides what *may*
    /// run together, and this decides how much of that actually does.
    #[arg(long = "max-parallel", default_value_t = 4)]
    pub max_parallel: usize,

    /// Give up on a node still running after this many seconds.
    ///
    /// No timeout by default. A node can legitimately sit for a long time —
    /// `blocked` means it is waiting for a person to approve something — and
    /// killing it would throw the work away to no purpose. Set one for an
    /// unattended run.
    #[arg(long = "timeout")]
    pub timeout: Option<u64>,

    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Debug, Clone, Args)]
pub struct AgentSpawnArgs {
    /// The child's prompt.
    ///
    /// Self-contained: a child does not inherit its parent's transcript, so
    /// everything it needs to know goes here.
    pub prompt: String,

    /// A name for the child, shown on its pill.
    #[arg(long = "name")]
    pub name: Option<String>,

    /// Parent it to this conversation instead of the targeted pane's.
    #[arg(long = "parent")]
    pub parent: Option<String>,

    /// Restrict the child to these tools. Repeatable, or comma-separated.
    ///
    /// Each value is the preset `read-only`, or a ToolType name such as
    /// `READ_FILES` or `RUN_SHELL_COMMAND`. Omit for no restriction;
    /// `--allow-tools ''` is a policy of no tools at all.
    ///
    /// Withholding `SUBAGENT` and `RUN_AGENTS` is what stops a child spawning
    /// children of its own — a harder guarantee than the depth cap, which only
    /// bounds `warpctrl agent spawn` itself.
    #[arg(long = "allow-tools", value_delimiter = ',', num_args = 0..)]
    pub allow_tools: Option<Vec<String>>,

    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Debug, Clone, Args)]
pub struct AgentCancelArgs {
    /// The conversation to stop, from `warpctrl agent list`.
    pub conversation: String,

    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Debug, Clone, Args)]
pub struct AgentSettleArgs {
    /// The conversation to settle, from `warpctrl agent list`.
    pub conversation: String,

    /// Bring the thread back instead of settling it.
    #[arg(long)]
    pub undo: bool,

    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Debug, Clone, Args)]
pub struct AgentRevealArgs {
    /// The conversation to reveal, from `warpctrl agent list`.
    ///
    /// `is_hidden` there is the field that says which conversations this does
    /// anything for.
    pub conversation: String,

    /// Where to put it.
    ///
    /// `pane` splits it off beside the pane that spawned it; `tab` opens it in
    /// a new tab; `swap` puts it into the targeted pane, as clicking its pill
    /// does. The swapped-out pane is not closed and the swap is reversible,
    /// but the caller loses sight of what was there — which is why it is not
    /// the default over a socket.
    ///
    /// Spelled `--as` rather than `--pane`/`--tab` because those names already
    /// belong to the target selectors every command carries.
    #[arg(long = "as", value_name = "TARGET", default_value = "pane")]
    pub reveal_target: CliRevealTarget,

    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum CliRevealTarget {
    Pane,
    Tab,
    Swap,
}

#[derive(Debug, Clone, Subcommand)]
pub enum RemoteCommand {
    /// Windows Subsystem for Linux.
    #[command(subcommand)]
    Wsl(RemoteWslCommand),
}

#[derive(Debug, Clone, Subcommand)]
pub enum RemoteWslCommand {
    /// List the WSL distributions installed on this machine.
    ///
    /// `available` is false when `wsl.exe` could not be run at all, which is a
    /// different answer from an empty list on a machine that has WSL with
    /// nothing installed.
    List(TargetArgs),

    /// Attach Warp's remote-development server to a WSL distribution.
    ///
    /// Targets a terminal session, the way the SSH transport attaches to the
    /// pane running `ssh`. Returns once the setup pipeline has started, not
    /// once the server is serving.
    Connect(RemoteWslConnectArgs),
}

#[derive(Debug, Clone, Args)]
pub struct RemoteWslConnectArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Distribution to run the remote server in.
    ///
    /// Defaults to the target pane's own distribution when it is already
    /// running a WSL shell. Required otherwise.
    #[arg(long)]
    pub distro: Option<String>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum EventsCommand {
    /// Print where the live event stream is, and when this credential expires.
    ///
    /// The stream itself is server-sent events over `GET`, which is not a shape
    /// this request/response CLI can hold open — so this answers the URL and
    /// leaves the reading to `curl -N` or anything else that speaks SSE. The
    /// bearer token is not printed: it is already in the caller's hands, and a
    /// secret echoed to a terminal is a secret in a scrollback.
    Subscribe(TargetArgs),

    /// Follow the live event stream, printing one JSON line per event.
    ///
    /// The `tail -f` of what agents are doing. Runs until the credential
    /// expires — five minutes — or you interrupt it; the bearer token never
    /// leaves the process, so there is nothing to paste and nothing to leak.
    ///
    /// Note that subscribing is itself what turns the log on: events flow to a
    /// subscriber whether or not `WARP_FORK_EVENT_LOG` named a directory.
    Tail(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SlashCommand {
    /// List Warp's slash commands.
    ///
    /// `is_orchestration` is whether `slash run` will execute it without
    /// `--force`; `submits_prompt` is whether it sends its argument to the
    /// agent rather than acting on the UI.
    List(TargetArgs),

    /// Run a slash command.
    ///
    /// Commands outside the orchestration set — `/logout`, `/exit`, `/clear`
    /// and the rest of the account and appearance verbs — are refused without
    /// `--force`, so a mistyped command name cannot end the session.
    Run(SlashRunArgs),
}

#[derive(Debug, Clone, Args)]
pub struct SlashRunArgs {
    /// The command name, with or without the leading `/`.
    pub command: String,

    /// The argument, for commands that take one: the instructions to
    /// `/compact-and`, the prompt to `/agent`, the name to `/model`.
    pub argument: Option<String>,

    /// Run a command outside the orchestration allowlist.
    #[arg(long = "force")]
    pub force: bool,

    #[command(flatten)]
    pub target: TargetArgs,
}

/// Exact selectors for a target within the selected Warp instance.
#[derive(Debug, Clone, Args, Default)]
pub struct TargetArgs {
    /// Target a specific local Warp instance id from `warpctrl instance list`.
    #[arg(long = "instance", conflicts_with = "pid")]
    pub instance: Option<String>,

    /// Target a specific local Warp process id.
    #[arg(long = "pid", conflicts_with = "instance")]
    pub pid: Option<u32>,

    /// Target the active window or an opaque window id.
    #[arg(long = "window", conflicts_with_all = ["window_index", "window_title"])]
    pub window: Option<String>,

    /// Target a window by scoped index when the handler supports it.
    #[arg(long = "window-index", conflicts_with_all = ["window", "window_title"])]
    pub window_index: Option<u32>,

    /// Target a window by exact title when the handler supports it.
    #[arg(long = "window-title", conflicts_with_all = ["window", "window_index"])]
    pub window_title: Option<String>,

    /// Target the active tab or an opaque tab id.
    #[arg(long = "tab", conflicts_with_all = ["tab_index", "tab_title"])]
    pub tab: Option<String>,

    /// Target a tab by scoped index when the handler supports it.
    #[arg(long = "tab-index", conflicts_with_all = ["tab", "tab_title"])]
    pub tab_index: Option<u32>,

    /// Target a tab by exact title when the handler supports it.
    #[arg(long = "tab-title", conflicts_with_all = ["tab", "tab_index"])]
    pub tab_title: Option<String>,

    /// Target the active pane or an opaque pane id.
    #[arg(long = "pane", conflicts_with = "pane_index")]
    pub pane: Option<String>,

    /// Target a pane by scoped index when the handler supports it.
    #[arg(long = "pane-index", conflicts_with = "pane")]
    pub pane_index: Option<u32>,

    /// Target the active session or an opaque session id.
    #[arg(long = "session")]
    pub session: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct TabCreateArgs {
    #[arg(long = "type", value_enum)]
    pub tab_type: Option<CliTabType>,

    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Debug, Clone, Args)]
pub struct TabActivateArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "previous", conflicts_with_all = ["next", "last"])]
    pub previous: bool,

    #[arg(long = "next", conflicts_with_all = ["previous", "last"])]
    pub next: bool,

    #[arg(long = "last", conflicts_with_all = ["previous", "next"])]
    pub last: bool,
}

#[derive(Debug, Clone, Args)]
pub struct TabMoveArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "direction", value_enum)]
    pub direction: CliTabMoveDirection,
}

#[derive(Debug, Clone, Args)]
pub struct TabMergeArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Which side of the active tab's focused pane the tab lands on.
    #[arg(long = "direction", value_enum)]
    pub direction: CliCardinalDirection,
}

#[derive(Debug, Clone, Args)]
pub struct TabCloseArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "active", conflicts_with_all = ["others", "right_of"])]
    pub active: bool,

    #[arg(long = "others", conflicts_with_all = ["active", "right_of"])]
    pub others: bool,

    #[arg(long = "right-of", conflicts_with_all = ["active", "others"])]
    pub right_of: bool,
}

#[derive(Debug, Clone, Args)]
pub struct PaneSplitArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "direction", value_enum)]
    pub direction: CliCardinalDirection,
}

#[derive(Debug, Clone, Args)]
pub struct PaneNavigateArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "direction", value_enum)]
    pub direction: CliDirection,
}

#[derive(Debug, Clone, Args)]
pub struct PaneResizeArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "direction", value_enum)]
    pub direction: CliCardinalDirection,

    #[arg(long = "amount")]
    pub amount: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct TextTargetArgs {
    pub text: String,

    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Debug, Clone, Args)]
pub struct PageQueryArgs {
    #[arg(long = "page")]
    pub page: Option<String>,

    #[arg(long = "query")]
    pub query: Option<String>,

    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Debug, Clone, Args)]
pub struct QueryArgs {
    #[arg(long = "query")]
    pub query: Option<String>,

    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Debug, Clone, Args)]
pub struct FileOpenArgs {
    pub path: String,

    #[arg(long = "line")]
    pub line: Option<u32>,

    #[arg(long = "column")]
    pub column: Option<u32>,

    #[arg(long = "new-tab")]
    pub new_tab: bool,

    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Debug, Clone, Args)]
pub struct RenameArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub title: String,
}

#[derive(Debug, Clone, Args)]
pub struct ColorSetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub color: String,
}

#[derive(Debug, Clone, Args)]
pub struct ThemeSetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub name: String,
}

#[derive(Debug, Clone, Args)]
pub struct ThemeSystemSetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(action = clap::ArgAction::Set)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Args)]
pub struct SettingSetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub key: String,

    pub value: String,
}

#[derive(Debug, Clone, Args)]
pub struct SettingToggleArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub key: String,
}

#[derive(Debug, Clone, Args)]
pub struct NamespaceTargetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "namespace")]
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct SettingGetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Allowlisted setting key.
    pub key: String,
}

#[derive(Debug, Clone, Args)]
pub struct KeybindingGetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Keybinding action name.
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliTabType {
    Terminal,
    Agent,
    CloudAgent,
    Default,
}

impl From<CliTabType> for local_control::protocol::TabType {
    fn from(value: CliTabType) -> Self {
        match value {
            CliTabType::Terminal => Self::Terminal,
            CliTabType::Agent => Self::Agent,
            CliTabType::CloudAgent => Self::CloudAgent,
            CliTabType::Default => Self::Default,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliCardinalDirection {
    Left,
    Right,
    Up,
    Down,
}

impl From<CliCardinalDirection> for local_control::protocol::Direction {
    fn from(value: CliCardinalDirection) -> Self {
        match value {
            CliCardinalDirection::Left => Self::Left,
            CliCardinalDirection::Right => Self::Right,
            CliCardinalDirection::Up => Self::Up,
            CliCardinalDirection::Down => Self::Down,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliDirection {
    Left,
    Right,
    Up,
    Down,
    Previous,
    Next,
}

impl From<CliDirection> for local_control::protocol::Direction {
    fn from(value: CliDirection) -> Self {
        match value {
            CliDirection::Left => Self::Left,
            CliDirection::Right => Self::Right,
            CliDirection::Up => Self::Up,
            CliDirection::Down => Self::Down,
            CliDirection::Previous => Self::Previous,
            CliDirection::Next => Self::Next,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliTabMoveDirection {
    Left,
    Right,
    Previous,
    Next,
}

impl From<CliTabMoveDirection> for local_control::protocol::Direction {
    fn from(value: CliTabMoveDirection) -> Self {
        match value {
            CliTabMoveDirection::Left => Self::Left,
            CliTabMoveDirection::Right => Self::Right,
            CliTabMoveDirection::Previous => Self::Previous,
            CliTabMoveDirection::Next => Self::Next,
        }
    }
}

pub fn run(args: ControlArgs) -> ExitCode {
    ExitCode::from(run_exit_code(args))
}

pub fn run_and_exit(args: ControlArgs) -> ! {
    std::process::exit(i32::from(run_exit_code(args)))
}

fn run_exit_code(args: ControlArgs) -> u8 {
    let output_format = args.output_format;
    match run_inner(args) {
        Ok(()) => 0,
        Err(error) => {
            if let Err(write_error) = write_control_error(&error, output_format) {
                eprintln!(
                    "error: failed to render local-control error: {}",
                    write_error.message
                );
            }
            1
        }
    }
}

fn run_inner(args: ControlArgs) -> Result<(), local_control::protocol::ControlError> {
    let output_format = args.output_format;
    match args.command {
        ControlCommand::Instance(command) => run_instance_command(command, output_format),
        ControlCommand::App(command) => run_app_command(command, output_format),
        ControlCommand::Capability(command) => run_capability_command(command, output_format),
        ControlCommand::Action(command) => run_action_catalog_command(command, output_format),
        // Never returns: the MCP server owns stdio until the client closes it.
        ControlCommand::Mcp => mcp::run_and_exit(),
        ControlCommand::Window(command) => run_window_command(command, output_format),
        ControlCommand::Tab(command) => run_tab_command(command, output_format),
        ControlCommand::Pane(command) => run_pane_command(command, output_format),
        ControlCommand::Session(command) => run_session_command(command, output_format),
        ControlCommand::Input(command) => run_input_command(command, output_format),
        ControlCommand::Theme(command) => run_theme_command(command, output_format),
        ControlCommand::Appearance(command) => run_appearance_command(command, output_format),
        ControlCommand::Setting(command) => run_setting_command(command, output_format),
        ControlCommand::Keybinding(command) => run_keybinding_command(command, output_format),
        ControlCommand::File(command) => run_file_command(command, output_format),
        ControlCommand::Drive(command) => run_drive_command(command, output_format),
        ControlCommand::Agent(command) => run_agent_command(command, output_format),
        ControlCommand::Graph(command) => graph::run_graph_command(command, output_format),
        ControlCommand::Slash(command) => run_slash_command(command, output_format),
        ControlCommand::Events(command) => run_events_command(command, output_format),
        ControlCommand::Remote(command) => run_remote_command(command, output_format),
        ControlCommand::Surface(command) => run_surface_command(command, output_format),
        ControlCommand::Completions { shell } => generate_completions_to_stdout(shell),
    }
}

#[cfg(test)]
pub(crate) use commands::render_human_readable_for_test;
#[cfg(test)]
pub(crate) use completions::generate_completion_string;
#[cfg(test)]
pub(crate) use output::ErrorSummary;

#[cfg(test)]
#[path = "../local_control_tests.rs"]
mod tests;
