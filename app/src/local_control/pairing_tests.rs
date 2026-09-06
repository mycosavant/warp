use super::*;

fn at(minute: u32) -> DateTime<Utc> {
    chrono::NaiveDate::from_ymd_opt(2026, 8, 26)
        .and_then(|date| date.and_hms_opt(12, minute, 0))
        .expect("a valid instant")
        .and_utc()
}

/// The property the whole three-step shape exists for. A pairing code is the
/// only secret in this flow that gets *shown* — to a camera, a scrollback,
/// whoever else is in the room — so a photograph of the QR taken while it was on
/// screen must be worth nothing a moment later.
#[test]
fn a_pairing_code_can_be_spent_exactly_once() {
    let mut pairings = Pairings::default();
    let issued = pairings.issue_code(at(0), Scope::Watch);

    assert!(pairings.redeem(&issued.code, at(0)).is_ok());
    assert!(
        pairings.redeem(&issued.code, at(0)).is_err(),
        "a replayed code must buy nothing"
    );
}

/// The second half of the same property: unspent is not the same as safe.
#[test]
fn a_pairing_code_stops_working_when_it_expires() {
    let mut pairings = Pairings::default();
    let issued = pairings.issue_code(at(0), Scope::Watch);

    assert!(
        pairings.redeem(&issued.code, at(1)).is_ok(),
        "a code must survive long enough to walk across the room"
    );

    let stale = pairings.issue_code(at(0), Scope::Watch);
    assert!(pairings.redeem(&stale.code, at(5)).is_err());
}

#[test]
fn a_paired_device_stops_being_paired_after_a_working_day() {
    let mut pairings = Pairings::default();
    let code = pairings.issue_code(at(0), Scope::Watch);
    let device = pairings
        .redeem(&code.code, at(0))
        .expect("pairing succeeds");

    let expires_at = device.expires_at.expect("a watch pairing has a clock");
    assert!(pairings.verify_device(&device.token, at(0)).is_ok());
    assert!(
        pairings
            .verify_device(&device.token, expires_at + Duration::seconds(1))
            .is_err()
    );
}

/// A control pairing has no clock (2026-09-06): a phone handed a conversation
/// at breakfast is still driving it after dinner, and what ends it is Stop
/// sharing, the conversation going away, or Warp closing. The watch pairing,
/// minted by a variable rather than a gesture, keeps its twelve hours.
#[test]
fn a_control_pairing_outlives_the_working_day_and_a_watch_pairing_does_not() {
    let mut pairings = Pairings::default();
    let control = Scope::Control {
        conversation_id: "c-1".to_owned(),
    };
    let watcher = {
        let code = pairings.issue_code(at(0), Scope::Watch);
        pairings.redeem(&code.code, at(0)).expect("pairs")
    };
    let driver = {
        let code = pairings.issue_code(at(0), control.clone());
        pairings.redeem(&code.code, at(0)).expect("pairs")
    };
    assert!(watcher.expires_at.is_some());
    assert_eq!(driver.expires_at, None, "no clock to report");

    let thirteen_hours_on = at(0) + Duration::hours(13);
    assert!(
        pairings
            .verify_device(&watcher.token, thirteen_hours_on)
            .is_err(),
        "the watch pairing lapsed"
    );
    assert_eq!(
        pairings
            .verify_device(&driver.token, thirteen_hours_on)
            .expect("the control pairing is still valid"),
        control
    );
    assert!(pairings.is_controlling("c-1", thirteen_hours_on));

    // And it still ends the way it is meant to.
    assert_eq!(pairings.revoke_conversation("c-1"), 1);
    assert!(
        pairings
            .verify_device(&driver.token, thirteen_hours_on)
            .is_err()
    );
}

/// The two secrets are drawn from the same generator and are the same shape, so
/// nothing about them *looks* different — only the list they are checked against
/// keeps them apart. A device token that could be redeemed as a pairing code
/// would turn a twelve-hour secret into an unlimited one.
#[test]
fn the_two_secrets_are_not_interchangeable() {
    let mut pairings = Pairings::default();
    let code = pairings.issue_code(at(0), Scope::Watch);
    let device = pairings
        .redeem(&code.code, at(0))
        .expect("pairing succeeds");

    assert!(
        pairings.redeem(&device.token, at(0)).is_err(),
        "a device token must not buy another device token"
    );

    let unspent = pairings.issue_code(at(0), Scope::Watch);
    assert!(
        pairings.verify_device(&unspent.code, at(0)).is_err(),
        "a pairing code must not authenticate as a device"
    );
}

#[test]
fn a_secret_this_instance_never_issued_is_refused() {
    let mut pairings = Pairings::default();
    pairings.issue_code(at(0), Scope::Watch);
    let stranger = AuthToken::generate();

    assert!(pairings.redeem(&stranger, at(0)).is_err());
    assert!(pairings.verify_device(&stranger, at(0)).is_err());
}

/// The security boundary of the feature, asserted by name rather than by
/// property, because `ActionKind` records no read/write bit to assert against.
///
/// Every action listed here is remote code execution on the paired machine:
/// `input.insert` + `input.submit` types into a terminal and presses return,
/// `agent.prompt` and `slash.run` drive the agent, and `remote.wsl.connect`
/// reaches another host entirely. If a future task wants one of these reachable
/// from a phone, this test is where the argument has to be made.
#[test]
fn a_paired_device_cannot_reach_the_actions_that_execute() {
    for action in [
        ActionKind::InputInsert,
        ActionKind::InputSubmit,
        ActionKind::AgentPrompt,
        ActionKind::AgentSpawn,
        ActionKind::SlashRun,
        ActionKind::RemoteWslConnect,
        ActionKind::FileOpen,
        ActionKind::DriveObjectCreate,
        ActionKind::SettingSet,
        ActionKind::WindowClose,
    ] {
        assert!(
            ensure_pairable_under(action, &Scope::Watch).is_err(),
            "{} must not be reachable by scanning a QR code",
            action.as_str()
        );
    }
}

/// Deliberately a change-detector. Widening what a scanned QR grants is exactly
/// the change that should not pass unnoticed, so the list is pinned and this
/// test is the place the next person states why they grew it.
///
/// T11.5 grew it by two. `agent.approvals` is a read that reports strictly less
/// than the already-pairable `events.subscribe` carries live. `agent.deny` is
/// the first write here and earns it by being monotone — Escape on an agent
/// that is already waiting, so the most it can cause is that something proposed
/// does not happen.
///
/// T14.21 grew it by one, and sharpened the criterion in doing so. "Monotone"
/// is too loose to be the test — `kill` and `rm -rf` both make less happen. The
/// real line is that an action may only prevent *proposed future effects* and
/// may not destroy *existing durable state*, which is what keeps `window.close`
/// out (it discards unsaved panes) while letting `agent.cancel` in. Cancel is
/// Stop and not Kill, it completes a loop whose read half — `agent.list`'s
/// `quiet_for_seconds` — was already granted, and its honest delta from deny is
/// that it can land mid-side-effect; the module docs carry that argument in
/// full.
///
/// Board item 6 grew it by one, and it is the widest read on the list:
/// `agent.trace` returns the conversation's record -- prompts, full tool
/// inputs, results, the agent's thinking where recorded. Asked for by the
/// maintainer on 2026-09-05 so a phone can watch a run; argued in the module
/// docs against what `events.subscribe` already grants. A read, still: a
/// stolen token learns and cannot act.
#[test]
fn a_paired_device_gets_the_read_surface_and_the_safe_half_of_answering() {
    assert_eq!(
        PAIRABLE_ACTIONS,
        [
            ActionKind::AppPing,
            ActionKind::AgentList,
            ActionKind::EventsSubscribe,
            ActionKind::AgentApprovals,
            ActionKind::AgentDeny,
            ActionKind::AgentCancel,
            ActionKind::AgentTrace,
        ]
        .as_slice()
    );
    for action in PAIRABLE_ACTIONS {
        assert!(ensure_pairable_under(*action, &Scope::Watch).is_ok());
    }
}

/// The asymmetry T11.5 was built around, pinned from the outside: saying no
/// travels to a phone and saying yes does not, unless the machine's owner sets
/// `WARP_FORK_REMOTE_APPROVE`.
///
/// Asserted against the *default* environment rather than by setting the
/// variable, because `remote_approve_enabled` reads a process-global and this
/// test runs beside others. What that leaves untested here is the switched-on
/// branch; `fork::a_paired_device_says_yes_only_when_the_owner_says_so` covers
/// the decision itself without a process-global.
#[test]
fn saying_yes_does_not_travel_by_default_and_saying_no_does() {
    assert!(ensure_pairable_under(ActionKind::AgentDeny, &Scope::Watch).is_ok());

    let error = ensure_pairable_under(ActionKind::AgentApprove, &Scope::Watch)
        .expect_err("refused by default");
    assert!(error.message.contains("agent.approve"));
    assert!(
        error.message.contains("WARP_FORK_REMOTE_APPROVE"),
        "the one refusal that is a local choice must say so: {}",
        error.message
    );
}

/// The refusal names what *is* allowed, because the alternative is a client
/// author guessing one action at a time against a server that only says no.
#[test]
fn the_refusal_says_what_a_device_may_do_instead() {
    let error = ensure_pairable_under(ActionKind::InputSubmit, &Scope::Watch).expect_err("refused");

    assert!(error.message.contains("input.submit"));
    assert!(error.message.contains("agent.list"));
}

/// A URL that carries a secret in its path or query is a secret in every access
/// log between here and the device. The fragment is the one part of a URL that
/// is never sent to a server.
#[test]
fn the_code_rides_in_the_fragment() {
    let code = AuthToken::from_secret("s3cret");
    let url = pair_url("192.168.1.5:41234", "/v1/pair", &code);

    assert_eq!(url, "https://192.168.1.5:41234/v1/pair#s3cret");
    // The certificate, by contrast, is fetched in the clear by a phone that
    // cannot yet trust the listener, and carries no secret.
    assert_eq!(
        ca_url("192.168.1.5:41234"),
        "http://192.168.1.5:41234/ca.crt"
    );
    let (addressed, fragment) = url.split_once('#').expect("a fragment");
    assert!(
        !addressed.contains("s3cret"),
        "the part a server would log must not contain the code"
    );
    assert_eq!(fragment, "s3cret");
}

/// The remote-control scope (2026-09-05): a code minted for one conversation
/// buys `agent.prompt` and `agent.approve` on top of the watch list, with no
/// switch, because the gesture that minted it is the consent. Pinned as a
/// whole list, like the watch list above, so the widening is visible here.
#[test]
fn a_code_minted_for_one_conversation_buys_driving_it() {
    let control = Scope::Control {
        conversation_id: "c-1".to_owned(),
    };
    let mut expected = pairable_actions();
    expected.push(ActionKind::AgentPrompt);
    if !expected.contains(&ActionKind::AgentApprove) {
        expected.push(ActionKind::AgentApprove);
    }
    assert_eq!(control.actions(), expected);
    assert!(ensure_pairable_under(ActionKind::AgentPrompt, &control).is_ok());
    assert!(ensure_pairable_under(ActionKind::AgentApprove, &control).is_ok());
    // Still not the executing half of the catalog: a conversation is the
    // unit handed over, and a pane is not.
    for action in [
        ActionKind::InputSubmit,
        ActionKind::AgentSpawn,
        ActionKind::SlashRun,
        ActionKind::WindowClose,
    ] {
        assert!(ensure_pairable_under(action, &control).is_err());
    }
    assert_eq!(control.conversation(), Some("c-1"));
    assert_eq!(Scope::Watch.conversation(), None);

    // And what it buys has no clock: the device that spends a control code
    // carries no expiry, where a watch device carries twelve hours.
    let mut pairings = Pairings::default();
    let code = pairings.issue_code(at(0), control);
    let driver = pairings.redeem(&code.code, at(0)).expect("pairs");
    assert_eq!(driver.expires_at, None);
}

/// The scope rides from the code to the device that spent it, and the device
/// answers with it on every later check: that is what the credential route
/// reads to confine the grant.
#[test]
fn a_device_holds_the_scope_of_the_code_it_spent() {
    let mut pairings = Pairings::default();
    let control = Scope::Control {
        conversation_id: "c-1".to_owned(),
    };
    let watch_code = pairings.issue_code(at(0), Scope::Watch);
    let control_code = pairings.issue_code(at(0), control.clone());

    let watcher = pairings.redeem(&watch_code.code, at(0)).expect("pairs");
    let driver = pairings.redeem(&control_code.code, at(0)).expect("pairs");
    assert_eq!(watcher.scope, Scope::Watch);
    assert_eq!(driver.scope, control);
    assert_eq!(
        pairings
            .verify_device(&watcher.token, at(1))
            .expect("paired"),
        Scope::Watch
    );
    assert_eq!(
        pairings
            .verify_device(&driver.token, at(1))
            .expect("paired"),
        control
    );
}

/// Stop sharing ends it: the driving device is cut off, an unscanned code for
/// the conversation is dead, and a watching device loses nothing.
#[test]
fn revoking_a_conversation_cuts_off_its_devices_and_codes_and_nothing_else() {
    let mut pairings = Pairings::default();
    let control = Scope::Control {
        conversation_id: "c-1".to_owned(),
    };
    let watcher = {
        let code = pairings.issue_code(at(0), Scope::Watch);
        pairings.redeem(&code.code, at(0)).expect("pairs")
    };
    let driver = {
        let code = pairings.issue_code(at(0), control.clone());
        pairings.redeem(&code.code, at(0)).expect("pairs")
    };
    let unscanned = pairings.issue_code(at(0), control);
    assert!(pairings.is_controlling("c-1", at(0)));
    assert!(!pairings.is_controlling("c-2", at(0)));

    assert_eq!(pairings.revoke_conversation("c-1"), 1);

    assert!(!pairings.is_controlling("c-1", at(0)));
    assert!(pairings.verify_device(&driver.token, at(0)).is_err());
    assert!(pairings.redeem(&unscanned.code, at(0)).is_err());
    assert!(pairings.verify_device(&watcher.token, at(0)).is_ok());
}
