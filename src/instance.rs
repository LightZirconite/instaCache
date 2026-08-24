//! One window per profile.
//!
//! Launching instaCache twice for the same profile should focus the window
//! that already exists, not start a second browser process tree sharing one
//! cookie jar. GtkApplication used to provide this through its D-Bus name; Qt
//! has no equivalent, so it is done here with a Unix socket named after the
//! profile.
//!
//! The socket doubles as the channel for the URL the second launch was given,
//! which is what makes `instacache https://…` from a link handler open in the
//! running window.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

/// Where the socket for a profile lives. `XDG_RUNTIME_DIR` is the correct
/// place — it is per-user, cleaned up at logout, and not world-writable — with
/// a temporary-directory fallback for the sessions that do not set it.
pub fn socket_path(profile: &str) -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    base.join(format!("instacache-{profile}.sock"))
}

pub enum Claim {
    /// This process owns the profile. Poll the listener for later launches.
    Owner(UnixListener),
    /// Another process owns it and has been told about `url`.
    Handed,
}

/// Claims the profile, or hands `url` to whoever already holds it.
///
/// A socket left behind by a process that died is not a running instance: the
/// connection attempt fails, and the stale file is removed rather than being
/// treated as an owner, which would otherwise make the app unstartable until
/// somebody deleted it by hand.
pub fn claim(profile: &str, url: Option<&str>) -> std::io::Result<Claim> {
    let path = socket_path(profile);

    if let Ok(mut stream) = UnixStream::connect(&path) {
        let line = url.unwrap_or("");
        let _ = writeln!(stream, "{line}");
        let _ = stream.flush();
        return Ok(Claim::Handed);
    }

    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;
    Ok(Claim::Owner(listener))
}

/// Accepts every launch that has queued up since the last call.
///
/// Returns one entry per launch: `Some(url)` when it named one, `None` when it
/// only meant "focus the window". Never blocks, so it is safe to call from a
/// UI timer.
pub fn drain(listener: &UnixListener) -> Vec<Option<String>> {
    let mut launches = Vec::new();
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // A peer that connects and then stalls must not freeze the UI.
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
                let mut line = String::new();
                let _ = BufReader::new(stream).read_line(&mut line);
                let line = line.trim();
                launches.push((!line.is_empty()).then(|| line.to_string()));
            }
            Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }
    launches
}

/// Removes the socket. Called on the way out so the next launch does not have
/// to clean up after this one.
pub fn release(profile: &str) {
    let _ = std::fs::remove_file(socket_path(profile));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_socket_is_named_after_the_profile() {
        let a = socket_path("default");
        let b = socket_path("work");
        assert_ne!(a, b);
        assert!(a.to_string_lossy().contains("instacache-default"));
    }

    #[test]
    fn a_second_launch_is_handed_to_the_first() {
        let profile = format!("test-{}", std::process::id());
        let Ok(Claim::Owner(listener)) = claim(&profile, None) else {
            panic!("the first claim should own the profile");
        };

        assert!(matches!(
            claim(&profile, Some("https://example.invalid/x")),
            Ok(Claim::Handed)
        ));
        assert!(matches!(claim(&profile, None), Ok(Claim::Handed)));

        let launches = drain(&listener);
        assert_eq!(
            launches,
            vec![Some("https://example.invalid/x".to_string()), None]
        );

        release(&profile);
    }

    #[test]
    fn a_stale_socket_does_not_block_startup() {
        let profile = format!("stale-{}", std::process::id());
        let path = socket_path(&profile);
        std::fs::write(&path, b"not a live socket").unwrap();

        assert!(
            matches!(claim(&profile, None), Ok(Claim::Owner(_))),
            "a leftover file must not be mistaken for a running instance"
        );
        release(&profile);
    }
}
