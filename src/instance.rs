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
    /// This process owns the profile. [`listen`] hears later launches.
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
    Ok(Claim::Owner(listener))
}

/// Hears every later launch on a thread of its own, and hands each one to
/// `launched`: `Some(url)` when it named one, `None` when it only meant
/// "focus the window".
///
/// This used to be drained from the scene's timer, which therefore had to run
/// twelve times a second while the window was hidden just so a launch from
/// the menu would not wait. Blocking on the socket here costs nothing at all
/// until somebody launches.
pub fn listen(listener: UnixListener, launched: impl Fn(Option<String>) + Send + 'static) {
    let _ = listener.set_nonblocking(false);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => launched(read_launch(stream)),
                // A failed accept is about that one peer; the socket itself
                // is still listening. The pause keeps a socket that fails
                // every time from spinning a core.
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(200)),
            }
        }
    });
}

/// What one launch asked for. A peer that connects and then stalls gives up
/// after a moment rather than holding the next launch back.
fn read_launch(stream: UnixStream) -> Option<String> {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
    let mut line = String::new();
    let _ = BufReader::new(stream).read_line(&mut line);
    let line = line.trim();
    (!line.is_empty()).then(|| line.to_string())
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

        let (sender, launches) = std::sync::mpsc::channel();
        listen(listener, move |launch| {
            let _ = sender.send(launch);
        });
        let timeout = std::time::Duration::from_secs(5);
        assert_eq!(
            launches.recv_timeout(timeout).ok(),
            Some(Some("https://example.invalid/x".to_string()))
        );
        assert_eq!(launches.recv_timeout(timeout).ok(), Some(None));

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
