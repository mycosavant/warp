//! Reading the last agent reply aloud, through a command the user names.
//!
//! Warp's share is deciding *what* is said and handing it over: the reply's
//! text goes to the command's stdin, and stopping kills the command. How it
//! sounds belongs to the command. The fork was built against `pocket-speak`
//! (`mycosavant/pocket-tts`), which strips Markdown itself, so the text is
//! handed over as the Markdown the agent wrote.
//!
//! One reading at a time: starting another stops the one in progress, the
//! way a second press of a media key would.

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Child, Stdio};
use std::sync::Mutex;

use anyhow::{Context as _, anyhow};

use crate::ai::agent::conversation::AIConversation;
use crate::ai::agent::{AIAgentOutputMessage, AIAgentOutputMessageType, AIAgentTextSection};

static READING: Mutex<Option<Child>> = Mutex::new(None);

/// The last reply in `conversation`: what the agent said, from the most recent
/// exchange that carried a user query to the end.
///
/// One reply can span several exchanges when the agent calls tools between
/// pieces of prose, which is why this takes the same span as the AI block's
/// "Copy output" (`get_output_text_since_preceding_user_query`) rather than the
/// last exchange alone.
pub fn last_reply_text(conversation: &AIConversation) -> Option<String> {
    let exchanges: Vec<(bool, String)> = conversation
        .root_task_exchanges()
        .map(|exchange| {
            let spoken = exchange
                .output_status
                .output()
                .map(|output| spoken_text(&output.get().messages))
                .unwrap_or_default();
            (exchange.has_user_query(), spoken)
        })
        .collect();
    join_last_reply(&exchanges)
}

/// The agent's own prose in one exchange's output, and nothing else.
///
/// **Not `format_output_for_copy`, and the first version was.** Copy output
/// writes Warp's own notes (`WarpNote`, among them the ACP mode disclosure) and
/// tool rows into the text, which is right for a clipboard and wrong for a
/// voice: measured 2026-09-13 in a running Warp, the first reply of an ACP
/// session was read aloud as about forty seconds of Warp explaining the
/// agent's permission mode before the agent's own two sentences. So this keeps
/// `Text` messages only, and within them plain-text sections only. Code,
/// tables, images and diagrams are left out too, so a reader that does not
/// strip Markdown still does not recite a code block.
pub(crate) fn spoken_text(messages: &[AIAgentOutputMessage]) -> String {
    let mut parts = Vec::new();
    for message in messages {
        let AIAgentOutputMessageType::Text(text) = &message.message else {
            continue;
        };
        for section in &text.sections {
            if let AIAgentTextSection::PlainText { text } = section {
                parts.push(text.text().to_string());
            }
        }
    }
    parts.join("\n")
}

/// `exchanges` is `(has_user_query, output)` in conversation order.
pub(crate) fn join_last_reply(exchanges: &[(bool, String)]) -> Option<String> {
    let start = exchanges
        .iter()
        .rposition(|(has_query, _)| *has_query)
        .unwrap_or(0);
    let parts: Vec<&str> = exchanges[start..]
        .iter()
        .map(|(_, output)| output.trim())
        .filter(|output| !output.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// Starts reading `text` with `command`, stopping any reading in progress.
pub fn speak(command: &str, args: &str, text: &str) -> anyhow::Result<()> {
    if command.is_empty() {
        return Err(anyhow!(
            "No read-aloud command is configured. Set `agents.voice.read_aloud.command` in \
             settings.toml to a command that reads text on stdin, such as pocket-speak."
        ));
    }
    stop();

    let mut process = command::blocking::Command::new(command);
    // As in `local_transcriber`: clearing the flags drops the crate's default
    // CREATE_BREAKAWAY_FROM_JOB and keeps CREATE_NO_WINDOW. A reader should not
    // outlive Warp, and breakaway is refused inside a job that disallows it.
    #[cfg(windows)]
    process.creation_flags(0);
    let mut child = process
        .args(args.split_whitespace())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to start read-aloud command `{command}`"))?;

    let mut stdin = child
        .stdin
        .take()
        .context("read-aloud stdin was not piped")?;
    let text = text.to_owned();
    // Written off the calling thread: a reader that consumes stdin slowly
    // would otherwise block the UI for as long as the reply is long.
    std::thread::Builder::new()
        .name("read-aloud-stdin".to_owned())
        .spawn(move || {
            // A write error means the reader stopped early, which `stop` or the
            // command itself already accounts for.
            let _ = stdin.write_all(text.as_bytes());
        })
        .context("Failed to start the read-aloud writer thread")?;
    if let Some(stderr) = child.stderr.take() {
        let name = command.to_owned();
        std::thread::Builder::new()
            .name("read-aloud-stderr".to_owned())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    log::warn!("read-aloud `{name}`: {line}");
                }
            })
            .context("Failed to start the read-aloud stderr thread")?;
    }

    *READING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(child);
    Ok(())
}

/// Stops the reading in progress. Returns whether one was still running.
pub fn stop() -> bool {
    let Some(mut child) = READING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
    else {
        return false;
    };
    let running = matches!(child.try_wait(), Ok(None));
    if running {
        let _ = child.kill();
    }
    // Reaps the process either way, so a finished reader leaves no zombie.
    let _ = child.wait();
    running
}

#[cfg(test)]
#[path = "read_aloud_tests.rs"]
mod tests;
