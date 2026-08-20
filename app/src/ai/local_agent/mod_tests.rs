//! Drives the real `claude` binary.
//!
//! Ignored by default: it needs Claude Code installed and authenticated, and it
//! spends tokens. Run it with
//!
//!     cargo test -p warp --lib ai::local_agent -- --ignored --nocapture
//!
//! What it buys is the half the translator tests cannot reach — that the
//! process is spawned with arguments Claude accepts, that the prompt survives
//! the stdin handoff, that the lines arrive parseable, and that a session
//! resumed by the token Warp round-trips really is the same conversation.

use futures::executor::block_on;
use warp_multi_agent_api as api;

use super::*;

fn turn(prompt: &str, session: Option<String>) -> Turn {
    Turn {
        prompt: prompt.to_owned(),
        session,
        task_id: "task-1".to_owned(),
        task_needs_announcing: true,
        working_directory: None,
        distro: None,
    }
}

/// Collects a whole turn.
fn collect(turn: Turn) -> Vec<api::ResponseEvent> {
    block_on(async {
        let stream = run(turn).await.expect("claude should start");
        stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|event| event.unwrap_or_else(|error| panic!("stream error: {error}")))
            .collect()
    })
}

fn conversation_id(events: &[api::ResponseEvent]) -> String {
    events
        .iter()
        .find_map(|event| match &event.r#type {
            Some(api::response_event::Type::Init(init)) => Some(init.conversation_id.clone()),
            _ => None,
        })
        .expect("every turn opens with a StreamInit")
}

fn agent_text(events: &[api::ResponseEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match &event.r#type {
            Some(api::response_event::Type::ClientActions(actions)) => Some(&actions.actions),
            _ => None,
        })
        .flatten()
        .filter_map(|action| match &action.action {
            Some(api::client_action::Action::AddMessagesToTask(add)) => Some(&add.messages),
            _ => None,
        })
        .flatten()
        .filter_map(|message| match &message.message {
            Some(api::message::Message::AgentOutput(output)) => Some(output.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
#[ignore = "spawns the real `claude`: needs Claude Code authenticated, and spends tokens"]
fn a_real_turn_produces_a_well_formed_stream() {
    let events = collect(turn(
        "Reply with exactly the word pong and nothing else.",
        None,
    ));

    assert!(
        matches!(
            events.first().and_then(|event| event.r#type.as_ref()),
            Some(api::response_event::Type::Init(_))
        ),
        "first event was {:?}",
        events.first()
    );
    assert!(
        matches!(
            events.last().and_then(|event| event.r#type.as_ref()),
            Some(api::response_event::Type::Finished(_))
        ),
        "a stream that ends without StreamFinished is retried three times; \
         last event was {:?}",
        events.last()
    );
    assert!(
        agent_text(&events).to_lowercase().contains("pong"),
        "the prompt did not survive the stdin handoff: {:?}",
        agent_text(&events)
    );

    let created = events.iter().any(|event| match &event.r#type {
        Some(api::response_event::Type::ClientActions(actions)) => {
            actions.actions.iter().any(|action| {
                matches!(
                    action.action,
                    Some(api::client_action::Action::CreateTask(_))
                )
            })
        }
        _ => false,
    });
    assert!(created, "a new conversation must be told about its task");
}

#[test]
#[ignore = "spawns the real `claude` twice: needs Claude Code authenticated, and spends tokens"]
fn the_conversation_token_really_resumes_the_same_conversation() {
    // The claim the whole session design rests on: Warp round-trips
    // `StreamInit.conversation_id` and handing it back as `--resume` continues
    // the same Claude session. If it did not, this fork would have a stateless
    // agent that forgets between turns and no test would say so.
    let first = collect(turn(
        "Remember this word, it is important: rhinoceros. Reply with just: ok",
        None,
    ));
    let session = conversation_id(&first);
    assert!(!session.is_empty(), "no session id came back");

    let mut second_turn = turn(
        "What was the word I asked you to remember? Reply with just that word.",
        Some(session.clone()),
    );
    second_turn.task_needs_announcing = false;
    let second = collect(second_turn);

    assert_eq!(
        conversation_id(&second),
        session,
        "the resumed turn reported a different session, so `--resume` missed"
    );
    assert!(
        agent_text(&second).to_lowercase().contains("rhinoceros"),
        "the second turn did not remember the first: {:?}",
        agent_text(&second)
    );
}

#[test]
fn a_missing_binary_is_reported_in_the_stream_rather_than_swallowed() {
    // Not ignored: it must not find a binary, which is the point.
    let events = block_on(async {
        let mut turn = turn("hello", None);
        turn.working_directory = Some("/definitely/not/a/directory".to_owned());
        run(turn).await.err()
    });

    let error = events.expect("a working directory that does not exist cannot be spawned into");
    assert!(
        format!("{error:#}").contains("claude"),
        "the error should name what failed to start: {error:#}"
    );
}

#[test]
fn a_windows_session_runs_claude_directly_in_its_own_directory() {
    let spawn = spawn_for(None, Some(r"C:\dev\warp"), vec!["--print".to_owned()]);

    assert_eq!(spawn.program, "claude");
    assert_eq!(spawn.arguments, vec!["--print".to_owned()]);
    assert_eq!(spawn.working_directory.as_deref(), Some(r"C:\dev\warp"));
}

#[test]
fn a_wsl_session_runs_claude_inside_the_distribution() {
    // The bug this is named after: Warp on Windows was handed the session's
    // Linux working directory and passed it to `current_dir`, which fails with
    // "The directory name is invalid" before Claude ever runs. See T6.1.
    let spawn = spawn_for(
        Some("Ubuntu"),
        Some("/home/effatha/git/warp"),
        vec!["--print".to_owned(), "--verbose".to_owned()],
    );

    assert_eq!(spawn.program, "wsl.exe");
    assert_eq!(
        spawn.working_directory, None,
        "`wsl.exe` is a Windows process: it must not be asked to enter a Linux path"
    );
    assert_eq!(
        spawn.arguments,
        vec![
            "--distribution",
            "Ubuntu",
            "--cd",
            "/home/effatha/git/warp",
            "--exec",
            "/bin/sh",
            "-lc",
            r#"exec claude "$@""#,
            "claude",
            "--print",
            "--verbose",
        ]
    );
}

#[test]
fn a_wsl_session_with_no_working_directory_still_starts() {
    // A session that has not reported a cwd yet must not become `--cd` with an
    // empty argument, which `wsl.exe` rejects outright.
    let spawn = spawn_for(Some("Ubuntu"), None, Vec::new());

    assert!(
        !spawn.arguments.iter().any(|argument| argument == "--cd"),
        "no directory means no `--cd`: {:?}",
        spawn.arguments
    );
    assert_eq!(
        spawn.arguments.first().map(String::as_str),
        Some("--distribution")
    );
}

#[test]
fn the_prompt_never_becomes_shell_syntax() {
    // Claude's arguments ride as positional parameters after the script, so a
    // prompt or a session id cannot be read as shell. The script itself is
    // fixed text.
    let spawn = spawn_for(
        Some("Ubuntu"),
        None,
        vec!["--resume".to_owned(), "; rm -rf ~".to_owned()],
    );

    let script = spawn
        .arguments
        .iter()
        .position(|argument| argument == "-lc")
        .map(|index| spawn.arguments[index + 1].clone())
        .expect("the login shell takes a script");
    assert_eq!(script, r#"exec claude "$@""#);
    assert!(
        spawn.arguments.contains(&"; rm -rf ~".to_owned()),
        "the argument travels intact, as an argument: {:?}",
        spawn.arguments
    );
}
