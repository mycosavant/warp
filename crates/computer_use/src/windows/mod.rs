//! Windows implementation of computer use actions using the Win32 SendInput
//! API for input and GDI for screenshots.

mod background;
mod dpi;
mod enumerate;
mod keyboard;
mod mouse;
mod screenshot;

use async_trait::async_trait;
pub use enumerate::{enumerate_windows, list_windows};
use warpui_core::r#async::Timer;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS, HDESK, OpenInputDesktop,
};

// Video recording is not yet implemented on Windows; reuse the no-op recorder.
pub use crate::noop::Recorder;
use crate::{Action, ActionResult, Options, Target, TargetedAction};

/// Returns whether computer_use can drive input on this machine right now.
///
/// Reports `false` when there is no accessible input desktop (e.g., the process is running under
/// Session 0 as a Windows service, the workstation is locked, or the user has switched to a
/// different secure desktop). In those cases `SendInput` silently no-ops and GDI desktop capture
/// fails, so we'd rather fail fast here than surface the error mid-action.
pub fn is_supported_on_current_platform() -> bool {
    probe_input_desktop_available()
}

/// Reports whether background, per-window control is available.
///
/// It is, via posted messages — see `background.rs`. What "background" buys here is narrower
/// than on X11: the cursor never moves and the user keeps their pointer, but the target window
/// still has to be the foreground one, because a posted drag on an inactive window is inert.
/// That was measured, not assumed; the A/B is in `background.rs`'s module comment.
pub fn background_supported() -> bool {
    true
}

/// Shared probe used by both [`is_supported_on_current_platform`] and [`Actor::new`] so the
/// "can we drive input right now?" logic lives in one place. This still runs the probe on each
/// call (it's a cheap `OpenInputDesktop` / `CloseDesktop` round-trip) — we don't cache it because
/// availability can change at runtime (workstation lock, secure desktop swap, Remote Desktop
/// reconnect).
fn probe_input_desktop_available() -> bool {
    InputDesktop::acquire().is_some()
}

/// RAII wrapper for an `HDESK` returned by `OpenInputDesktop`. Guarantees the handle is closed
/// (or at least that a close attempt is made and logged on failure) even if the caller returns
/// early. Modeled after the GDI handle guards in `screenshot.rs`.
struct InputDesktop(HDESK);

impl InputDesktop {
    fn acquire() -> Option<Self> {
        // SAFETY: `OpenInputDesktop` has no preconditions. We pass `false` for inheritance and
        // request no specific access (just probing for existence).
        let handle =
            unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(0)) };
        handle.ok().map(Self)
    }
}

impl Drop for InputDesktop {
    fn drop(&mut self) {
        // SAFETY: `self.0` is a valid HDESK returned by `OpenInputDesktop` and has not been
        // closed yet.
        unsafe {
            if let Err(e) = CloseDesktop(self.0) {
                log::warn!("CloseDesktop failed in InputDesktop::drop: {e}");
            }
        }
    }
}

/// Actor holds Keyboard/Mouse state unconditionally — both are cheap to construct and have no
/// side effects — so a `perform_actions` call can recover as soon as an input desktop is
/// reachable again, even if `Actor::new` ran while the desktop was temporarily inaccessible
/// (workstation locked at startup, RDP disconnect, etc.). Supportability is decided per call by
/// [`probe_input_desktop_available`] rather than being cached in the actor's shape.
pub struct Actor {
    keyboard: keyboard::Keyboard,
    mouse: mouse::Mouse,
    /// Posted-message input for window targets. Held on the actor, not built per call, so a
    /// drag that spans two `perform_actions` batches keeps its button-state bookkeeping —
    /// the same reason the X11 backend owns its agent seat.
    background_mouse: background::BackgroundMouse,
    background_keyboard: background::BackgroundKeyboard,
}

impl Actor {
    pub fn new() -> Self {
        Self {
            keyboard: keyboard::Keyboard::new(),
            mouse: mouse::Mouse::new(),
            background_mouse: background::BackgroundMouse::default(),
            background_keyboard: background::BackgroundKeyboard,
        }
    }
}

impl Default for Actor {
    fn default() -> Self {
        Self::new()
    }
}

/// Error returned by `perform_actions` when the input desktop is inaccessible at call time
/// (workstation lock, secure desktop swap, RDP reconnect, Session 0 service, …).
const NO_INPUT_DESKTOP_ERROR: &str = "Computer use is not available: no accessible input desktop";

#[async_trait]
impl super::Actor for Actor {
    fn platform(&self) -> Option<super::Platform> {
        // Live probe so callers can use `platform().is_some()` as a current "can drive input"
        // signal. Matches the Linux `Unsupported`-returns-None convention.
        if probe_input_desktop_available() {
            Some(super::Platform::Windows)
        } else {
            None
        }
    }

    async fn perform_actions(
        &mut self,
        actions: &[TargetedAction],
        options: Options,
    ) -> Result<ActionResult, String> {
        // Probe at the top of every call so transient loss of the input desktop (workstation
        // lock, secure desktop swap, RDP reconnect) surfaces as a descriptive error instead of
        // letting `SendInput` silently no-op. Cheap `OpenInputDesktop`/`CloseDesktop` round-trip.
        if !probe_input_desktop_available() {
            return Err(NO_INPUT_DESKTOP_ERROR.to_string());
        }
        let background = options.background_enabled;
        let keyboard = &mut self.keyboard;
        let mouse = &mut self.mouse;
        let background_mouse = &mut self.background_mouse;
        let background_keyboard = &mut self.background_keyboard;
        let mut drove_a_window = false;

        // Validate window targets before performing anything, so a bad id cannot leave a batch
        // half-applied — and, on a drag, cannot leave a button held with no release to come.
        for targeted in actions {
            if background && let Target::Window { window_id: 0, .. } = targeted.target {
                return Err(
                    "A window target requires a non-zero window id. Select a window \
                            from the enumerated window list."
                        .to_string(),
                );
            }
        }

        for targeted in actions {
            let action: &Action = &targeted.action;
            // `background_enabled: false` is the legacy contract: act on the screen regardless
            // of what the caller asked for.
            let target = if background {
                targeted.target
            } else {
                Target::Screen
            };
            let window_id = match target {
                Target::Window { window_id, .. } => {
                    drove_a_window = true;
                    Some(window_id)
                }
                Target::Screen => None,
            };

            match (window_id, action) {
                (_, Action::Wait(duration)) => {
                    Timer::after(*duration).await;
                }

                // Window-targeted: posted messages, no cursor movement.
                (Some(id), Action::MouseDown { button, at }) => {
                    background_mouse.button_down(id, button, *at)?
                }
                (Some(id), Action::MouseUp { button }) => background_mouse.button_up(id, button)?,
                (Some(id), Action::MouseMove { to }) => background_mouse.move_to(id, *to)?,
                (
                    Some(id),
                    Action::MouseWheel {
                        at,
                        direction,
                        distance,
                    },
                ) => background_mouse.scroll(id, *at, direction, distance)?,
                (Some(id), Action::TypeText { text }) => background_keyboard.type_text(id, text)?,
                (Some(id), Action::KeyDown { key }) => background_keyboard.key_down(id, key)?,
                (Some(id), Action::KeyUp { key }) => background_keyboard.key_up(id, key)?,

                // Screen-targeted: the desktop, as before.
                (None, Action::MouseDown { button, at }) => {
                    mouse.move_to(*at)?;
                    mouse.button_down(button)?;
                }
                (None, Action::MouseUp { button }) => mouse.button_up(button)?,
                (None, Action::MouseMove { to }) => mouse.move_to(*to)?,
                (
                    None,
                    Action::MouseWheel {
                        at,
                        direction,
                        distance,
                    },
                ) => {
                    mouse.move_to(*at)?;
                    mouse.scroll(direction, distance)?;
                }
                (None, Action::TypeText { text }) => keyboard.type_text(text)?,
                (None, Action::KeyDown { key }) => keyboard.key_down(key)?,
                (None, Action::KeyUp { key }) => keyboard.key_up(key)?,
            }
        }

        let (screenshot, captured_window) = match options.screenshot_params {
            Some(mut params) => {
                if !background {
                    params.target = Target::Screen;
                }
                match params.target {
                    Target::Window { window_id: 0, .. } => {
                        return Err("A window target requires a non-zero window id. Select a \
                                    window from the enumerated window list."
                            .to_string());
                    }
                    Target::Window { window_id, .. } => {
                        let (shot, captured) = screenshot::take_window(window_id, params)?;
                        (Some(shot), Some(captured))
                    }
                    Target::Screen => (Some(screenshot::take(params)?), None),
                }
            }
            None => (None, None),
        };

        // A window-targeted batch never moved the cursor, so reporting the desktop cursor
        // would be answering a question nobody asked; report the posted position instead.
        let cursor_position = if drove_a_window {
            background_mouse.last_position()
        } else {
            Some(mouse.current_position()?)
        };

        Ok(ActionResult {
            screenshot,
            cursor_position,
            windows: if background {
                enumerate::enumerate_windows()
            } else {
                Vec::new()
            },
            captured_window,
        })
    }
}
