//! Fetching a URL without linking an HTTP stack into the binary.
//!
//! The whole point of this project is a small dynamically-linked executable,
//! and pulling in a TLS stack and an async runtime to read a few kilobytes
//! would undo a good part of that. Every machine that can install instaCache
//! already has curl or wget, so those do the work — Windows included, which
//! has shipped `curl.exe` in `System32` since Windows 10 1803.

use std::process::Command;

/// Long enough for a slow connection, short enough that a launch is never held
/// up noticeably by a server that has gone away.
pub const TIMEOUT_SECS: u64 = 15;

/// The bytes at `url`, or `None` if there is no downloader, the request fails,
/// or the response is empty.
///
/// Redirects are followed: both the release API and site icons live behind
/// them.
pub fn get(url: &str) -> Option<Vec<u8>> {
    let timeout = TIMEOUT_SECS.to_string();

    let mut command = if which("curl").is_some() {
        let mut command = Command::new("curl");
        command.args(["-fsSL", "--max-time", &timeout, url]);
        command
    } else if which("wget").is_some() {
        let mut command = Command::new("wget");
        command.args(["-qO-", "--timeout", &timeout, url]);
        command
    } else {
        return None;
    };
    #[cfg(windows)]
    crate::windows::no_console(&mut command);
    let output = command.output().ok()?;

    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }
    Some(output.stdout)
}

/// The same, decoded as text. Used for reading a page's markup.
pub fn get_text(url: &str) -> Option<String> {
    get(url).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// Where `program` is on `PATH`. On Windows a program is a file ending in
/// `.exe`, which nobody types and every caller here leaves out.
pub fn which(program: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    let file = if cfg!(windows) {
        format!("{program}.exe")
    } else {
        program.to_string()
    };
    std::env::split_paths(&path)
        .map(|dir| dir.join(&file))
        .find(|candidate| candidate.is_file())
}
