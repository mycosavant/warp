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
}

/// **The note speaks after the answer, because it is only ever read after the
/// answer** — and this assertion is the inverse of the one it replaces.
///
/// The old test required the note to contain *"Whether the agent honours the
/// request"*, on the reasoning that *"requesting is not receiving, and the note
/// must not blur that"*. That principle is right and was being applied one
/// step too early. `mod.rs` sends `session/set_mode` and returns on the error
/// path **before** anything reads [`Decision::note`], so the only way this
/// string reaches a person is a turn in which the agent already accepted. The
/// hedge was therefore never true when displayed: it told a reader Warp did not
/// know something that `mode::acknowledged`, two lines above the emit, had just
/// recorded.
///
/// Measured on the wire 2026-09-01 before it was changed — the note rendered,
/// the agent had accepted, and the sentence disclaimed it.
///
/// So the pin is on both halves: no hedge, and the pre-request mode named in the
/// past tense, since by render time the session has left it. The lead is
/// anchored to the *turn* rather than to the session — see
/// `a_resumed_turn_does_not_claim_the_session_opened_in_the_resumed_mode` for
/// why "opened" was wrong on a resume.
#[test]
fn the_request_note_reports_the_answer_rather_than_hedging_about_it() {
    let decision = Decision::of("conv-request-tense", Some(&claude_modes()), Some("default"));
    let note = decision.note().expect("a request is worth saying");

    assert!(
        !note.contains("honours the request"),
        "the hedge is settled before this renders, and repeating it understates \
         what Warp knows: {note}"
    );
    assert!(
        note.contains("the agent accepted"),
        "say what the agent answered, since the emit path guarantees it: {note}"
    );
    assert!(
        note.contains("This turn began with the session in"),
        "the pre-request mode is history by the time this is read, and the claim is \
         anchored to the turn because that is what the reply describes: {note}"
    );
    assert!(
        !note.contains("This session is in the agent's `auto`"),
        "naming the mode the session just left as the one it is in is the \
         defect `an_acknowledged_mode_becomes_the_reported_one` fixed for the \
         status field: {note}"
    );
}

/// **A resumed turn must not be told it "opened" in the mode it resumed in.**
///
/// Caught in review, by reasoning about an agent nobody here has run. `state` is
/// *this turn's* reply, and every turn after the first is a `session/load` — so
/// a lead anchored to the session's beginning is a claim the input cannot
/// support. `claude-agent-acp` hides this perfectly: it comes back in `auto`
/// every time, which matches what was last disclosed, so its resumed turns go
/// quiet through `RequestQuietly` and never re-render this arm. The measured run
/// could not have found it.
///
/// This is the shape the fixture cannot produce, so it is built by hand: an
/// agent that *persists* the requested mode across a load. Turn 2 then carries a
/// current mode differing from the one last told, which makes it news again and
/// re-renders the note.
#[test]
fn a_resumed_turn_does_not_claim_the_session_opened_in_the_resumed_mode() {
    let conversation = "conv-persisting-agent";
    forget(conversation);

    // Turn 1: the session opens in `auto` and Warp asks for `default`.
    let first = Decision::of(conversation, Some(&claude_modes()), Some("default"));
    assert!(first.note().is_some(), "the first turn is news");

    // Turn 2: `session/load` returns the agent still in `default`, unlike
    // `claude-agent-acp`, which always reverts.
    let mut resumed = claude_modes();
    resumed.current_mode_id = SessionModeId::from("default".to_owned());
    let second = Decision::of(conversation, Some(&resumed), Some("default"));

    let note = second
        .note()
        .expect("a mode differing from the one last told is news again");
    assert!(
        !note.contains("opened in"),
        "this session opened in `auto` and merely resumed in `default`; saying it \
         opened in `default` states a fact the load reply does not carry: {note}"
    );
    assert!(
        note.contains("This turn began with the session in"),
        "turn-anchored, which is true on `session/new` and `session/load` alike: {note}"
    );
}

/// The un-asked case keeps the present tense, because nothing moves the session
/// on that path — [`Decision::Disclose`] sends no `set_mode` at all.
#[test]
fn the_disclose_note_keeps_the_present_tense() {
    let decision = Decision::of("conv-disclose-tense", Some(&claude_modes()), None);
    let note = decision
        .note()
        .expect("an undisclosed mode is worth saying");

    assert!(
        note.contains("This session is in"),
        "nothing was requested, so the session is still where it opened: {note}"
    );
}

/// **Reported, never sent.** The spec requires the id be one the agent
/// advertised, so an unknown one buys a protocol error instead of a sentence —
/// and a *silent* failure would leave a person believing a mode was requested
/// when it was not, which is strictly worse than the error.
/// **Refused, not noted — and this is a correction to the first cut.**
///
/// That version printed the problem and ran the turn anyway, reasoning that the
/// note said plainly what had happened. But the thing the note is *about* is a
/// session running under a policy the person did not choose, which is the
/// failure this module exists to end. `WARP_FORK_ACP_MODE` is
/// `WARP_FORK_CONTROL_BIND`-shaped: a typo would otherwise silently mean
/// something. Unlike `CONTROL_BIND` — where refusing to start would take away
/// `warpctrl window close` — refusing here costs only the turn.
#[test]
fn a_requested_mode_the_agent_does_not_offer_refuses_the_turn() {
    let decision = Decision::of("conv-unknown", Some(&claude_modes()), Some("architect"));

    assert_eq!(decision.mode(), None, "nothing is sent");
    assert_eq!(
        decision.note(),
        None,
        "and this is not a note, it is a stop"
    );
    let reason = decision.refusal().expect("the turn must not run");
    assert!(
        reason.contains("`architect`") && reason.contains("does not offer"),
        "names what was asked for and why it did not happen: {reason}"
    );
    assert!(
        reason.contains("rather than run under a mode you did not choose"),
        "and says why refusing beats continuing: {reason}"
    );
    assert!(
        reason.contains("`bypassPermissions`"),
        "the offered list is the evidence for the claim about what is available: {reason}"
    );
}

/// A refusal is never rationed by the news gate.
///
/// The gate spares a *reader* a repeated paragraph. A refusal is not read and
/// then continued past — it stops the turn — so a second attempt with the same
/// typo must fail exactly as loudly as the first, or the second turn would run.
#[test]
fn a_refusal_repeats_for_every_turn_that_earns_it() {
    let conversation = "conv-refuse-twice";
    forget(conversation);

    let first = Decision::of(conversation, Some(&claude_modes()), Some("architect"));
    let second = Decision::of(conversation, Some(&claude_modes()), Some("architect"));

    assert!(first.refusal().is_some());
    assert!(
        second.refusal().is_some(),
        "a second turn under the same typo must not quietly run"
    );
}

/// The offered list carries the agent's descriptions, not just its ids.
///
/// Ids alone are not a usable lever: `dontAsk` and `auto` both sound like
/// not-asking and only one of them is. The person picks from the agent's own
/// words, and Warp recommends nothing.
#[test]
fn the_offered_modes_carry_their_descriptions() {
    let decision = Decision::of("conv-lever", Some(&claude_modes()), None);

    let note = decision.note().expect("modes exist");
    assert!(
        note.contains("`default` (“Standard behavior, prompts for dangerous operations”)"),
        "the id a person would set, with what the agent says it does: {note}"
    );
    assert!(
        note.contains("WARP_FORK_ACP_MODE"),
        "and the lever that sets it: {note}"
    );
}

/// `agent.list` reports the mode even when nothing was said to anyone.
///
/// The two answer different questions: the note is rationed because a human
/// reader tires, and an orchestrator polling a status field must not inherit
/// that rationing.
#[test]
fn the_current_mode_is_readable_after_the_note_goes_quiet() {
    let conversation = "conv-status";
    forget(conversation);

    let _first = Decision::of(conversation, Some(&claude_modes()), None);
    let second = Decision::of(conversation, Some(&claude_modes()), None);

    assert_eq!(second.note(), None, "the second turn says nothing");
    assert_eq!(
        current_for(conversation).as_deref(),
        Some("auto"),
        "but the status field still knows"
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
        !note.contains("“”"),
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
    // Scoped to the sentence about the *current* mode rather than the whole
    // note, and the difference is the finding: since the offered list started
    // carrying descriptions, `plan`'s appears legitimately further down. The
    // invariant was never "this string is absent" — it is "`ghost` is not
    // described using another mode's words" — and a whole-note assertion
    // conflated the two, then failed on correct behaviour.
    let about_the_current_mode = note
        .split_once("Warp did not choose it")
        .expect("the disclosure always says this")
        .0;
    assert!(
        !about_the_current_mode.contains("Planning mode"),
        "and does not borrow another mode's description: {about_the_current_mode}"
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

    let note = changed("conv-autonomous", Some(&state), &state.current_mode_id);

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

    let note = changed(
        "conv-surprise",
        Some(&state),
        &SessionModeId::from("surprise".to_owned()),
    );

    assert!(note.contains("`surprise`"), "names it: {note}");
    assert!(
        note.contains("did not list"),
        "and says why nothing more is shown: {note}"
    );
    // **This arm shipped with eighteen spaces in the middle of it**, from
    // T14.18 on 2026-08-30 until 2026-09-01 — a lost line-continuation `\`,
    // which keeps the newline's indentation inside the literal. The test above
    // passed throughout, because `contains` does not care what lies between the
    // fragments it looks for. Nothing in the toolchain reads prose, and this is
    // the rarest arm in the module, so it went to a person's screen unseen.
    //
    // Pinned on the whole module's output rather than on this one string: a
    // sentence Warp shows about a policy it cannot see is this module's entire
    // product, and mangled whitespace is the one defect in it that no reviewer
    // reading a diff will catch.
    //
    // Worth knowing what this pins: descriptions are quoted from the agent
    // verbatim, so an agent whose own description contained two spaces would
    // trip it. That is acceptable and deliberate -- the fixture's text is
    // fixed, and the assertion is about Warp's formatting of everything
    // *around* the quote. If it ever fires on a real description, the answer is
    // to narrow the assertion, not to widen the formatting.
    assert!(
        !note.contains("  "),
        "no run of spaces inside a sentence a person reads: {note:?}"
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

/// **The status field must report where the session ended up, not where it
/// started** — found by running it, on a session `agent.list` called `auto`
/// moments after Warp had moved it to `default`.
///
/// The recorded mode came from `session/new`'s reply and nothing updated it on
/// success. A confidently wrong status field is worse than an absent one: an
/// orchestrator reading `auto` concludes a classifier is answering, on the one
/// session where it demonstrably is not.
#[test]
fn an_acknowledged_mode_becomes_the_reported_one() {
    let conversation = "conv-ack";
    forget(conversation);

    let decision = Decision::of(conversation, Some(&claude_modes()), Some("default"));
    assert_eq!(
        current_for(conversation).as_deref(),
        Some("auto"),
        "before the agent answers, the session is still where it opened"
    );

    acknowledged(conversation, decision.mode().expect("a mode was requested"));

    assert_eq!(
        current_for(conversation).as_deref(),
        Some("default"),
        "and after it answers, the status field says so"
    );
}
