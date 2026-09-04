//! The rules that do not need a view context. The control itself is verified by
//! running it — the fork's standard for anything with a surface.

use super::*;

fn view() -> AcpApprovalView {
    AcpApprovalView::new("conv-1".to_owned())
}

/// A fresh control is not armed. The first tap on *Yes* can never answer.
#[test]
fn nothing_is_armed_to_begin_with() {
    assert!(!view().armed_for("turn-1:0"));
}

/// Arming is per request, and this is the point of it.
///
/// Three surfaces can answer one question — the panel, `warpctrl`, and the
/// console — and a turn can end under any of them. So between a person reading
/// this request and tapping twice, the request can be gone and a different one
/// can be parked in its place. A boolean `armed` would send the second tap to
/// the newcomer carrying a decision made about its predecessor, which is the
/// stale-answer hazard the control plane's digest exists to prevent.
#[test]
fn arming_one_request_does_not_arm_the_next_one() {
    let mut view = view();
    view.armed = Some("turn-1:0".to_owned());

    assert!(view.armed_for("turn-1:0"), "the armed request is armed");
    assert!(
        !view.armed_for("turn-1:1"),
        "a request that arrived after the first tap is not armed by it"
    );
}

/// A conversation with nothing parked has no question, so the control renders
/// nothing rather than an empty frame. `waiting_for` is process-global and this
/// id belongs to no test that parks anything.
#[test]
fn a_conversation_with_nothing_parked_has_no_question() {
    assert!(
        AcpApprovalView::new("conv-t1416-empty".to_owned())
            .current()
            .is_none()
    );
}

// ── What the card says a call is (COMPOSER) ──────────────────────────────

/// **The measured shape, from a real 44-ask session.** The agent writes a
/// sentence for a person — *"Compare local HEAD to Windows checkout HEAD"* — and
/// the card used to render it inside one escaped JSON blob beside a multi-line
/// command, which made the one human-readable field the hardest thing to find.
#[test]
fn a_shell_call_leads_with_what_the_agent_says_it_is_doing() {
    let input = serde_json::json!({
        "command": "git log --oneline -1\necho \"---\"\ngit -C /mnt/c/dev/warp log --oneline -1",
        "description": "Compare local HEAD to Windows checkout HEAD",
    })
    .to_string();

    assert_eq!(
        super::describe_tool_input(&input),
        vec![
            (
                "it says",
                "Compare local HEAD to Windows checkout HEAD".to_owned()
            ),
            (
                "the call",
                "git log --oneline -1\necho \"---\"\ngit -C /mnt/c/dev/warp log --oneline -1"
                    .to_owned()
            ),
        ],
    );
}

/// **Nothing is dropped, and this is the assertion that keeps it true.** The
/// payload is where a call's specifics live; a card that quietly discarded a key
/// it did not recognise would understate the call's reach, which is the exact
/// failure `acts_on` was built for. `content` is the case that matters — the
/// bytes a write would put on disk are part of what is being agreed to.
#[test]
fn a_key_the_card_does_not_know_about_is_still_shown() {
    let input = serde_json::json!({
        "file_path": "/home/effatha/git/warp/notes.md",
        "content": "hello\n",
        "some_future_field": 7,
    })
    .to_string();

    let rendered = super::describe_tool_input(&input)
        .iter()
        .map(|(label, value)| format!("{label} {value}"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("/home/effatha/git/warp/notes.md"));
    assert!(
        rendered.contains("hello"),
        "content must survive: {rendered}"
    );
    assert!(
        rendered.contains("some_future_field"),
        "an unknown key must survive under its own name: {rendered}"
    );
}

/// An agent that sends something other than an object is describing its call the
/// only way it knows, and inventing structure for it would be Warp making a
/// claim the agent did not.
#[test]
fn a_payload_that_is_not_an_object_is_passed_through_unchanged() {
    assert_eq!(
        super::describe_tool_input("just a string"),
        vec![("the call", "just a string".to_owned())],
    );
}

// ── Layered (COMPOSER item 6) ────────────────────────────────────────────

fn parked(input: Option<&str>, approve_selects: Option<&str>) -> ParkedRequest {
    ParkedRequest {
        approval_id: "req-1:7".to_owned(),
        agent: "test-agent".to_owned(),
        title: Some("Write out.txt".to_owned()),
        tool_name: Some("edit".to_owned()),
        tool_input: input.map(str::to_owned),
        session_directory: Some("/tmp/project".to_owned()),
        session_id: Some("ses_abc".to_owned()),
        conversation_id: "conv-1".to_owned(),
        acts_on: vec!["/tmp/project/out.txt".to_owned()],
        options_offered: vec![::local_control::protocol::OfferedOption {
            name: "Allow once".to_owned(),
            warp_can_select: true,
        }],
        approve_selects: approve_selects.map(str::to_owned),
        approve_refused_because: None,
    }
}

/// **Decision first, disclosure one interaction away, nothing dropped.** The
/// agent's title is the headline, its own sentence stays beside it, and every
/// other line the card used to draw is behind the toggle in the order it was.
#[test]
fn the_description_stays_on_top_and_the_call_goes_behind_the_toggle() {
    let input = serde_json::json!({
        "command": "cargo test -p warp",
        "description": "Run the app crate's tests",
    })
    .to_string();
    let layers = super::layered(&parked(Some(&input), Some("allow_once")));

    assert_eq!(layers.headline, "Write out.txt");
    assert_eq!(
        layers.always,
        vec![("it says", "Run the app crate's tests".to_owned())]
    );
    assert_eq!(
        layers.details,
        vec![
            ("the call", "cargo test -p warp".to_owned()),
            ("acts on", "/tmp/project/out.txt".to_owned()),
            (
                "offered",
                ::local_control::protocol::OfferedOption::render_list(
                    &parked(None, None).options_offered
                )
            ),
        ]
    );
}

/// The one line that must never be behind a toggle is the reason there is no
/// yes: a person deciding needs to know they are looking at a setting, not a
/// fault, before they look for the button that is not there.
#[test]
fn the_reason_there_is_no_yes_is_always_visible() {
    let mut request = parked(None, None);
    request.approve_refused_because = Some("the agent offered no single-shot yes".to_owned());
    let layers = super::layered(&request);

    assert!(
        layers
            .always
            .contains(&("no yes", "the agent offered no single-shot yes".to_owned())),
        "{layers:?}"
    );
    assert!(!layers.details.iter().any(|(label, _)| *label == "no yes"));
}

/// Without a title the headline is the call itself, never a blank.
#[test]
fn a_request_with_no_title_is_headlined_by_its_call() {
    let mut request = parked(Some(r#"{"command": "ls -la"}"#), Some("allow_once"));
    request.title = None;

    assert_eq!(super::layered(&request).headline, "ls -la");
}

/// Opening the details is per request, for the reason arming is: the
/// question a person opened can be gone and another parked in its place before
/// they read it, and the newcomer arrives closed.
#[test]
fn details_opened_on_one_request_are_not_open_on_the_next() {
    let mut view = view();
    view.details_open_for = Some("turn-1:0".to_owned());

    assert!(view.details_open_for("turn-1:0"));
    assert!(!view.details_open_for("turn-1:1"));
}
