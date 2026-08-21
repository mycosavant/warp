use std::fmt::{self, Debug};
use std::ops::{Deref, DerefMut};

/// Wrapper type for values which may contain user input.
///
/// Use this to prevent logging user input in production builds. In local development builds, the
/// value will still be shown.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct UserInput<T>(T);

impl<T> UserInput<T> {
    pub fn new<U: Into<T>>(value: U) -> Self {
        Self(value.into())
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for UserInput<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for UserInput<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Debug> Debug for UserInput<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if cfg!(debug_assertions) {
            f.debug_tuple("UserInput").field(&self.0).finish()
        } else {
            f.debug_struct("UserInput").finish_non_exhaustive()
        }
    }
}

#[cfg(test)]
mod tests {
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
}
