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
//! call is answered, the notification is clicked or dismissed, the page
//! withdraws it, the window is opened, or thirty seconds pass. Ringing is always played here, because no notification
//! server repeats a sound, and it checks Do Not Disturb first.
//!
//! Everything goes over the one session bus connection in `bus.rs`, through
//! one thread that owns every notification's state, and one more that listens
//! for what the server says about them. A notification no longer holds a
//! thread and a connection of its own while it waits to be clicked; on a
//! desktop that keeps notifications in a history, those used to pile up all
//! day. And the library that did it unwrapped D-Bus errors, which with
//! `panic = "abort"` took the whole application down with a hiccup on the bus.
//!
//! Sounds are named from the freedesktop sound theme and played by whichever
//! player the system has, the same way `http.rs` uses curl rather than a
//! linked HTTP stack.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::Value;

use crate::bus;

/// What a notification is about, which decides how it sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// instaCache's own: a finished download, an update. Silent.
    Plain,
    /// Something a page sent, such as a new message.
    Message,
    /// Says only that something arrived, and not what or from whom:
    /// Instagram's "You have unseen notifications.", or instaCache's own
    /// "New message" read off the unread count. Held back for a moment and
    /// dropped if a notification that says more arrives — see `Coalescer`.
    Nudge,
    /// The page took a message back: its notification is closed, and nothing
    /// is shown in its place.
    Unsent,
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
    /// The web notification's own `tag`, or empty.
    ///
    /// The Notification API says two notifications sharing a tag are the same
    /// notification, and the second replaces the first. Instagram uses it the
    /// way a phone does: a ringing call is re-posted over and over under one
    /// tag. Ignoring it is what turned a single call into a screen full of
    /// notifications.
    pub tag: String,
    /// The scene's name for the page notification behind this one, so a click
    /// reaches the notification that was clicked rather than the latest one.
    /// `0` for instaCache's own notifications, which have no page behind them.
    pub key: u32,
}

/// What happened to a notification, for the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Clicked. The window comes to the front, and the page behind it, if
    /// any, is told — which is what makes Instagram open the right thread.
    Clicked(u32),
    /// Dismissed, or expired, on the desktop. The page is told it closed.
    Closed(u32),
    /// Replaced by a later notification under the same tag, or too old to
    /// keep track of. The scene lets go of it without telling the page
    /// anything, because nothing happened to it.
    Forgotten(u32),
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

/// What the notification thread can be asked for.
enum Request {
    /// Show this, and sound it if it asks for a sound.
    Show(Toast),
    /// The sound a message makes, and nothing on screen.
    ///
    /// For a message that arrived in a thread the user is not reading while
    /// the window is in front: they can see it there, so a notification over
    /// the top says nothing, but a messenger is still expected to be heard.
    Ping,
    /// The page closed its notification. It leaves the desktop too, and a
    /// call stops ringing.
    Withdraw(u32),
    /// What the notification server said.
    Server(Signal),
    /// The listener lost the bus.
    BusLost,
}

/// A signal from the notification server.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Signal {
    ActionInvoked { id: u32, action: String },
    Closed { id: u32 },
}

/// The channels to the notification threads.
pub struct Alerts {
    commands: Sender<Request>,
    ring: Sender<Ring>,
}

impl Alerts {
    pub fn show(&self, toast: Toast) {
        let _ = self.commands.send(Request::Show(toast));
    }

    pub fn ping(&self) {
        let _ = self.commands.send(Request::Ping);
    }

    pub fn withdraw(&self, key: u32) {
        let _ = self.commands.send(Request::Withdraw(key));
    }

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

/// How long a nudge waits for the notification that says more, before it is
/// shown on its own.
const NUDGE_GRACE: Duration = Duration::from_secs(4);
/// How close a nudge and a detailed notification must be to count as one
/// arrival told twice.
const NUDGE_ECHO: Duration = Duration::from_secs(20);
/// Every nudge is posted under this one tag, so a second replaces the first
/// rather than piling up behind it.
const NUDGE_TAG: &str = "instacache-nudge";

/// How many notifications are kept track of. A server that keeps them in a
/// history may never say they closed; past this, the oldest is let go of. It
/// can still be clicked, and then only raises the window.
const TRACKED: usize = 64;

const DESTINATION: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";

/// Starts the threads and returns the channels to them. `report` hears what
/// happened to a notification, on the notification thread.
pub fn spawn(identity: Identity, report: impl Fn(Outcome) + Send + 'static) -> Alerts {
    let (command_tx, command_rx) = channel::<Request>();
    let (ring_tx, ring_rx) = channel::<Ring>();

    std::thread::spawn(move || ring_loop(ring_rx));

    let ring = ring_tx.clone();
    let commands = command_tx.clone();
    std::thread::spawn(move || {
        Notifier {
            identity,
            report: Box::new(report),
            ring,
            commands,
            registry: Registry::default(),
            coalescer: Coalescer::default(),
            server: None,
        }
        .run(command_rx)
    });

    Alerts {
        commands: command_tx,
        ring: ring_tx,
    }
}

/// What the server can do, asked once per connection.
struct Server {
    connection: Connection,
    plays_sounds: bool,
    /// Whether the body is read as markup, in which case `<3` has to be
    /// written `&lt;3` or the server drops it, and the rest of the line with
    /// it.
    markup: bool,
}

struct Notifier {
    identity: Identity,
    report: Box<dyn Fn(Outcome) + Send>,
    ring: Sender<Ring>,
    /// Handed to the listener, which feeds the server's signals back in.
    commands: Sender<Request>,
    registry: Registry,
    coalescer: Coalescer,
    server: Option<Server>,
}

impl Notifier {
    fn run(mut self, commands: Receiver<Request>) {
        loop {
            // A held nudge sets how long to wait: when it is due, it is shown.
            let command = match self.coalescer.due() {
                Some(due) => {
                    match commands.recv_timeout(due.saturating_duration_since(Instant::now())) {
                        Ok(command) => command,
                        Err(RecvTimeoutError::Timeout) => {
                            if let Some(toast) = self.coalescer.release(Instant::now()) {
                                if let Some(id) = self.post(toast) {
                                    self.coalescer.nudge_shown(id, Instant::now());
                                }
                            }
                            continue;
                        }
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                None => match commands.recv() {
                    Ok(command) => command,
                    Err(_) => return,
                },
            };
            match command {
                Request::Show(toast) => self.show(toast),
                Request::Ping => {
                    if !notifications_inhibited() {
                        play_once(MESSAGE_SOUND);
                    }
                }
                Request::Withdraw(key) => self.withdraw(key),
                Request::Server(signal) => self.server_said(signal),
                Request::BusLost => {
                    self.server = None;
                    bus::forget();
                }
            }
        }
    }

    /// The connection, with the listener already subscribed: it has to hear
    /// about a notification before that notification exists, or a quick
    /// click is lost.
    fn server(&mut self) -> Option<&Server> {
        if self.server.is_none() {
            let connection = bus::session()?;
            let rule = zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender(DESTINATION)
                .and_then(|rule| rule.interface(INTERFACE))
                .and_then(|rule| rule.path(PATH))
                .map(|rule| rule.build());
            let signals = match rule
                .and_then(|rule| MessageIterator::for_match_rule(rule, &connection, None))
            {
                Ok(signals) => signals,
                Err(error) => {
                    eprintln!("instacache: cannot listen to the notification server: {error}");
                    return None;
                }
            };
            let commands = self.commands.clone();
            std::thread::spawn(move || listen(signals, commands));

            let capabilities: Vec<String> = connection
                .call_method(
                    Some(DESTINATION),
                    PATH,
                    Some(INTERFACE),
                    "GetCapabilities",
                    &(),
                )
                .and_then(|reply| reply.body().deserialize())
                .unwrap_or_default();
            self.server = Some(Server {
                connection,
                plays_sounds: capabilities.iter().any(|cap| cap == "sound"),
                markup: capabilities.iter().any(|cap| cap == "body-markup"),
            });
        }
        self.server.as_ref()
    }

    /// Decides what a notification becomes — shown now, held as a nudge,
    /// dropped as an echo, or a withdrawal — and posts it if it is shown.
    fn show(&mut self, mut toast: Toast) {
        let now = Instant::now();
        match toast.kind {
            Kind::Unsent => {
                // Instagram re-posts an unsent message's notification under
                // the same tag to say it is gone. Gone is what the desktop
                // should show: nothing.
                if let Some(id) = self.registry.showing(&toast.tag) {
                    self.retract(id);
                }
                self.forget(toast.key);
                return;
            }
            Kind::Nudge => {
                toast.tag = NUDGE_TAG.to_string();
                for key in self.coalescer.nudge(toast, now) {
                    self.forget(key);
                }
                return;
            }
            Kind::Message | Kind::Call => {
                let (dropped, shown_nudge) = self.coalescer.detailed(now);
                for key in dropped {
                    self.forget(key);
                }
                if let Some(id) = shown_nudge {
                    self.retract(id);
                }
            }
            Kind::Plain => {}
        }
        self.post(toast);
    }

    /// Puts a notification on the desktop, and returns the id the server
    /// gave it.
    fn post(&mut self, toast: Toast) -> Option<u32> {
        // A repeat under a tag we are already showing replaces that one.
        // Without this a ringing call, which Instagram re-posts every few
        // seconds, fills the screen with copies of itself.
        let replaces = self.registry.showing(&toast.tag).unwrap_or(0);
        if self.server().is_none() {
            self.failed(&toast);
            return None;
        }
        let server = self.server.as_ref()?;

        let mut hints: HashMap<&str, Value> = HashMap::new();
        hints.insert(
            "desktop-entry",
            Value::from(self.identity.desktop_entry.as_str()),
        );
        match toast.kind {
            Kind::Plain | Kind::Unsent => {}
            Kind::Message | Kind::Nudge => {
                hints.insert("category", Value::from("im.received"));
                if toast.sound {
                    if server.plays_sounds {
                        hints.insert("sound-name", Value::from(MESSAGE_SOUND));
                    } else if !notifications_inhibited() {
                        play_once(MESSAGE_SOUND);
                    }
                }
            }
            Kind::Call => {
                // Stays on screen until acted on, like a phone's call screen.
                hints.insert("urgency", Value::from(2u8));
                hints.insert("resident", Value::from(true));
                hints.insert("category", Value::from("call.incoming"));
                if toast.sound {
                    let _ = self.ring.send(Ring::Start);
                }
            }
        }
        let actions: &[&str] = if toast.clickable {
            &["default", "Open"]
        } else {
            &[]
        };
        let body = if server.markup {
            escape_markup(&toast.body)
        } else {
            toast.body.clone()
        };

        let sent = server
            .connection
            .call_method(
                Some(DESTINATION),
                PATH,
                Some(INTERFACE),
                "Notify",
                &(
                    self.identity.app_name.as_str(),
                    replaces,
                    self.identity.icon.as_str(),
                    toast.title.as_str(),
                    body.as_str(),
                    actions,
                    hints,
                    -1i32,
                ),
            )
            .and_then(|reply| reply.body().deserialize::<u32>());
        match sent {
            Ok(id) => {
                for key in self.registry.record(
                    id,
                    Entry {
                        key: toast.key,
                        kind: toast.kind,
                        tag: toast.tag,
                    },
                ) {
                    (self.report)(Outcome::Forgotten(key));
                }
                Some(id)
            }
            Err(error) => {
                eprintln!("instacache: could not post a notification: {error}");
                self.failed(&toast);
                None
            }
        }
    }

    /// Takes one of our notifications off the desktop because a later one
    /// made it pointless. The scene lets go of the page notification behind
    /// it, which was neither clicked nor dismissed.
    fn retract(&mut self, id: u32) {
        let Some(entry) = self.registry.take(id) else {
            return;
        };
        if let Some(server) = self.server.as_ref() {
            let _ = server.connection.call_method(
                Some(DESTINATION),
                PATH,
                Some(INTERFACE),
                "CloseNotification",
                &(id),
            );
        }
        self.forget(entry.key);
    }

    /// A page notification that will never be shown or will never be heard
    /// of again: the scene can let go of it.
    fn forget(&self, key: u32) {
        if key != 0 {
            (self.report)(Outcome::Forgotten(key));
        }
    }

    /// A notification that never reached the desktop: nothing will ever be
    /// said about it, so the scene lets go of it now, and it does not ring.
    fn failed(&self, toast: &Toast) {
        if toast.kind == Kind::Call {
            let _ = self.ring.send(Ring::Stop);
        }
        if toast.key != 0 {
            (self.report)(Outcome::Forgotten(toast.key));
        }
    }

    fn withdraw(&mut self, key: u32) {
        let Some((id, entry)) = self.registry.take_key(key) else {
            return;
        };
        if entry.kind == Kind::Call {
            let _ = self.ring.send(Ring::Stop);
        }
        if let Some(server) = self.server.as_ref() {
            let _ = server.connection.call_method(
                Some(DESTINATION),
                PATH,
                Some(INTERFACE),
                "CloseNotification",
                &(id),
            );
        }
    }

    fn server_said(&mut self, signal: Signal) {
        match signal {
            Signal::ActionInvoked { id, action } => {
                // The bus carries every application's notifications; an id
                // that is not ours is somebody else's click.
                let Some(entry) = self.registry.get(id) else {
                    return;
                };
                if entry.kind == Kind::Call {
                    let _ = self.ring.send(Ring::Stop);
                }
                if action == "default" {
                    (self.report)(Outcome::Clicked(entry.key));
                }
            }
            Signal::Closed { id } => {
                let Some(entry) = self.registry.take(id) else {
                    return;
                };
                if entry.kind == Kind::Call {
                    let _ = self.ring.send(Ring::Stop);
                }
                if entry.key != 0 {
                    (self.report)(Outcome::Closed(entry.key));
                }
            }
        }
    }
}

/// Feeds the server's signals to the notification thread until the bus goes.
fn listen(signals: MessageIterator, commands: Sender<Request>) {
    for message in signals {
        let Ok(message) = message else {
            continue;
        };
        let header = message.header();
        let signal = match header.member().map(|member| member.as_str()) {
            Some("ActionInvoked") => message
                .body()
                .deserialize::<(u32, String)>()
                .ok()
                .map(|(id, action)| Signal::ActionInvoked { id, action }),
            Some("NotificationClosed") => message
                .body()
                .deserialize::<(u32, u32)>()
                .ok()
                .map(|(id, _reason)| Signal::Closed { id }),
            _ => None,
        };
        if let Some(signal) = signal {
            if commands.send(Request::Server(signal)).is_err() {
                return;
            }
        }
    }
    let _ = commands.send(Request::BusLost);
}

/// One notification on the desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    key: u32,
    kind: Kind,
    tag: String,
}

/// Which of the desktop's notifications are ours, and what each one is.
///
/// Kept apart from D-Bus so the bookkeeping can be tested: every key the
/// scene hands over must eventually be reported closed or forgotten, or the
/// scene holds on to its page notification for ever.
#[derive(Default)]
struct Registry {
    by_id: HashMap<u32, Entry>,
    /// Ids, oldest first, for letting go of the oldest.
    order: VecDeque<u32>,
}

impl Registry {
    /// The id of the notification showing under `tag`, if any.
    fn showing(&self, tag: &str) -> Option<u32> {
        if tag.is_empty() {
            return None;
        }
        self.by_id
            .iter()
            .find(|(_, entry)| entry.tag == tag)
            .map(|(id, _)| *id)
    }

    fn get(&self, id: u32) -> Option<&Entry> {
        self.by_id.get(&id)
    }

    /// Records what the server answered `id` for, and returns the keys the
    /// scene can let go of: the notification this replaced, and the oldest
    /// one once there are too many to keep.
    fn record(&mut self, id: u32, entry: Entry) -> Vec<u32> {
        let mut forgotten = Vec::new();
        // Under the same tag, the old notification is superseded whether the
        // server kept its id or gave the new one another.
        if let Some(old) = self.showing(&entry.tag).filter(|old| *old != id) {
            if let Some(old) = self.take(old) {
                forgotten.push(old.key);
            }
        }
        if let Some(old) = self.take(id) {
            forgotten.push(old.key);
        }
        self.by_id.insert(id, entry);
        self.order.push_back(id);
        while self.order.len() > TRACKED {
            if let Some(oldest) = self.order.pop_front() {
                if let Some(old) = self.by_id.remove(&oldest) {
                    forgotten.push(old.key);
                }
            }
        }
        forgotten.retain(|key| *key != 0);
        forgotten
    }

    fn take(&mut self, id: u32) -> Option<Entry> {
        let entry = self.by_id.remove(&id)?;
        self.order.retain(|other| *other != id);
        Some(entry)
    }

    fn take_key(&mut self, key: u32) -> Option<(u32, Entry)> {
        if key == 0 {
            return None;
        }
        let id = self
            .by_id
            .iter()
            .find(|(_, entry)| entry.key == key)
            .map(|(id, _)| *id)?;
        self.take(id).map(|entry| (id, entry))
    }
}

/// One arrival, told once.
///
/// Instagram often says the same thing twice: "You have unseen
/// notifications." beside the message itself, and instaCache's count of
/// unread messages can rise just before Instagram's push lands. Both are
/// nudges — they say something arrived and no more — and the notification
/// that names the sender is the one worth keeping. So a nudge waits
/// `NUDGE_GRACE` for that notification before it is shown; one that comes
/// within `NUDGE_ECHO` after it is dropped; and a nudge already on screen is
/// taken off when the detailed one follows it.
///
/// Kept apart from D-Bus and the clock, so the rules can be tested.
#[derive(Default)]
struct Coalescer {
    /// The nudge waiting to be shown, and when.
    held: Option<(Toast, Instant)>,
    /// When the last detailed notification was shown.
    detailed_at: Option<Instant>,
    /// The nudge on screen, by its desktop id, and when it went up.
    shown: Option<(u32, Instant)>,
}

impl Coalescer {
    /// When the held nudge is due, if one is held.
    fn due(&self) -> Option<Instant> {
        self.held.as_ref().map(|(_, due)| *due)
    }

    /// A nudge arrived. Returns the keys of the page notifications that will
    /// never be shown: this one, if it is an echo, or the one it replaces.
    fn nudge(&mut self, toast: Toast, now: Instant) -> Vec<u32> {
        if self
            .detailed_at
            .is_some_and(|at| now.saturating_duration_since(at) < NUDGE_ECHO)
        {
            return vec![toast.key];
        }
        let replaced = self.held.replace((toast, now + NUDGE_GRACE));
        replaced.map(|(old, _)| old.key).into_iter().collect()
    }

    /// A detailed notification is about to be shown. Returns the keys of the
    /// held nudge it makes pointless, and the desktop id of a nudge on screen
    /// to take off.
    fn detailed(&mut self, now: Instant) -> (Vec<u32>, Option<u32>) {
        self.detailed_at = Some(now);
        let dropped = self.held.take().map(|(old, _)| old.key);
        let shown = self
            .shown
            .take()
            .filter(|(_, at)| now.saturating_duration_since(*at) < NUDGE_ECHO)
            .map(|(id, _)| id);
        (dropped.into_iter().collect(), shown)
    }

    /// The held nudge, if its time has come.
    fn release(&mut self, now: Instant) -> Option<Toast> {
        if self.due().is_some_and(|due| due <= now) {
            self.held.take().map(|(toast, _)| toast)
        } else {
            None
        }
    }

    fn nudge_shown(&mut self, id: u32, now: Instant) {
        self.shown = Some((id, now));
    }
}

/// The body as markup that says exactly what the text said. The spec allows a
/// handful of tags and entities; a server that reads them would take `<3`
/// for the start of a tag.
fn escape_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
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
    let Some(connection) = bus::session() else {
        return false;
    };
    connection
        .call_method(
            Some(DESTINATION),
            PATH,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(INTERFACE, "Inhibited"),
        )
        .ok()
        .and_then(|reply| {
            reply
                .body()
                .deserialize::<zbus::zvariant::OwnedValue>()
                .ok()
        })
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or(false)
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

/// What Instagram posts when it says only that something is waiting.
///
/// Captured from a real account, not guessed, like everything in these
/// lists; another language is added once its words have been seen. See
/// "Instagram does not say "call" in any language you guessed" in AGENTS.md.
const NUDGE_PHRASES: &[&str] = &["you have unseen notifications"];

/// What Instagram posts, under the message's own tag, when its sender unsent
/// it. Captured from a real account.
const UNSENT_PHRASES: &[&str] = &["was unsent by the sender"];

/// What a page's notification is.
pub fn classify(title: &str, body: &str, ring_for_calls: bool) -> Kind {
    let text = format!("{title} {body}").to_lowercase();
    let says = |phrases: &[&str]| phrases.iter().any(|phrase| text.contains(phrase));
    if says(UNSENT_PHRASES) {
        Kind::Unsent
    } else if says(NUDGE_PHRASES) {
        Kind::Nudge
    } else if ring_for_calls && says(CALL_PHRASES) {
        Kind::Call
    } else {
        Kind::Message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: u32, tag: &str) -> Entry {
        Entry {
            key,
            kind: Kind::Message,
            tag: tag.to_string(),
        }
    }

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

    #[test]
    fn a_repeat_under_a_tag_replaces_and_lets_go_of_the_old_one() {
        let mut registry = Registry::default();
        assert_eq!(registry.record(7, entry(1, "call")), Vec::<u32>::new());
        assert_eq!(registry.showing("call"), Some(7));
        // The server kept the id, as it should for `replaces_id`.
        assert_eq!(registry.record(7, entry(2, "call")), vec![1]);
        // A server that hands out a new id anyway still replaces.
        assert_eq!(registry.record(8, entry(3, "call")), vec![2]);
        assert_eq!(registry.get(7), None);
        assert_eq!(registry.get(8).map(|e| e.key), Some(3));
        // Another tag is another notification.
        assert_eq!(registry.record(9, entry(4, "thread")), Vec::<u32>::new());
        assert_eq!(registry.showing(""), None, "no tag is never a match");
    }

    #[test]
    fn a_withdrawn_notification_is_found_by_its_key() {
        let mut registry = Registry::default();
        registry.record(5, entry(10, ""));
        registry.record(6, entry(11, ""));
        assert_eq!(registry.take_key(11).map(|(id, _)| id), Some(6));
        assert_eq!(registry.take_key(11), None, "only once");
        assert_eq!(registry.take_key(0), None, "our own have no key");
        assert!(registry.get(5).is_some());
    }

    #[test]
    fn a_history_that_never_closes_anything_stays_bounded() {
        let mut registry = Registry::default();
        let mut forgotten = Vec::new();
        for n in 1..=(TRACKED as u32 + 3) {
            forgotten.extend(registry.record(n, entry(n, "")));
        }
        assert_eq!(forgotten, vec![1, 2, 3], "the oldest are let go of");
        assert_eq!(registry.by_id.len(), TRACKED);
        assert_eq!(registry.order.len(), TRACKED);
    }

    #[test]
    fn our_own_notifications_are_never_reported_to_the_scene() {
        let mut registry = Registry::default();
        registry.record(1, entry(0, NUDGE_TAG));
        assert_eq!(registry.record(2, entry(0, NUDGE_TAG)), Vec::<u32>::new());
    }

    #[test]
    fn what_says_nothing_and_what_takes_back_are_told_apart() {
        assert_eq!(
            classify("Instagram", "You have unseen notifications.", true),
            Kind::Nudge
        );
        assert_eq!(
            classify(
                "Instagram",
                "This message is no longer available because it was unsent by the sender.",
                true
            ),
            Kind::Unsent
        );
        // Neither depends on ringing being switched on.
        assert_eq!(
            classify("Instagram", "You have unseen notifications.", false),
            Kind::Nudge
        );
        assert_eq!(
            classify("Instagram", "anna: on se voit ?", true),
            Kind::Message
        );
    }

    fn toast(key: u32) -> Toast {
        Toast {
            title: String::new(),
            body: String::new(),
            clickable: true,
            kind: Kind::Nudge,
            sound: true,
            tag: String::new(),
            key,
        }
    }

    #[test]
    fn a_nudge_waits_and_gives_way_to_the_message_it_announced() {
        let start = Instant::now();
        let mut c = Coalescer::default();
        assert!(c.nudge(toast(1), start).is_empty());
        assert_eq!(c.due(), Some(start + NUDGE_GRACE));
        assert!(c.release(start).is_none(), "not due yet");

        let (dropped, shown) = c.detailed(start + Duration::from_secs(1));
        assert_eq!(dropped, vec![1], "the held nudge is let go of");
        assert_eq!(shown, None);
        assert_eq!(c.due(), None, "and nothing is left to show");
    }

    #[test]
    fn a_nudge_after_the_message_is_an_echo() {
        let start = Instant::now();
        let mut c = Coalescer::default();
        c.detailed(start);
        assert_eq!(c.nudge(toast(2), start + Duration::from_secs(5)), vec![2]);
        assert_eq!(c.due(), None);
        // Long after, a nudge is news again.
        assert!(c.nudge(toast(3), start + NUDGE_ECHO * 2).is_empty());
        assert!(c.due().is_some());
    }

    #[test]
    fn a_nudge_alone_is_shown_once_due_and_taken_off_by_a_late_message() {
        let start = Instant::now();
        let mut c = Coalescer::default();
        c.nudge(toast(4), start);
        let due = c.due().unwrap_or(start);
        assert_eq!(c.release(due).map(|t| t.key), Some(4));
        c.nudge_shown(77, due);

        let (dropped, shown) = c.detailed(due + Duration::from_secs(3));
        assert!(dropped.is_empty());
        assert_eq!(shown, Some(77), "the nudge on screen is taken off");

        // One that has been up for a while stays: it was its own arrival.
        c.nudge_shown(78, due);
        let (_, shown) = c.detailed(due + NUDGE_ECHO * 2);
        assert_eq!(shown, None);
    }

    #[test]
    fn a_second_nudge_replaces_the_one_still_held() {
        let start = Instant::now();
        let mut c = Coalescer::default();
        c.nudge(toast(5), start);
        assert_eq!(c.nudge(toast(6), start + Duration::from_secs(1)), vec![5]);
        let due = c.due().unwrap_or(start);
        assert_eq!(c.release(due).map(|t| t.key), Some(6));
    }

    #[test]
    fn a_heart_survives_a_server_that_reads_markup() {
        assert_eq!(escape_markup("<3 & more >"), "&lt;3 &amp; more &gt;");
        assert_eq!(escape_markup("plain"), "plain");
    }
}
