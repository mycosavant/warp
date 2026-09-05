use std::path::PathBuf;

use super::*;

/// The phase 0 run (`.fork/runs/viewer-phase0-2026-09-05/`): one panel turn,
/// two tool calls under `auto`, Claude Code 2.1.257. Real files, unedited.
const CLAUDE_CODE_2_1_257: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../.fork/viewer/fixtures/claude-code-2.1.257.jsonl"
));
const WARP_EVENTS_ACP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../.fork/viewer/fixtures/warp-events-acp-2026-09-05.jsonl"
));

const LINKED: &str = "dc585483-6b63-4ded-9b07-f58c85f28502";

fn kinds<'a>(rows: &'a [Row], kind: &str) -> Vec<&'a Row> {
    rows.iter().filter(|row| row.kind == kind).collect()
}

/// Measured 2026-09-05 with one `claude -p` turn from `slug probe/a_b.c`.
#[test]
fn the_slug_is_every_non_alphanumeric_as_a_dash() {
    assert_eq!(slug("/home/effatha/git/warp"), "-home-effatha-git-warp");
    assert_eq!(
        slug("/home/effatha/.claude/jobs"),
        "-home-effatha--claude-jobs"
    );
    assert_eq!(slug("/tmp/x/slug probe/a_b.c"), "-tmp-x-slug-probe-a-b-c");
    assert_eq!(
        slug("/home/effatha/git/GitSync"),
        "-home-effatha-git-GitSync"
    );
}

#[test]
fn warps_lines_name_the_join_key_and_the_cwd() {
    let (rows, facts) = warp_rows(WARP_EVENTS_ACP);

    assert_eq!(facts.linked_session_id.as_deref(), Some(LINKED));
    assert_eq!(facts.cwd.as_deref(), Some("/home/effatha/git/warp"));
    assert_eq!(rows.len(), 9);
    assert!(rows.iter().all(|row| row.from == Record::Warp));

    let starts = kinds(&rows, "tool_start");
    assert_eq!(starts.len(), 2);
    assert!(starts.iter().all(|row| row.who == Who::Agent));
    assert!(starts.iter().all(|row| {
        row.call_id
            .as_deref()
            .is_some_and(|id| id.starts_with("toolu_"))
    }));
    // The ACP path logs the kind and no input: the harness file is the record.
    assert_eq!(starts[0].text.as_deref(), Some("read"));

    let mode = &kinds(&rows, "session_mode")[0];
    assert_eq!(
        mode.who,
        Who::Warp,
        "Warp's disclosure is Warp's, not the agent's"
    );
    assert!(mode.text.as_deref().unwrap().contains("current `auto`"));

    assert_eq!(kinds(&rows, "session_start")[0].who, Who::Person);
    assert!(
        rows.iter().all(|row| row.at.is_some()),
        "every Warp line has a ts"
    );
}

#[test]
fn the_harness_file_yields_the_calls_with_their_full_input() {
    let (rows, facts) = harness_rows(CLAUDE_CODE_2_1_257);

    assert_eq!(facts.versions.iter().collect::<Vec<_>>(), vec!["2.1.257"]);
    assert_eq!(facts.lines, 24);
    // 1 prompt, 2 tool_use, 2 tool_result, 2 text: everything else is
    // bookkeeping (attachment, queue-operation, atis-latch, last-prompt).
    assert_eq!(facts.skipped, 17);

    let uses = kinds(&rows, "tool_use");
    assert_eq!(uses.len(), 2);
    assert!(uses.iter().all(|row| row.who == Who::Agent));
    let read = uses
        .iter()
        .find(|row| row.text.as_deref().unwrap().starts_with("Read "))
        .expect("the Read call");
    assert!(
        read.text
            .as_deref()
            .unwrap()
            .contains(r#""file_path":"/home/effatha/git/warp/CLAUDE.md""#),
        "the full input, which Warp's log does not have: {:?}",
        read.text
    );

    let results = kinds(&rows, "tool_result");
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|row| row.error.is_none()));
    assert!(results.iter().all(|row| row.call_id.is_some()));

    let prompts = kinds(&rows, "prompt");
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].who, Who::Person);

    let usage = rows.last().expect("rows");
    assert_eq!(usage.kind, "usage");
    assert_eq!(usage.who, Who::Harness);
    assert_eq!(usage.raw["assistant_messages"], 4);
    assert!(usage.raw["output_tokens"].as_u64().unwrap() > 0);
}

/// The measurement phase 0 made, as a test: every call Warp logged is in the
/// harness's file under the same id, and the merge is in time order.
#[test]
fn the_join_holds_on_the_phase_0_fixtures() {
    let (warp, facts) = warp_rows(WARP_EVENTS_ACP);
    let trace = build(
        "ebacd6e3".to_owned(),
        PathBuf::from("events.jsonl"),
        warp,
        facts,
        Some(PathBuf::from("harness.jsonl")),
        Some(CLAUDE_CODE_2_1_257),
        None,
    );

    assert_eq!(trace.header.linked_session_id.as_deref(), Some(LINKED));
    assert_eq!(trace.header.harness, Some("claude-code"));
    assert_eq!(trace.header.harness_versions, vec!["2.1.257"]);
    assert_eq!(trace.header.warp_lines, 9);

    let logged: Vec<&str> = kinds(&trace.rows, "tool_start")
        .iter()
        .filter_map(|row| row.call_id.as_deref())
        .collect();
    let recorded: Vec<&str> = kinds(&trace.rows, "tool_use")
        .iter()
        .filter_map(|row| row.call_id.as_deref())
        .collect();
    assert_eq!(logged.len(), 2);
    for id in &logged {
        assert!(
            recorded.contains(id),
            "{id} logged by Warp but not in the harness file"
        );
    }

    // The two files were stamped by two clocks, Windows and the distribution's,
    // and the harness's reads about five seconds ahead. Disclosed in the header
    // rather than corrected; the join is what orders a call, not the clocks.
    assert_eq!(trace.header.joined_calls, 2);
    let offset = trace.header.clock_offset_ms.expect("two joined calls");
    assert!(
        (5_000..6_000).contains(&offset),
        "harness minus Warp, median over the joined calls: {offset} ms"
    );

    let times: Vec<_> = trace.rows.iter().filter_map(|row| row.at).collect();
    assert_eq!(
        times.len(),
        trace.rows.len(),
        "every row on these fixtures has a time"
    );
    assert!(
        times.windows(2).all(|w| w[0] <= w[1]),
        "rows are in time order"
    );

    let whos: std::collections::BTreeSet<_> = trace
        .rows
        .iter()
        .map(|row| format!("{:?}", row.who))
        .collect();
    assert_eq!(
        whos.into_iter().collect::<Vec<_>>(),
        vec!["Agent", "Harness", "Person", "Warp"],
        "all four provenances appear on one ordinary turn"
    );
}

/// Shapes copied from a real session file (2026-08-21), summary cut short.
/// The summary line follows the boundary; the observability page said it
/// preceded it, and this fixture is the correction.
#[test]
fn a_compaction_is_a_boundary_row_then_the_summary_and_nothing_before_it_is_lost() {
    let text = concat!(
        r#"{"parentUuid":null,"isSidechain":false,"type":"user","message":{"role":"user","content":"the first ask"},"uuid":"u1","timestamp":"2026-08-21T01:50:00.000Z","version":"2.0.65","cwd":"/tmp"}"#,
        "\n",
        r#"{"parentUuid":null,"logicalParentUuid":"u1","isSidechain":false,"type":"system","subtype":"compact_boundary","content":"Conversation compacted","isMeta":false,"timestamp":"2026-08-21T01:55:49.279Z","uuid":"b1","level":"info","compactMetadata":{"trigger":"manual","preTokens":24133,"durationMs":28809}}"#,
        "\n",
        r#"{"parentUuid":"b1","isSidechain":false,"type":"user","message":{"role":"user","content":"This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion."},"isCompactSummary":true,"isVisibleInTranscriptOnly":true,"uuid":"s1","timestamp":"2026-08-21T01:55:49.300Z","version":"2.0.65","cwd":"/tmp"}"#,
        "\n",
    );
    let (rows, facts) = harness_rows(text);

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].kind, "prompt");
    assert_eq!(rows[0].text.as_deref(), Some("the first ask"));
    assert_eq!(rows[1].kind, "system/compact_boundary");
    assert_eq!(rows[1].who, Who::Harness);
    assert_eq!(rows[1].raw["compactMetadata"]["preTokens"], 24133);
    assert_eq!(rows[2].kind, "compact_summary");
    assert_eq!(
        rows[2].who,
        Who::Harness,
        "the summary is the harness's words, not the person's"
    );
    assert!(
        rows[2]
            .text
            .as_deref()
            .unwrap()
            .starts_with("This session is being continued")
    );
    assert_eq!(facts.versions.len(), 1);
}

#[test]
fn an_unparseable_line_is_a_row_and_not_a_failure() {
    let (rows, facts) =
        harness_rows("{\"type\":\"user\",\"message\":{\"content\":\"hi\"}}\nnot json at all\n");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].kind, "unparsed");
    assert_eq!(rows[1].who, Who::Harness);
    assert_eq!(rows[1].text.as_deref(), Some("not json at all"));
    assert_eq!(facts.lines, 2);

    let (rows, _) = warp_rows("garbage\n");
    assert_eq!(rows[0].kind, "unparsed");
    assert_eq!(rows[0].who, Who::Warp);
}

/// Two real lines from `.fork/runs/run-2026-09-01/events/`, trimmed.
#[test]
fn warps_permission_lines_are_warps_words_on_the_agents_call() {
    let text = concat!(
        r#"{"ts":"2026-09-01T02:31:26.946Z","seq":10,"agent":"claude-agent-acp","event":"permission_request","source":"acp_agent","session_id":"2d326d26","linked_session_id":"372a5ed6","call_id":"toolu_01XDMsxcSQJCLqnv8QYQYhU2","cwd":"/home/effatha/git/warp","project":"warp","tool_name":"edit","tool_input_preview":"{\"file_path\":\"/home/effatha/git/warp/CLAUDE.md\"}","can_approve":true,"applied":true}"#,
        "\n",
        r#"{"ts":"2026-09-01T02:31:51.772Z","seq":11,"agent":"claude-agent-acp","event":"permission_replied","source":"acp_agent","session_id":"2d326d26","linked_session_id":"372a5ed6","call_id":"toolu_01XDMsxcSQJCLqnv8QYQYhU2","cwd":"/home/effatha/git/warp","project":"warp","summary":"approval 1343b951:0","decision":"denied","answered_by":"control_plane","applied":true}"#,
        "\n",
        r#"{"ts":"2026-09-01T02:31:52.000Z","seq":12,"agent":"claude-agent-acp","event":"tool_complete","source":"acp_agent","session_id":"2d326d26","linked_session_id":"372a5ed6","call_id":"toolu_01XDMsxcSQJCLqnv8QYQYhU2","cwd":"/home/effatha/git/warp","error_type":"error","applied":true}"#,
        "\n",
    );
    let (rows, facts) = warp_rows(text);

    assert_eq!(facts.linked_session_id.as_deref(), Some("372a5ed6"));
    assert_eq!(rows[0].who, Who::Warp);
    assert_eq!(rows[0].kind, "permission_request");
    assert_eq!(
        rows[0].call_id.as_deref(),
        Some("toolu_01XDMsxcSQJCLqnv8QYQYhU2")
    );
    assert_eq!(rows[1].who, Who::Warp);
    assert_eq!(rows[1].text.as_deref(), Some("denied by control_plane"));
    assert_eq!(rows[1].error, None, "a denial is a decision, not an error");
    assert_eq!(rows[2].kind, "tool_complete");
    assert_eq!(rows[2].error, Some(true));
}

#[test]
fn without_a_linked_session_the_trace_is_warps_half_and_says_so() {
    let text = r#"{"ts":"2026-09-05T22:42:51.050Z","seq":0,"agent":"warp","event":"session_start","source":"in_process","session_id":"c1","cwd":"/home/effatha/git/warp","summary":"hello","applied":true}"#;
    let (warp, facts) = warp_rows(text);
    assert_eq!(facts.linked_session_id, None);
    let trace = build(
        "c1".to_owned(),
        PathBuf::from("events.jsonl"),
        warp,
        facts,
        None,
        None,
        Some("no linked session".to_owned()),
    );

    assert_eq!(trace.header.harness, None);
    assert_eq!(trace.header.harness_file, None);
    assert_eq!(trace.header.harness_lines, 0);
    assert_eq!(trace.header.note.as_deref(), Some("no linked session"));
    assert_eq!(trace.header.joined_calls, 0);
    assert_eq!(trace.header.clock_offset_ms, None);
    assert_eq!(trace.rows.len(), 1);
    assert_eq!(trace.rows[0].who, Who::Person);
}

#[test]
fn thinking_and_a_failed_result_are_rows_with_their_own_marks() {
    let text = concat!(
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"let me look"},{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"false"}}],"usage":{"input_tokens":10,"output_tokens":5}},"timestamp":"2026-09-05T00:00:00.000Z","version":"2.1.257"}"#,
        "\n",
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","is_error":true,"content":"exit 1"}]},"timestamp":"2026-09-05T00:00:01.000Z","version":"2.1.257"}"#,
        "\n",
        r#"{"type":"permission-mode","permissionMode":"auto","sessionId":"x"}"#,
        "\n",
    );
    let (rows, _) = harness_rows(text);

    assert_eq!(rows[0].kind, "thinking");
    assert_eq!(rows[0].who, Who::Agent);
    assert_eq!(rows[1].kind, "tool_use");
    assert_eq!(rows[1].text.as_deref(), Some(r#"Bash {"command":"false"}"#));
    assert_eq!(rows[2].kind, "tool_result");
    assert_eq!(rows[2].error, Some(true));
    assert_eq!(rows[2].text.as_deref(), Some("exit 1"));
    assert_eq!(rows[3].kind, "permission_mode");
    assert_eq!(rows[3].text.as_deref(), Some("auto"));
    assert_eq!(rows[3].ts, None, "a bookkeeping line has no timestamp");
    assert_eq!(rows[4].kind, "usage");
}

/// Rows with no time sort after every row with one, in file order.
#[test]
fn merge_is_stable_and_keeps_untimed_rows_last() {
    let (warp, _) = warp_rows(WARP_EVENTS_ACP);
    let (harness, _) = harness_rows(concat!(
        r#"{"type":"permission-mode","permissionMode":"auto","sessionId":"x"}"#,
        "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"same instant"}]},"timestamp":"2026-09-05T22:42:55.955Z","version":"2.1.257"}"#,
        "\n",
    ));
    let rows = merge(warp, harness);

    let last = rows.last().unwrap();
    assert_eq!(last.kind, "permission_mode");
    let same_instant: Vec<_> = rows
        .iter()
        .filter(|row| row.ts.as_deref() == Some("2026-09-05T22:42:55.955Z"))
        .collect();
    assert_eq!(same_instant.len(), 2);
    assert_eq!(
        same_instant[0].from,
        Record::Warp,
        "Warp's row first on a tie"
    );
    assert_eq!(same_instant[1].kind, "text");
}
