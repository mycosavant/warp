//! Action catalog and metadata used for discovery, permissions, and CLI support.
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

/// Level of Warp hierarchy or orthogonal product noun an action targets.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetScope {
    Instance,
    Window,
    Tab,
    Pane,
    Session,
    Input,
    Settings,
    Appearance,
    Surface,
    File,
    Keybinding,
    Action,
    Capability,
    Drive,
    Agent,
    Slash,
}

/// Whether an action has an app-side implementation in this stack layer.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionImplementationStatus {
    Implemented,
    Stub,
}

/// Typed parameter contract for a catalog action.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionParameterSpec {
    None,
    ActionName,
    BindingName,
    BooleanValue,
    ColorValue,
    Direction,
    FileOpen,
    Key,
    KeyValue,
    Namespace,
    PageQuery,
    Query,
    Rename,
    Resize,
    TabActivate,
    TabClose,
    TabCreate,
    Text,
    ThemeName,
    AgentPrompt,
    AgentRead,
    AgentSpawn,
    AgentCancel,
    AgentSettle,
    AgentReveal,
    SlashRun,
    DriveObjectList,
    DriveObjectGet,
    DriveObjectCreate,
    DriveObjectTrash,
    RemoteWslConnect,
}

/// Typed result contract for a catalog action.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionResultSpec {
    Acknowledgement,
    ActiveTarget,
    AppearanceState,
    CapabilityList,
    CapabilityMetadata,
    DriveSyncExport,
    DriveSyncImport,
    DriveSyncStatus,
    InstanceList,
    InstanceMetadata,
    KeybindingList,
    KeybindingMetadata,
    SettingList,
    SettingValue,
    SurfaceList,
    TargetList,
    TargetMetadata,
    ThemeList,
    ThemeState,
    AgentConversationList,
    AgentConversation,
    AgentTranscript,
    AgentSpawnedChild,
    AgentCancellation,
    AgentSettled,
    AgentRevelation,
    SlashCommandList,
    DriveObjectList,
    DriveObject,
    DriveObjectWritten,
    DriveObjectTrashed,
    RemoteWslDistroList,
    RemoteWslConnectStarted,
    MainPane,
    VisorStatus,
}

/// Discoverable metadata describing one local-control action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionMetadata {
    pub kind: ActionKind,
    pub name: String,
    pub implementation_status: ActionImplementationStatus,
    pub target_scope: TargetScope,
    pub parameter_spec: ActionParameterSpec,
    pub result_spec: ActionResultSpec,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct ActionSpec {
    name: &'static str,
    implementation_status: ActionImplementationStatus,
    target_scope: TargetScope,
    parameter_spec: ActionParameterSpec,
    result_spec: ActionResultSpec,
}

macro_rules! define_action_catalog {
    ($(
        $group:ident {
            $(
                $variant:ident => {
                    name: $name:literal,
                    status: $status:ident,
                    target: $target:ident,
                    params: $params:ident,
                    result: $result:ident $(,)?
                }
            ),+ $(,)?
        }
    )+ $(,)?) => {
        /// Stable protocol name for every approved `warpctrl` action.
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum ActionKind {
            $($(#[serde(rename = $name)] $variant,)+)+
        }

        impl ActionKind {
            pub const ALL: &[Self] = &[$($(Self::$variant,)+)+];

            pub fn as_str(self) -> &'static str {
                self.spec().name
            }

            pub fn metadata(self) -> ActionMetadata {
                let spec = self.spec();
                ActionMetadata {
                    kind: self,
                    name: spec.name.to_owned(),
                    implementation_status: spec.implementation_status,
                    target_scope: spec.target_scope,
                    parameter_spec: spec.parameter_spec,
                    result_spec: spec.result_spec,
                }
            }

            pub fn implemented_metadata() -> Vec<ActionMetadata> {
                Self::ALL
                    .iter()
                    .copied()
                    .map(Self::metadata)
                    .filter(|metadata| metadata.implementation_status == ActionImplementationStatus::Implemented)
                    .collect()
            }

            pub fn is_implemented(self) -> bool {
                self.spec().implementation_status == ActionImplementationStatus::Implemented
            }

            fn spec(self) -> ActionSpec {
                match self {
                    $($(Self::$variant => ActionSpec {
                        name: $name,
                        implementation_status: ActionImplementationStatus::$status,
                        target_scope: TargetScope::$target,
                        parameter_spec: ActionParameterSpec::$params,
                        result_spec: ActionResultSpec::$result,
                    },)+)+
                }
            }
        }
    };
}

define_action_catalog! {
    instance {
        InstanceList => { name: "instance.list", status: Implemented, target: Instance, params: None, result: InstanceList },
        InstanceInspect => { name: "instance.inspect", status: Implemented, target: Instance, params: None, result: InstanceMetadata },
    }

    app {
        AppPing => { name: "app.ping", status: Implemented, target: Instance, params: None, result: InstanceMetadata },
        AppVersion => { name: "app.version", status: Implemented, target: Instance, params: None, result: InstanceMetadata },
        AppActive => { name: "app.active", status: Implemented, target: Instance, params: None, result: ActiveTarget },
        AppFocus => { name: "app.focus", status: Implemented, target: Instance, params: None, result: Acknowledgement },
    }

    capability {
        CapabilityList => { name: "capability.list", status: Implemented, target: Capability, params: None, result: CapabilityList },
        CapabilityInspect => { name: "capability.inspect", status: Implemented, target: Capability, params: ActionName, result: CapabilityMetadata },
    }

    window {
        WindowList => { name: "window.list", status: Implemented, target: Window, params: None, result: TargetList },
        WindowInspect => { name: "window.inspect", status: Implemented, target: Window, params: None, result: TargetMetadata },
        WindowCreate => { name: "window.create", status: Implemented, target: Window, params: TabCreate, result: Acknowledgement },
        WindowFocus => { name: "window.focus", status: Implemented, target: Window, params: None, result: Acknowledgement },
        WindowClose => { name: "window.close", status: Implemented, target: Window, params: None, result: Acknowledgement },
        WindowVisorToggle => { name: "window.visor.toggle", status: Implemented, target: Window, params: None, result: Acknowledgement },
        WindowVisorStatus => { name: "window.visor.status", status: Implemented, target: Window, params: None, result: VisorStatus },
    }

    tab {
        TabList => { name: "tab.list", status: Implemented, target: Tab, params: None, result: TargetList },
        TabInspect => { name: "tab.inspect", status: Implemented, target: Tab, params: None, result: TargetMetadata },
        TabCreate => { name: "tab.create", status: Implemented, target: Tab, params: TabCreate, result: Acknowledgement },
        TabMerge => { name: "tab.merge", status: Implemented, target: Tab, params: Direction, result: Acknowledgement },
        TabActivate => { name: "tab.activate", status: Implemented, target: Tab, params: TabActivate, result: Acknowledgement },
        TabMove => { name: "tab.move", status: Implemented, target: Tab, params: Direction, result: Acknowledgement },
        TabClose => { name: "tab.close", status: Implemented, target: Tab, params: TabClose, result: Acknowledgement },
        TabRename => { name: "tab.rename", status: Implemented, target: Tab, params: Rename, result: Acknowledgement },
        TabResetName => { name: "tab.reset_name", status: Implemented, target: Tab, params: None, result: Acknowledgement },
        TabColorSet => { name: "tab.color.set", status: Implemented, target: Tab, params: ColorValue, result: Acknowledgement },
        TabColorClear => { name: "tab.color.clear", status: Implemented, target: Tab, params: None, result: Acknowledgement },
    }

    pane {
        PaneList => { name: "pane.list", status: Implemented, target: Pane, params: None, result: TargetList },
        PaneInspect => { name: "pane.inspect", status: Implemented, target: Pane, params: None, result: TargetMetadata },
        PaneSplit => { name: "pane.split", status: Implemented, target: Pane, params: Direction, result: Acknowledgement },
        PaneFocus => { name: "pane.focus", status: Implemented, target: Pane, params: None, result: Acknowledgement },
        PaneNavigate => { name: "pane.navigate", status: Implemented, target: Pane, params: Direction, result: Acknowledgement },
        PaneResize => { name: "pane.resize", status: Implemented, target: Pane, params: Resize, result: Acknowledgement },
        PaneMaximize => { name: "pane.maximize", status: Implemented, target: Pane, params: None, result: Acknowledgement },
        PaneUnmaximize => { name: "pane.unmaximize", status: Implemented, target: Pane, params: None, result: Acknowledgement },
        PaneClose => { name: "pane.close", status: Implemented, target: Pane, params: None, result: Acknowledgement },
        PaneRename => { name: "pane.rename", status: Implemented, target: Pane, params: Rename, result: Acknowledgement },
        PaneResetName => { name: "pane.reset_name", status: Implemented, target: Pane, params: None, result: Acknowledgement },
        PaneMainGet => { name: "pane.main.get", status: Implemented, target: Pane, params: None, result: MainPane },
        PaneMainSet => { name: "pane.main.set", status: Implemented, target: Pane, params: None, result: MainPane },
        PaneMainClear => { name: "pane.main.clear", status: Implemented, target: Pane, params: None, result: MainPane },
    }

    session {
        SessionList => { name: "session.list", status: Implemented, target: Session, params: None, result: TargetList },
        SessionInspect => { name: "session.inspect", status: Implemented, target: Session, params: None, result: TargetMetadata },
        SessionActivate => { name: "session.activate", status: Implemented, target: Session, params: None, result: Acknowledgement },
        SessionPrevious => { name: "session.previous", status: Implemented, target: Session, params: None, result: Acknowledgement },
        SessionNext => { name: "session.next", status: Implemented, target: Session, params: None, result: Acknowledgement },
        SessionReopenClosed => { name: "session.reopen_closed", status: Implemented, target: Session, params: None, result: Acknowledgement },
    }

    input {
        InputInsert => { name: "input.insert", status: Implemented, target: Input, params: Text, result: Acknowledgement },
        InputReplace => { name: "input.replace", status: Implemented, target: Input, params: Text, result: Acknowledgement },
        InputSubmit => { name: "input.submit", status: Implemented, target: Input, params: Text, result: Acknowledgement },
    }

    theme {
        ThemeList => { name: "theme.list", status: Implemented, target: Appearance, params: None, result: ThemeList },
        ThemeGet => { name: "theme.get", status: Implemented, target: Appearance, params: None, result: ThemeState },
        ThemeSet => { name: "theme.set", status: Implemented, target: Appearance, params: ThemeName, result: Acknowledgement },
        ThemeSystemSet => { name: "theme.system.set", status: Implemented, target: Appearance, params: BooleanValue, result: Acknowledgement },
        ThemeLightSet => { name: "theme.light.set", status: Implemented, target: Appearance, params: ThemeName, result: Acknowledgement },
        ThemeDarkSet => { name: "theme.dark.set", status: Implemented, target: Appearance, params: ThemeName, result: Acknowledgement },
    }

    appearance {
        AppearanceGet => { name: "appearance.get", status: Implemented, target: Appearance, params: None, result: AppearanceState },
        AppearanceFontSizeIncrease => { name: "appearance.font_size.increase", status: Implemented, target: Appearance, params: None, result: Acknowledgement },
        AppearanceFontSizeDecrease => { name: "appearance.font_size.decrease", status: Implemented, target: Appearance, params: None, result: Acknowledgement },
        AppearanceFontSizeReset => { name: "appearance.font_size.reset", status: Implemented, target: Appearance, params: None, result: Acknowledgement },
        AppearanceZoomIncrease => { name: "appearance.zoom.increase", status: Implemented, target: Appearance, params: None, result: Acknowledgement },
        AppearanceZoomDecrease => { name: "appearance.zoom.decrease", status: Implemented, target: Appearance, params: None, result: Acknowledgement },
        AppearanceZoomReset => { name: "appearance.zoom.reset", status: Implemented, target: Appearance, params: None, result: Acknowledgement },
    }

    setting {
        SettingList => { name: "setting.list", status: Implemented, target: Settings, params: Namespace, result: SettingList },
        SettingGet => { name: "setting.get", status: Implemented, target: Settings, params: Key, result: SettingValue },
        SettingSet => { name: "setting.set", status: Implemented, target: Settings, params: KeyValue, result: Acknowledgement },
        SettingToggle => { name: "setting.toggle", status: Implemented, target: Settings, params: Key, result: Acknowledgement },
    }

    keybinding {
        KeybindingList => { name: "keybinding.list", status: Implemented, target: Keybinding, params: None, result: KeybindingList },
        KeybindingGet => { name: "keybinding.get", status: Implemented, target: Keybinding, params: BindingName, result: KeybindingMetadata },
    }

    action {
        ActionList => { name: "action.list", status: Implemented, target: Action, params: None, result: CapabilityList },
        ActionInspect => { name: "action.inspect", status: Implemented, target: Action, params: ActionName, result: CapabilityMetadata },
    }

    surface {
        SurfaceList => { name: "surface.list", status: Implemented, target: Instance, params: None, result: SurfaceList },
        SurfaceSettingsOpen => { name: "surface.settings.open", status: Implemented, target: Surface, params: PageQuery, result: Acknowledgement },
        SurfaceCommandPaletteOpen => { name: "surface.command_palette.open", status: Implemented, target: Surface, params: Query, result: Acknowledgement },
        SurfaceCommandSearchOpen => { name: "surface.command_search.open", status: Implemented, target: Surface, params: Query, result: Acknowledgement },
        SurfaceThemePickerOpen => { name: "surface.theme_picker.open", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceKeybindingsOpen => { name: "surface.keybindings.open", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceWarpDriveOpen => { name: "surface.warp_drive.open", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceWarpDriveToggle => { name: "surface.warp_drive.toggle", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceResourceCenterToggle => { name: "surface.resource_center.toggle", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceAiAssistantToggle => { name: "surface.ai_assistant.toggle", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceCodeReviewOpen => { name: "surface.code_review.open", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceCodeReviewToggle => { name: "surface.code_review.toggle", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceProjectExplorerOpen => { name: "surface.project_explorer.open", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceGlobalSearchOpen => { name: "surface.global_search.open", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceConversationListOpen => { name: "surface.conversation_list.open", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceLeftPanelToggle => { name: "surface.left_panel.toggle", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceRightPanelToggle => { name: "surface.right_panel.toggle", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceVerticalTabsOpen => { name: "surface.vertical_tabs.open", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceVerticalTabsToggle => { name: "surface.vertical_tabs.toggle", status: Implemented, target: Surface, params: None, result: Acknowledgement },
        SurfaceAgentManagementOpen => { name: "surface.agent_management.open", status: Implemented, target: Surface, params: None, result: Acknowledgement },
    }

    file {
        FileOpen => { name: "file.open", status: Implemented, target: File, params: FileOpen, result: Acknowledgement },
    }

    // Fork-local. Namespaced under `drive.sync` rather than `drive` because
    // upstream retired a whole `drive.*` group — `drive.list`, `drive.open`,
    // `drive.object.*`, `drive.workflow.run` — and pins them as unparseable in
    // `malformed_and_removed_action_names_are_not_deserialized`. These are not
    // a revival of those; they drive the git-backed mirror and nothing else.
    drive {
        DriveSyncStatus => { name: "drive.sync.status", status: Implemented, target: Drive, params: None, result: DriveSyncStatus },
        DriveSyncExport => { name: "drive.sync.export", status: Implemented, target: Drive, params: None, result: DriveSyncExport },
        DriveSyncImport => { name: "drive.sync.import", status: Implemented, target: Drive, params: None, result: DriveSyncImport },

        // T1.12. `drive.sync.*` moves the whole store to and from a directory;
        // these three reach one object at a time, which is what an agent that
        // has been asked for "a workflow that does X" actually needs. Reading
        // and writing both speak the mirror's file format rather than a second
        // shape invented here — so `drive object get` prints something `drive
        // object create` accepts, and the format is documented by being the
        // one already on disk.
        DriveObjectList => { name: "drive.object.list", status: Implemented, target: Drive, params: DriveObjectList, result: DriveObjectList },
        DriveObjectGet => { name: "drive.object.get", status: Implemented, target: Drive, params: DriveObjectGet, result: DriveObject },
        DriveObjectCreate => { name: "drive.object.create", status: Implemented, target: Drive, params: DriveObjectCreate, result: DriveObjectWritten },
        DriveObjectTrash => { name: "drive.object.trash", status: Implemented, target: Drive, params: DriveObjectTrash, result: DriveObjectTrashed },
    }

    // Fork-local. `warpctrl` could open every surface and type into exactly one
    // of them — the terminal — so an agent could start a shell command and
    // nothing else. `input.submit` runs its text as a *command*: sending
    // `/agent do the thing` that way reaches `bash`, not the agent
    // (`.fork/TASKS.md`, T6.5). These are the missing half.
    agent {
        AgentList => { name: "agent.list", status: Implemented, target: Instance, params: None, result: AgentConversationList },
        AgentPrompt => { name: "agent.prompt", status: Implemented, target: Agent, params: AgentPrompt, result: AgentConversation },
        // `Instance` rather than `Agent`: a conversation id addresses this on
        // its own, and it outlives the pane that showed it.
        AgentRead => { name: "agent.read", status: Implemented, target: Instance, params: AgentRead, result: AgentTranscript },
        // `Agent`: the targeted pane supplies the default parent conversation.
        AgentSpawn => { name: "agent.spawn", status: Implemented, target: Agent, params: AgentSpawn, result: AgentSpawnedChild },
        AgentCancel => { name: "agent.cancel", status: Implemented, target: Instance, params: AgentCancel, result: AgentCancellation },
        // `Instance`: settling is a fact about a thread, not about a pane, and
        // the threads most worth settling have no pane open (T8.3).
        AgentSettle => { name: "agent.settle", status: Implemented, target: Instance, params: AgentSettle, result: AgentSettled },
        // `Agent` rather than `Instance`: `swap` replaces the contents of a
        // pane, so which pane is part of the request.
        AgentReveal => { name: "agent.reveal", status: Implemented, target: Agent, params: AgentReveal, result: AgentRevelation },
    }

    // Fork-local. The slash-command registry is where Warp keeps the verbs an
    // agent needs to manage a conversation rather than merely hold one —
    // `/compact`, `/fork-and-compact`, `/plan`, `/queue`, `/model`. They all
    // route through one function, `Input::execute_slash_command`, so exposing
    // the registry costs one action rather than one per verb.
    slash {
        SlashList => { name: "slash.list", status: Implemented, target: Slash, params: None, result: SlashCommandList },
        SlashRun => { name: "slash.run", status: Implemented, target: Slash, params: SlashRun, result: Acknowledgement },
    }

    // Fork-local (`.fork/IDEAS.md`, I16). Warp's remote-development stack has
    // one transport, SSH, reached only when warpify notices a submitted `ssh`
    // command. `WslTransport` is the second, and a WSL connection has no
    // equivalent ambient trigger — Zed's is an explicit "Add WSL Distro" menu
    // entry. This is the data source that entry needs, and the answer to
    // "is this machine even a candidate" before any UI is built.
    //
    // `Instance` scope: a distribution belongs to the machine, not to a pane.
    remote {
        RemoteWslList => { name: "remote.wsl.list", status: Implemented, target: Instance, params: None, result: RemoteWslDistroList },

        // `Session` scope: a remote server attaches to one terminal session,
        // the same way the SSH transport attaches to the pane running `ssh`.
        // The difference is that `ssh` announces itself and a WSL pane does
        // not, which is why this is an action rather than a hook.
        RemoteWslConnect => { name: "remote.wsl.connect", status: Implemented, target: Session, params: RemoteWslConnect, result: RemoteWslConnectStarted },
    }
}
