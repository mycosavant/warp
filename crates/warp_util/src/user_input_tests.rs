use super::UserInput;

/// The whole point of the type, pinned.
///
/// `warpui_core`'s dispatcher logs every action's `Debug` at `INFO`, and
/// `EditorAction::UserInsert` carries a `UserInput` holding the character
/// that was typed. So this one `cfg!` is the only thing standing between a
/// release build and a plaintext keystroke log on disk — and until now it
/// was a hand-written `Debug` impl with no test behind it. A careless
/// `#[derive(Debug)]` would compile, pass every other test, and start
/// writing what people type into `~/.local/state/warp-oss/warp-oss.log`.
///
/// Written while measuring the "log spam on window move" question, which
/// turned out to be misattributed: `workspace:save_app` is a few dozen
/// lines a run, and the volume is the dispatcher — 2975 `UserInsert` lines
/// in one two-minute stretch of a held key.
#[test]
fn user_input_is_redacted_unless_this_is_a_development_build() {
    let rendered = format!("{:?}", UserInput::<String>::new("hunter2"));

    if cfg!(debug_assertions) {
        // Development builds show it on purpose; that is what the type's
        // doc comment promises, and it is why a debug build's log contains
        // what you typed.
        assert!(rendered.contains("hunter2"), "{rendered}");
    } else {
        assert!(!rendered.contains("hunter2"), "{rendered}");
        assert!(rendered.contains("UserInput"), "{rendered}");
    }
}
