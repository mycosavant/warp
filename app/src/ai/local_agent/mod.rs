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
//! review, command approval and block UI do not participate; Claude's own
//! permission settings govern, and Warp is never consulted —
//! `warpctrl agent approvals` stays empty for the whole of a turn that writes
//! files.
//!
//! **This paragraph used to end "in `-p` mode that means read-only tools work
//! and anything needing approval is denied", and running it says otherwise**
//! (T14.7 Phase 0). `-p` is not read-only: with the user's own
//! `defaultMode: auto`, `Write` ran and asked nobody, which is that setting
//! working rather than a gap. And a call the settings *deny* does not hang for
//! want of a TTY — it comes back as a tool error, which Claude narrates in prose
//! and continues past. The honest limitation is neither "read-only" nor
//! "denied": it is that the decision was made in a file Warp never read.
//! Wiring Warp's tool execution back in needs `--input-format stream-json` so
//! results can be fed back mid-turn, and is the next step rather than part of
//! the spike.
//!
//! It does mean nothing else in Warp observes those tools, which is why they are
//! projected to the fork's event log from here instead (T11.1c): `translate.rs`
//! accumulates a [`ToolEvent`] per `tool_use` and per `tool_result`, and the
//! stream below drains them into `event_log::local_agent` under Warp's own
//! conversation id. That is a record, not participation — nothing in the app
//! reads it.
//!
//! Also not done: model selection (Claude Code picks its own), attachments,
//! MCP context, and every input type other than a user query and a `/compact`
//! — those fall through to upstream untouched.

mod tools;
mod translate;

use std::collections::VecDeque;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context as _, anyhow};
use chrono::Utc;
use futures::channel::oneshot;
use futures::stream::{self, Stream, StreamExt as _};
use futures_lite::io::BufReader;
use futures_lite::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _};
use uuid::Uuid;
use warp_terminal::shell::ShellLaunchData;

pub(crate) use self::translate::ToolEvent;
use self::translate::{Mode, Translator};
use crate::ai::agent::AIAgentInput;
use crate::ai::agent::api::{Event, RequestParams, ResponseStream};
use crate::event_log::local_agent::TurnContext;
use crate::server::server_api::AIApiError;

/// Whether this request is one the local agent handles.
///
/// A plain user query, and a `/compact`. Passive suggestions, conversation
/// resume, code review, project init and the rest keep going to upstream's
/// implementation — they have server-side behaviour this spike does not
/// reproduce, and silently answering them from a local model would be worse
/// than not answering.
pub(crate) fn handles(params: &RequestParams) -> bool {
    ask(params).is_some()
}

/// What a request is asking for, in the only two dialects this speaks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ask {
    /// A user's turn, as typed.
    Query(String),
    /// `/compact` — summarize the conversation and drop what the summary
    /// covers.
    ///
    /// Upstream this is a server-side operation over the message list the
    /// client uploads. Here it is Claude's own `/compact`, run against the
    /// session Claude is already holding, because *that* is the context under
    /// pressure: this fork sends Claude a prompt and Claude keeps the
    /// transcript, so compacting Warp's copy would free nothing.
    Compact {
        /// `/compact <instructions>`, when the user gave any. Both ends spell
        /// it the same way, so this passes straight through.
        instructions: Option<String>,
    },
}

impl Ask {
    /// What gets written to Claude's stdin.
    fn prompt(&self) -> String {
        match self {
            Self::Query(query) => query.clone(),
            Self::Compact { instructions: None } => "/compact".to_owned(),
            Self::Compact {
                instructions: Some(instructions),
            } => format!("/compact {instructions}"),
        }
    }
}

fn ask(params: &RequestParams) -> Option<Ask> {
    params.input.iter().find_map(|input| match input {
        AIAgentInput::UserQuery { query, .. } => Some(Ask::Query(query.clone())),
        AIAgentInput::SummarizeConversation { prompt, .. } => Some(Ask::Compact {
            instructions: prompt
                .as_ref()
                .map(|prompt| prompt.trim().to_owned())
                .filter(|prompt| !prompt.is_empty()),
        }),
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
    ask: Ask,
    /// The Claude session to continue, i.e. Warp's conversation token. `None`
    /// starts a new one.
    session: Option<String>,
    /// Warp's own id for this conversation, which is *not* [`Self::session`].
    ///
    /// Carried only for the event log (T11.1c): it is the key the turn's frame
    /// is already filed under, so tool events filed under it land in the same
    /// file rather than in one named after Claude's session.
    conversation_id: String,
    task_id: String,
    task_needs_announcing: bool,
    working_directory: Option<String>,
    /// The WSL distribution the session lives in, when it is a WSL session.
    /// See [`spawn_for`] for what that changes.
    distro: Option<String>,
    /// The tools this turn's agent may use, when it has been restricted.
    ///
    /// `None` is "no policy", which is what a conversation a person is having
    /// carries. `Some` is a child agent spawned with an allowlist — see
    /// [`tools`] for why this has to be honoured here rather than upstream of
    /// the intercept.
    allowed_tools: Option<Vec<warp_multi_agent_api::ToolType>>,
}

impl Turn {
    fn from_request(params: &RequestParams) -> anyhow::Result<Self> {
        let ask = ask(params)
            .ok_or_else(|| anyhow!("The local agent was handed a request it does not serve."))?;
        let session = params
            .conversation_token
            .as_ref()
            .map(|token| token.as_str().to_owned());

        // A conversation with no token has never had a turn, so there is no
        // Claude session behind it and nothing to compact. Refusing here says
        // that; running anyway would start a *fresh* session and compact it,
        // and Claude would answer "Not enough messages to compact" — a true
        // sentence about the wrong conversation.
        if matches!(ask, Ask::Compact { .. }) && session.is_none() {
            return Err(anyhow!(
                "There is nothing to compact: this conversation has not run a turn yet, so the \
                 local agent has no Claude session to summarize."
            ));
        }

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
            ask,
            session,
            conversation_id: params.conversation_id.to_string(),
            task_id,
            task_needs_announcing,
            working_directory: params.session_context.current_working_directory().clone(),
            distro: match params.session_context.shell() {
                Some(ShellLaunchData::WSL { distro }) => Some(distro.clone()),
                _ => None,
            },
            allowed_tools: params.supported_tools_override.clone(),
        })
    }
}

/// How one turn is spawned.
///
/// Split out from the spawn itself so the decision can be asserted without a
/// `claude` binary, a WSL distribution, or a Windows host.
#[derive(Debug, PartialEq, Eq)]
struct Spawn {
    program: &'static str,
    arguments: Vec<String>,
    working_directory: Option<String>,
}

/// Decides how to run one Claude turn.
///
/// A WSL session's working directory is a Linux path — `/home/you/project` —
/// while Warp on Windows is a Windows process, so `current_dir` on it fails
/// outright: `ERROR_DIRECTORY`, surfaced as "The directory name is invalid".
/// That was this module's own bug, found by running it (`.fork/TASKS.md` T6.1).
///
/// Converting the path to its `\\wsl$\<distro>\...` UNC form would start the
/// process but move the cost: Claude would then read every file through the 9p
/// redirector, measured at ~13× the same tree on the Windows disk and ~50× the
/// same tree from inside the distribution. An agent is a file-reading workload,
/// so that is the whole job made slow.
///
/// So Claude is run *inside* the distribution — the same treatment
/// [`warp_util::git`] gives `git`, for the same reason: run the tool where the
/// files are.
fn spawn_for(
    distro: Option<&str>,
    working_directory: Option<&str>,
    arguments: Vec<String>,
) -> Spawn {
    let Some(distro) = distro else {
        return Spawn {
            program: "claude",
            arguments,
            working_directory: working_directory.map(str::to_owned),
        };
    };

    let mut translated = vec!["--distribution".to_owned(), distro.to_owned()];
    if let Some(directory) = working_directory {
        translated.push("--cd".to_owned());
        translated.push(directory.to_owned());
    }
    // A login shell rather than `--exec claude`: `wsl.exe --exec` searches only
    // a minimal default `PATH` (`/usr/bin`, `/bin`, …), and `claude` is normally
    // installed under the user's home — nvm, `~/.local/bin` — not there.
    // Arguments ride along as positional parameters, so no shell quoting is
    // involved and a prompt cannot become syntax.
    translated.extend([
        "--exec".to_owned(),
        "/bin/sh".to_owned(),
        "-lc".to_owned(),
        r#"exec claude "$@""#.to_owned(),
        "claude".to_owned(),
    ]);
    translated.extend(arguments);

    Spawn {
        program: "wsl.exe",
        arguments: translated,
        // Deliberately unset: `--cd` supplies it inside the distribution, and
        // `wsl.exe` is a Windows process that could not enter it anyway.
        working_directory: None,
    }
}

async fn run(turn: Turn) -> anyhow::Result<impl Stream<Item = Event> + Send + use<>> {
    let Turn {
        ask,
        session,
        conversation_id,
        task_id,
        task_needs_announcing,
        working_directory,
        distro,
        allowed_tools,
    } = turn;
    let log = TurnContext::new(conversation_id, working_directory.clone());
    let prompt = ask.prompt();
    let request_id = Uuid::new_v4().to_string();
    let started_at = Utc::now();

    let mut arguments = vec![
        "--print".to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        // `--print --output-format stream-json` refuses to run without it.
        "--verbose".to_owned(),
    ];
    let mode = match &ask {
        Ask::Query(query) => Mode::Query {
            prompt: query.clone(),
        },
        Ask::Compact { instructions } => Mode::Compact {
            // `Turn::from_request` refuses a compact with no session, so this
            // is the id `--resume` is about to be given.
            session: session.clone().unwrap_or_default(),
            instructions: instructions.clone(),
        },
    };
    match session {
        Some(id) => arguments.extend(["--resume".to_owned(), id]),
        None => arguments.extend(["--session-id".to_owned(), Uuid::new_v4().to_string()]),
    }
    if let Some(allowed_tools) = allowed_tools.as_deref() {
        arguments.extend(tools::permission_arguments(allowed_tools));
    }

    let spawn = spawn_for(distro.as_deref(), working_directory.as_deref(), arguments);
    let mut command = command::r#async::Command::new(spawn.program);
    command.args(&spawn.arguments);
    if let Some(directory) = &spawn.working_directory {
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

    // Named after what was actually run. The first version of this said only
    // "needs the Claude Code CLI on PATH", which was a confident wrong answer
    // when the real failure was the WSL working directory — the cause was in
    // the `Caused by:` line all along, being contradicted by the sentence above
    // it.
    let mut child = command.spawn().with_context(|| match &distro {
        Some(distro) => format!(
            "Could not start `claude` in the {distro} distribution. The local agent runs the \
             Claude Code CLI inside WSL, where the session's files are, so it has to be installed \
             there (https://claude.com/claude-code)."
        ),
        None => "Could not start `claude`. The local agent needs the Claude Code CLI on PATH \
                 (https://claude.com/claude-code)."
            .to_owned(),
    })?;

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
        translator: Translator::new(task_id, task_needs_announcing, request_id, mode, started_at),
        log,
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
    /// What every event-log line of this turn shares (T11.1c).
    log: TurnContext,
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
                    // Drained here rather than written by the translator, so
                    // that file stays free of a filesystem. The log is a
                    // no-op unless something asked for one, and these events
                    // are tool-call paced, so this costs nothing when off.
                    for event in state.translator.take_tool_events() {
                        crate::event_log::local_agent::record(&state.log, &event);
                    }
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
