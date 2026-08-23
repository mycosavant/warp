//! Top-level window enumeration on Windows.
//!
//! This exists for the same reason the X11 version does: a window target needs a concrete
//! window id, and the id has to come from somewhere. Before this, `use_computer windows`
//! answered "only supported on macOS and Linux (X11)" on the one platform this fork's GUI
//! is actually used on.
//!
//! `EnumWindows` rather than `Process::MainWindowHandle`: Warp puts every window in one
//! process and Windows nominates exactly one of them as "main", so a tab torn out into a
//! second window is invisible to that API — and the nomination *moves*, so the same call
//! answers a different window before and after a tear-out.

use std::ffi::c_void;

use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetClientRect, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible,
};
use windows::core::BOOL;

/// `EnumWindows` continues while the callback returns non-zero.
const CONTINUE_ENUMERATION: BOOL = BOOL(1);

use crate::WindowInfo;

/// Windows narrower or shorter than this are tool tips, drop shadows, IME candidate hosts and
/// the various 1x1 message sinks every GUI toolkit leaves lying around. None of them are
/// things an agent means to target, and listing them buries the ones that are.
const MIN_USEFUL_EXTENT: i32 = 64;

/// Collector handed to `EnumWindows` through its `LPARAM`.
struct Collector {
    windows: Vec<WindowInfo>,
}

/// Reads a `GetWindowTextW`/`GetClassNameW`-style UTF-16 result into a `String`.
///
/// Both APIs return the number of characters written, *excluding* the terminator, and both
/// can legitimately return 0 (a window with no title). The buffer is deliberately generous:
/// truncation here would silently produce a different title than the one on screen.
fn read_utf16<F>(mut fill: F) -> String
where
    F: FnMut(&mut [u16]) -> i32,
{
    let mut buffer = [0u16; 512];
    let written = fill(&mut buffer);
    if written <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..written as usize])
}

/// `EnumWindows` callback. Returning `FALSE` would stop enumeration early, so this always
/// returns `TRUE` — a window we cannot read is skipped, not a reason to truncate the list.
unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the `&mut Collector` this module passed to `EnumWindows`, and the
    // borrow outlives the call because `EnumWindows` is synchronous.
    let collector = unsafe { &mut *(lparam.0 as *mut Collector) };

    // SAFETY: `hwnd` comes from the enumerator, so it is valid for the duration of the callback.
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return CONTINUE_ENUMERATION;
        }

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return CONTINUE_ENUMERATION;
        }
        if rect.right - rect.left < MIN_USEFUL_EXTENT || rect.bottom - rect.top < MIN_USEFUL_EXTENT
        {
            return CONTINUE_ENUMERATION;
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let title = read_utf16(|buf| GetWindowTextW(hwnd, buf));
        // The X11 implementation uses the window class as `app_name`; keeping that here means
        // the two platforms' listings mean the same thing column for column.
        let app_name = read_utf16(|buf| GetClassNameW(hwnd, buf));

        collector.windows.push(WindowInfo {
            window_id: hwnd.0 as u32,
            pid: pid as i32,
            app_name,
            title,
            layer: 0,
        });
    }

    CONTINUE_ENUMERATION
}

/// Every visible, non-minimised top-level window, in `EnumWindows` order — which is front to
/// back, so the first match for a title is the one on top.
pub fn enumerate_windows() -> Vec<WindowInfo> {
    let mut collector = Collector {
        windows: Vec::new(),
    };
    // SAFETY: the callback only dereferences the pointer we pass here, and `EnumWindows`
    // returns before `collector` goes out of scope.
    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            LPARAM(&mut collector as *mut Collector as isize),
        );
    }
    collector.windows
}

/// Human-readable listing, in the same shape the X11 implementation prints so a person moving
/// between platforms reads the same columns.
pub fn list_windows() -> String {
    let mut out = String::from("window#     pid     bounds(x,y,w,h)  class  title\n");
    for window in enumerate_windows() {
        let bounds = window_rect(window.window_id)
            .map(|r| {
                format!(
                    "({},{},{},{})",
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top
                )
            })
            .unwrap_or_else(|| "(?)".to_owned());
        out.push_str(&format!(
            "{:<11} {:<7} {:<16} {:<6} {}\n",
            window.window_id, window.pid, bounds, window.app_name, window.title
        ));
    }
    out
}

/// Recovers an `HWND` from the `u32` window id carried on the wire.
///
/// Window handles are pointer-sized but only ever use the low 32 bits, which is why the
/// cross-platform `Target::Window` can carry one in a `u32` at all. The cast back has to go
/// through `i32` so a handle with the high bit set sign-extends the way Win32 expects.
pub fn hwnd_from_id(window_id: u32) -> HWND {
    HWND(window_id as i32 as isize as *mut c_void)
}

/// The window's outer rectangle in screen pixels.
pub fn window_rect(window_id: u32) -> Option<RECT> {
    let mut rect = RECT::default();
    // SAFETY: `GetWindowRect` validates the handle and fails cleanly on a stale one.
    unsafe { GetWindowRect(hwnd_from_id(window_id), &mut rect) }
        .ok()
        .map(|()| rect)
}

/// Translates a window-local point (the coordinate space a window screenshot is in, with
/// `(0,0)` at the top-left of the window *rect*) into the client coordinates that mouse
/// messages carry.
///
/// The two differ by the frame: a window with a caption and borders has a client origin below
/// and inside its window rect. Getting this wrong is not a crash, it is every click landing a
/// title bar's height too high, which reads as "the coordinates are just wrong".
pub fn window_local_to_client(window_id: u32, x: i32, y: i32) -> Option<(i32, i32)> {
    let rect = window_rect(window_id)?;
    let mut origin = POINT::default();
    // SAFETY: valid handle, and `origin` is a local we own.
    if !unsafe { ClientToScreen(hwnd_from_id(window_id), &mut origin) }.as_bool() {
        return None;
    }
    Some((rect.left + x - origin.x, rect.top + y - origin.y))
}

/// The client area size, used to reject points that would be posted outside the window.
pub fn client_size(window_id: u32) -> Option<(i32, i32)> {
    let mut rect = RECT::default();
    // SAFETY: valid handle; `GetClientRect` fails cleanly on a stale one.
    unsafe { GetClientRect(hwnd_from_id(window_id), &mut rect) }
        .ok()
        .map(|()| (rect.right, rect.bottom))
}
