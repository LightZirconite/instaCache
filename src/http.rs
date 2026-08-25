//! Fetching a URL without linking an HTTP stack into the binary.
//!
//! The whole point of this project is a small dynamically-linked executable,
//! and pulling in a TLS stack and an async runtime to read a few kilobytes
//! would undo a good part of that. Every machine that can install instaCache
//! already has curl or wget, so those do the work.

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

    let output = if which("curl").is_some() {
        Command::new("curl")
            .args(["-fsSL", "--max-time", &timeout, url])
            .output()
            .ok()?
    } else if which("wget").is_some() {
        Command::new("wget")
            .args(["-qO-", "--timeout", &timeout, url])
            .output()
            .ok()?
    } else {
        return None;
    };

    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }
    Some(output.stdout)
}

/// The same, decoded as text. Used for reading a page's markup.
pub fn get_text(url: &str) -> Option<String> {
    get(url).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

pub fn which(program: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}
