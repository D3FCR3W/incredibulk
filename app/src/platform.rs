//! The platform-specific corners.
//!
//! Everything here works on some systems and not others, so each entry point
//! degrades to a neutral answer rather than being cfg'd out at the call site.
//! Callers stay platform-agnostic; only the answers change.

#[cfg(target_os = "windows")]
use std::path::PathBuf;

/// A counter that changes whenever anything is put on the clipboard.
///
/// When available this makes polling nearly free: no clipboard read happens
/// unless the counter moved, which matters because reading a large image off
/// the clipboard several times a second is not cheap. Platforms without an
/// equivalent return `None` and the caller falls back to reading every tick.
#[cfg(target_os = "windows")]
pub fn clipboard_generation() -> Option<u64> {
    // SAFETY: no arguments, no pointers, and the call is documented as safe to
    // make from any thread without owning the clipboard.
    let n = unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() };
    Some(n as u64)
}

#[cfg(not(target_os = "windows"))]
pub fn clipboard_generation() -> Option<u64> {
    None
}

/// The list of files currently on the clipboard, if that is what it holds.
///
/// Copying files in Explorer puts a `CF_HDROP` handle on the clipboard and
/// usually no text at all, so without this a file copy looks like an empty
/// clipboard and the session silently ignores it.
#[cfg(target_os = "windows")]
pub fn clipboard_files() -> Option<Vec<PathBuf>> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::UI::Shell::DragQueryFileW;

    /// Standard clipboard format for a list of dropped files. Spelled out
    /// rather than pulled from a feature-gated constant module.
    const CF_HDROP: u32 = 15;
    /// Passing this index to DragQueryFileW asks for the count instead of a path.
    const COUNT_QUERY: u32 = 0xFFFF_FFFF;

    // SAFETY: the clipboard is opened and unconditionally closed on every path
    // out of this block. The HDROP is owned by the clipboard, so it is only
    // read, never freed, and only while the clipboard is held open. Each
    // buffer is sized from the length DragQueryFileW itself reports.
    unsafe {
        if IsClipboardFormatAvailable(CF_HDROP) == 0 {
            return None;
        }
        // Another application can hold the clipboard open for a moment.
        // Failing to open it is a reason to try again next tick, not an error.
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }

        let handle = GetClipboardData(CF_HDROP);
        if handle.is_null() {
            CloseClipboard();
            return None;
        }

        let count = DragQueryFileW(handle, COUNT_QUERY, std::ptr::null_mut(), 0);
        let mut paths = Vec::with_capacity(count as usize);
        for index in 0..count {
            let len = DragQueryFileW(handle, index, std::ptr::null_mut(), 0);
            if len == 0 {
                continue;
            }
            // The reported length excludes the terminator, which the call
            // still writes, so the buffer needs one more slot than the path.
            let mut buffer = vec![0u16; len as usize + 1];
            let written = DragQueryFileW(handle, index, buffer.as_mut_ptr(), buffer.len() as u32);
            if written == 0 {
                continue;
            }
            paths.push(PathBuf::from(OsString::from_wide(&buffer[..written as usize])));
        }

        CloseClipboard();
        if paths.is_empty() { None } else { Some(paths) }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn clipboard_files() -> Option<Vec<std::path::PathBuf>> {
    // macOS would read NSFilenamesPboardType, and Linux the text/uri-list
    // target. Neither is implemented, so a file copy is simply not seen there.
    None
}

/// The window that currently has the keyboard, so it can be handed back.
///
/// Showing the stack window activates it, because that is the only way to keep
/// the toolkit's own record of what is visible correct: it applies a flag
/// *diff*, so a window it believes is hidden can never be hidden again. Rather
/// than going behind its back, the window is shown normally and whatever the
/// user was working in is given the focus straight back.
#[cfg(target_os = "windows")]
pub fn foreground_window() -> Option<isize> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    // SAFETY: a read-only query taking no arguments.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() { None } else { Some(hwnd as isize) }
}

#[cfg(not(target_os = "windows"))]
pub fn foreground_window() -> Option<isize> {
    None
}

/// Give the keyboard back to a window that had it a moment ago.
#[cfg(target_os = "windows")]
pub fn restore_foreground(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsWindow, SetForegroundWindow};

    // SAFETY: the handle was the foreground window microseconds ago. It is
    // checked for validity first in case it closed in between, and both calls
    // tolerate a stale handle by failing rather than faulting.
    unsafe {
        let handle = hwnd as *mut core::ffi::c_void;
        if IsWindow(handle) != 0 {
            SetForegroundWindow(handle);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn restore_foreground(_hwnd: isize) {}

/// Title of the focused window, for the `{source}` token.
///
/// Best effort by design. A missing source renders as an empty string rather
/// than failing a capture.
#[cfg(target_os = "windows")]
pub fn foreground_title() -> Option<String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    };

    // SAFETY: every call below is a read-only query against a window handle
    // the OS just handed us. The buffer passed to GetWindowTextW is sized from
    // GetWindowTextLengthW plus room for the terminator.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let written = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if written <= 0 {
            return None;
        }
        let title = String::from_utf16_lossy(&buf[..written as usize]);
        Some(trim_title(&title))
    }
}

#[cfg(not(target_os = "windows"))]
pub fn foreground_title() -> Option<String> {
    // Reading the focused window is a per-desktop affair elsewhere: AXUIElement
    // behind an Accessibility grant on macOS, and on Linux it depends on the
    // compositor, with Wayland refusing outright. Left unimplemented rather
    // than half-implemented, so `{source}` is simply blank there.
    None
}

/// Keep the label short and single-line. Window titles run long and the HUD
/// shows them next to every row.
#[allow(dead_code)]
fn trim_title(raw: &str) -> String {
    const MAX: usize = 80;
    let flat = raw.replace(['\n', '\r', '\t'], " ");
    let flat = flat.trim();
    if flat.chars().count() <= MAX {
        return flat.to_string();
    }
    let head: String = flat.chars().take(MAX - 1).collect();
    format!("{head}\u{2026}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_flattened_and_capped() {
        assert_eq!(trim_title("  a\nb\tc  "), "a b c");
        let long = "x".repeat(200);
        assert_eq!(trim_title(&long).chars().count(), 80);
    }

    #[test]
    fn generation_is_stable_when_nothing_is_copied() {
        // Whatever the platform answers, two immediate reads must agree,
        // otherwise the watcher would treat every tick as a change.
        assert_eq!(clipboard_generation(), clipboard_generation());
    }

    #[test]
    fn reading_files_never_leaves_the_clipboard_open() {
        // The clipboard is a single global lock. If a query forgot to close
        // it, the second call here would fail and every other application on
        // the machine would start failing to copy. Both calls must behave the
        // same whether or not files happen to be on the clipboard right now.
        let first = clipboard_files();
        let second = clipboard_files();
        assert_eq!(first, second);
    }
}
