//! Per-window input on Windows, by posting messages instead of driving the desktop.
//!
//! The rest of this backend is `SetCursorPos` + `SendInput`, which is the whole machine: it
//! moves the user's cursor, it goes wherever the foreground window is, and a drag that lasts
//! two seconds is two seconds during which the person at the keyboard cannot use their mouse.
//! That is why `background_supported()` used to answer `false` here while macOS and Linux
//! both answered `true`.
//!
//! `PostMessage` to one `HWND` is the way out, and it is not new to this fork — `click.ps1`
//! and `keys.ps1` in the tooling directory have used it for months. This is the same idea,
//! extended to press-move-release and moved into the crate so `use_computer drag --pid
//! --window-id` means the same thing on Windows as it does on X11.
//!
//! **Two limits, both measured rather than assumed (2026-08-23, Warp's own window):**
//!
//! * **The target window must be the foreground window.** Posted messages reach an inactive
//!   window's queue — a posted click on an inactive Warp *does* select the tab under it — but
//!   a posted drag on an inactive window does nothing at all. A/B: focus window 0, drag,
//!   tabs reorder; focus window 1, repeat the identical drag on window 0, nothing moves.
//!   The cursor still never moves, which is the part that matters, but this is not a way to
//!   drive a window the user is not looking at.
//! * **Modifiers are not expressible.** Posted messages do not set the thread's key state, so
//!   `ctrl-shift-<key>` arrives as a bare `<key>`. This is the same wall `keys.ps1` documents,
//!   and it is why `Target::Screen` (SendInput) still exists for the cases that need it.

use pathfinder_geometry::vector::Vector2I;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, VIRTUAL_KEY, VkKeyScanExW};
use windows::Win32::UI::WindowsAndMessaging::{
    PostMessageW, WHEEL_DELTA, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_XBUTTONDOWN, WM_XBUTTONUP,
};

use super::enumerate::{client_size, hwnd_from_id, window_local_to_client};
use crate::{Key, MouseButton, ScrollDirection, ScrollDistance};

/// `MK_*` button-state bits and the `XBUTTON*` discriminators, spelled out rather than
/// imported: the `windows` crate files `MK_LBUTTON` under `Win32::System::SystemServices` as a
/// `MODIFIERKEYS_FLAGS`, which would mean pulling in a whole feature to name five constants
/// that have not changed since Windows 95.
const MK_LBUTTON: u16 = 0x0001;
const MK_RBUTTON: u16 = 0x0002;
const MK_MBUTTON: u16 = 0x0010;
const MK_XBUTTON1: u16 = 0x0020;
const MK_XBUTTON2: u16 = 0x0040;
const XBUTTON1: u16 = 0x0001;
const XBUTTON2: u16 = 0x0002;

/// Pixels per wheel notch used when a caller asks for `ScrollDistance::Pixels`. The
/// screen-targeted path reads the user's `SPI_GETWHEELSCROLLLINES` and scales by monitor DPI;
/// a posted wheel message is delivered to one window and never consulted by the shell, so the
/// nominal notch is the honest conversion here rather than a guess dressed up as a setting.
const NOMINAL_PIXELS_PER_NOTCH: i32 = 48;

/// Tracks which buttons a posted drag is holding, because every mouse message carries the
/// *current* button state in its `wParam` and a move with an empty `wParam` mid-drag reads to
/// the receiving application as "the button came up somewhere I did not see".
#[derive(Default)]
pub struct BackgroundMouse {
    held: u16,
    last_local: Option<Vector2I>,
}

/// `MAKELPARAM`: low word x, high word y, both signed 16-bit on the wire.
fn make_lparam(x: i32, y: i32) -> LPARAM {
    LPARAM((((y & 0xFFFF) << 16) | (x & 0xFFFF)) as isize)
}

fn button_masks(button: &MouseButton) -> (u16, u32, u32, u16) {
    // (state bit, down message, up message, XBUTTON discriminator)
    match button {
        MouseButton::Left => (MK_LBUTTON, WM_LBUTTONDOWN, WM_LBUTTONUP, 0),
        MouseButton::Right => (MK_RBUTTON, WM_RBUTTONDOWN, WM_RBUTTONUP, 0),
        MouseButton::Middle => (MK_MBUTTON, WM_MBUTTONDOWN, WM_MBUTTONUP, 0),
        MouseButton::Back => (MK_XBUTTON1, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1),
        MouseButton::Forward => (MK_XBUTTON2, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON2),
    }
}

impl BackgroundMouse {
    fn post(
        &self,
        window_id: u32,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Result<(), String> {
        // SAFETY: `hwnd_from_id` produces a handle the caller selected from the enumerated
        // window list; `PostMessageW` fails cleanly on a stale one rather than faulting.
        unsafe { PostMessageW(Some(hwnd_from_id(window_id)), message, wparam, lparam) }
            .map_err(|e| format!("PostMessage to window {window_id} failed: {e}"))
    }

    /// Converts a window-local point and rejects anything outside the client area, rather than
    /// posting a coordinate the window will hit-test against nothing.
    fn client_point(&self, window_id: u32, at: Vector2I) -> Result<(i32, i32), String> {
        let (cx, cy) = window_local_to_client(window_id, at.x(), at.y()).ok_or_else(|| {
            format!("Could not resolve window {window_id}; it may have closed mid-batch.")
        })?;
        let (width, height) = client_size(window_id)
            .ok_or_else(|| format!("Could not read the client area of window {window_id}."))?;
        if cx < 0 || cy < 0 || cx >= width || cy >= height {
            return Err(format!(
                "({}, {}) is outside window {window_id}'s client area ({width}x{height}).",
                at.x(),
                at.y()
            ));
        }
        Ok((cx, cy))
    }

    pub fn move_to(&mut self, window_id: u32, at: Vector2I) -> Result<(), String> {
        let (cx, cy) = self.client_point(window_id, at)?;
        self.post(
            window_id,
            WM_MOUSEMOVE,
            WPARAM(self.held as usize),
            make_lparam(cx, cy),
        )?;
        self.last_local = Some(at);
        Ok(())
    }

    pub fn button_down(
        &mut self,
        window_id: u32,
        button: &MouseButton,
        at: Vector2I,
    ) -> Result<(), String> {
        let (cx, cy) = self.client_point(window_id, at)?;
        let (mask, down, _, xbutton) = button_masks(button);
        // A move to the press point first: hit-testing that only updates on motion would
        // otherwise resolve the press against wherever the last posted move left it.
        self.post(
            window_id,
            WM_MOUSEMOVE,
            WPARAM(self.held as usize),
            make_lparam(cx, cy),
        )?;
        self.held |= mask;
        let wparam = WPARAM(((xbutton as usize) << 16) | self.held as usize);
        self.post(window_id, down, wparam, make_lparam(cx, cy))?;
        self.last_local = Some(at);
        Ok(())
    }

    pub fn button_up(&mut self, window_id: u32, button: &MouseButton) -> Result<(), String> {
        let at = self.last_local.unwrap_or_else(|| Vector2I::new(0, 0));
        let (cx, cy) = self.client_point(window_id, at)?;
        let (mask, _, up, xbutton) = button_masks(button);
        self.held &= !mask;
        let wparam = WPARAM(((xbutton as usize) << 16) | self.held as usize);
        self.post(window_id, up, wparam, make_lparam(cx, cy))
    }

    pub fn scroll(
        &mut self,
        window_id: u32,
        at: Vector2I,
        direction: &ScrollDirection,
        distance: &ScrollDistance,
    ) -> Result<(), String> {
        // Unlike every other mouse message, WM_MOUSEWHEEL and WM_MOUSEHWHEEL carry *screen*
        // coordinates. Posting client coordinates here scrolls the wrong thing on a window
        // that is not at the origin, silently.
        let rect = super::enumerate::window_rect(window_id)
            .ok_or_else(|| format!("Could not resolve window {window_id} for a scroll."))?;
        let screen_x = rect.left + at.x();
        let screen_y = rect.top + at.y();

        let notches = match distance {
            ScrollDistance::Clicks(n) => *n,
            ScrollDistance::Pixels(px) => {
                let n = px / NOMINAL_PIXELS_PER_NOTCH;
                // Never round a requested scroll down to nothing.
                if n == 0 && *px != 0 { px.signum() } else { n }
            }
        };
        let (message, sign) = match direction {
            ScrollDirection::Up => (WM_MOUSEWHEEL, 1),
            ScrollDirection::Down => (WM_MOUSEWHEEL, -1),
            ScrollDirection::Right => (WM_MOUSEHWHEEL, 1),
            ScrollDirection::Left => (WM_MOUSEHWHEEL, -1),
        };
        let delta = (notches.abs() * sign * WHEEL_DELTA as i32) as i16;
        let wparam = WPARAM((((delta as u16) as usize) << 16) | self.held as usize);
        self.post(window_id, message, wparam, make_lparam(screen_x, screen_y))
    }

    pub fn last_position(&self) -> Option<Vector2I> {
        self.last_local
    }
}

/// Posted keyboard input for one window.
///
/// `WM_KEYDOWN`/`WM_KEYUP` rather than `WM_CHAR`: the fork measured that posted `WM_CHAR`
/// never reaches Warp's editor at all, while posted virtual-key messages do.
#[derive(Default)]
pub struct BackgroundKeyboard;

impl BackgroundKeyboard {
    fn virtual_key(&self, key: &Key) -> Result<u16, String> {
        match key {
            Key::Keycode(code) => {
                u16::try_from(*code).map_err(|_| format!("{code} is not a virtual-key code."))
            }
            Key::Char(ch) => {
                let mut units = [0u16; 2];
                let encoded = ch.encode_utf16(&mut units);
                if encoded.len() != 1 {
                    return Err(format!(
                        "{ch:?} is outside the Basic Multilingual Plane; use TypeText instead."
                    ));
                }
                // SAFETY: no preconditions; a null layout means "the calling thread's".
                let layout = unsafe { GetKeyboardLayout(0) };
                // SAFETY: `units[0]` is a valid UTF-16 code unit.
                let scan = unsafe { VkKeyScanExW(units[0], layout) };
                if scan == -1 {
                    return Err(format!("{ch:?} has no key on the active keyboard layout."));
                }
                Ok((scan as u16) & 0x00FF)
            }
        }
    }

    fn post(&self, window_id: u32, message: u32, vk: u16) -> Result<(), String> {
        // SAFETY: see `BackgroundMouse::post`.
        unsafe {
            PostMessageW(
                Some(hwnd_from_id(window_id)),
                message,
                WPARAM(VIRTUAL_KEY(vk).0 as usize),
                // Repeat count 1; scan code and the transition/extended flags are left at 0.
                // Applications that read them from the message rather than calling
                // `GetKeyState` will see a plain, non-extended key, which is what we mean.
                LPARAM(if message == WM_KEYUP {
                    0xC000_0001u32 as i32 as isize
                } else {
                    1
                }),
            )
        }
        .map_err(|e| format!("PostMessage to window {window_id} failed: {e}"))
    }

    pub fn key_down(&mut self, window_id: u32, key: &Key) -> Result<(), String> {
        let vk = self.virtual_key(key)?;
        self.post(window_id, WM_KEYDOWN, vk)
    }

    pub fn key_up(&mut self, window_id: u32, key: &Key) -> Result<(), String> {
        let vk = self.virtual_key(key)?;
        self.post(window_id, WM_KEYUP, vk)
    }

    pub fn type_text(&mut self, window_id: u32, text: &str) -> Result<(), String> {
        for ch in text.chars() {
            let key = Key::Char(ch);
            self.key_down(window_id, &key)?;
            self.key_up(window_id, &key)?;
        }
        Ok(())
    }
}
