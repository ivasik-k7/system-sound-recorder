//! Starting child processes without putting a window on the screen.
//!
//! Speech engines are console programs. Windows gives a console program a
//! console, and if the parent does not say otherwise that console is a black
//! window that appears, steals focus and vanishes again. Doing that once per
//! utterance makes the machine unusable while a meeting is being transcribed.
//!
//! The flag has to be set by whoever creates the process. There is no way to
//! suppress it from outside, and the usual workarounds — a `.vbs` wrapper, a
//! detached `conhost` — break the pipes the output is read from.

use std::process::Command;

/// `CREATE_NO_WINDOW`: run the child with no console of its own.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Suppress the console window a child process would otherwise be given.
///
/// Does nothing anywhere but Windows, where the problem does not exist.
pub fn without_window(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}
