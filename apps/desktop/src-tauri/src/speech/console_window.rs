//! Starting a helper program without a console window.
//!
//! This module owns the platform concept "a program of ours that has no window of its own". The
//! release application is a Windows GUI program, and a console program it starts (a speech worker)
//! would otherwise open a console window that flashes on screen. Nothing above this file knows
//! which platform needs the flag, and no capability (speech, OCR, retrieval) may ever contain a
//! `#[cfg]` because of it. The two blocks are paired and adjacent on purpose: Windows and macOS are
//! both mandatory targets.

use std::process::Command;

/// Asks Windows not to create a console window for the program. Not an error if it cannot.
#[cfg(windows)]
pub fn hide(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    /// `CREATE_NO_WINDOW` of the Win32 process creation flags.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

/// Nothing to ask: a macOS helper started from an application has no window unless it draws one.
#[cfg(not(windows))]
pub fn hide(_command: &mut Command) {}
