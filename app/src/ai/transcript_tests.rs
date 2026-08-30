use super::*;

fn exchange(input: &str, output: &str) -> Exchange {
    Exchange {
        input: input.to_owned(),
        output: output.to_owned(),
    }
}

#[test]
fn the_file_is_named_for_warps_conversation_not_the_agents_session() {
    // T14.15's defect was one turn's evidence split across two files named by
    // different ids. This keys on the same id the event log does, so a person
    // holding a conversation id finds both.
    let path = path_for(Path::new("/tmp/t"), "5e47e8c1-f77d-4187-836a-e05729bf7132");
    assert_eq!(
        path,
        Path::new("/tmp/t/5e47e8c1-f77d-4187-836a-e05729bf7132.md")
    );
}

#[test]
fn the_pointer_says_grep_rather_than_read() {
    // The whole feature turns on this word. An agent told to *read* a 160,000
    // character transcript spends the context the compaction just freed.
    let line = pointer(Path::new("/home/x/.local/state/warp/transcripts/abc.md"));
    assert!(line.contains("grep"), "pointer must say grep: {line}");
    assert!(
        !line.contains("read it") || line.contains("rather than reading it whole"),
        "pointer must not invite a whole read: {line}"
    );
    assert!(line.contains("/home/x/.local/state/warp/transcripts/abc.md"));
}

#[test]
fn the_pointer_asserts_nothing_about_whether_the_agent_forgot() {
    // Warp cannot see the agent's context and must not claim to. The file is
    // offered; the agent decides whether it needs it.
    let line = pointer(Path::new("/t/a.md"));
    for invented in ["compact", "forgot", "lost", "summar", "truncat"] {
        assert!(
            !line.to_lowercase().contains(invented),
            "pointer claims something Warp cannot know ({invented}): {line}"
        );
    }
}

#[test]
fn exchanges_are_numbered_from_one_and_oldest_first() {
    let text = render(
        "conv",
        &[
            exchange("first question", "first answer"),
            exchange("second question", "second answer"),
        ],
    );
    let first = text.find("## Exchange 1").expect("exchange 1");
    let second = text.find("## Exchange 2").expect("exchange 2");
    assert!(first < second, "oldest must come first");
    assert!(text.find("first answer").unwrap() < text.find("second question").unwrap());
}

#[test]
fn an_exchange_with_no_output_says_so_rather_than_leaving_a_blank() {
    // A turn still running, or one that errored before speaking, has no output.
    // An empty section reads like the agent chose to say nothing.
    let text = render("conv", &[exchange("asked", "   ")]);
    assert!(text.contains("(no output recorded)"), "{text}");
}

#[test]
fn the_header_tells_a_reader_to_search_it() {
    // The pointer can be lost to the very compaction this exists for, so the
    // instruction is repeated inside the file where a grep will land on it.
    let text = render("conv", &[exchange("q", "a")]);
    assert!(text.contains("Search this file"), "{text}");
}

#[test]
fn writing_replaces_rather_than_appends() {
    // An exchange can be edited or retried after first appearing. Appending
    // would accumulate drafts and stop agreeing with what the panel shows.
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "conv", &[exchange("one", "1")]).expect("first write");
    let path = write(
        dir.path(),
        "conv",
        &[exchange("one", "1"), exchange("two", "2")],
    )
    .expect("second write");
    let text = std::fs::read_to_string(&path).expect("read back");
    assert_eq!(text.matches("## Exchange 1").count(), 1, "{text}");
    assert!(text.contains("## Exchange 2"), "{text}");
}

#[test]
fn no_partial_file_is_left_behind() {
    // The agent greps this at times Warp does not control, so the write is
    // atomic and the temporary must not survive to be found by a glob.
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "conv", &[exchange("q", "a")]).expect("write");
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("partial"))
        .collect();
    assert!(leftovers.is_empty(), "left {leftovers:?}");
}

#[test]
fn the_panel_is_told_once_and_not_every_turn() {
    // The pointer rides every prompt because a compaction would eat one sent
    // only once. Saying so in the panel every turn would be noise about a fact
    // that has not changed.
    forget("conv-once");
    assert!(needs_announcing("conv-once"), "first turn must announce");
    assert!(!needs_announcing("conv-once"), "second turn must not");
    assert!(!needs_announcing("conv-once"), "nor any later one");
    forget("conv-once");
}

#[test]
fn two_conversations_are_announced_independently() {
    forget("conv-a");
    forget("conv-b");
    assert!(needs_announcing("conv-a"));
    assert!(
        needs_announcing("conv-b"),
        "one conversation's announcement must not silence another's"
    );
    forget("conv-a");
    forget("conv-b");
}

#[test]
fn the_announcement_names_the_path_and_how_to_stop() {
    // Warp is adding words to someone else's conversation. The person is
    // entitled to know where the file is and how to decline.
    let text = announcement(Path::new("/t/conv.md"));
    assert!(text.contains("/t/conv.md"), "{text}");
    assert!(text.contains("WARP_FORK_TRANSCRIPT"), "{text}");
}
