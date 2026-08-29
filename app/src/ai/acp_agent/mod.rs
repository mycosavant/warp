//! Any agent that is not Claude, in Oz's seat (`.fork/TASKS.md`, T14.5).
//!
//! # What this is a sibling of
//!
//! `ai::local_agent` answers Warp's one agent-transport function from the local
//! `claude` CLI. This answers it from **whatever agent you name**, over the
//! Agent Client Protocol:
//!
//! ```text
//! WARP_FORK_ACP_COMMAND="opencode acp"
//! ```
//!
//! Naming the command is the switch — see [`crate::fork::acp_agent_command`].
//! The seam is the same one: `ai::agent::api::generate_multi_agent_output`, one
//! async fn with `RequestParams` in and a stream of `ResponseEvent` out, and
//! nothing above it knows whether the events came off an SSE socket, a pipe, or
//! a JSON-RPC connection to a program the user chose.
//!
//! Claude deliberately does **not** come this way. Reaching it over ACP means an
//! `npx` shim in front of the CLI `local_agent` already drives directly, so this
//! path is for the other agents — which is also why it was not allowed to be
//! built until one of them had been run (T14.5's gate: `opencode` 1.18.25).
//!
//! # A permission request waits for a person (T14.6)
//!
//! It is listed by `agent.approvals`, said in the conversation rather than
//! swallowed, and answered by `agent.deny` — or, for the requests
//! `acp_permission` can bound to a single call, by `agent.approve`. The
//! responder is parked on the connection's own task and there is **no
//! deadline**: no answer is not no.
//!
//! Which requests those are is decided once, at park time, by
//! [`approvable`] — the shared rule, not a copy of it. What stays refused is
//! anything a *binary* yes cannot honestly carry: a `switch_mode` request, a
//! tool kind this build cannot bound, an option declaring a policy change, and
//! a request carrying no `rawInput`, since approving a one-line title is not
//! approving a command.
//!
//! # This is not read-only, and calling it that was measured false
//!
//! Warp only ever answers the questions an agent chooses to ask, and **an agent
//! is free to ask nothing.** Measured 2026-08-28 through this very path, with
//! `opencode` at its own default permissions and no user configuration:
//!
//! ```text
//! prompt: Create a file called proof.txt containing the word hello.
//! output: `write` `tmp/…/proof.txt` File `proof.txt` created with content `hello`.
//! $ ls    proof.txt        ← written; Warp was never asked, so denied nothing
//! ```
//!
//! The T5 spike had a real guarantee behind the same sentence — `claude -p`
//! refuses its own tools — and that guarantee does **not** transfer. An ACP agent
//! runs tools under its own policy, which Warp cannot see and does not read; the
//! consent report's caveat says exactly this and was corrected to say it a day
//! before this module described itself as read-only anyway.
//!
//! So: this denies what it is asked about. It is **not** a sandbox, not a
//! read-only mode, and not a guarantee about what the agent will do to the
//! machine. Claiming otherwise would be `local_agent/tools.rs:17-20`'s stated
//! nightmare — *"worse than no allowlist, because it reads as a guarantee."*
//!
//! **And a yes makes that sharper rather than softer.** Warp can now select a
//! single-shot allow, which permits exactly the one call a person was shown —
//! and permits nothing about the calls the agent never asks about, which is
//! still most of them. The number of unasked calls is not changed by the answer
//! to an asked one.
//!
//! `warp_cli`'s `acp_permission` is what decides, and it is **shared rather than
//! reimplemented**: its allowlist, its `switch_mode` rule and its `_meta`
//! reading all guard the allow side, and a second copy of an allow rule is a
//! second place for it to be wrong. That constraint was written here while this
//! module still said no to everything, and is now discharged by [`approvable`].
//!
//! # Also not done
//!
//! **A second turn is refused**, honestly and out loud, because every turn starts
//! a fresh session and an agent with no memory of the conversation above it is
//! worse than no answer. See [`Turn::from_request`].
//!
//! `/compact` (the protocol has no compaction), model selection, attachments,
//! MCP context, and Warp's own tools. Those fall through untouched.
//!
//! **The agent's process inherits Warp's working directory**, because
//! `AcpAgentConfig` carries a command, args and env and no cwd. The *session*
//! cwd is carried properly, in `session/new`, and that is what the agent's tools
//! use — measured.
//!
//! **And the session cwd is also where the agent finds its own configuration,
//! which makes the pane's directory a security-relevant input.** An earlier
//! version of this comment said `opencode` reads `opencode.json` from the
//! *process* directory; that was wrong, and only looked right because Warp had
//! been launched from the same place the pane was in. Measured properly, with
//! the process directory held fixed and only the pane's cwd varied: in a
//! directory with no `opencode.json` the agent ran on a fallback model **and
//! with default permissions**, executing a shell command in `$HOME` without
//! sending a single `session/request_permission`; in a directory with one, the
//! same prompt produced a request that reached Warp and was denied.
//!
//! So the user's own agent policy applies or does not apply depending on a
//! directory Warp chose. Nothing here can fix that — it is how project-scoped
//! configuration works — but nothing here may imply otherwise either, which is
//! the T14.3 rule. See `.fork/TASKS.md` T14.6.

pub(crate) mod registry;
mod translate;

use std::str::FromStr as _;
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    ContentBlock, InitializeRequest, NewSessionRequest, PermissionOption, PermissionOptionId,
    PermissionOptionKind, PromptRequest, RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SelectedPermissionOutcome, SessionNotification, TextContent,
};
use agent_client_protocol::{AcpAgent, Agent, ConnectionTo};
use anyhow::{Context as _, anyhow};
use chrono::Utc;
use futures::channel::{mpsc, oneshot};
use futures::stream::{self, Stream, StreamExt as _};
use uuid::Uuid;
use warp_cli::local_control::acp_permission;

use self::translate::Translator;
use crate::ai::agent::AIAgentInput;
use crate::ai::agent::api::{Event, RequestParams, ResponseStream};
use crate::server::server_api::AIApiError;

/// Whether this request is one the ACP path handles.
///
/// A plain user query and nothing else. `/compact` is *not* handled here, and
/// that is a protocol fact rather than a gap: ACP has no compaction operation,
/// so there is nothing to send. Letting it fall through means a `/compact` still
/// reaches whichever implementation can serve it.
pub(crate) fn handles(params: &RequestParams) -> bool {
    query(params).is_some()
}

fn query(params: &RequestParams) -> Option<String> {
    params.input.iter().find_map(|input| match input {
        AIAgentInput::UserQuery { query, .. } => Some(query.clone()),
        _ => None,
    })
}

/// What a person is told when they send a second turn.
///
/// A constant so the tests pin the sentence that actually ships. It is the only
/// thing they will see, so it says what happened and what to do, and names no
/// protocol machinery they cannot act on.
const CANNOT_CONTINUE: &str = "This build starts a new agent session for every turn, so it cannot continue this \
     conversation — the agent would answer with no memory of what you see above. Start a new \
     conversation instead.";

/// One turn, in ACP's terms rather than Warp's.
///
/// Split out from [`RequestParams`] for the same reason `local_agent::Turn` is:
/// `RequestParams` has a private field and cannot be built from this module,
/// which would otherwise put everything below out of reach of a test.
pub(crate) struct Turn {
    prompt: String,
    /// The ACP session this conversation is already holding, if any.
    ///
    /// Only ever `None` on a turn that runs: `from_request` refuses a second
    /// turn. Kept on the struct so a test can assert *that* rather than assert
    /// the absence of a field, and so `session/load` (T14.6) has its place.
    #[expect(
        dead_code,
        reason = "read by tests; the field is where session/load lands"
    )]
    session: Option<String>,
    task_id: String,
    task_needs_announcing: bool,
    working_directory: Option<String>,
}

impl Turn {
    fn from_request(params: &RequestParams) -> anyhow::Result<Self> {
        let prompt = query(params)
            .ok_or_else(|| anyhow!("The ACP agent was handed a request it does not serve."))?;
        let session = params
            .conversation_token
            .as_ref()
            .map(|token| token.as_str().to_owned());

        // **Refused rather than silently answered, and that is a correction.**
        // This shipped starting a fresh ACP session on every turn, with the
        // limitation recorded in a doc comment. Measured through the panel, the
        // limitation is not a footnote — it is the agent contradicting the
        // transcript directly above it:
        //
        //   turn 1  "Create a file called proof.txt…"  → `write`, file created
        //   turn 2  "What word did you just put in it?"
        //           → "I haven't written to or modified any files yet in this session."
        //
        // Warp shows one continuous conversation to an agent that remembers none
        // of it, which is `local_agent`'s own named hazard — "a true sentence
        // about the wrong conversation" — with the roles swapped. A refusal
        // cannot mislead; an amnesiac answer presented as continuous can.
        //
        // The real fix is `session/load`, which `opencode` advertises
        // (`loadSession: true`) and which needs a capability check and a decision
        // about the history it replays. T14.6.
        if session.is_some() {
            return Err(anyhow!(CANNOT_CONTINUE));
        }

        // An existing conversation already has a task; a new one does not, and
        // the client learns about it from the CreateTask this mints an id for.
        let (task_id, task_needs_announcing) = match params.tasks.first() {
            Some(task) => (task.id.clone(), false),
            None => (Uuid::new_v4().to_string(), true),
        };
        Ok(Self {
            prompt,
            session,
            task_id,
            task_needs_announcing,
            working_directory: params.session_context.current_working_directory().clone(),
        })
    }
}

/// Runs one turn against the configured ACP agent.
///
/// Never returns `Err`: a failure to start is reported *in* the stream, the same
/// way a transport failure is, so the conversation shows an error instead of the
/// request vanishing.
pub(crate) async fn generate(
    command: String,
    params: RequestParams,
    cancellation_rx: oneshot::Receiver<()>,
) -> ResponseStream {
    let turn = match Turn::from_request(&params) {
        Ok(turn) => turn,
        Err(error) => {
            return Box::pin(stream::once(async move {
                Err(Arc::new(AIApiError::Other(error))) as Event
            }));
        }
    };
    Box::pin(run(command, turn).take_until(cancellation_rx))
}

/// The connection, as a stream.
///
/// # Why there is no task and no thread here
///
/// `agent-client-protocol` runs a connection through a *scoped* call —
/// `connect_with(agent, |connection| async { … })` — which produces a future,
/// not a stream. Warp's seam wants a stream. The bridge is an unbounded channel
/// plus [`stream::select`]: the driver future pushes events into the sender and
/// yields nothing itself, and selecting the two means **whoever polls the
/// returned stream is what drives the connection**. No executor is borrowed and
/// no thread is spawned, which is the same property the `warpctrl` probe has for
/// the same underlying reason — ACP reaches the OS through `async-io` and
/// `blocking`, both of which drive their own threads.
///
/// It also gives cancellation for free and in the right direction: dropping the
/// stream drops the driver, which drops the connection, which closes the agent's
/// stdin. `generate` wraps this in `take_until(cancellation_rx)`, so a cancelled
/// turn tears the child down rather than leaking it.
fn run(command: String, turn: Turn) -> impl Stream<Item = Event> + Send + use<> {
    let (tx, rx) = mpsc::unbounded::<Event>();
    let started_at = Utc::now();
    let request_id = Uuid::new_v4().to_string();
    let translator = Arc::new(Mutex::new(Translator::new(
        turn.task_id,
        turn.task_needs_announcing,
        request_id,
        turn.prompt.clone(),
        started_at,
    )));

    let driver = drive(
        command,
        turn.prompt,
        turn.working_directory,
        Arc::clone(&translator),
        tx.clone(),
    );
    // The driver yields no items of its own; everything it produces goes through
    // the channel, so its arm is mapped away. `select` ends when both ends do,
    // and the channel ends when `drive` drops the last sender.
    let pump = stream::once(driver).filter_map(|()| std::future::ready(None));
    stream::select(rx, pump)
}

/// One `initialize` → `session/new` → `session/prompt` exchange.
///
/// Failures are reported into the stream rather than returned, because by the
/// time this runs the caller already has a stream and a conversation waiting on
/// it.
async fn drive(
    command: String,
    prompt: String,
    working_directory: Option<String>,
    translator: Arc<Mutex<Translator>>,
    tx: mpsc::UnboundedSender<Event>,
) {
    if let Err(error) = exchange(
        command,
        prompt,
        working_directory,
        Arc::clone(&translator),
        tx.clone(),
    )
    .await
    {
        // **Which of these two you need depends on whether the stream exists,
        // and getting it wrong loses the error in silence.** Measured on T14.6
        // by putting the agent binary out of `PATH`: the conversation went to
        // `status: "error"` with no message in the panel *and none in the log*,
        // because `Translator::open` runs only once the agent has named its
        // session, so a failure to spawn produced a `StreamFinished` addressed
        // to a stream Warp had never been told about.
        //
        // After `open`, a `StreamFinished` is right: the client synthesizes an
        // "unexpected EOF" for a stream that stops without one, which reads as a
        // Warp bug rather than as the agent failing. Flush first, because a turn
        // that fails part-way should still show whatever the agent had said.
        //
        // Before `open`, the only thing Warp is holding is the stream itself, so
        // the error has to *be* an item — the same shape `generate` already uses
        // for a refused continuation, which T14.5 measured reaching the log
        // verbatim.
        let opened = emit(&translator, |translator| translator.stream_was_opened());
        if opened {
            let closing = emit(&translator, |translator| {
                let mut events = translator.flush();
                events.push(translator.failed(format!("{error:#}")));
                events
            });
            for event in closing {
                let _ = tx.unbounded_send(Ok(event));
            }
        } else {
            let _ = tx.unbounded_send(Err(Arc::new(AIApiError::Other(error))));
        }
    }
}

async fn exchange(
    command: String,
    prompt: String,
    working_directory: Option<String>,
    translator: Arc<Mutex<Translator>>,
    tx: mpsc::UnboundedSender<Event>,
) -> anyhow::Result<()> {
    let agent = AcpAgent::from_str(&command)
        .map_err(|error| anyhow!("{error}"))
        .with_context(|| format!("Could not read WARP_FORK_ACP_COMMAND: {command:?}"))?;

    let cwd = match working_directory {
        Some(directory) => std::path::PathBuf::from(directory),
        None => std::env::current_dir()
            .context("The ACP agent needs a working directory and this one could not be read")?,
    };

    let updates = (Arc::clone(&translator), tx.clone());
    // The program name rather than the whole command line: an entry says which
    // agent is waiting, and the arguments are Warp's configuration, not the
    // question being asked.
    let program = command
        .split_whitespace()
        .next()
        .unwrap_or(&command)
        .to_owned();
    let permissions = (
        Arc::clone(&translator),
        tx.clone(),
        cwd.to_string_lossy().into_owned(),
        program,
    );
    let session = (Arc::clone(&translator), tx.clone());

    agent_client_protocol::Client
        .builder()
        .on_receive_notification(
            async move |notification: SessionNotification, _cx| {
                let (translator, tx) = &updates;
                let events = {
                    let mut translator = translator.lock().expect(
                        "the translator lock is held \
                                                                   only for field assignments",
                    );
                    translator.on_update(&notification.update)
                };
                for event in events {
                    let _ = tx.unbounded_send(Ok(event));
                }
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |request: RequestPermissionRequest, responder, connection| {
                let (translator, tx, directory, program) = &permissions;
                // Still no, always — nothing here can show a person what they
                // would be agreeing to, so nothing here may agree. What changed
                // in T14.6 is *when* the no is said: the request is now listed
                // as waiting on a person, and a person's `agent.deny` is what
                // ends it. A denial needs no surface, which is the asymmetry
                // T11.5 established and T14.4 measured the cost of breaking.
                let (outcome, _) = deny(&request);
                // One lock for all three, because the locations join reads state
                // the notification handler writes — taking the lock twice would
                // leave a window where a `tool_call_update` lands between them.
                let (turn, session, acts_on) = emit(translator, |t| {
                    (
                        t.request_id(),
                        t.session_id(),
                        t.locations_for(&request.tool_call.tool_call_id.to_string())
                            .unwrap_or_default(),
                    )
                });
                let parked = parked_request(
                    responder.id(),
                    &turn,
                    &request,
                    program,
                    directory.clone(),
                    session,
                    acts_on,
                );
                let event = emit(translator, |translator| {
                    translator.note(asking_note(&parked))
                });
                let _ = tx.unbounded_send(Ok(event));

                wait_for_a_person(&connection, responder, outcome, parked)
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(agent, |connection: ConnectionTo<Agent>| async move {
            let (translator, tx) = session;
            connection
                .send_request(InitializeRequest::new(ProtocolVersion::V1))
                .block_task()
                .await?;

            let new_session = connection
                .send_request(NewSessionRequest::new(cwd))
                .block_task()
                .await?;
            // The agent's session id becomes Warp's conversation token, and the
            // client hands it back on the next request. Emitted before the
            // prompt so a turn that fails mid-way still leaves the conversation
            // addressable.
            let session_id = new_session.session_id.clone();
            for event in {
                let mut translator = translator
                    .lock()
                    .expect("the translator lock is uncontended");
                translator.open(session_id.to_string())
            } {
                let _ = tx.unbounded_send(Ok(event));
            }

            let answer = connection
                .send_request(PromptRequest::new(
                    new_session.session_id.clone(),
                    vec![ContentBlock::Text(TextContent::new(prompt))],
                ))
                .block_task()
                .await?;

            // The buffered tail first, or the agent's last sentence — usually
            // its whole answer — is dropped on the floor. See `Translator::flush`.
            let closing = emit(&translator, |translator| {
                let mut events = translator.flush();
                events.push(translator.finished(answer.stop_reason));
                events
            });
            for event in closing {
                let _ = tx.unbounded_send(Ok(event));
            }
            Ok(())
        })
        .await
        .map_err(|error| spawn_failure_or(error, &command))
}

/// Turns a connection error into something a person can act on.
///
/// The raw error for an agent that is not on `PATH` is
/// `Internal error: {"spawned_at": "…/jsonrpc.rs:1732:39", "data": "No such file
/// or directory (os error 2)"}` — a crate's line number and an errno, naming
/// neither the command nor the reason. Measured on T14.6.
///
/// The `PATH` sentence is added only for the errno that actually means it, and
/// it is a *guess named as one*: the same errno would arise if the agent existed
/// and its own launcher were missing. Warp cannot tell those apart, so it does
/// not claim to.
fn spawn_failure_or(error: impl std::fmt::Display, command: &str) -> anyhow::Error {
    let error = error.to_string();
    if error.contains("os error 2") || error.contains("No such file or directory") {
        return anyhow!(
            "Could not start the agent named by WARP_FORK_ACP_COMMAND ({command:?}): \
             no such file or directory. The usual cause is that the program is not on \
             the PATH of the process Warp was launched from, which is often shorter \
             than the PATH in a terminal — `nvm` and other version managers add their \
             shims from a login shell only. Underlying error: {error}"
        );
    }
    anyhow!("The agent exchange failed: {error}")
}

/// Describes one permission request the way the control plane needs it.
fn parked_request(
    id: &agent_client_protocol::schema::v1::RequestId,
    turn: &str,
    request: &RequestPermissionRequest,
    agent: &str,
    session_directory: String,
    session_id: Option<String>,
    acts_on: Vec<String>,
) -> registry::ParkedRequest {
    // Carried verbatim rather than summarised. The measured payload puts the
    // command here and *not* in `locations`, so this is where the specifics are.
    let tool_input = request
        .tool_call
        .fields
        .raw_input
        .as_ref()
        .map(|input| input.to_string());
    let (approve_selects, approve_refused_because) = approvable(request, tool_input.is_some());
    registry::ParkedRequest {
        // Scoped to the turn, because a JSON-RPC id is only unique within one
        // connection — measured, two concurrent agents both opened with `0`.
        approval_id: format!("{turn}:{id}"),
        agent: agent.to_owned(),
        title: request.tool_call.fields.title.clone(),
        tool_name: request
            .tool_call
            .fields
            .kind
            .and_then(|kind| serde_json::to_value(kind).ok())
            .and_then(|kind| kind.as_str().map(str::to_owned)),
        tool_input,
        session_directory: Some(session_directory),
        session_id,
        acts_on,
        options_offered: request
            .options
            .iter()
            .map(|option| option.name.clone())
            .collect(),
        approve_selects,
        approve_refused_because,
    }
}

/// What a **yes** on this request would select, or why it is refused.
///
/// Decided here, once, while the real request is in hand — see
/// [`registry::ParkedRequest::approve_selects`] for why freezing it matters.
///
/// **Two gates, and only one of them is this fork's own.** The first is the
/// shared `acp_permission::choose`, which is `warp_cli`'s and is reused rather
/// than reimplemented — a second copy of an allow rule is a second place for it
/// to be wrong. It declines a `switch_mode` request, a tool kind this build
/// cannot bound to one call, and any option declaring a policy change, all
/// because a *binary* yes cannot honestly carry those. It also writes the
/// refusal sentence, which is why none is written here.
///
/// The second gate is local and is about disclosure rather than about scope: an
/// entry with no `rawInput` shows a person the agent's own one-line title and
/// nothing else, and approving a title is not approving a tool call. The module
/// rule this enforces is `acp_permission`'s — *an option may only be selected by
/// a surface capable of showing what that option declares* — applied to the one
/// case where the surface has nothing to show.
fn approvable(
    request: &RequestPermissionRequest,
    shows_the_call: bool,
) -> (Option<String>, Option<String>) {
    if !shows_the_call {
        return (
            None,
            Some(
                "this request carries no tool input, so the only thing Warp could show you is \
                 the agent's own one-line summary — and approving a summary is not approving a \
                 command. Answer it at the agent, or deny."
                    .to_owned(),
            ),
        );
    }
    match acp_permission::choose(request, acp_permission::Decision::Allow) {
        acp_permission::Choice::Select(option_id) => (Some(option_id.to_string()), None),
        acp_permission::Choice::Cancel { reason } => (None, Some(reason)),
    }
}

/// What the conversation says while a request is waiting on a person.
///
/// Two facts with two attributions, and **no sentence about whose rules
/// governed this call**, because Warp cannot know that: measured on T14.6, the
/// pane's directory decides whether the user's own agent configuration loads at
/// all, and nothing on the wire distinguishes "your rules allowed it" from "the
/// agent's defaults did".
fn asking_note(parked: &registry::ParkedRequest) -> String {
    let what = parked.title.as_deref().unwrap_or("a request to act");
    let id = &parked.approval_id;
    // **Per entry, not per population.** This said "Warp cannot say yes to this
    // yet" on every request, which was true when nothing could be approved and
    // became false the moment something could. A refusal whose stated reason has
    // gone false is the T14.2 failure — a person concluding the feature is
    // broken — so the sentence is derived from the frozen decision rather than
    // asserted.
    let how = match &parked.approve_selects {
        Some(_) => format!(
            "Answer yes with `warpctrl agent approve {id}` or no with \
             `warpctrl agent deny {id}` — both take the `digest` that \
             `warpctrl agent approvals` reports. A yes covers this one call and \
             nothing after it."
        ),
        None => {
            let why = parked
                .approve_refused_because
                .as_deref()
                .unwrap_or("Warp cannot say yes to this one");
            format!(
                "Warp will not say yes to this: {why} \
                 Answer no with `warpctrl agent deny {id}`, or cancel the turn."
            )
        }
    };
    let mut note = format!(
        "The agent is waiting for permission: **{what}**. {how} A paired device can \
                 answer too, though *yes* only travels there when WARP_FORK_REMOTE_APPROVE is set."
    );
    // Said before the session directory, because it is the more specific answer
    // to the question a person is actually asking — *where does this happen* —
    // and because leaving it out invites reading the session directory as the
    // answer. When the agent named nothing this stays silent: an absent claim is
    // not a claim about `$HOME`, and filling it in from the session directory
    // would be Warp inventing the one fact T14.6 measured as decisive.
    if !parked.acts_on.is_empty() {
        note.push_str(&format!(
            "\n\nIt says this acts on `{}`.",
            parked.acts_on.join("`, `")
        ));
    }
    if let Some(directory) = parked.session_directory.as_deref() {
        note.push_str(&format!(
            "\n\nThis session runs in `{directory}` — Warp chose that from the pane. \
             The agent resolves its own permission rules from there, and Warp cannot see them."
        ));
    }
    note
}

/// Holds the request open until a person answers it, without blocking the
/// connection's dispatch loop.
///
/// The reply happens **here, on the connection's own task**, which is the
/// arrangement T14.6's spike measured: held 180s, answered within 5s of an
/// out-of-band signal, survived cancellation, and two of them at once resolved
/// independently. The only thing that changed since is where the signal comes
/// from — a `oneshot` from [`registry`] rather than a file appearing.
///
/// **There is no deadline, and that is deliberate.** The spike had one only
/// because a file might never appear; here the resolutions are lifecycle events
/// that all reach through a parked responder — someone answers, the turn is
/// cancelled, or the connection goes away and drops this task with it. An
/// auto-denial on a timer would be a decision taken on the person's behalf and
/// reported as if they had made it, and *no answer is not no* is the grammar
/// this fork already uses for `permission_requests_received: 0`. The keystroke
/// path this joins has no timeout either: a CLI agent's prompt waits as long as
/// it waits.
fn wait_for_a_person(
    connection: &ConnectionTo<Agent>,
    responder: agent_client_protocol::Responder<RequestPermissionResponse>,
    denial: RequestPermissionOutcome,
    request: registry::ParkedRequest,
) -> Result<(), agent_client_protocol::Error> {
    // Resolved before the request is handed to the registry, because after that
    // it is the map's and a caller could already be answering it.
    let allowed = request.approve_selects.clone();
    let (waiting, answer) = registry::park(request);

    connection.spawn(async move {
        // Held for exactly as long as the wait, so a turn that is cancelled stops
        // advertising a question nobody can answer any more.
        let _waiting = waiting;
        // **Every path that is not an explicit, permitted yes is a no**, and the
        // arms are spelled out rather than collapsed so that stays readable.
        // `Err` is the entry having gone without an answer, which can only happen
        // if the turn is already ending. `Ok(Allow)` on an entry with nothing to
        // select cannot be reached — the control plane refuses it by the same
        // frozen field — and denies here anyway, because failing closed has to be
        // true at both ends and not only at the one being looked at.
        let outcome = match answer.await {
            Ok(registry::Decision::Allow) => match allowed {
                Some(option_id) => RequestPermissionOutcome::Selected(
                    SelectedPermissionOutcome::new(PermissionOptionId::from(option_id)),
                ),
                None => denial,
            },
            Ok(registry::Decision::Deny) | Err(_) => denial,
        };
        // A failure to deliver is the agent having gone away, which is not this
        // task's problem to report — and returning `Err` would take the whole
        // connection down with it, per `ConnectionTo::spawn`'s own docs.
        let _ = responder.respond(RequestPermissionResponse::new(outcome));
        Ok(())
    })
}

/// How to say no to one permission request, and what to tell the person.
///
/// `reject_once` when the agent offers one, and [`RequestPermissionOutcome::Cancelled`]
/// otherwise — which is also a no, and the `warpctrl` probe confirmed against a
/// live agent that it is treated as one. Deliberately never reads `_meta` and
/// never looks at the tool kind: those exist to *withhold* a yes, and there is
/// no yes here to withhold.
fn deny(request: &RequestPermissionRequest) -> (RequestPermissionOutcome, String) {
    let note = format!(
        "Warp denied this: **{}**. Run the agent directly if you need it to act without being \
         asked each time.",
        request
            .tool_call
            .fields
            .title
            .as_deref()
            .unwrap_or("a request to act")
    );
    let outcome = match request
        .options
        .iter()
        .find(|option| option.kind == PermissionOptionKind::RejectOnce)
        .map(|option: &PermissionOption| option.option_id.clone())
    {
        Some(option_id) => {
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id))
        }
        None => RequestPermissionOutcome::Cancelled,
    };
    (outcome, note)
}

/// Runs one translator call, without holding the lock across an await.
///
/// The guard is deliberately confined to this function: `std::sync::Mutex` held
/// across an `.await` is the classic way to deadlock an executor, and every call
/// site here is inside an async block.
fn emit<T>(translator: &Mutex<Translator>, f: impl FnOnce(&mut Translator) -> T) -> T {
    let mut translator = translator
        .lock()
        .expect("the translator lock is held only for field assignments");
    f(&mut translator)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
