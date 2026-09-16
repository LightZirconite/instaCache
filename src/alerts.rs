//! Desktop notifications, the sounds that go with them, and the ringing of an
//! incoming call.
//!
//! A message gets one short sound, the way Discord or Telegram do it. Where
//! the notification server can play sounds itself — it says so with the
//! `sound` capability, as KDE Plasma does — the sound is only named in the
//! notification, and the server plays it and honours Do Not Disturb. Anywhere
//! else it is played here.
//!
//! A call rings: a sound played once is easy to miss, so it repeats until the
//! call is answered, the notification is clicked or dismissed, the window is
//! opened, or thirty seconds pass. Ringing is always played here, because no
//! notification server repeats a sound, and it checks Do Not Disturb first.
//!
//! Sounds are named from the freedesktop sound theme and played by whichever
//! player the system has, the same way `http.rs` uses curl rather than a
//! linked HTTP stack.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// What a notification is about, which decides how it sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// instaCache's own: a finished download, an update. Silent.
    Plain,
    /// Something a page sent, such as a new message.
    Message,
    /// An incoming call. Rings.
    Call,
}

/// A notification to show.
pub struct Toast {
    pub title: String,
    pub body: String,
    /// Whether a click should raise the window and reach the page.
    pub clickable: bool,
    pub kind: Kind,
    /// Whether a message may make a sound at all.
    pub sound: bool,
}

/// How the notifications of this profile identify themselves.
#[derive(Debug, Clone)]
pub struct Identity {
    pub app_name: String,
    pub icon: String,
    /// The desktop entry, without `.desktop`. Plasma and GNOME use it to file
    /// the notification under the right application and its settings.
    pub desktop_entry: String,
}

/// The channels to the notification threads.
pub struct Alerts {
    pub toasts: Sender<Toast>,
    ring: Sender<Ring>,
}

impl Alerts {
    pub fn stop_ringing(&self) {
        let _ = self.ring.send(Ring::Stop);
    }
}

enum Ring {
    Start,
    Stop,
}

/// The freedesktop sound theme names used.
const MESSAGE_SOUND: &str = "message-new-instant";
const CALL_SOUND: &str = "phone-incoming-call";

/// How long a call rings before giving up, the way a phone stops.
const RING_LIMIT: Duration = Duration::from_secs(30);
/// The silence between two rings.
const RING_PAUSE: Duration = Duration::from_millis(900);

/// Starts the threads and returns the channels to them, and the receiver that
/// hears about notifications being clicked.
pub fn spawn(identity: Identity) -> (Alerts, Receiver<()>) {
    let (toast_tx, toast_rx) = channel::<Toast>();
    let (ring_tx, ring_rx) = channel::<Ring>();
    let (activated_tx, activated_rx) = channel::<()>();

    std::thread::spawn(move || ring_loop(ring_rx));

    let ring = ring_tx.clone();
    std::thread::spawn(move || {
        // Asked once, and only when the first notification needs to know: a
        // D-Bus connection opened at startup lives for the whole session, and
        // instaCache opened none before a notification until this module.
        let mut server_plays_sounds: Option<bool> = None;

        while let Ok(toast) = toast_rx.recv() {
            let server_plays_sounds = *server_plays_sounds.get_or_insert_with(|| {
                notify_rust::get_capabilities()
                    .map(|caps| caps.iter().any(|cap| cap == "sound"))
                    .unwrap_or(false)
            });
            let identity = identity.clone();
            let activated = activated_tx.clone();
            let ring = ring.clone();
            // One thread per notification. Waiting for a click blocks, and a
            // call's notification stays up until somebody acts on it: on one
            // shared thread, every message after it would wait too.
            std::thread::spawn(move || {
                show(toast, &identity, server_plays_sounds, &activated, &ring)
            });
        }
    });

    (
        Alerts {
            toasts: toast_tx,
            ring: ring_tx,
        },
        activated_rx,
    )
}

fn show(
    toast: Toast,
    identity: &Identity,
    server_plays_sounds: bool,
    activated: &Sender<()>,
    ring: &Sender<Ring>,
) {
    use notify_rust::{Hint, Notification, Urgency};

    let mut notification = Notification::new();
    notification
        .summary(&toast.title)
        .body(&toast.body)
        .icon(&identity.icon)
        .appname(&identity.app_name)
        .hint(Hint::DesktopEntry(identity.desktop_entry.clone()));

    match toast.kind {
        Kind::Plain => {}
        Kind::Message => {
            notification.hint(Hint::Category("im.received".to_string()));
            if toast.sound {
                if server_plays_sounds {
                    notification.hint(Hint::SoundName(MESSAGE_SOUND.to_string()));
                } else if !notifications_inhibited() {
                    play_once(MESSAGE_SOUND);
                }
            }
        }
        Kind::Call => {
            // Stays on screen until acted on, like a phone's call screen.
            notification
                .urgency(Urgency::Critical)
                .hint(Hint::Resident(true))
                .hint(Hint::Category("call.incoming".to_string()));
            if toast.sound {
                let _ = ring.send(Ring::Start);
            }
        }
    }

    if toast.clickable {
        notification.action("default", "Open");
    }

    match notification.show() {
        Ok(handle) => {
            if toast.clickable || toast.kind == Kind::Call {
                handle.wait_for_action(|action| {
                    if action == "default" {
                        let _ = activated.send(());
                    }
                    // Clicked or dismissed, a call stops ringing either way.
                    if toast.kind == Kind::Call {
                        let _ = ring.send(Ring::Stop);
                    }
                });
            }
        }
        Err(error) => {
            eprintln!("instacache: could not post a notification: {error}");
            if toast.kind == Kind::Call {
                let _ = ring.send(Ring::Stop);
            }
        }
    }
}

fn ring_loop(commands: Receiver<Ring>) {
    while let Ok(command) = commands.recv() {
        if let Ring::Start = command {
            ring_until_stopped(&commands);
        }
    }
}

fn ring_until_stopped(commands: &Receiver<Ring>) {
    let deadline = Instant::now() + RING_LIMIT;
    while Instant::now() < deadline {
        if notifications_inhibited() {
            return;
        }
        let Some(mut player) = play(CALL_SOUND) else {
            return;
        };
        // Poll the player, so a stop cuts the ring short instead of waiting
        // for the sound to end.
        loop {
            match commands.recv_timeout(Duration::from_millis(100)) {
                Ok(Ring::Stop) | Err(RecvTimeoutError::Disconnected) => {
                    let _ = player.kill();
                    let _ = player.wait();
                    return;
                }
                Ok(Ring::Start) | Err(RecvTimeoutError::Timeout) => {}
            }
            if !matches!(player.try_wait(), Ok(None)) {
                break;
            }
        }
        match commands.recv_timeout(RING_PAUSE) {
            Ok(Ring::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(Ring::Start) | Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

/// Plays a sound and forgets about it, on a thread that reaps the player.
fn play_once(sound: &str) {
    if let Some(mut player) = play(sound) {
        std::thread::spawn(move || {
            let _ = player.wait();
        });
    }
}

fn play(sound: &str) -> Option<Child> {
    let (program, args) = player_command(sound, |program| which(program).is_some(), sound_file)?;
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

/// The command that plays `sound`, from what is installed.
///
/// `canberra-gtk-play` first, because it resolves the name through the user's
/// sound theme. Otherwise the freedesktop theme's file, through PipeWire or
/// PulseAudio.
fn player_command(
    sound: &str,
    installed: impl Fn(&str) -> bool,
    file_for: impl Fn(&str) -> Option<PathBuf>,
) -> Option<(&'static str, Vec<String>)> {
    if installed("canberra-gtk-play") {
        return Some((
            "canberra-gtk-play",
            vec!["-i".to_string(), sound.to_string()],
        ));
    }
    let file = file_for(sound)?.to_string_lossy().into_owned();
    ["pw-play", "paplay"]
        .into_iter()
        .find(|player| installed(player))
        .map(|player| (player, vec![file]))
}

/// `sounds/freedesktop/stereo/<name>.oga` in the XDG data directories.
fn sound_file(sound: &str) -> Option<PathBuf> {
    let dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
    std::env::split_paths(&dirs)
        .map(|dir| dir.join(format!("sounds/freedesktop/stereo/{sound}.oga")))
        .find(|candidate| candidate.is_file())
}

/// Whether the desktop is in Do Not Disturb, through the notification
/// server's `Inhibited` property. A server that does not have one is not.
fn notifications_inhibited() -> bool {
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(proxy) = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    ) else {
        return false;
    };
    proxy.get_property::<bool>("Inhibited").unwrap_or(false)
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// Phrases an incoming call's notification uses, lower-cased, in the
/// languages Instagram is most used in.
///
/// Instagram publishes no marker for a call notification, so this is read
/// from the words. It errs towards not ringing: a phrase has to say that
/// somebody *is calling*, which a message rarely does.
const CALL_PHRASES: &[&str] = &[
    "is calling you",
    "calling you…",
    "calling you...",
    "incoming call",
    "incoming video call",
    "incoming audio call",
    "vous appelle",
    "t'appelle",
    "t’appelle",
    "appel entrant",
    "appel vidéo entrant",
    "appel audio entrant",
    "te está llamando",
    "llamada entrante",
    "ruft dich an",
    "eingehender anruf",
    "está te ligando",
    "chamada recebida",
    "ti sta chiamando",
    "chiamata in arrivo",
];

/// What a page's notification is.
pub fn classify(title: &str, body: &str, ring_for_calls: bool) -> Kind {
    if !ring_for_calls {
        return Kind::Message;
    }
    let text = format!("{title} {body}").to_lowercase();
    if CALL_PHRASES.iter().any(|phrase| text.contains(phrase)) {
        Kind::Call
    } else {
        Kind::Message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_is_told_from_a_message() {
        assert_eq!(classify("someone", "is calling you", true), Kind::Call);
        assert_eq!(
            classify("Instagram", "someone vous appelle", true),
            Kind::Call
        );
        assert_eq!(classify("Incoming video call", "someone", true), Kind::Call);
        assert_eq!(
            classify("someone", "sent you a message", true),
            Kind::Message
        );
        assert_eq!(
            classify("someone", "haha I'll call you later", true),
            Kind::Message
        );
        assert_eq!(classify("someone", "appel ce soir ?", true), Kind::Message);
    }

    #[test]
    fn a_site_that_does_not_ring_never_rings() {
        assert_eq!(classify("someone", "is calling you", false), Kind::Message);
    }

    #[test]
    fn the_sound_theme_is_preferred_then_the_file() {
        let file = |_: &str| Some(PathBuf::from("/s/phone-incoming-call.oga"));
        let everything = |_: &str| true;
        assert_eq!(
            player_command(CALL_SOUND, everything, file).unwrap().0,
            "canberra-gtk-play"
        );

        let pipewire_only = |program: &str| program == "pw-play";
        assert_eq!(
            player_command(CALL_SOUND, pipewire_only, file),
            Some(("pw-play", vec!["/s/phone-incoming-call.oga".to_string()]))
        );

        let nothing = |_: &str| false;
        assert_eq!(player_command(CALL_SOUND, nothing, file), None);
        assert_eq!(
            player_command(CALL_SOUND, pipewire_only, |_: &str| None),
            None
        );
    }
}
