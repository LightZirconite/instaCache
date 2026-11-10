//! What instaCache needs from Windows itself.
//!
//! A handful of functions from libraries every Windows has, declared here
//! rather than pulled in through a binding crate, the same way `main.rs`
//! declares the two libc calls it needs on Linux. Everything that differs
//! between the two systems and is not a Qt matter goes through this file, so
//! there is one place to read to know what instaCache asks of Windows.

use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::process::Command;

#[link(name = "kernel32")]
extern "system" {
    fn AttachConsole(process: u32) -> i32;
    fn GetStdHandle(which: u32) -> *mut c_void;
}

#[link(name = "shell32")]
extern "system" {
    fn SetCurrentProcessExplicitAppUserModelID(id: *const u16) -> i32;
    fn SHQueryUserNotificationState(state: *mut i32) -> i32;
}

#[link(name = "winmm")]
extern "system" {
    fn PlaySoundW(sound: *const u16, module: *mut c_void, flags: u32) -> i32;
}

/// `AttachConsole`'s "the console of whoever started me".
const ATTACH_PARENT_PROCESS: u32 = u32::MAX;

/// `GetStdHandle`'s name for standard output, `(DWORD)-11`.
const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;

/// Starts a console program without opening a console window for it.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const SND_ASYNC: u32 = 0x0001;
const SND_NODEFAULT: u32 = 0x0002;
const SND_LOOP: u32 = 0x0008;
const SND_ALIAS: u32 = 0x0001_0000;

/// `SHQueryUserNotificationState` answers, for the states that mean "leave
/// the user alone": busy in a full-screen application, presenting, or in
/// quiet hours.
const QUNS_BUSY: i32 = 2;
const QUNS_RUNNING_D3D_FULL_SCREEN: i32 = 3;
const QUNS_PRESENTATION_MODE: i32 = 4;
const QUNS_QUIET_TIME: i32 = 6;

fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain(Some(0)).collect()
}

/// `instacache.exe` is a windowed program, so a terminal that starts it gets
/// no output back unless the program asks for that terminal's console. This
/// asks, which is what makes `instacache --help` print where it was typed. A
/// launch from the Start menu has no parent console, and nothing happens.
/// Output already sent somewhere — into a file, or a pipe the installer
/// reads — is left where it was sent.
pub fn attach_console() {
    // SAFETY: no pointers; failure only means there is no console to attach.
    unsafe {
        if GetStdHandle(STD_OUTPUT_HANDLE).is_null() {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

/// Keeps a console program run from the window — curl, reg, PowerShell —
/// from flashing a black console over it.
pub fn no_console(command: &mut Command) -> &mut Command {
    command.creation_flags(CREATE_NO_WINDOW)
}

/// Gives this process its own task bar identity. Without it every window of
/// `instacache.exe` is grouped under one button, whatever site it shows —
/// the Windows side of what `sites::window_class` does on Linux. Must run
/// before the first window exists.
pub fn set_app_id(id: &str) {
    let id = wide(id);
    // SAFETY: `id` is a NUL-terminated wide string that outlives the call.
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(id.as_ptr());
    }
}

/// Whether Windows says notifications should stay quiet: a full-screen game
/// or video, a presentation, or quiet hours. Windows already silences the
/// notifications themselves; this is for the sounds instaCache plays.
pub fn notifications_inhibited() -> bool {
    let mut state = 0i32;
    // SAFETY: `state` is a valid place for the one value written.
    let queried = unsafe { SHQueryUserNotificationState(&mut state) };
    queried == 0
        && matches!(
            state,
            QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE | QUNS_QUIET_TIME
        )
}

/// Plays a sound of the user's sound scheme by its event name, such as
/// `Notification.IM`, without waiting for it. A name the scheme does not
/// have plays nothing rather than the default beep.
pub fn play_sound(event: &str, repeat: bool) {
    let event = wide(event);
    let mut flags = SND_ALIAS | SND_ASYNC | SND_NODEFAULT;
    if repeat {
        flags |= SND_LOOP;
    }
    // SAFETY: `event` is NUL-terminated and Windows copies it before the
    // asynchronous playback returns.
    unsafe {
        PlaySoundW(event.as_ptr(), std::ptr::null_mut(), flags);
    }
}

/// Stops whatever `play_sound` started.
pub fn stop_sound() {
    // SAFETY: a null sound is the documented way to stop playback.
    unsafe {
        PlaySoundW(std::ptr::null(), std::ptr::null_mut(), 0);
    }
}
