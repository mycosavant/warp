//! Claude in Oz's seat: the agent conversation, driven from this machine.
//!
//! # What this replaces
//!
//! Warp's whole agent surface — the panel, the blocks, the diffs, the todo
//! list, conversation history — hangs off a single function:
//!
//! ```ignore
//! ai::agent::api::generate_multi_agent_output(server_api, params, cancel)
//!     -> Result<ResponseStream, ConvertToAPITypeError>
//! ```
//!
//! In goes a [`RequestParams`] (the client's entire task list, plus whatever is
//! new this turn). Out comes a stream of `warp_multi_agent_api::ResponseEvent`.
//! Upstream that call POSTs a protobuf `Request` to `{server}/ai/multi-agent`
//! and decodes base64url protobuf off an SSE stream. Nothing above it knows
//! that; it only knows the stream.
//!
//! So this module implements that one function differently. The 70-method
//! `AIClient` trait is not on this path at all — see `.fork/TASKS.md` T5.1.
//!
//! # The protocol is a mutation log, not a token stream
//!
//! The events are not "here is some text". They are `CreateTask`,
//! `AddMessagesToTask`, `UpdateTaskMessage` with a field mask,
//! `BeginTransaction`/`Commit`/`Rollback` — remote mutations against a store
//! the client owns. And because the client sends its whole task list back on
//! every request, the transcript lives on this machine already. That is what
//! makes a local implementation possible at all: nothing has to be recovered
//! from a server, because the server never held it.
//!
//! # Session continuity, for free
//!
//! `StreamInit.conversation_id` is stored by the client as the conversation's
//! server token and handed back as `params.conversation_token` on the next
//! request. Report Claude's session id there and Warp's own round-tripping
//! becomes the session store: first turn `--session-id <uuid>`, every turn
//! after `--resume <uuid>`. This module keeps no state between turns.
//!
//! # What the spike does not do
//!
//! **Claude runs its own tools.** Tool activity is reported to Warp as text,
//! never as a `ToolCall` message — a ToolCall is an instruction, and Warp's
//! action model would execute a tool Claude had already run. So Warp's diff
//! review, command approval and block UI do not participate yet; Claude's own
//! permission prompts govern, and in `-p` mode that means read-only tools work
//! and anything needing approval is denied. Wiring Warp's tool execution back
//! in needs `--input-format stream-json` so results can be fed back mid-turn,
//! and is the next step rather than part of the spike.
//!
//! Also not done: model selection (Claude Code picks its own), attachments,
//! MCP context, and every input type other than a user query — those fall
//! through to upstream untouched.

mod translate;

use std::collections::VecDeque;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context as _, anyhow};
use futures::channel::oneshot;
use futures::stream::{self, Stream, StreamExt as _};
use futures_lite::io::BufReader;
use futures_lite::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _};
use uuid::Uuid;

use self::translate::Translator;
use crate::ai::agent::AIAgentInput;
use crate::ai::agent::api::{Event, RequestParams, ResponseStream};
use crate::server::server_api::AIApiError;

/// Whether this request is one the local agent handles.
///
/// Only a plain user query. Passive suggestions, conversation resume, code
/// review, project init and the rest keep going to upstream's implementation —
/// they have server-side behaviour this spike does not reproduce, and silently
/// answering them from a local model would be worse than not answering.
pub(crate) fn handles(params: &RequestParams) -> bool {
    user_query(params).is_some()
}

fn user_query(params: &RequestParams) -> Option<&str> {
    params.input.iter().find_map(|input| match input {
        AIAgentInput::UserQuery { query, .. } => Some(query.as_str()),
        _ => None,
    })
}

/// Runs one turn against the local `claude` CLI.
///
/// Never returns `Err`: a failure to start is reported *in* the stream, the
/// same way a transport failure is, so the conversation shows an error instead
/// of the request vanishing.
pub(crate) async fn generate(
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
    match run(turn).await {
        Ok(events) => Box::pin(events.take_until(cancellation_rx)),
        Err(error) => Box::pin(stream::once(async move {
            Err(Arc::new(AIApiError::Other(error))) as Event
        })),
    }
}

/// One turn, in terms of the `claude` CLI rather than of Warp.
///
/// Split out from [`RequestParams`] so the process half can be exercised
/// against the real CLI: `RequestParams` has a private field and cannot be
/// built from this module, which would otherwise put every line below here
/// out of reach of a test.
pub(crate) struct Turn {
    prompt: String,
    /// The Claude session to continue, i.e. Warp's conversation token. `None`
    /// starts a new one.
    session: Option<String>,
    task_id: String,
    task_needs_announcing: bool,
    working_directory: Option<String>,
}

impl Turn {
    fn from_request(params: &RequestParams) -> anyhow::Result<Self> {
        let prompt = user_query(params)
            .ok_or_else(|| anyhow!("The local agent was handed a request with no user query."))?
            .to_owned();

        // An existing conversation already has a task; a new one does not, and
        // the client learns about it from the CreateTask this mints an id for.
        //
        // The *first* task, not the right one: upstream splits a conversation
        // across tasks for subagents and orchestration, and picking correctly
        // means reading `Dependencies`. One local Claude is one agent, so it
        // has one task, and the day that stops being true this is where it
        // shows.
        let (task_id, task_needs_announcing) = match params.tasks.first() {
            Some(task) => (task.id.clone(), false),
            None => (Uuid::new_v4().to_string(), true),
        };

        Ok(Self {
            prompt,
            session: params
                .conversation_token
                .as_ref()
                .map(|token| token.as_str().to_owned()),
            task_id,
            task_needs_announcing,
            working_directory: params.session_context.current_working_directory().clone(),
        })
    }
}

async fn run(turn: Turn) -> anyhow::Result<impl Stream<Item = Event> + Send + use<>> {
    let Turn {
        prompt,
        session,
        task_id,
        task_needs_announcing,
        working_directory,
    } = turn;
    let request_id = Uuid::new_v4().to_string();

    let mut command = command::r#async::Command::new("claude");
    command
        .arg("--print")
        .arg("--output-format")
        .arg("stream-json")
        // `--print --output-format stream-json` refuses to run without it.
        .arg("--verbose");
    match session {
        Some(id) => {
            command.arg("--resume").arg(id);
        }
        None => {
            command.arg("--session-id").arg(Uuid::new_v4().to_string());
        }
    }
    if let Some(directory) = working_directory {
        command.current_dir(directory);
    }
    command
        // The prompt goes over stdin, not argv: it can be arbitrarily long and
        // contain anything, and neither limit nor quoting rule is worth
        // discovering from a user's paste.
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Warp is streaming from this child and it must not outlive the app.
        // On Windows the `command` crate defaults to CREATE_BREAKAWAY_FROM_JOB
        // so that shells survive Warp; its async wrapper exposes no way to
        // clear that, so a hard kill of Warp can still leak this child. Normal
        // shutdown drops the stream, which kills it here.
        .kill_on_drop(true);

    let mut child = command.spawn().context(
        "Could not start `claude`. The local agent needs the Claude Code CLI on PATH \
         (https://claude.com/claude-code).",
    )?;

    let mut stdin = child.stdin.take().context("claude stdin was not piped")?;
    stdin.write_all(prompt.as_bytes()).await?;
    // Claude reads to EOF before answering, so this close *is* "end of prompt".
    drop(stdin);

    let stdout = child.stdout.take().context("claude stdout was not piped")?;
    let stderr = child.stderr.take().context("claude stderr was not piped")?;

    Ok(events(TurnState {
        _child: child,
        lines: Box::pin(BufReader::new(stdout).lines()),
        stderr: Box::pin(stderr),
        translator: Translator::new(task_id, task_needs_announcing, request_id),
        pending: VecDeque::new(),
        ended: false,
    }))
}

/// Generic over the child handle only so the concrete `async_process::Child`
/// never has to be named: `async-process` is a dev-dependency of this crate,
/// reachable here solely through the `command` wrapper.
struct TurnState<C> {
    /// Held only so the child stays alive as long as the stream does, and dies
    /// with it.
    _child: C,
    lines: Pin<Box<dyn Stream<Item = std::io::Result<String>> + Send>>,
    stderr: Pin<Box<dyn futures_lite::AsyncRead + Send>>,
    translator: Translator,
    /// One line can carry several messages, and the stream yields one event at
    /// a time.
    pending: VecDeque<Event>,
    ended: bool,
}

fn events<C: Send + 'static>(state: TurnState<C>) -> impl Stream<Item = Event> + Send {
    stream::unfold(state, |mut state| async move {
        loop {
            if let Some(event) = state.pending.pop_front() {
                return Some((event, state));
            }
            if state.ended {
                return None;
            }

            match state.lines.next().await {
                Some(Ok(line)) => {
                    state
                        .pending
                        .extend(state.translator.on_line(&line).into_iter().map(Ok));
                }
                Some(Err(error)) => {
                    state.ended = true;
                    state.pending.push_back(Err(Arc::new(AIApiError::Other(
                        anyhow::Error::from(error).context("Reading from `claude` failed."),
                    ))));
                }
                None => {
                    state.ended = true;
                    if !state.translator.saw_result() {
                        // The client turns a stream that ends without a
                        // StreamFinished into `UnexpectedEof`, which reads as a
                        // dropped connection. Claude's own complaint is more
                        // use than that, and it is on stderr.
                        let detail = drain(&mut state.stderr).await;
                        state
                            .pending
                            .push_back(Err(Arc::new(AIApiError::Other(anyhow!(
                                "`claude` exited without finishing the turn.{detail}"
                            )))));
                    }
                }
            }
        }
    })
}

/// Reads whatever the child left on stderr, for the error message.
///
/// Only called after stdout has closed, so the process is gone and this cannot
/// block on a live pipe. It can still truncate: nothing drains stderr while the
/// turn runs, so a child that wrote more than the pipe buffer holds would have
/// blocked, and only the first buffer's worth survives. Claude's stderr is a
/// line or two in practice.
async fn drain(stderr: &mut Pin<Box<dyn futures_lite::AsyncRead + Send>>) -> String {
    let mut buffer = String::new();
    if stderr.read_to_string(&mut buffer).await.is_err() {
        return String::new();
    }
    let detail = buffer.trim();
    if detail.is_empty() {
        String::new()
    } else {
        format!(" It said: {detail}")
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
