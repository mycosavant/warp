//! The rules in [`super`], which are all about what Warp is willing to say.
//!
//! Wording is asserted on here more than is usual in this repo, and
//! deliberately: this module's entire product is a sentence a person reads
//! about a policy Warp cannot see. A test that only checked which variant came
//! back would pass while the note claimed something false.

use super::*;

fn mode(id: &str, description: Option<&str>) -> SessionMode {
    let mut mode = SessionMode::new(SessionModeId::from(id.to_owned()), id.to_owned());
    mode.description = description.map(str::to_owned);
    mode
}

/// `claude-agent-acp` 0.70.0, measured 2026-08-30 — the case this module exists
/// for, kept verbatim rather than simplified.
fn claude_modes() -> SessionModeState {
    SessionModeState::new(
        SessionModeId::from("auto".to_owned()),
        vec![
            mode(
                "auto",
                Some("Use a model classifier to approve/deny permission prompts"),
            ),
            mode(
                "default",
                Some("Standard behavior, prompts for dangerous operations"),
            ),
            mode("acceptEdits", Some("Auto-accept file edit operations")),
            mode("plan", Some("Planning mode, no actual tool execution")),
            mode(
                "dontAsk",
                Some("Don't prompt for permissions, deny if not pre-approved"),
            ),
            mode("bypassPermissions", Some("Bypass all permission checks")),
        ],
    )
}

/// **The finding, as a test.** An agent that decides for itself says so, in its
/// own words, and Warp claims nothing about what the mode means.
#[test]
fn a_session_that_started_in_the_agents_own_mode_discloses_it() {
    let decision = Decision::of("conv-disclose", Some(&claude_modes()), None);

    let note = decision.note().expect("modes exist, so there is a note");
    assert!(
        note.contains("`auto`"),
        "the id is what a person sets to change it: {note}"
    );
    assert!(
        note.contains("Use a model classifier to approve/deny permission prompts"),
        "the agent's own description, verbatim: {note}"
    );
    assert!(
        note.contains("cannot tell what it permits"),
        "Warp says what it does not know, rather than implying it checked: {note}"
    );
    assert_eq!(
        decision.mode(),
        None,
        "nothing was asked for, so nothing is sent"
    );
}

/// **`opencode` is silent, and that is the design.** An agent with no modes
/// gets no note; a note per session saying so would train a person to skip the
/// notes that matter.
#[test]
fn an_agent_with_no_modes_produces_no_note() {
    assert_eq!(
        Decision::of("conv-none", None, None),
        Decision::NothingToSay
    );
    assert_eq!(
        Decision::of(
            "conv-empty",
            Some(&SessionModeState::new(
                SessionModeId::from("solo".to_owned()),
                vec![]
            )),
            None
        ),
        Decision::NothingToSay,
        "an empty list is the same case as no list"
    );
}

/// A person naming an advertised id gets it sent, and gets told what they asked
/// for in the agent's words rather than their own.
#[test]
fn a_requested_mode_the_agent_offers_is_sent() {
    let decision = Decision::of("conv-request", Some(&claude_modes()), Some("default"));

    assert_eq!(
        decision.mode().map(|id| id.0.to_string()),
        Some("default".to_owned())
    );
    let note = decision.note().expect("a request is worth saying");
    assert!(
        note.contains("Standard behavior, prompts for dangerous operations"),
        "the description of what was asked for, not just its id: {note}"
    );
    assert!(
        note.contains("Whether the agent honours the request"),
        "requesting is not receiving, and the note must not blur that: {note}"
    );
}

/// **Reported, never sent.** The spec requires the id be one the agent
/// advertised, so an unknown one buys a protocol error instead of a sentence —
/// and a *silent* failure would leave a person believing a mode was requested
/// when it was not, which is strictly worse than the error.
#[test]
fn a_requested_mode_the_agent_does_not_offer_is_reported_and_not_sent() {
    let decision = Decision::of("conv-unknown", Some(&claude_modes()), Some("architect"));

    assert_eq!(decision.mode(), None, "nothing is sent");
    let note = decision.note().expect("this is exactly when to speak up");
    assert!(
        note.contains("`architect`") && note.contains("does not offer"),
        "names what was asked for and why it did not happen: {note}"
    );
    assert!(
        note.contains("`auto`"),
        "and says which mode is actually in force: {note}"
    );
    assert!(
        note.contains("`bypassPermissions`"),
        "the offered list is the evidence for the claim about what is available: {note}"
    );
}

/// A mode with no description is reported by id alone.
///
/// The alternative — Warp writing a description from the id — is Warp
/// explaining a policy it cannot see, which is the one thing this module exists
/// not to do. Empty quotes would be worse still: they read as the agent having
/// said something blank.
#[test]
fn a_mode_the_agent_did_not_describe_is_not_described_by_warp() {
    let state = SessionModeState::new(
        SessionModeId::from("quiet".to_owned()),
        vec![mode("quiet", None)],
    );

    let decision = Decision::of("conv-undescribed", Some(&state), None);
    let note = decision.note().expect("modes exist");
    assert!(
        note.contains("gave no description"),
        "says the description is missing: {note}"
    );
    assert!(
        !note.contains("\"\""),
        "and does not render an empty pair of quotes as though it were one: {note}"
    );
}

/// A blank description is the same case as an absent one.
///
/// Split from the test above because the two arrive by different routes — a
/// missing JSON field and a field containing whitespace — and an agent sending
/// `""` is likelier than one sending no field at all.
#[test]
fn a_blank_description_is_treated_as_none() {
    let state = SessionModeState::new(
        SessionModeId::from("quiet".to_owned()),
        vec![mode("quiet", Some("   "))],
    );

    let decision = Decision::of("conv-blank", Some(&state), None);
    let note = decision.note().expect("modes exist");
    assert!(note.contains("gave no description"), "{note}");
}

/// The advertised list not containing the current mode is reported, not
/// smoothed over.
///
/// Nothing in the spec forbids it, and the tempting repair — describing the
/// nearest listed mode instead — would attach one mode's meaning to another's
/// id, which is the same error as generalising an id across agents, one scope
/// smaller.
#[test]
fn a_current_mode_missing_from_the_list_is_said_plainly() {
    let state = SessionModeState::new(
        SessionModeId::from("ghost".to_owned()),
        vec![mode("plan", Some("Planning mode"))],
    );

    let decision = Decision::of("conv-ghost", Some(&state), None);
    let note = decision.note().expect("modes exist");
    assert!(note.contains("`ghost`"), "names the mode in force: {note}");
    assert!(
        note.contains("did not include"),
        "and says why there is nothing to show for it: {note}"
    );
    assert!(
        !note.contains("Planning mode"),
        "and does not borrow another mode's description: {note}"
    );
}

/// An autonomous change says *the agent* did it.
///
/// The distinction is the whole value of the notification: a person reading
/// "this session is now in X" cannot tell whether their own
/// `WARP_FORK_ACP_MODE` was honoured late or the agent moved on its own, and
/// only the second is news.
#[test]
fn an_autonomous_change_names_the_agent_as_the_one_who_made_it() {
    let mut state = claude_modes();
    state.current_mode_id = SessionModeId::from("bypassPermissions".to_owned());

    let note = changed(Some(&state), &state.current_mode_id);

    assert!(
        note.contains("changed this session's mode on its own"),
        "attributes the change: {note}"
    );
    assert!(
        note.contains("Bypass all permission checks"),
        "and carries the agent's own description of where it landed: {note}"
    );
}

/// A mode change to an id the agent never advertised still produces a usable
/// sentence.
///
/// `current_mode_update` carries only an id, and nothing obliges an agent to
/// have listed it. Reporting the id and saying there is no description beats
/// both alternatives: silence hides a policy change, and inventing a
/// description explains a policy Warp cannot see.
#[test]
fn a_change_to_an_unadvertised_mode_reports_the_id_and_says_so() {
    let state = claude_modes();

    let note = changed(Some(&state), &SessionModeId::from("surprise".to_owned()));

    assert!(note.contains("`surprise`"), "names it: {note}");
    assert!(
        note.contains("did not list"),
        "and says why nothing more is shown: {note}"
    );
}

/// **A long conversation is told once, not once a turn.**
///
/// Every turn after the first resumes with `session/load`, whose reply carries
/// `modes` exactly like `session/new`'s — so the unguarded version of this put
/// the same paragraph above every turn. That is the failure `NothingToSay`
/// already argues against one case over: a note that always appears is a note
/// nobody reads, and this one has to be read on the turn where the answer
/// changes.
#[test]
fn a_conversation_already_told_is_not_told_again() {
    let conversation = "conv-repeat";
    forget(conversation);

    let first = Decision::of(conversation, Some(&claude_modes()), None);
    let second = Decision::of(conversation, Some(&claude_modes()), None);

    assert!(first.note().is_some(), "the first turn says it");
    assert_eq!(
        second,
        Decision::NothingToSay,
        "and the second does not repeat it"
    );
}

/// …but a mode that *changed* is news again.
///
/// This is the case the gate exists to preserve rather than the one it exists
/// to suppress: a session whose policy moves between turns must say so, and a
/// gate keyed on "have we ever spoken" instead of "is this what we said" would
/// swallow exactly that.
#[test]
fn a_mode_that_changed_between_turns_is_told_again() {
    let conversation = "conv-changed";
    forget(conversation);
    let mut later = claude_modes();
    later.current_mode_id = SessionModeId::from("bypassPermissions".to_owned());

    let first = Decision::of(conversation, Some(&claude_modes()), None);
    let second = Decision::of(conversation, Some(&later), None);

    assert!(first.note().is_some());
    let note = second.note().expect("the mode moved, so this is news");
    assert!(
        note.contains("Bypass all permission checks"),
        "and the note describes where it moved to: {note}"
    );
}

/// A repeated turn still re-sends the request, and only stops narrating it.
///
/// `session/set_mode` is idempotent and a resumed session may have come back in
/// a different mode than it left, so rationing the *send* along with the
/// telling would make a long conversation quietly drift back to the agent's
/// default.
#[test]
fn a_repeated_request_is_still_sent_while_the_note_goes_quiet() {
    let conversation = "conv-quiet-request";
    forget(conversation);

    let first = Decision::of(conversation, Some(&claude_modes()), Some("default"));
    let second = Decision::of(conversation, Some(&claude_modes()), Some("default"));

    assert!(first.note().is_some(), "said once");
    assert_eq!(second.note(), None, "and not again");
    assert_eq!(
        second.mode().map(|id| id.0.to_string()),
        Some("default".to_owned()),
        "but the mode is still requested"
    );
}
