use local_control::protocol::{VisorState, VisorStatusResult};

/// The state names are the wire contract — a script polls for `"open"` after a
/// toggle. They are produced by `#[serde(rename_all = "snake_case")]`, which
/// makes them a *derived* consequence of the Rust variant names rather than
/// something anyone chose, so renaming a variant would silently change them.
#[test]
fn the_state_names_are_snake_case_and_stable() {
    let names: Vec<String> = [
        VisorState::Absent,
        VisorState::Open,
        VisorState::PendingOpen,
        VisorState::Hidden,
    ]
    .iter()
    .map(|state| serde_json::to_value(state).expect("serializes").to_string())
    .collect();

    assert_eq!(
        names,
        vec!["\"absent\"", "\"open\"", "\"pending_open\"", "\"hidden\"",]
    );
}

/// `absent` and `hidden` both mean "nothing on screen", and a caller that
/// collapsed them would be wrong about what the next toggle does: from
/// `absent` it builds a window, from `hidden` it reveals the existing one.
/// The window id is what makes them distinguishable without trusting the state
/// name, so the two must not serialize alike.
#[test]
fn absent_and_hidden_are_distinguishable() {
    let absent = serde_json::to_value(VisorStatusResult {
        state: VisorState::Absent,
        window_id: None,
        opens_agent: true,
        hotkey_enabled: false,
        hotkey: None,
    })
    .expect("serializes");

    let hidden = serde_json::to_value(VisorStatusResult {
        state: VisorState::Hidden,
        window_id: Some("2".to_owned()),
        opens_agent: true,
        hotkey_enabled: false,
        hotkey: None,
    })
    .expect("serializes");

    assert_ne!(absent, hidden);
    assert_eq!(absent["window_id"], serde_json::Value::Null);
    assert_eq!(hidden["window_id"], "2");
}

/// An enabled hotkey with nothing bound to it is a real state, not a
/// contradiction: `global_hotkey.dedicated_window.enabled` and its keybinding
/// are separate settings, and the settings UI lets you switch the first on
/// without choosing a key. Reporting them as one boolean would tell a user
/// their hotkey works when no key was ever registered.
#[test]
fn an_enabled_hotkey_can_still_be_unbound() {
    let value = serde_json::to_value(VisorStatusResult {
        state: VisorState::Absent,
        window_id: None,
        opens_agent: true,
        hotkey_enabled: true,
        hotkey: None,
    })
    .expect("serializes");

    assert_eq!(value["hotkey_enabled"], true);
    assert_eq!(value["hotkey"], serde_json::Value::Null);
}
