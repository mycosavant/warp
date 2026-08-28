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
//! # A spike, on the same terms the last one had
//!
//! **Every permission request is denied**, and the denial is said in the
//! conversation rather than swallowed. So read-only turns work, and anything the
//! agent needs consent for does not.
//!
//! That is not a shortcut, it is the shape `local_agent` shipped in and for the
//! same reason — its module docs say *"Claude's own permission prompts govern,
//! and in `-p` mode that means read-only tools work and anything needing
//! approval is denied. Wiring Warp's tool execution back in ... is the next step
//! rather than part of the spike."* Answering **yes** needs a surface that can
//! show what is being agreed to, and Warp has none yet; T14.4 measured what goes
//! wrong when something says yes without one. That surface is T14.6.
//!
//! Because this only ever says **no**, it needs none of
//! `warp_cli`'s `acp_permission` — that module's allowlist, its `switch_mode`
//! rule and its `_meta` reading all guard the *allow* side, and T14.4 established
//! that refusing is unconditional: declining leaves the session with the policy
//! it already had. **When T14.6 can say yes, that module must be shared rather
//! than reimplemented here**, because a second copy of an allow rule is a second
//! place for it to be wrong.
//!
//! # Also not done
//!
//! `/compact` (the protocol has no compaction), model selection, attachments,
//! MCP context, and Warp's own tools. Those fall through untouched.
//!
//! **The agent's process inherits Warp's working directory**, because
//! `AcpAgentConfig` carries a command, args and env and no cwd. The *session*
//! cwd is carried properly, in `session/new`, and that is what the agent's tools
//! use — measured. What it can change is where an agent looks for its **own**
//! configuration: `opencode` reads `opencode.json` from the process directory.
//! Named rather than solved, because solving it per agent is what ACP exists to
//! avoid.

mod translate;

use std::str::FromStr as _;
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    ContentBlock, InitializeRequest, NewSessionRequest, PermissionOption, PermissionOptionKind,
    PromptRequest, RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionNotification, TextContent,
};
use agent_client_protocol::{AcpAgent, Agent, ConnectionTo};
use anyhow::{Context as _, anyhow};
use chrono::Utc;
use futures::channel::{mpsc, oneshot};
use futures::stream::{self, Stream, StreamExt as _};
use uuid::Uuid;

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

/// One turn, in ACP's terms rather than Warp's.
///
/// Split out from [`RequestParams`] for the same reason `local_agent::Turn` is:
/// `RequestParams` has a private field and cannot be built from this module,
/// which would otherwise put everything below out of reach of a test.
pub(crate) struct Turn {
    prompt: String,
    /// The ACP session to resume, i.e. Warp's conversation token.
    ///
    /// Carried and **not yet used**: `session/load` is an optional capability
    /// (`opencode` advertises it, and whether the agent named in
    /// `WARP_FORK_ACP_COMMAND` does is not knowable in advance), so resuming
    /// needs a capability check this spike does not make. Every turn is a new
    /// session for now, which is a real limitation and is why it is a named
    /// field rather than a dropped one.
    _session: Option<String>,
    task_id: String,
    task_needs_announcing: bool,
    working_directory: Option<String>,
}

impl Turn {
    fn from_request(params: &RequestParams) -> anyhow::Result<Self> {
        let prompt = query(params)
            .ok_or_else(|| anyhow!("The ACP agent was handed a request it does not serve."))?;
        // An existing conversation already has a task; a new one does not, and
        // the client learns about it from the CreateTask this mints an id for.
        let (task_id, task_needs_announcing) = match params.tasks.first() {
            Some(task) => (task.id.clone(), false),
            None => (Uuid::new_v4().to_string(), true),
        };
        Ok(Self {
            prompt,
            _session: params
                .conversation_token
                .as_ref()
                .map(|token| token.as_str().to_owned()),
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
        // A `StreamFinished` rather than an `Err`: the client synthesizes an
        // "unexpected EOF" for a stream that stops without one, which reads as a
        // Warp bug rather than as the agent failing to start.
        // Flush first here too: a turn that fails part-way should still show
        // whatever the agent had already said.
        let closing = emit(&translator, |translator| {
            let mut events = translator.flush();
            events.push(translator.failed(format!("{error:#}")));
            events
        });
        for event in closing {
            let _ = tx.unbounded_send(Ok(event));
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
    let permissions = (Arc::clone(&translator), tx.clone());
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
            async move |request: RequestPermissionRequest, responder, _connection| {
                let (translator, tx) = &permissions;
                // No, always, and the reason is in the module docs: nothing here
                // can show a person what they would be agreeing to, so nothing
                // here may agree. A denial needs no such surface, which is the
                // asymmetry T11.5 established and T14.4 measured the cost of
                // breaking.
                let (outcome, note) = deny(&request);
                let event = emit(translator, |translator| translator.note(note));
                let _ = tx.unbounded_send(Ok(event));
                responder.respond(RequestPermissionResponse::new(outcome))
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
        .map_err(|error| anyhow!("The agent exchange failed: {error}"))
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
        "Warp denied this: **{}**. This build can only say no to an agent's permission \
         requests — there is no surface yet that could show you what saying yes would allow. \
         Run the agent directly if you need it to make changes.",
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
