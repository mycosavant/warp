//! An Agent Client Protocol probe.
//!
//! T14 decided that ACP is the adapter contract for every agent that is not
//! Claude. This is the step that proves it before any app surface is committed
//! to: spawn an ACP agent, run `initialize` → `session/new` → `session/prompt`,
//! and print every `SessionUpdate` the agent sends as one JSON object per line.
//!
//! **Its real output is a mapping table.** The app work — `app/src/ai/acp_agent/`
//! as a sibling of `local_agent/` — has to turn `SessionUpdate` variants into the
//! `ResponseEvent` constructors in `local_agent/translate.rs`. Guessing which
//! variants a real agent actually emits, and in what order, is how that mapping
//! goes quietly wrong. So this prints what genuinely arrived, from a real agent,
//! in order, in a form `jq` can read.
//!
//! Deliberately **not** a catalog action. A catalog action pays a four-test pin
//! tax (two count assertions in different crates, an exhaustive CLI match, and a
//! parseable-example list), and this is a probe whose job is to be deleted once
//! the mapping is known or promoted once it is trusted. It is a sibling of `mcp`
//! and `completions`, which are also `ControlCommand` variants and also not
//! actions.
//!
//! There is no async runtime in this crate — `mcp.rs` says so and means it. None
//! is added here either: `agent-client-protocol` reaches the OS through
//! `async-io` and `blocking`, both of which drive their own threads, so
//! `futures::executor::block_on` is a sufficient host for the whole exchange.

use std::str::FromStr;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome,
    SessionNotification, SetSessionModeRequest, TextContent,
};
use agent_client_protocol::{AcpAgent, Agent, ConnectionTo};
use local_control::protocol::{ControlError, ErrorCode};

use super::{AcpProbeArgs, acp_consent, acp_permission};

fn invalid(message: impl Into<String>) -> ControlError {
    ControlError::new(ErrorCode::InvalidParams, message)
}

fn failed(message: impl Into<String>) -> ControlError {
    ControlError::new(ErrorCode::Internal, message)
}

/// The directory the session will run in, resolved to an absolute path.
///
/// Separate from the exchange so it can be tested without an agent — every way
/// this can go wrong is a wrong answer about the filesystem, and none of them
/// need a model to demonstrate.
///
/// The working directory is a parameter rather than an assumption. T13.3 shipped
/// a review node that read the wrong tree because a spawned agent inherited a cwd
/// nobody had named, and the run still looked like a success. An ACP session
/// carries its cwd explicitly, so name it explicitly — and make it absolute,
/// because a relative path reproduces that failure exactly: it resolves
/// somewhere, silently, and the run still looks fine.
fn session_directory(
    requested: Option<&std::path::Path>,
) -> Result<std::path::PathBuf, ControlError> {
    let cwd = match requested {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|error| failed(format!("cannot read the working directory: {error}")))?,
    };
    if !cwd.is_dir() {
        return Err(invalid(format!(
            "--cwd is not a directory: {}",
            cwd.display()
        )));
    }
    cwd.canonicalize()
        .map_err(|error| invalid(format!("cannot resolve --cwd: {error}")))
}

/// Run one prompt against one ACP agent and print what came back.
pub(super) fn run_probe(args: AcpProbeArgs) -> Result<(), ControlError> {
    if args.command.trim().is_empty() {
        return Err(invalid("--command cannot be empty"));
    }

    let cwd = session_directory(args.cwd.as_deref())?;

    let approve = args.approve;
    let prompt = args.prompt.clone();
    let command = args.command.clone();
    let mode = args.mode.clone();

    // Shared because the two handlers and the exchange all write to it, and
    // `std` rather than a runtime's lock because there is no runtime here and the
    // critical sections are a few field assignments (`acp_consent.rs`).
    let ledger = std::sync::Arc::new(std::sync::Mutex::new(acp_consent::Ledger::new()));
    let updates = std::sync::Arc::clone(&ledger);
    let requests = std::sync::Arc::clone(&ledger);
    let session_modes = std::sync::Arc::clone(&ledger);

    futures::executor::block_on(async move {
        let agent = AcpAgent::from_str(&command)
            .map_err(|error| invalid(format!("cannot read --command: {error}")))?;

        agent_client_protocol::Client
            .builder()
            .on_receive_notification(
                async move |notification: SessionNotification, _cx| {
                    emit("update", &notification.update);
                    if let Ok(mut ledger) = updates.lock() {
                        ledger.observe_update(&notification.update);
                    }
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                async move |request: RequestPermissionRequest, responder, _connection| {
                    // Denial is the default, and `--approve` is the only thing that
                    // changes it. This is the asymmetry T11.5 established for
                    // `agent.approve`/`agent.deny` and T13.3 restated for review
                    // verdicts: saying no can only ever make less happen, so it needs
                    // no switch, and saying yes is the one that does.
                    emit("permission_request", &request);
                    if let Ok(mut ledger) = requests.lock() {
                        ledger.observe_request(&request);
                    }
                    let decision = if approve {
                        acp_permission::Decision::Allow
                    } else {
                        acp_permission::Decision::Deny
                    };
                    // Which option answers a decision is a question with a typed
                    // answer, and taking `options.first()` got it wrong against the
                    // one agent it was ever run on. See `acp_permission.rs`.
                    let (outcome, selected) = match acp_permission::choose(&request, decision) {
                        acp_permission::Choice::Select(option_id) => {
                            if !approve {
                                eprintln!(
                                    "acp: denied a permission request; pass --approve to allow them"
                                );
                            }
                            // By name as well as id, because the report quotes what
                            // the agent called the option rather than renaming it.
                            let name = request
                                .options
                                .iter()
                                .find(|option| option.option_id == option_id)
                                .map(|option| option.name.clone());
                            (
                                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                                    option_id,
                                )),
                                name,
                            )
                        }
                        acp_permission::Choice::Cancel { reason } => {
                            eprintln!("acp: {reason}");
                            (RequestPermissionOutcome::Cancelled, None)
                        }
                    };
                    emit("permission_answer", &outcome);
                    if let Ok(mut ledger) = requests.lock() {
                        let answered = match &outcome {
                            RequestPermissionOutcome::Selected(_) => "selected",
                            _ => "cancelled",
                        };
                        ledger.observe_answer(
                            &request.tool_call.tool_call_id,
                            answered,
                            selected.as_deref(),
                        );
                    }
                    responder.respond(RequestPermissionResponse::new(outcome))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(agent, |connection: ConnectionTo<Agent>| async move {
                let initialized = connection
                    .send_request(InitializeRequest::new(ProtocolVersion::V1))
                    .block_task()
                    .await?;
                // The agent naming itself is the whole argument for ACP over a
                // closed `Harness` enum, so it is recorded rather than logged.
                emit("initialized", &initialized);

                let session = connection
                    .send_request(NewSessionRequest::new(cwd))
                    .block_task()
                    .await?;
                // The agent volunteering its permission mode here is what
                // falsified T14.3's premise that Warp is blind to it.
                emit("session", &session);
                if let Ok(mut ledger) = session_modes.lock() {
                    ledger.observe_session(session.modes.clone());
                }

                // Before the prompt, because a mode asked for after the work is
                // done governs nothing. Requesting is recorded before sending —
                // see `Ledger::observe_mode_request` for the race that matters.
                if let Some(mode) = mode {
                    if let Ok(mut ledger) = session_modes.lock() {
                        ledger.observe_mode_request(&mode);
                    }
                    let acknowledged = connection
                        .send_request(SetSessionModeRequest::new(
                            session.session_id.clone(),
                            mode.clone(),
                        ))
                        .block_task()
                        .await;
                    match acknowledged {
                        Ok(response) => {
                            emit("mode_acknowledged", &response);
                            if let Ok(mut ledger) = session_modes.lock() {
                                ledger.observe_mode_acknowledgement(&mode);
                            }
                        }
                        // Stopping rather than prompting anyway. `--mode` names a
                        // policy, and continuing under a different one is exactly
                        // the shape of the `--approve` bug: a flag that quietly
                        // did the opposite of what it said.
                        Err(error) => {
                            let offered = session
                                .modes
                                .as_ref()
                                .map(|modes| {
                                    modes
                                        .available_modes
                                        .iter()
                                        .map(|mode| mode.id.to_string())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_else(|| "nothing — it declared no modes".to_owned());
                            eprintln!(
                                "acp: the agent refused --mode {mode}: {error}; it offered: {offered}"
                            );
                            if let Ok(ledger) = session_modes.lock() {
                                emit("consent_report", &ledger.report());
                            }
                            return Err(error);
                        }
                    }
                }

                let answer = connection
                    .send_request(PromptRequest::new(
                        session.session_id.clone(),
                        vec![ContentBlock::Text(TextContent::new(prompt))],
                    ))
                    .block_task()
                    .await?;
                emit("stopped", &answer);

                // Last, because it is a summary of everything above and a person
                // reading the tail of a transcript should find it there.
                if let Ok(ledger) = session_modes.lock() {
                    emit("consent_report", &ledger.report());
                }

                Ok(())
            })
            .await
            .map_err(|error| failed(format!("the agent exchange failed: {error}")))
    })
}

/// Print one record as a single line of JSON.
///
/// One object per line, so the transcript is a JSONL file — the same shape
/// `WARP_FORK_EVENT_LOG` already writes, and the shape `jq` reads without
/// arguments. Serialization is not allowed to abort a run: a record that cannot
/// be rendered is reported on stderr and the exchange continues, because the
/// point of a probe is to see everything that arrived, and dying on the first
/// unrepresentable message would hide the rest.
fn emit<T: serde::Serialize>(kind: &str, payload: &T) {
    match record(kind, payload) {
        Ok(line) => println!("{line}"),
        Err(error) => eprintln!("acp: cannot render a {kind} record: {error}"),
    }
}

/// The line `emit` would print. Split out so the transcript's shape is pinned by
/// a test rather than by whatever a live agent happened to send.
fn record<T: serde::Serialize>(kind: &str, payload: &T) -> Result<String, serde_json::Error> {
    let value = serde_json::to_value(payload)?;
    Ok(serde_json::json!({ "kind": kind, "payload": value }).to_string())
}

#[cfg(test)]
#[path = "acp_tests.rs"]
mod tests;
