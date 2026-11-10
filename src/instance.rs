//! One window per profile.
//!
//! Launching instaCache twice for the same profile should focus the window
//! that already exists, not start a second browser process tree sharing one
//! cookie jar. GtkApplication used to provide this through its D-Bus name; Qt
//! has no equivalent, so it is done here with a Unix socket named after the
//! profile.
//!
//! Windows has no Unix socket in the standard library, so there the owner
//! listens on a loopback TCP port and writes the port, with a secret, into a
//! file in the user's own temporary directory. Any process on the machine can
//! reach a loopback port; only this user can read the file, and a connection
//! that does not open with the secret is ignored. The owner answers a good
//! one, so a port that was freed and reused by something else is never
//! mistaken for a running instaCache.
//!
//! The socket doubles as the channel for the URL the second launch was given,
//! which is what makes `instacache https://…` from a link handler open in the
//! running window.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

/// What the owner of a profile listens on.
#[cfg(unix)]
pub type Listener = UnixListener;

/// What the owner of a profile listens on, and the secret a launch must open
/// with.
#[cfg(windows)]
pub struct Listener {
    socket: std::net::TcpListener,
    secret: String,
}

/// Where the socket for a profile lives — on Windows, the file that says
/// where it is. `XDG_RUNTIME_DIR` is the correct place — it is per-user,
/// cleaned up at logout, and not world-writable — with a temporary-directory
/// fallback for the sessions that do not set it, which on Windows is the
/// user's own `%TEMP%`.
pub fn socket_path(profile: &str) -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    base.join(format!("instacache-{profile}.sock"))
}

pub enum Claim {
    /// This process owns the profile. [`listen`] hears later launches.
    Owner(Listener),
    /// Another process owns it and has been told about `url`.
    Handed,
}

/// Claims the profile, or hands `url` to whoever already holds it.
///
/// A socket left behind by a process that died is not a running instance: the
/// connection attempt fails, and the stale file is removed rather than being
/// treated as an owner, which would otherwise make the app unstartable until
/// somebody deleted it by hand.
#[cfg(unix)]
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

/// What the owner answers a launch that knew the secret, on Windows.
#[cfg(windows)]
const ACKNOWLEDGED: &str = "instacache";

#[cfg(windows)]
pub fn claim(profile: &str, url: Option<&str>) -> std::io::Result<Claim> {
    let path = socket_path(profile);

    if let Some((port, secret)) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| parse_address(&text))
    {
        if hand_over(port, &secret, url.unwrap_or("")) {
            return Ok(Claim::Handed);
        }
    }

    let socket = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    let port = socket.local_addr()?.port();
    let secret = new_secret();
    std::fs::write(&path, format!("{port} {secret}\n"))?;
    Ok(Claim::Owner(Listener { socket, secret }))
}

/// `"<port> <secret>"`, as the owner wrote it.
#[cfg(windows)]
fn parse_address(text: &str) -> Option<(u16, String)> {
    let mut words = text.split_whitespace();
    let port = words.next()?.parse().ok()?;
    let secret = words.next()?.to_string();
    Some((port, secret))
}

/// Tells the owner on `port` about a launch, and whether it was really
/// instaCache that heard it.
#[cfg(windows)]
fn hand_over(port: u16, secret: &str, line: &str) -> bool {
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let timeout = std::time::Duration::from_millis(500);
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&address, timeout) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(timeout));
    if writeln!(stream, "{secret}\n{line}").is_err() || stream.flush().is_err() {
        return false;
    }
    let mut answer = String::new();
    let _ = BufReader::new(stream).read_line(&mut answer);
    answer.trim() == ACKNOWLEDGED
}

/// 128 bits nobody else can guess, from the keys the standard library seeds
/// its hash maps with — random per process, from the operating system.
#[cfg(windows)]
fn new_secret() -> String {
    use std::hash::{BuildHasher, Hasher};
    let half = || {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0),
        );
        hasher.finish()
    };
    format!("{:016x}{:016x}", half(), half())
}

/// Hears every later launch on a thread of its own, and hands each one to
/// `launched`: `Some(url)` when it named one, `None` when it only meant
/// "focus the window".
///
/// This used to be drained from the scene's timer, which therefore had to run
/// twelve times a second while the window was hidden just so a launch from
/// the menu would not wait. Blocking on the socket here costs nothing at all
/// until somebody launches.
pub fn listen(listener: Listener, launched: impl Fn(Option<String>) + Send + 'static) {
    #[cfg(unix)]
    let socket = listener;
    #[cfg(windows)]
    let Listener { socket, secret } = listener;
    let _ = socket.set_nonblocking(false);
    std::thread::spawn(move || {
        for stream in socket.incoming() {
            match stream {
                #[cfg(unix)]
                Ok(stream) => launched(read_launch(stream)),
                #[cfg(windows)]
                Ok(stream) => {
                    if let Some(launch) = read_launch(stream, &secret) {
                        launched(launch);
                    }
                }
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
#[cfg(unix)]
fn read_launch(stream: UnixStream) -> Option<String> {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
    let mut line = String::new();
    let _ = BufReader::new(stream).read_line(&mut line);
    let line = line.trim();
    (!line.is_empty()).then(|| line.to_string())
}

/// A launch, if the peer knew the secret: its first line is the secret, its
/// second the URL. Anything else is dropped without an answer.
#[cfg(windows)]
fn read_launch(stream: std::net::TcpStream, secret: &str) -> Option<Option<String>> {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut given = String::new();
    reader.read_line(&mut given).ok()?;
    if given.trim() != secret {
        return None;
    }
    let mut line = String::new();
    let _ = reader.read_line(&mut line);
    let mut stream = stream;
    let _ = writeln!(stream, "{ACKNOWLEDGED}");
    let line = line.trim();
    Some((!line.is_empty()).then(|| line.to_string()))
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

        let (sender, launches) = std::sync::mpsc::channel();
        listen(listener, move |launch| {
            let _ = sender.send(launch);
        });

        assert!(matches!(
            claim(&profile, Some("https://example.invalid/x")),
            Ok(Claim::Handed)
        ));
        assert!(matches!(claim(&profile, None), Ok(Claim::Handed)));

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
