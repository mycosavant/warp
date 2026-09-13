use super::*;
use crate::ai::agent::{AIAgentText, MessageId};

fn text_message(id: &str, text: &str) -> AIAgentOutputMessage {
    AIAgentOutputMessage {
        id: MessageId::new(id.to_string()),
        message: AIAgentOutputMessageType::Text(AIAgentText {
            sections: vec![AIAgentTextSection::PlainText {
                text: text.to_string().into(),
            }],
        }),
        citations: Vec::new(),
    }
}

/// The failure this exists for, measured in a running Warp: the ACP mode
/// disclosure is a `WarpNote` in the same output as the agent's reply, and it
/// was read aloud first.
#[test]
fn warp_notes_are_not_read_aloud() {
    let note = AIAgentOutputMessage {
        id: MessageId::new("note".to_string()),
        message: AIAgentOutputMessageType::WarpNote {
            headline: "This session is in the agent's `default` mode.".to_string(),
            detail: AIAgentText {
                sections: vec![AIAgentTextSection::PlainText {
                    text: "Warp did not choose it.".to_string().into(),
                }],
            },
        },
        citations: Vec::new(),
    };
    let reply = text_message(
        "reply",
        "The read-aloud test finished. Patching. Testing. Done.",
    );
    assert_eq!(
        spoken_text(&[note, reply]),
        "The read-aloud test finished. Patching. Testing. Done."
    );
}

#[test]
fn consecutive_agent_text_messages_are_all_read() {
    let messages = [
        text_message("a", "First part."),
        text_message("b", "Second part."),
    ];
    assert_eq!(spoken_text(&messages), "First part.\nSecond part.");
}

fn exchange(has_query: bool, output: &str) -> (bool, String) {
    (has_query, output.to_owned())
}

#[test]
fn the_last_reply_starts_at_the_most_recent_user_query() {
    let exchanges = [
        exchange(true, "First answer."),
        exchange(true, "Second answer, part one."),
        exchange(false, ""),
        exchange(false, "Part two, after a tool call."),
    ];
    assert_eq!(
        join_last_reply(&exchanges).as_deref(),
        Some("Second answer, part one.\n\nPart two, after a tool call.")
    );
}

#[test]
fn a_query_with_no_output_yet_has_nothing_to_read() {
    let exchanges = [exchange(true, "Earlier answer."), exchange(true, "  ")];
    assert_eq!(join_last_reply(&exchanges), None);
}

#[test]
fn without_any_user_query_everything_is_the_reply() {
    let exchanges = [exchange(false, "Only output.")];
    assert_eq!(join_last_reply(&exchanges).as_deref(), Some("Only output."));
}

#[test]
fn an_empty_command_is_refused_with_the_setting_named() {
    let error = speak("", "", "hello").unwrap_err().to_string();
    assert!(error.contains("agents.voice.read_aloud.command"), "{error}");
}

/// Calibrates `stop`: a reader that would run for a minute is reported running
/// and killed, and a second stop finds nothing.
#[cfg(unix)]
#[test]
fn stop_kills_a_reader_still_running() {
    speak("sleep", "60", "ignored").expect("sleep starts");
    let started = std::time::Instant::now();
    assert!(stop(), "a sleeping reader should be reported as running");
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    assert!(!stop(), "nothing should be left to stop");
}

#[cfg(unix)]
#[test]
fn the_reply_reaches_the_command_on_stdin() {
    let dir = std::env::temp_dir().join(format!("read-aloud-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("heard.txt");
    let script = dir.join("reader.sh");
    std::fs::write(&script, format!("#!/bin/sh\ncat > '{}'\n", out.display())).unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    speak(script.to_str().unwrap(), "", "Read **this** aloud.").expect("reader starts");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline
        && std::fs::read_to_string(&out)
            .map(|s| s.is_empty())
            .unwrap_or(true)
    {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "Read **this** aloud."
    );
    stop();
    let _ = std::fs::remove_dir_all(&dir);
}
