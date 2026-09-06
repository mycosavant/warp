use std::path::PathBuf;

use super::*;
use crate::local_control::trace::{build, warp_rows};

const CLAUDE_CODE_2_1_257: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../.fork/viewer/fixtures/claude-code-2.1.257.jsonl"
));
const WARP_EVENTS_ACP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../.fork/viewer/fixtures/warp-events-acp-2026-09-05.jsonl"
));

fn phase0() -> Trace {
    let (warp, facts) = warp_rows(WARP_EVENTS_ACP);
    build(
        "ebacd6e3".to_owned(),
        PathBuf::from("events.jsonl"),
        warp,
        facts,
        Some(PathBuf::from("harness.jsonl")),
        Some(CLAUDE_CODE_2_1_257),
        None,
    )
}

fn from_lines(warp: &str, harness: &str) -> Trace {
    let (rows, facts) = warp_rows(warp);
    build(
        "c1".to_owned(),
        PathBuf::from("events.jsonl"),
        rows,
        facts,
        Some(PathBuf::from("harness.jsonl")),
        Some(harness),
        None,
    )
}

#[test]
fn text_draws_each_call_once_with_both_clocks_beside_it() {
    let text = render_text(&phase0());

    assert_eq!(text.matches("▸ ").count(), 2, "one item per call:\n{text}");
    assert!(
        text.contains("▸ Read {\"file_path\":\"/home/effatha/git/warp/CLAUDE.md\",\"limit\":1}")
    );
    assert!(
        text.contains("▸ Bash git log --oneline -1"),
        "a shell command is shown as itself"
    );
    assert!(text.contains("✓ done"));
    assert!(
        text.contains("warp 22:42:55.955 → 22:42:56.754 · harness 22:43:01.279 → 22:43:01.380"),
        "both files' stamps on the call, disagreeing:\n{text}"
    );
    assert!(text.contains("harness clock +5482 ms"));
    assert!(text.contains("H  usage: 4 assistant messages"));

    let prompt = "Use your Read tool to read the first line of CLAUDE.md";
    assert_eq!(
        text.matches(prompt).count(),
        1,
        "the harness's copy of the prompt is dropped when Warp's has the same text:\n{text}"
    );
    let prompt_at = text.find(prompt).unwrap();
    let first_call = text.find("▸ ").unwrap();
    assert!(
        prompt_at < first_call,
        "the prompt is drawn where Warp stamped it, before the calls, not where the \
         harness's skewed clock put it:\n{text}"
    );
    for gutter in [" P  ", " A  ", " W  ", " H  "] {
        assert!(text.contains(gutter), "gutter {gutter:?} missing:\n{text}");
    }
    assert!(
        text.contains("W  mode: current `auto`"),
        "Warp's disclosure in Warp's column"
    );
    assert!(
        text.contains("W  stop\n"),
        "stop is drawn as a state, not as the prompt again"
    );
}

#[test]
fn text_marks_a_denial_and_a_failure_differently() {
    let warp = concat!(
        r#"{"ts":"2026-09-01T02:31:26.946Z","event":"permission_request","source":"acp_agent","session_id":"c1","linked_session_id":"s1","call_id":"toolu_deny","cwd":"/tmp","tool_name":"edit","tool_input_preview":"{\"file_path\":\"CLAUDE.md\"}","can_approve":true}"#,
        "\n",
        r#"{"ts":"2026-09-01T02:31:51.772Z","event":"permission_replied","source":"acp_agent","session_id":"c1","linked_session_id":"s1","call_id":"toolu_deny","cwd":"/tmp","decision":"denied","answered_by":"control_plane"}"#,
        "\n",
        r#"{"ts":"2026-09-01T02:32:00.000Z","event":"permission_request","source":"acp_agent","session_id":"c1","linked_session_id":"s1","call_id":"toolu_hang","cwd":"/tmp","tool_name":"execute","tool_input_preview":"sleep 9"}"#,
        "\n",
    );
    let harness = concat!(
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_fail","name":"Bash","input":{"command":"false"}}],"usage":{"input_tokens":1,"output_tokens":1}},"timestamp":"2026-09-01T02:33:00.000Z","version":"2.1.257"}"#,
        "\n",
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_fail","is_error":true,"content":"exit 1"}]},"timestamp":"2026-09-01T02:33:01.000Z","version":"2.1.257"}"#,
        "\n",
    );
    let text = render_text(&from_lines(warp, harness));

    assert!(
        text.contains("▸ edit {\"file_path\":\"CLAUDE.md\"}\n"),
        "Warp's preview when the harness has no input:\n{text}"
    );
    assert!(
        text.contains("⊘ denied · Warp: denied by control_plane"),
        "{text}"
    );
    assert!(text.contains("? asked, never answered"), "{text}");
    assert!(text.contains("✗ failed"), "{text}");
    assert!(
        text.contains("\n                exit 1\n"),
        "the result under the call:\n{text}"
    );
    assert_eq!(text.matches("▸ ").count(), 3);
}

/// An edit's input is a diff, in both forms (2026-09-06): the file on the
/// input line, `-`/`+` lines under it, and in the page every line escaped.
#[test]
fn an_edit_is_drawn_as_a_diff_not_as_two_strings() {
    let harness = concat!(
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_edit","name":"Edit","input":{"file_path":"/tmp/a.rs","old_string":"fn a() {}\nfn b() {}\n","new_string":"fn a() {}\nfn c() -> u8 { 1 }\n"}}],"usage":{"input_tokens":1,"output_tokens":1}},"timestamp":"2026-09-06T02:33:00.000Z","version":"2.1.257"}"#,
        "\n",
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_edit","content":"ok"}]},"timestamp":"2026-09-06T02:33:01.000Z","version":"2.1.257"}"#,
        "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_write","name":"Write","input":{"file_path":"/tmp/b.html","content":"<b>x</b>\n"}}],"usage":{"input_tokens":1,"output_tokens":1}},"timestamp":"2026-09-06T02:34:00.000Z","version":"2.1.257"}"#,
        "\n",
    );
    let trace = from_lines("", harness);
    let text = render_text(&trace);
    assert!(
        text.contains("▸ Edit /tmp/a.rs\n"),
        "the input line is the file:\n{text}"
    );
    for line in ["  fn a() {}\n", "- fn b() {}\n", "+ fn c() -> u8 { 1 }\n"] {
        assert!(text.contains(line), "{line:?} in:\n{text}");
    }
    assert!(
        !text.contains("old_string"),
        "the JSON is not dumped beside the diff:\n{text}"
    );
    assert!(text.contains("▸ Write /tmp/b.html\n"), "{text}");
    assert!(
        text.contains("+ <b>x</b>\n"),
        "a Write is all additions:\n{text}"
    );

    let html = render_html(&trace);
    assert!(html.contains("<pre class=\"diff\">"));
    assert!(
        html.contains("<span class=\"del\">- fn b() {}</span>"),
        "{html}"
    );
    assert!(
        html.contains("<span class=\"add\">+ &lt;b&gt;x&lt;/b&gt;</span>"),
        "escaped:\n{html}"
    );
    assert!(
        !html.contains("<b>x</b>"),
        "the agent's markup never reaches the page as markup"
    );
}

#[test]
fn text_draws_a_compaction_as_a_rule_with_the_summary_folded_under_it() {
    let harness = concat!(
        r#"{"type":"user","message":{"role":"user","content":"before"},"uuid":"u1","timestamp":"2026-08-21T01:50:00.000Z","version":"2.0.65"}"#,
        "\n",
        r#"{"type":"system","subtype":"compact_boundary","content":"Conversation compacted","timestamp":"2026-08-21T01:55:49.279Z","uuid":"b1","compactMetadata":{"trigger":"manual","preTokens":24133,"durationMs":28809}}"#,
        "\n",
        r#"{"type":"user","message":{"role":"user","content":"This session is being continued.\nline two\nline three"},"isCompactSummary":true,"uuid":"s1","timestamp":"2026-08-21T01:55:49.300Z","version":"2.0.65"}"#,
        "\n",
        r#"{"type":"user","message":{"role":"user","content":"after"},"uuid":"u2","timestamp":"2026-08-21T01:56:00.000Z","version":"2.0.65"}"#,
        "\n",
    );
    let text = render_text(&from_lines("", harness));

    let before = text.find("before").unwrap();
    let rule = text
        .find("──── compaction, manual, 24133 tokens before, 28.8 s ────")
        .expect("the rule");
    let summary = text
        .find("summary the agent continued from:\n                This session is being continued.\n                (+2 lines)")
        .expect("folded, indented under the gutter");
    let after = text.find("after").unwrap();
    assert!(before < rule && rule < summary && summary < after, "{text}");
}

#[test]
fn warps_copy_of_the_prompt_stays_when_it_is_an_excerpt() {
    let warp = r#"{"ts":"2026-09-05T22:42:51.050Z","event":"session_start","source":"in_process","session_id":"c1","cwd":"/tmp","summary":"a long prompt that Warp cut sho…"}"#;
    let harness = r#"{"type":"user","message":{"role":"user","content":"a long prompt that Warp cut short and the harness kept whole"},"timestamp":"2026-09-05T22:42:58.000Z","version":"2.1.257"}"#;
    let text = render_text(&from_lines(warp, harness));

    assert_eq!(text.matches(" P  ").count(), 2, "{text}");
}

#[test]
fn html_loads_nothing_runs_nothing_and_escapes_everything() {
    let page = render_html(&phase0());

    assert!(page.starts_with("<!doctype html>"));
    assert!(!page.contains("<script"), "no script at all");
    assert!(
        !page.contains("<link"),
        "no stylesheet or font from anywhere"
    );
    assert!(!page.contains(" src="), "nothing fetched");
    assert!(
        !page.contains("http://") && !page.contains("https://"),
        "no URL to anywhere"
    );
    assert_eq!(
        page.matches("class=\"row who-agent call state-done\"")
            .count(),
        2
    );
    assert!(
        page.contains("warp 22:42:55.955 → 22:42:56.754 · harness 22:43:01.279 → 22:43:01.380")
    );
    assert!(page.contains("<footer>usage: 4 assistant messages"));
    assert!(page.contains("reads +5482 ms against Warp&#39;s, disclosed and not applied"));

    let hostile = r#"{"type":"user","message":{"role":"user","content":"<script>alert(1)</script> & \"quotes\""},"timestamp":"2026-09-05T22:42:58.000Z","version":"2.1.257"}"#;
    let page = render_html(&from_lines("", hostile));
    assert!(!page.contains("<script>alert"), "{page}");
    assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;quotes&quot;"));
}

#[test]
fn html_folds_a_compaction_summary_and_a_long_result() {
    let harness = concat!(
        r#"{"type":"system","subtype":"compact_boundary","content":"Conversation compacted","timestamp":"2026-08-21T01:55:49.279Z","compactMetadata":{"trigger":"auto","preTokens":100}}"#,
        "\n",
        r#"{"type":"user","message":{"role":"user","content":"the summary"},"isCompactSummary":true,"timestamp":"2026-08-21T01:55:49.300Z","version":"2.0.65"}"#,
        "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"seq 9"}}],"usage":{}},"timestamp":"2026-08-21T01:56:00.000Z","version":"2.0.65"}"#,
        "\n",
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"1\n2\n3\n4\n5\n6\n7\n8\n9"}]},"timestamp":"2026-08-21T01:56:01.000Z","version":"2.0.65"}"#,
        "\n",
    );
    let page = render_html(&from_lines("", harness));

    assert!(
        page.contains(
            "<div class=\"boundary\">compaction, auto, 100 tokens before · 01:55:49.279</div>"
        ),
        "{page}"
    );
    assert!(page.contains("<details><summary>summary the agent continued from</summary><pre>the summary</pre></details>"));
    assert!(
        page.contains("<details><summary>9 lines</summary><pre>1\n2"),
        "a long result folds:\n{page}"
    );
}
