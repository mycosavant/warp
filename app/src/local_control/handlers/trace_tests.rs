use super::*;

/// The phase 0 run: session directory `/home/effatha/git/warp`, file under
/// `/home/effatha/.claude/projects`. The cwd's own user leads, the listing
/// follows without repeating it, and `/root` is last.
#[test]
fn the_session_directorys_own_home_is_tried_first_and_root_last() {
    let homes = guest_homes(
        "/home/effatha/git/warp",
        &["alice".to_owned(), "effatha".to_owned()],
    );
    assert_eq!(homes, ["/home/effatha", "/home/alice", "/root"]);
}

/// A session directory outside `/home` says nothing about the user, so the
/// listing is the whole guess.
#[test]
fn a_cwd_outside_home_leads_with_nothing() {
    let homes = guest_homes("/tmp/scratch", &["effatha".to_owned()]);
    assert_eq!(homes, ["/home/effatha", "/root"]);
    assert_eq!(guest_homes("/home/", &[]), ["/root"]);
}

/// A tail cursor is a count of non-empty lines, on both files, so the tail
/// of a file is what the parser would have seen after that many rows.
#[test]
fn a_tail_skips_exactly_the_rows_already_sent() {
    let text = "{\"a\":1}\n\n{\"a\":2}\n{\"a\":3}\n";
    assert_eq!(trace::tail(text, 0), "{\"a\":1}\n{\"a\":2}\n{\"a\":3}\n");
    assert_eq!(trace::tail(text, 2), "{\"a\":3}\n");
    assert_eq!(trace::tail(text, 3), "");
    assert_eq!(trace::tail(text, 9), "");
}

/// The header of a tail reports totals over the file, not the slice, so the
/// pair is the next cursor.
#[test]
fn a_tails_header_counts_the_whole_file() {
    let (rows, facts) = trace::warp_rows("{\"event\":\"stop\",\"ts\":\"2026-09-05T00:00:00Z\"}\n");
    let built = trace::build(
        "c".to_owned(),
        PathBuf::from("events.jsonl"),
        rows,
        facts,
        None,
        None,
        None,
    )
    .after(4, 7);
    assert_eq!(built.header.warp_lines, 5);
    assert_eq!(built.header.harness_lines, 7);
    assert_eq!(built.header.warp_after, 4);
    assert_eq!(built.header.harness_after, 7);
    assert_eq!(built.rows.len(), 1);
}
