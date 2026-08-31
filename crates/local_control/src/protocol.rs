//! Wire protocol envelopes and error types for Warp local control.
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use crate::catalog::{
    ActionImplementationStatus, ActionKind, ActionMetadata, ActionParameterSpec, ActionResultSpec,
    PROTOCOL_VERSION, TargetScope,
};
pub use crate::selectors::{
    PaneSelector, PaneTarget, SessionSelector, SessionTarget, TabSelector, TabTarget,
    TargetSelector, WindowSelector, WindowTarget,
};

/// Common layout direction values accepted by pane and tab mutations.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
    Previous,
    Next,
}

/// Tab type accepted by `tab.create` and `window.create`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabType {
    Terminal,
    Agent,
    CloudAgent,
    Default,
}

/// Mode accepted by `tab.activate`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabActivationMode {
    Target,
    Previous,
    Next,
    Last,
}

/// Mode accepted by `tab.close`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabCloseMode {
    Target,
    Active,
    Others,
    RightOf,
}

/// Empty parameters for actions whose catalog parameter spec is `none`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionNameParams {
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingNameParams {
    pub binding_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BooleanValueParams {
    pub value: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorValueParams {
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectionParams {
    pub direction: Direction,
}

/// Parameters for opening a file in Warp's app/editor state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileOpenParams {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(default)]
    pub new_tab: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyParams {
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyValueParams {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamespaceParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageQueryParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameParams {
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResizeParams {
    pub direction: Direction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabActivateParams {
    pub mode: TabActivationMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabCloseParams {
    pub mode: TabCloseMode,
}

/// Parameters for `tab.create` and `window.create`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabCreateParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_type: Option<TabType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextParams {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeNameParams {
    pub theme_name: String,
}

/// Parameters for `agent.prompt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentPromptParams {
    /// The prompt. Newlines are allowed here, unlike [`TextParams`]: that
    /// restriction exists so one `input.submit` runs exactly one shell command,
    /// and a prompt is not a command.
    pub prompt: String,
    /// The conversation to continue. `None` starts a new one.
    ///
    /// A conversation is addressed rather than a pane because that is the unit
    /// an agent hands work to — the pane it lives in can be split, moved
    /// between tabs, or closed and reopened underneath it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
}

/// Parameters for `agent.read`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentReadParams {
    /// The conversation to read. Required — unlike `agent.prompt` there is no
    /// sensible default, since "the conversation in front of a pane" is exactly
    /// the one an orchestrator already knows about.
    pub conversation_id: String,
    /// Return only the last N exchanges. `None` returns all of them.
    ///
    /// The common case is `1`: an orchestrator waiting on a child wants the
    /// answer, not the whole transcript it already dispatched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<u32>,
    /// Include tool-call results in the output text.
    ///
    /// Off by default because it is the difference between an answer and a
    /// session log: every file read, every command run and its full stdout are
    /// in there, and a caller that pastes the result into another agent's
    /// prompt pays for all of it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_tool_results: bool,
}

/// Parameters for `agent.spawn`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSpawnParams {
    /// The self-contained prompt the child starts from.
    ///
    /// Self-contained is the operative word: a child does not inherit its
    /// parent's transcript, so anything it needs to know has to be here.
    pub prompt: String,
    /// A name for the child, shown on its pill and used as its title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The conversation to parent it to. Defaults to the one in front of the
    /// targeted pane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_conversation_id: Option<String>,
    /// The tools the child may use. Omit for no restriction.
    ///
    /// Each entry is either a preset — `read-only` — or a `ToolType` name such
    /// as `READ_FILES` or `RUN_SHELL_COMMAND`, case-insensitive and accepting
    /// dashes for underscores. An empty list is a policy and means no tools,
    /// which is not the same as omitting the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_tools: Option<Vec<String>>,
}

/// The result of `agent.spawn`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSpawnResult {
    pub conversation_id: String,
    pub parent_conversation_id: String,
    /// How deep the child sits: a conversation a person started is 0.
    pub depth: u32,
    /// The tools it may use, resolved from `allow_tools`.
    ///
    /// Echoed back because the request is written in presets and the policy is
    /// enforced in `ToolType`s. A caller that asked for `read-only` should be
    /// able to see exactly what that turned out to mean rather than trust a
    /// name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
}

/// Parameters for `agent.cancel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCancelParams {
    pub conversation_id: String,
}

/// Params for `agent.settle` (T8.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettleParams {
    pub conversation_id: String,
    /// `false` unsettles. Explicit rather than a toggle, because a caller that
    /// cannot see the current state would otherwise have to read it first and
    /// race anyone else changing it.
    #[serde(default = "default_true")]
    pub settled: bool,
}

fn default_true() -> bool {
    true
}

/// One thing an agent is waiting on a person for, as `agent.approvals` reports
/// it (T11.5).
///
/// **Every field above `digest` is what a person is being asked to decide on**,
/// and `digest` is a hash over exactly those fields. `agent.approve` requires the
/// digest back, so an answer can only ever land on the request it was shown —
/// see [`AgentApproveParams::digest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApproval {
    /// The pane the agent is running in — the same id `pane.list` reports.
    ///
    /// A pane rather than a conversation id, because a CLI agent is a *process
    /// in a terminal*: it has no `AIConversation` and nothing in Warp's history
    /// model. The pane is the only handle both ends already share.
    pub approval_id: String,
    /// Which CLI agent is asking: `claude`, `codex`, `gemini`, …
    pub agent: String,
    /// Which population this entry came from: `pane` or `acp`.
    ///
    /// **Fork (T14.6). Stated by the server, never derived by the client**, and
    /// that is the whole reason it exists. The two populations are built by two
    /// different functions that each know first-hand which they are; a client
    /// guessing from the shape of `approval_id`, or from whether `tab_id` is
    /// set, would be reading a fact off an incidental field.
    ///
    /// It is load-bearing because [`Self::cwd`] means **different things** in
    /// the two. For `pane` it is the agent's own working directory, reported
    /// over OSC. For `acp` it is the directory *Warp* chose for the session and
    /// sent in `session/new` — which is not necessarily where the call acts, and
    /// on T14.6 was measured deciding whether the user's permission rules loaded
    /// at all. One label for both would be wrong for one of them, and a card
    /// that grew a *Yes* button is not a place to be vague about which
    /// directory is being shown.
    pub source: String,
    /// `permission` when a tool is named, `question` otherwise.
    ///
    /// **A derivation, not something the agent said.** A permission request and
    /// an "ask the user" both arrive as the same blocked state; the only thing
    /// that tells them apart afterwards is whether a tool name came with it.
    /// Both are answerable, because both are a prompt with a highlighted default
    /// — but "allow" on a `question` means "take the first option", not "yes".
    pub kind: String,
    /// The agent's own one-line description of what it wants to do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// The tool being requested — `Bash`, `Write`, `Edit`, …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// The command for `Bash`, the path for a file tool. Complete, not
    /// truncated — but it is the *only* part of the tool input the OSC
    /// notification carries, so for `Write` it names the file and says nothing
    /// about the contents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<String>,
    /// Where the agent is working.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The agent's own session id, which is what ties this to the lines
    /// `WARP_FORK_EVENT_LOG` wrote for the same session — carried there as
    /// `linked_session_id` since T14.15, because the log files under Warp's
    /// conversation id so that a turn's tools and its frame share one file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
    /// The paths this request said it would touch, as the *agent* named them.
    ///
    /// **Fork (T14.6).** Distinct from [`Self::cwd`], which for an ACP request
    /// is Warp's own session directory. Measured live, a
    /// `session/request_permission` arrives with `locations: []` while the
    /// `tool_call_update` for the same `toolCallId` carried the path moments
    /// earlier, so this is recovered by joining the two — and the join is not a
    /// convenience: where a call acts is what decided, in that same session,
    /// whether the user's own permission rules were loaded at all.
    ///
    /// Empty means the agent never said. A surface must show that as unknown
    /// rather than fall back to `cwd`, because presenting Warp's directory as
    /// the call's is exactly the certainty this fork does not have.
    ///
    /// Always empty for the CLI-agent population: an OSC notification carries a
    /// tool name and a command preview, never a location list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acts_on: Vec<String>,
    /// SHA-256 over the fields above, hex. Hand it back to `agent.approve`.
    pub digest: String,
    /// Whether `agent.approve` would be accepted for *this* entry.
    ///
    /// **Fork (T14.6).** Deliberately below `digest` and deliberately not in it:
    /// this is a fact about Warp's policy, not about the question the agent
    /// asked, and folding it into the hash would change a digest without the
    /// question having changed.
    ///
    /// It exists because the listing and the answer disagreed, and the console
    /// believed the listing. `agent.approvals` reports **every** blocked session,
    /// `agent.approve` refuses any agent outside the verified set, and
    /// `console.js` drew its *Yes* from the paired device's action list — a
    /// per-device fact — with no per-entry check. So a phone with remote approve
    /// enabled showed a Yes on a `codex` or `opencode` row that could never work.
    /// The same trap would have swallowed T14.6's ACP entries whole, since none
    /// of them are approvable yet.
    #[serde(default)]
    pub can_approve: bool,
    /// Why not, when [`Self::can_approve`] is false — in a sentence meant for a
    /// person, because a screen showing only *No* has to say whether that is a
    /// setting or a fault.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approve_refused_because: Option<String>,
    /// The option id a yes would send back, for an ACP request that has one.
    ///
    /// **Fork (T14.6), and unlike its two neighbours this one *is* in the
    /// digest.** They describe Warp's policy; this describes the answer itself,
    /// so it is part of what a person is agreeing to and an answer has to be
    /// bound to it. Binding the id rather than the option's name is deliberate:
    /// the name is what was read and is already in [`Self::options_offered`],
    /// while the id is what actually goes on the wire.
    ///
    /// `None` for every CLI-agent entry — that path presses a key, and a key has
    /// no id — and for any ACP request Warp will not approve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approve_selects: Option<String>,
    /// Every answer the agent itself offered, by name.
    ///
    /// **Fork (T14.6).** Data, not controls. An ACP agent sends its options
    /// typed — `allow_once`, `allow_always`, `reject_once` — and the fact that
    /// one was *offered* is worth recording even where Warp will not render it:
    /// "the offer went unrecorded" is the finding `acp_permission`'s
    /// `is_more_than_an_answer` exists for. An `allow_always` carrying no
    /// declaration, which is measured to be what `opencode` sends, can never
    /// become a button — there is nothing to show a person but the name — but it
    /// can be written down.
    ///
    /// Empty for the CLI-agent population, whose prompt is drawn on a PTY and
    /// whose options Warp never sees.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options_offered: Vec<String>,
}

/// Result of `agent.approvals`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentApprovalsResult {
    pub approvals: Vec<PendingApproval>,
}

/// Parameters for `agent.approve` and `agent.deny` (T11.5).
///
/// **The decision is the action name, not a field here**, and that is a security
/// decision rather than a stylistic one. A paired device is granted a *list of
/// actions*, so anything that needs its own grant has to be its own action:
/// `agent.deny` can only ever make less happen and is pairable, while
/// `agent.approve` is a yes to whatever the agent proposed and is not — unless
/// the machine's owner turns it on. Folding both into a `decision` field would
/// have made that boundary unexpressible in the mechanism T11.4 built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentApproveParams {
    /// From [`PendingApproval::approval_id`].
    pub approval_id: String,
    /// From [`PendingApproval::digest`], unchanged.
    ///
    /// **Required, with no way to opt out.** Without it this action means "press
    /// a key on whatever that pane is asking now", and the thing being answered
    /// could have changed between the phone rendering it and the thumb landing.
    /// With it, an answer that arrives late is refused instead of applied to the
    /// wrong question.
    pub digest: String,
}

/// The result of `agent.approve` / `agent.deny`: what was sent, and where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentApproveResult {
    pub approval_id: String,
    pub decision: String,
    pub agent: String,
    /// The keystroke written to the agent's terminal — `enter` or `escape`.
    ///
    /// Reported because it is the whole mechanism, and hiding it would overstate
    /// what happened: Warp does not tell the agent "approved", it presses the
    /// key a person sitting there would have pressed. Whether the agent acted on
    /// it is answered by reading `agent.approvals` again, not by this field.
    pub keystroke: String,
}

/// Where `agent.reveal` should put a conversation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRevealTarget {
    /// Split it off beside the pane it was spawned from. The default: the only
    /// one of the three that adds a surface rather than taking one over, which
    /// matters when the caller cannot see what it is about to replace.
    #[default]
    Pane,
    /// Open it in a new tab.
    Tab,
    /// Swap it into the targeted pane, as clicking its pill does.
    Swap,
}

/// Parameters for `agent.reveal`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRevealParams {
    pub conversation_id: String,
    #[serde(default)]
    pub target: AgentRevealTarget,
}

/// Parameters for `slash.run`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlashRunParams {
    /// The command name as it appears in the menu, with or without the leading
    /// `/` — `compact` and `/compact` both resolve.
    pub command: String,
    /// The argument, for commands that take one: the instructions to
    /// `/compact-and`, the prompt to `/agent`, the name to `/model`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<String>,
    /// Run a command outside the orchestration allowlist.
    ///
    /// The registry holds `/logout`, `/exit` and `/clear` next to `/compact`
    /// and `/plan`. An agent driving `warpctrl` should not end its own session
    /// by mistyping a command name, so anything not on the allowlist is refused
    /// unless this is set. See `slash_command_is_orchestration`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force: bool,
}

pub type KeybindingGetParams = BindingNameParams;
pub type KeybindingListParams = EmptyParams;
pub type SettingGetParams = KeyParams;
pub type SettingListParams = NamespaceParams;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionListResult {
    pub actions: Vec<ActionMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionInspectResult {
    pub action: ActionMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveTargetChain {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// One agent conversation, as `agent.list` reports it.
///
/// Deliberately flat and stringly-typed: this is what an orchestrating agent
/// reads to decide what to do next, and it has to survive `jq`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentConversationSummary {
    pub conversation_id: String,
    /// The conversation's own title — its task description, or failing that its
    /// first query. Absent for a conversation that has not been given one yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// `in_progress`, `success`, `error`, `transient_error`, `cancelled`,
    /// `blocked`, `waiting_for_events`. The one field a caller polling for "is
    /// it my turn yet" needs.
    pub status: String,
    /// What `blocked` is blocked on, when it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_action: Option<String>,
    /// Whether the agent is still working. True for `in_progress` only —
    /// `waiting_for_events` is quiescent, and `blocked` is waiting on a person.
    pub is_busy: bool,
    /// Whether the thread has been settled — dealt with, kept, and moved to the
    /// bottom of the inbox (T8.3). Omitted when false, so the ordinary listing
    /// is unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub settled: bool,
    /// The pane hosting it. Absent for a conversation whose terminal surface
    /// has been closed, which outlives the pane it was shown in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
    /// Whether that pane exists but is not on screen — a background child
    /// agent, hidden for `HiddenPaneReason::ChildAgent`.
    ///
    /// Reported rather than inferred from a missing `pane_id`, because the two
    /// are different situations and `agent.reveal` only answers one of them: a
    /// hidden pane can be shown, a closed one cannot.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_hidden: bool,
    /// Seconds since the agent last said anything, for a turn Warp is driving
    /// itself.
    ///
    /// **Fork (T14.10), and it is a symptom rather than a verdict.** `status`
    /// distinguishes working from finished and cannot distinguish working from
    /// wedged: a turn that stalled ran 36 minutes reporting `in_progress` with
    /// no pending approval to answer and no output to read, and was diagnosed
    /// only by `ps` and two screenshots. This is the number that was missing.
    ///
    /// It does not mean the turn is stuck. A long compile and a dead agent look
    /// identical from here, which is exactly why nothing acts on it — a person
    /// reads it and decides, and `agent.cancel` loses nothing if they are wrong.
    ///
    /// `None` for every conversation Warp is not driving through the ACP path
    /// right now, which is most of them. **Absent rather than zero**: a
    /// conversation that finished an hour ago is not quiet for no time at all,
    /// and a caller polling for a wedge must not find one everywhere it looks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet_for_seconds: Option<u64>,
    /// The last tool call the agent announced on this turn.
    ///
    /// **Fork (T14.10).** The pairing is what makes the number readable:
    /// *"quiet for 1100s, last seen `grep -rn kind_name`"* is a report, while
    /// either half alone is a puzzle. Set only by tool calls — a message chunk
    /// refreshes the clock without replacing this — so the description survives
    /// whatever the agent said last before going silent.
    ///
    /// `None` when the agent has announced no tool call yet, which is its own
    /// signal: a turn quiet from its first moment never got started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<String>,
    /// Whether this turn is quiet because it is waiting for a person to answer
    /// a permission request.
    ///
    /// **Fork (T14.10), added within the hour by using the two fields above.** A
    /// turn parked on an approval reported `quiet_for_seconds: 171` — true, and
    /// indistinguishable from the wedge the number exists to reveal. Waiting
    /// forever on a question is the *design*; a person who reads an alarming
    /// number for correct behaviour learns to discount the number, and a signal
    /// that gets discounted has stopped working.
    ///
    /// So this is the field that makes the other two readable: quiet **and**
    /// waiting is fine, quiet and not waiting is worth a look. What is waiting
    /// is in `agent.approvals`, which is where the answer is given.
    ///
    /// Omitted when false, so an ordinary listing is unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub waiting_for_you: bool,
    /// The session mode an ACP agent is running this conversation in (T14.18).
    ///
    /// **The field that explains a quiet conversation**, and it belongs beside
    /// the three above for the reason they belong beside each other: an
    /// orchestrator sees a session doing work and asking nothing, and
    /// `quiet_for_seconds` cannot tell it whether that is a wedge, a turn that
    /// needed no permission, or an agent whose own mode answers on the person's
    /// behalf. Measured: `claude-agent-acp` defaults to `auto`, a model
    /// classifier, and in it a whole turn wrote a file with Warp never asked.
    ///
    /// The agent's own id, verbatim and uninterpreted. **Not a safety signal**:
    /// Warp does not rank modes and cannot tell what any of them permits, so a
    /// reader comparing two ids is comparing two words the agent chose. What
    /// each one means is in the conversation note and the event log, in the
    /// agent's own description.
    ///
    /// Omitted when there is none — the local agent, Warp's own, and any ACP
    /// agent advertising no modes — so an ordinary listing is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentListResult {
    pub conversations: Vec<AgentConversationSummary>,
}

/// The result of `agent.prompt`: which conversation the prompt went to.
///
/// Returned rather than acknowledged, because an orchestrator that starts three
/// agents needs to be able to tell them apart afterwards, and `agent.list`
/// alone cannot say which of three new conversations was the one it just made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPromptResult {
    pub conversation_id: String,
    /// True when this call created the conversation rather than continuing one.
    pub created: bool,
}

/// One turn of a conversation, as `agent.read` reports it.
///
/// Input and output are separate fields rather than one `USER:`/`AGENT:`
/// transcript, because the caller is a program: the thing it usually wants is
/// the last `output`, and making it parse a formatted transcript to find that
/// would be the same mistake as `input.submit` returning a screenshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentExchangeSummary {
    /// Position in the conversation, oldest first and stable across calls, so a
    /// caller polling for new turns can ask "anything after 4?".
    pub index: u32,
    /// The user side of the turn. Absent for an exchange with no user query —
    /// the agent's own follow-on requests are exchanges too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    /// The agent side. Absent while a turn is still streaming its first token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Whether this turn has finished streaming.
    ///
    /// The last exchange of an `in_progress` conversation is the one that has
    /// not, and reading it gives a partial answer — worth knowing before
    /// handing it to another agent as a result.
    pub is_complete: bool,
    /// Why the turn failed, when it did.
    ///
    /// **Fork (T14.6).** Without this the read surface *lies by omission*, and it
    /// was measured doing so: a conversation whose panel was displaying a full
    /// error paragraph read back as an exchange with no `output` and nothing
    /// else — because `FinishedAIAgentOutput::output()` returns `None` for the
    /// `Error` variant, discarding the error *and* whatever the agent had
    /// already said. A caller polling `agent.read` could only conclude the agent
    /// had answered with silence.
    ///
    /// That matters more than a missing field usually would, because
    /// `agent.read` is how this fork checks its own agent paths, including from
    /// another agent. An instrument that reports "no output" for "failed, and
    /// here is why" sends every future investigation to the wrong place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The result of `agent.read`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentReadResult {
    /// The same summary `agent.list` gives, so a caller that reads a
    /// conversation does not have to list to find out whether it has finished.
    pub conversation: AgentConversationSummary,
    pub exchanges: Vec<AgentExchangeSummary>,
    /// How many exchanges the conversation has in total, which is what tells a
    /// caller using `last` whether it saw everything.
    pub exchange_count: u32,
    /// Whether tool-call results are in the output text.
    ///
    /// Not simply an echo of the request: including them needs the action model
    /// of the terminal surface that owns the conversation, and that surface can
    /// be gone. Reported so a caller can tell "no tools were used" from "the
    /// tool results were not reachable".
    pub included_tool_results: bool,
}

/// Result of `events.subscribe` (T11.2): where the stream is, and how long the
/// credential that opens it is good for.
///
/// The bearer token is deliberately **not** echoed here. The caller already has
/// it — it is what authorized this call — and a token that appears in a result
/// is a token that ends up in a shell scrollback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventStreamResult {
    /// Absolute URL of the SSE endpoint, on the same loopback origin as this
    /// request. Given rather than assembled, so a client never has to know the
    /// port convention.
    pub url: String,
    /// When the credential expires. The stream closes at this point and the
    /// client must obtain a new credential and reconnect; it is stated here so
    /// a client can schedule that rather than discover it as a disconnect.
    pub expires_at: DateTime<Utc>,
}

/// Result of `control.pair` (T11.4): a code to show, and what scanning it buys.
///
/// **This one deliberately does the thing [`EventStreamResult`] refuses to do**
/// — it returns a secret, which is how it ends up in a shell scrollback. That is
/// the trade the three-step pairing flow exists to make survivable: this code is
/// good for two minutes and for exactly one redemption, so a scrollback, a
/// screenshot or a photograph of the QR is worth nothing shortly after it was
/// taken. The long-lived secret, the device token, is never returned here and is
/// never displayed anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingResult {
    /// The URL to encode as a QR, with the code in its fragment so the half a
    /// server would log carries nothing.
    pub url: String,
    /// The same QR rendered as text, so a terminal client can show one without
    /// an image viewer.
    pub qr: String,
    /// When the code stops being spendable.
    pub expires_at: DateTime<Utc>,
    /// What a device that scans this will be able to ask for — stated up front,
    /// because "which of these 114 actions does my phone get" is the first
    /// question anyone should ask about a QR code. (Read the count off
    /// `catalog_has_exactly_*_retained_actions`, never off prose — this line
    /// said 110 for four increments after the catalog grew.)
    pub actions: Vec<String>,
}

/// Result of redeeming a pairing code at `POST /v1/pair` (T11.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedDeviceResult {
    /// The long-lived half. Returned exactly once, to the device that spent the
    /// code, over the connection that spent it.
    pub device_token: String,
    /// When the pairing lapses and the device has to be shown a new code.
    pub expires_at: DateTime<Utc>,
    /// The actions this token may be exchanged for credentials for. Given so a
    /// client can present a truthful capability list rather than discovering the
    /// boundary one refusal at a time.
    pub actions: Vec<String>,
}

/// Result of `agent.settle` (T8.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSettleResult {
    pub conversation_id: String,
    pub settled: bool,
    /// Whether this call changed anything. `false` means the thread was
    /// already in the requested state — not an error, and not refused, because
    /// the caller asked for a state rather than for a transition.
    pub changed: bool,
}

/// The result of `agent.cancel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCancelResult {
    pub conversation_id: String,
    /// Whether there was a turn to stop.
    ///
    /// `false` is not an error and the call is not refused for it: an
    /// orchestrator cancelling a child races the child finishing, and both
    /// outcomes leave the conversation in the state the caller asked for. This
    /// says which happened so a caller that cares can tell.
    pub was_running: bool,
    /// The status at the moment of the call, before the stop was dispatched.
    pub status: String,
}

/// The result of `agent.reveal`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRevealResult {
    pub conversation_id: String,
    /// Whether the pane was hidden before this call.
    ///
    /// `false` means the conversation was already on screen and this focused
    /// it, which is a reveal that a person would recognise as one and a
    /// program might not.
    pub was_hidden: bool,
    pub target: AgentRevealTarget,
}

/// One slash command, as `slash.list` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlashCommandSummary {
    pub name: String,
    /// Whether `slash.run` will execute it without `force`.
    pub is_orchestration: bool,
    /// Whether it submits its argument as a prompt to the agent, as `/agent`
    /// and `/compact-and` do, rather than acting on the UI.
    pub submits_prompt: bool,
    /// Whether it would run *in the targeted pane right now*.
    ///
    /// Availability is a property of the pane, not of the build: `/compact`
    /// needs an agent view with an active conversation, `/host` needs a
    /// configured default host. Without this a caller has to discover the
    /// difference by running commands and watching them do nothing.
    pub is_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlashListResult {
    pub commands: Vec<SlashCommandSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeSummary {
    pub name: String,
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeListResult {
    pub themes: Vec<ThemeSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeStateResult {
    pub name: String,
    pub follow_system_theme: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark_theme: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppearanceStateResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    pub follow_system_theme: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark_theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_zoom_percent: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingSummary {
    pub key: String,
    pub value: serde_json::Value,
    pub value_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingListResult {
    pub settings: Vec<SettingSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingGetResult {
    pub setting: SettingSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeybindingSummary {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keystroke: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_keystroke: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeybindingListResult {
    pub keybindings: Vec<KeybindingSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeybindingGetResult {
    pub keybinding: KeybindingSummary,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSummary {
    pub name: String,
    pub is_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceListResult {
    pub surfaces: Vec<SurfaceSummary>,
}

/// What `drive.sync.status` reports: where the mirror would go, and what would
/// go into it. Read-only, and the way to check the destination before running
/// an export that prunes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveSyncStatusResult {
    /// The configured directory, or `None` when the mirror is switched off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Whether that directory exists yet. An export creates it.
    pub path_exists: bool,
    /// Objects that would be written.
    pub objects: usize,
    /// Objects in a team drive or shared by someone else, which are not
    /// mirrored — reported so their absence is explained rather than silent.
    pub not_personal: usize,
    /// Objects whose payload could not be read. Should always be empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreadable: Vec<String>,
    /// Mirrored files with an unresolved merge in them, as `path:line (name)`.
    ///
    /// Both directions refuse while this is non-empty, so it is the first thing
    /// to look at when either one stops working.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicted: Vec<String>,
    /// Workflow aliases whose workflow is not in the mirror, and which
    /// therefore will not travel with it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub aliases_not_mirrored: usize,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

/// What `drive.sync.export` did. `written == 0` with a full `unchanged` is the
/// healthy steady state: it means `git status` is clean.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveSyncExportResult {
    pub path: String,
    pub written: usize,
    pub unchanged: usize,
    pub removed_files: usize,
    pub removed_directories: usize,
    /// Objects whose parent folder was missing, reparented to the top level.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub orphaned: Vec<String>,
    pub not_personal: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreadable: Vec<String>,
    /// Aliases left behind because their workflow is not in the mirror.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub aliases_not_mirrored: usize,
}

/// What `drive.sync.import` did to the store.
///
/// `trashed` counts objects the store had and the tree did not. They are
/// trashed rather than deleted, so the number is recoverable rather than final.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveSyncImportResult {
    pub path: String,
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub trashed: usize,
    /// Files that are not Warp Drive objects — a README, a note — with the
    /// reason each was left alone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored: Vec<String>,
    /// One identity found in more than one file. The first by path won.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duplicates: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreadable: Vec<String>,
    /// Workflow alias entries added or rewritten from the tree.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub aliases_set: usize,
    /// Alias entries dropped because the tree's workflow no longer lists them.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub aliases_removed: usize,
    /// Aliases taken from a workflow outside the mirror, which is a change to
    /// something the tree does not describe and so is named rather than counted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases_reassigned: Vec<String>,
}

/// Which objects `drive.object.list` should report.
///
/// Trashed objects are excluded by default and included on request rather than
/// the other way round: the trash is where things go to be forgotten, and a
/// list that silently mixed them in would have a caller acting on an object the
/// user believes they deleted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveObjectListParams {
    #[serde(default)]
    pub include_trashed: bool,
    /// Only objects of this type — `workflow`, `notebook`, `folder`,
    /// `env-vars`, or one of the other JSON types the store holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_type: Option<String>,
}

/// What `drive.object.list` reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveObjectListResult {
    pub objects: Vec<DriveObjectSummary>,
    /// Objects in a team drive or shared by someone else. Not listed, for the
    /// same reason they are not mirrored, and counted so their absence has an
    /// explanation.
    pub not_personal: usize,
    /// Objects matching the filter but hidden because they are in the trash.
    /// Zero when `include_trashed` was set.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub trashed_hidden: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreadable: Vec<String>,
}

/// One object, as `drive.object.list` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveObjectSummary {
    pub id: String,
    pub object_type: String,
    pub name: String,
    /// The containing folders by display name, outermost first. Empty at the
    /// top level of the drive.
    ///
    /// Names rather than the mirror's slugged directory names, because this
    /// answers "where is it in the panel" and the panel shows names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub trashed: bool,
    /// Shortcuts that run this workflow. Workflows only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Which object `drive.object.get` should return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveObjectGetParams {
    /// The object's `uid`, as `drive.object.list` reports it.
    pub id: String,
}

/// One object in full.
///
/// `contents` is the object's file exactly as `drive.sync.export` would write
/// it — front matter plus markdown for a notebook, a JSON envelope otherwise.
/// That is deliberate: it means the format a caller must produce to *create* an
/// object is one it can read back out of an existing one, so there is nothing
/// to document that is not already on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveObjectResult {
    #[serde(flatten)]
    pub summary: DriveObjectSummary,
    pub contents: String,
}

/// What to create, and where.
///
/// Deliberately *not* the file format `drive.object.get` returns, even though
/// symmetry would be pretty. That file's header opens with a `uid` and an
/// `owner`, and neither is the caller's to choose — an id supplied from
/// outside is how you overwrite an object by accident. Asking for a file and
/// then ignoring half its header would be a worse contract than asking for the
/// three things that are genuinely the caller's: what kind, what it is called,
/// and what is in it. The action that writes a caller-supplied identity on
/// purpose is `drive.sync.import`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveObjectCreateParams {
    /// `workflow`, `notebook`, `folder`, or a JSON-backed type such as
    /// `env-vars`.
    pub object_type: String,
    pub name: String,
    /// The object's body: markdown for a notebook, JSON for a workflow or any
    /// other JSON-backed type, and nothing at all for a folder.
    ///
    /// `drive.object.get` on an object of the same type prints a worked
    /// example, which is the intended way to learn the shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// The folder to create it in, by id. Top level when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

/// What `drive.object.create` made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveObjectWrittenResult {
    pub id: String,
    pub object_type: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<String>,
}

/// Which object to trash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveObjectTrashParams {
    pub id: String,
}

/// What `drive.object.trash` did.
///
/// Trashed, not deleted — the same rule an import follows, and for the same
/// reason: it is recoverable from the Warp Drive panel, so a caller that got
/// the id wrong has cost the user a restore rather than their work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveObjectTrashedResult {
    pub id: String,
    pub name: String,
    /// False when the object was already in the trash, which is not an error.
    pub trashed: bool,
}

/// Typed success payloads for catalog actions that need stable structured data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlResult {
    Acknowledgement { action: ActionKind },
    Metadata { data: serde_json::Value },
    Content { data: serde_json::Value },
}

/// Top-level request sent by a local-control client to a Warp instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub protocol_version: u32,
    pub request_id: Uuid,
    #[serde(default)]
    pub target: TargetSelector,
    pub action: Action,
}

impl RequestEnvelope {
    pub fn new(action: Action) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: Uuid::new_v4(),
            target: TargetSelector::default(),
            action,
        }
    }
}

/// Requested action and action-specific JSON parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub kind: ActionKind,
    #[serde(default)]
    pub params: serde_json::Value,
}

impl Action {
    pub fn new(kind: ActionKind) -> Self {
        Self {
            kind,
            params: serde_json::Value::Object(Default::default()),
        }
    }

    pub fn with_params<T: Serialize>(kind: ActionKind, params: T) -> Result<Self, ControlError> {
        Ok(Self {
            kind,
            params: serde_json::to_value(params).map_err(|err| {
                ControlError::with_details(
                    ErrorCode::InvalidParams,
                    format!("failed to serialize {} parameters", kind.as_str()),
                    err.to_string(),
                )
            })?,
        })
    }

    pub fn params_as<T: DeserializeOwned>(&self) -> Result<T, ControlError> {
        serde_json::from_value(self.params.clone()).map_err(|err| {
            ControlError::with_details(
                ErrorCode::InvalidParams,
                format!("failed to decode {} parameters", self.kind.as_str()),
                err.to_string(),
            )
        })
    }
}

/// Top-level response returned by a Warp instance for a control request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub response: ControlResponse,
}

impl ResponseEnvelope {
    pub fn ok(request_id: Uuid, data: serde_json::Value) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            response: ControlResponse::Ok { data },
        }
    }

    pub fn error(request_id: Uuid, error: ControlError) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            response: ControlResponse::Error { error },
        }
    }
}

/// Success or error payload for a control response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ControlResponse {
    Ok { data: serde_json::Value },
    Error { error: ControlError },
}

/// Error envelope used when a request cannot be decoded into a full request envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResponseEnvelope {
    pub protocol_version: u32,
    pub error: ControlError,
}

impl ErrorResponseEnvelope {
    pub fn new(error: ControlError) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            error,
        }
    }
}

/// Structured error returned by local-control protocol and transport layers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ControlError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ControlError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(
        code: ErrorCode,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            details: Some(details.into()),
        }
    }
}

/// Stable error code surfaced to CLI clients and automation.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    LocalControlDisabled,
    UnauthorizedLocalClient,
    InsufficientPermissions,
    ProtocolVersionUnsupported,
    InvalidRequest,
    InvalidSelector,
    InvalidParams,
    NoInstance,
    AmbiguousInstance,
    AmbiguousTarget,
    StaleTarget,
    TargetStateConflict,
    MissingTarget,
    TransportUnavailable,
    BridgeUnavailable,
    UnsupportedAction,
    NotAllowlisted,
    Internal,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_value(self).map_err(|_| std::fmt::Error)?;
        let Some(value) = value.as_str() else {
            return Err(std::fmt::Error);
        };
        f.write_str(value)
    }
}

/// Parameters for `remote.wsl.connect`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteWslConnectParams {
    /// Which distribution to run the remote server in.
    ///
    /// Optional, and the fallback is the point: when the targeted pane is
    /// already running a WSL shell, its own distribution is the obvious answer
    /// and asking for it again would be asking the caller to repeat something
    /// Warp already knows. Required only when the pane is not in WSL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
}

/// What `remote.wsl.connect` reports.
///
/// Deliberately named for *starting*: `RemoteServerManager::connect_session`
/// spawns onto the background executor and returns immediately, so a reply
/// here means the setup pipeline began, not that a server is serving. The
/// session's real state arrives later as `RemoteServerManagerEvent`s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteWslConnectStartedResult {
    /// The terminal session the remote server is being attached to.
    pub session_id: u64,
    pub distro: String,
    /// Whether `distro` came from the pane's own shell rather than the request.
    pub distro_from_pane: bool,
}

/// What `remote.wsl.list` reports.
///
/// `available` is deliberately separate from an empty `distros`. "This machine
/// has no WSL" and "this machine has WSL with nothing installed" are different
/// answers to a caller deciding whether to offer a WSL option at all, and a
/// bare empty list cannot distinguish them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteWslDistroListResult {
    /// Whether `wsl.exe` could be run at all.
    pub available: bool,
    /// Distribution names as `wsl.exe -l -q` reports them, in its order —
    /// which puts the default first.
    pub distros: Vec<String>,
}

/// What `pane.main.get`, `pane.main.set` and `pane.main.clear` report.
///
/// All three answer with the state *after* the call, so a caller never has to
/// follow a mutation with a read to know where it ended up.
///
/// `main_pane_id` is `None` both when no pane was ever designated and when the
/// designated one has since been closed — `PaneGroup::main_pane` validates on
/// read, so a dangling designation is indistinguishable from none, by design.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainPaneResult {
    /// The group's main pane, or `None` if it has none.
    pub main_pane_id: Option<String>,
    /// Its index within the group, for a caller that wants to name it the way
    /// `pane list` does. `None` whenever `main_pane_id` is.
    pub main_pane_index: Option<usize>,
    /// Whether that pane is a terminal, and so can actually anchor the working
    /// directory. A main pane holding an editor is legal and does *not* fall
    /// back to the active pane — it simply stops the ambient surfaces moving.
    pub anchors_working_directory: bool,
}

/// Lifecycle of the dedicated hotkey window, as `window.visor.status` reports it.
///
/// Deliberately four states rather than an `open: bool`. The window is created
/// once and thereafter shown and hidden, so "never created" and "created, then
/// hidden" behave differently on the next toggle — the first builds a window
/// and the second only reveals one — and a caller waiting for a visor to
/// appear needs to tell them apart.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisorState {
    /// No hotkey window has been created in this process.
    Absent,
    /// Created and on screen.
    Open,
    /// Created and shown, but not yet the key window. Warp passes through this
    /// state when it was not the focused app at the time.
    PendingOpen,
    /// Created and hidden off screen. The next toggle reveals this window
    /// rather than building another.
    Hidden,
}

/// What `window.visor.status` reports.
///
/// `window.visor.toggle` deliberately does *not* answer with this, unlike the
/// `pane.main.*` family: toggling is a queued global action that runs after
/// the control-plane request returns, so any state read alongside it would be
/// the state from before the toggle. Poll `window.visor.status` instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisorStatusResult {
    pub state: VisorState,
    /// The hotkey window's id, in the form `window list` reports, so the two
    /// can be joined. `None` exactly when `state` is `absent`.
    pub window_id: Option<String>,
    /// Whether a hotkey window created *now* would open in agent view.
    ///
    /// The effective answer, not the fork's setting: it is `false` with no AI
    /// enabled however the setting is set, and `true` when the default session
    /// mode is already `Agent` whatever the setting says. Reporting the
    /// setting alone would promise an agent and produce a terminal.
    ///
    /// Says nothing about a window that is already open — that one keeps
    /// whatever it was built with, and toggling only hides and reveals it.
    pub opens_agent: bool,
    /// Whether the global shortcut is switched on. `window.visor.toggle` works
    /// regardless: it is a direct dispatch and does not go through the
    /// shortcut, which is the only reason the visor is testable on a platform
    /// whose global grabs do not work.
    pub hotkey_enabled: bool,
    /// The configured shortcut in settings-file form (`"ctrl-shift-Q"`), or
    /// `None` if unbound. Unbound with `hotkey_enabled` true is a real and
    /// common state — the toggle is on and no key was ever chosen.
    pub hotkey: Option<String>,
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
