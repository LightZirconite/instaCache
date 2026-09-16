//! Everything QML is allowed to ask Rust.
//!
//! The QML scene owns widgets and nothing else. Which URL stays in the window,
//! where a download goes, what the offline page says, whether a dead renderer
//! should be reloaded again — all of that is decided here, where it can be
//! unit tested and where it does not have to be rewritten if the scene
//! changes.
//!
//! Calls arrive on the UI thread and must not block it. The two things that
//! would — a notification waiting to be clicked, and the update check —
//! happen on their own threads and are collected by polling.

use std::cell::{Cell, RefCell};
use std::os::unix::net::UnixListener;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use qmetaobject::*;

use crate::config::{Config, WindowState};
use crate::paths::Paths;
use crate::{badge, downloads, errorpage, instance, sites, updates, urls};
use crate::{APP_NAME, ICON_NAME};

/// Set by the termination signal handler, which may do nothing more than
/// this. The scene polls it and shuts down through the ordinary path, so the
/// window geometry is written by normal code rather than from inside a signal.
pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// A renderer that dies on every load must not be reloaded forever.
const MAX_CRASH_RELOADS: u32 = 3;
const CRASH_WINDOW: Duration = Duration::from_secs(120);

/// How often a long-running instance asks again whether an update check is
/// due. Asking is reading one small file; whether to go to the network is
/// still decided by `update_check_interval_hours`.
const UPDATE_RECHECK: Duration = Duration::from_secs(3600);

/// What the notification thread is asked to display.
struct Toast {
    title: String,
    body: String,
    /// Whether a click should raise the window and reach the page.
    clickable: bool,
}

#[derive(QObject, Default)]
pub struct Shell {
    base: qt_base_class!(trait QObject),

    // --- Read once by the scene at startup -------------------------------
    home_url: qt_property!(QString; READ home_url),
    user_agent: qt_property!(QString; READ user_agent),
    storage_path: qt_property!(QString; READ storage_path),
    cache_path: qt_property!(QString; READ cache_path),
    window_title: qt_property!(QString; READ window_title),
    show_loading_indicator: qt_property!(bool; READ show_loading_indicator),
    developer_tools: qt_property!(bool; READ developer_tools),
    remember_window_state: qt_property!(bool; READ remember_window_state),
    autoplay_without_gesture: qt_property!(bool; READ autoplay_without_gesture),
    notifications_enabled: qt_property!(bool; READ notifications_enabled),
    external_links_in_browser: qt_property!(bool; READ external_links_in_browser),
    context_menu: qt_property!(bool; READ context_menu),
    /// Whether closing the window hides it instead of quitting.
    run_in_background: qt_property!(bool; READ run_in_background),
    /// Started with `--background`: the window begins hidden. What a quiet
    /// restart for an update uses, so nothing appears on screen.
    start_hidden: qt_property!(bool; READ starts_hidden),
    /// Whether this window is Instagram, which is what the shortcuts that
    /// jump to the feed, Explore, Reels and Direct are written for.
    instagram_shortcuts: qt_property!(bool; READ instagram_shortcuts),
    /// The user's own CSS, or an empty string when there is none.
    user_stylesheet: qt_property!(QString; READ user_stylesheet),
    /// The user's own JavaScript, or an empty string when there is none.
    user_script: qt_property!(QString; READ user_script),
    /// Comma-separated, because a QML list property would need a metatype
    /// registration for one setting. Empty disables spell checking entirely.
    spell_check_languages: qt_property!(QString; READ spell_check_languages),

    // --- Asked per event --------------------------------------------------
    /// Whether a navigation stays inside the window. Security-relevant: this
    /// is what keeps a logged-in session away from arbitrary sites.
    is_internal: qt_method!(
        fn is_internal(&self, url: String) -> bool {
            urls::is_internal_in(&self.config().internal_domains, &url)
        }
    ),
    /// Whether the page at `origin` may use `feature`, named after the
    /// enumerator in `WebEngineView.Feature`.
    grants_permission: qt_method!(
        fn grants_permission(&self, origin: String, feature: String) -> bool {
            let config = self.config();
            let internal = urls::is_internal_in(&config.internal_domains, &origin);
            permission_granted(&config, internal, &feature)
        }
    ),
    is_engine_scheme: qt_method!(
        fn is_engine_scheme(&self, url: String) -> bool {
            urls::is_engine_scheme(&url)
        }
    ),
    open_externally: qt_method!(
        fn open_externally(&self, url: String) {
            open_externally(&url);
        }
    ),
    /// Geometry for a fresh window, as JSON. The screen size has to come from
    /// the scene because only Qt knows which screen the window landed on.
    initial_geometry: qt_method!(
        fn initial_geometry(&self, screen_width: i32, screen_height: i32) -> QString {
            self.initial_geometry_json(screen_width, screen_height)
                .into()
        }
    ),
    save_window_state: qt_method!(
        fn save_window_state(
            &self,
            width: i32,
            height: i32,
            x: i32,
            y: i32,
            maximized: bool,
            zoom: f64,
        ) {
            self.save_state(width, height, x, y, maximized, zoom);
        }
    ),
    /// Absolute path a download should be written to, or an empty string when
    /// the directory could not be created.
    download_destination: qt_method!(
        fn download_destination(&self, suggested: String) -> QString {
            downloads::destination_for(&suggested)
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default()
                .into()
        }
    ),
    error_page: qt_method!(
        fn error_page(&self, uri: String, message: String) -> QString {
            errorpage::render(&uri, &message).into()
        }
    ),
    crash_page: qt_method!(
        fn crash_page(&self, uri: String, reason: String) -> QString {
            errorpage::render_crash(&uri, &reason).into()
        }
    ),
    /// Records a renderer crash and answers whether to reload again.
    should_reload_after_crash: qt_method!(
        fn should_reload_after_crash(&self) -> bool {
            self.note_crash()
        }
    ),
    notify: qt_method!(
        fn notify(&self, title: String, body: String) {
            self.send_toast(title, body, true);
        }
    ),
    /// Drains everything that happened off the UI thread, as JSON:
    /// `{"present": bool, "urls": [...], "quit": bool}`.
    ///
    /// One call rather than three because the scene polls it on a timer, and a
    /// timer that wakes for three separate questions wakes three times.
    poll: qt_method!(
        fn poll(&self) -> QString {
            self.poll_json().into()
        }
    ),
    log: qt_method!(
        fn log(&self, message: String) {
            eprintln!("instacache: {message}");
        }
    ),
    /// Whether a downloaded update may take over right now without anyone
    /// noticing: the window is closed and no call is running.
    may_restart_quietly: qt_method!(
        fn may_restart_quietly(&self, window_visible: bool, in_call: bool) -> bool {
            quiet_restart_allowed(&self.update.borrow(), window_visible, in_call)
        }
    ),
    /// Asks `main` to start the updated binary once this one has quit, and
    /// answers whether it will. The scene quits only on `true`. `url` is the
    /// page to come back to; anything not internal is dropped.
    restart_for_update: qt_method!(
        fn restart_for_update(&self, hidden: bool, url: String) -> bool {
            self.request_restart(hidden, url)
        }
    ),
    /// Installs an update that could not be installed in the background,
    /// asking for the administrator password.
    install_update: qt_method!(
        fn install_update(&self) {
            self.install_elevated();
        }
    ),
    /// The window was closed and is now only hidden. Says so, once ever.
    window_hidden: qt_method!(
        fn window_hidden(&self) {
            self.note_hidden();
        }
    ),
    /// The main page's title changed; the unread badge follows it.
    title_changed: qt_method!(
        fn title_changed(&self, title: String) {
            self.update_badge(unread_count(&title));
        }
    ),

    // --- Not visible to QML ----------------------------------------------
    config: Option<Rc<Config>>,
    paths: Option<Rc<Paths>>,
    listener: Option<UnixListener>,
    updates: RefCell<Option<Receiver<updates::Outcome>>>,
    update_asked: Cell<Option<Instant>>,
    update: RefCell<UpdateState>,
    restart: RefCell<Option<Restart>>,
    hidden_at_start: bool,
    toasts: Option<Sender<Toast>>,
    badge: Option<Sender<u32>>,
    unread: Cell<u32>,
    activations: Option<Receiver<()>>,
    crash_attempts: Cell<u32>,
    crash_window_started: RefCell<Option<Instant>>,
    pending_url: RefCell<Option<String>>,
}

impl Shell {
    pub fn new(
        config: Rc<Config>,
        paths: Rc<Paths>,
        listener: Option<UnixListener>,
        start_url: Option<String>,
        start_hidden: bool,
    ) -> Self {
        let update_check = updates::check_in_background(&config, &paths);
        let (toasts, activations) = spawn_notifier();
        let badge = config
            .unread_badge
            .then(|| badge::spawn(sites::window_class(&paths.profile)));

        Self {
            config: Some(config),
            paths: Some(paths),
            listener,
            updates: RefCell::new(update_check),
            update_asked: Cell::new(Some(Instant::now())),
            toasts: Some(toasts),
            badge,
            activations: Some(activations),
            pending_url: RefCell::new(start_url),
            hidden_at_start: start_hidden,
            ..Default::default()
        }
    }

    /// What `main` should start once the scene has quit, if anything.
    pub fn take_restart(&self) -> Option<Restart> {
        self.restart.borrow_mut().take()
    }

    fn config(&self) -> Config {
        self.config.as_deref().cloned().unwrap_or_default()
    }

    fn home_url(&self) -> QString {
        // A URL given on the command line replaces the home page for this
        // launch only; it is taken here rather than in the scene so that the
        // scene has one source of truth for "what to load first".
        if let Some(url) = self.pending_url.borrow_mut().take() {
            return url.into();
        }
        self.config().home_url.into()
    }

    fn user_agent(&self) -> QString {
        self.config().user_agent.into()
    }

    fn storage_path(&self) -> QString {
        self.paths
            .as_ref()
            .map(|p| p.data.to_string_lossy().into_owned())
            .unwrap_or_default()
            .into()
    }

    fn cache_path(&self) -> QString {
        self.paths
            .as_ref()
            .map(|p| p.cache.to_string_lossy().into_owned())
            .unwrap_or_default()
            .into()
    }

    fn window_title(&self) -> QString {
        match self.paths.as_ref() {
            Some(paths) if !paths.is_default_profile() => {
                format!("{APP_NAME} — {}", paths.profile).into()
            }
            _ => APP_NAME.into(),
        }
    }

    fn show_loading_indicator(&self) -> bool {
        self.config().show_loading_indicator
    }

    fn developer_tools(&self) -> bool {
        self.config().developer_tools
    }

    fn remember_window_state(&self) -> bool {
        self.config().remember_window_state
    }

    fn autoplay_without_gesture(&self) -> bool {
        self.config().allow_autoplay_with_sound
    }

    fn notifications_enabled(&self) -> bool {
        self.config().notifications
    }

    fn external_links_in_browser(&self) -> bool {
        self.config().open_external_links_in_browser
    }

    fn context_menu(&self) -> bool {
        self.config().context_menu
    }

    fn run_in_background(&self) -> bool {
        self.config().run_in_background
    }

    fn starts_hidden(&self) -> bool {
        self.hidden_at_start
    }

    fn request_restart(&self, hidden: bool, url: String) -> bool {
        if !matches!(*self.update.borrow(), UpdateState::Ready(_)) {
            return false;
        }
        let internal = urls::is_internal_in(&self.config().internal_domains, &url);
        *self.restart.borrow_mut() = Some(Restart {
            hidden,
            url: internal.then_some(url),
        });
        true
    }

    fn install_elevated(&self) {
        let version = match &*self.update.borrow() {
            UpdateState::Available(version) if updates::can_install_elevated() => version.clone(),
            _ => return,
        };
        *self.update.borrow_mut() = UpdateState::Installing(version.clone());
        *self.updates.borrow_mut() = Some(updates::install_elevated_in_background(version));
    }

    fn instagram_shortcuts(&self) -> bool {
        urls::is_internal_in(&["instagram.com"], &self.config().home_url)
    }

    fn note_hidden(&self) {
        let Some(paths) = self.paths.as_ref() else {
            return;
        };
        let marker = paths.background_notice_marker();
        if marker.exists() {
            return;
        }
        if let Err(error) = std::fs::write(&marker, b"") {
            eprintln!("instacache: could not write {}: {error}", marker.display());
        }
        self.send_toast(
            format!("{APP_NAME} is still running"),
            "It opens instantly and keeps your notifications coming. \
             Ctrl+Q quits, and `run_in_background` turns this off."
                .to_string(),
            false,
        );
    }

    /// An instance that stays in the background may run for weeks, and the
    /// startup check alone would then never run again.
    fn recheck_updates(&self) {
        // Runs four times a second, so nothing is cloned until a check is due.
        let interval = self
            .config
            .as_deref()
            .map(|config| config.update_check_interval_hours)
            .unwrap_or_else(|| Config::default().update_check_interval_hours);
        let now = Instant::now();
        let pending = self.updates.borrow().is_some();
        let installed = matches!(
            *self.update.borrow(),
            UpdateState::Ready(_) | UpdateState::Installing(_)
        );
        if pending || !update_recheck_due(self.update_asked.get(), now, installed, interval) {
            return;
        }
        self.update_asked.set(Some(now));
        if let Some(paths) = self.paths.as_ref() {
            *self.updates.borrow_mut() = updates::check_in_background(&self.config(), paths);
        }
    }

    fn update_badge(&self, count: u32) {
        if self.unread.replace(count) == count {
            return;
        }
        if let Some(badge) = self.badge.as_ref() {
            let _ = badge.send(count);
        }
    }

    /// Read on every access rather than cached, so editing the file and
    /// reloading is enough to see the change.
    fn user_stylesheet(&self) -> QString {
        self.paths
            .as_ref()
            .and_then(|paths| std::fs::read_to_string(paths.user_stylesheet()).ok())
            .unwrap_or_default()
            .into()
    }

    fn user_script(&self) -> QString {
        self.paths
            .as_ref()
            .and_then(|paths| std::fs::read_to_string(paths.user_script()).ok())
            .unwrap_or_default()
            .into()
    }

    fn spell_check_languages(&self) -> QString {
        self.config().spell_checking_languages.join(",").into()
    }

    fn initial_geometry_json(&self, screen_width: i32, screen_height: i32) -> String {
        let config = self.config();
        let saved = self
            .paths
            .as_ref()
            .and_then(|paths| WindowState::load(paths))
            .filter(|_| config.remember_window_state);

        let state =
            saved.unwrap_or_else(|| WindowState::from_work_area((screen_width, screen_height)));
        let zoom = if config.remember_window_state {
            state.zoom
        } else {
            config.default_zoom
        };

        serde_json::json!({
            "width": state.width,
            "height": state.height,
            "x": state.x,
            "y": state.y,
            "maximized": state.maximized || config.start_maximized,
            "zoom": zoom,
        })
        .to_string()
    }

    fn save_state(&self, width: i32, height: i32, x: i32, y: i32, maximized: bool, zoom: f64) {
        let config = self.config();
        if !config.remember_window_state {
            return;
        }
        let Some(paths) = self.paths.as_ref() else {
            return;
        };
        let keep = keep_position(maximized, on_wayland());
        let state = WindowState {
            width,
            height,
            x: keep.then_some(x),
            y: keep.then_some(y),
            maximized,
            zoom,
        };
        state.save(paths);
    }

    fn note_crash(&self) -> bool {
        let expired = self
            .crash_window_started
            .borrow()
            .is_none_or(|started| started.elapsed() > CRASH_WINDOW);
        if expired {
            *self.crash_window_started.borrow_mut() = Some(Instant::now());
            self.crash_attempts.set(0);
        }

        let attempt = self.crash_attempts.get() + 1;
        self.crash_attempts.set(attempt);
        attempt <= MAX_CRASH_RELOADS
    }

    fn send_toast(&self, title: String, body: String, clickable: bool) {
        if !self.config().notifications {
            return;
        }
        if let Some(toasts) = self.toasts.as_ref() {
            let _ = toasts.send(Toast {
                title,
                body,
                clickable,
            });
        }
    }

    fn poll_json(&self) -> String {
        let mut present = false;
        let mut urls: Vec<String> = Vec::new();

        if let Some(listener) = self.listener.as_ref() {
            for launch in instance::drain(listener) {
                present = true;
                if let Some(url) = launch {
                    urls.push(url);
                }
            }
        }

        if let Some(activations) = self.activations.as_ref() {
            while activations.try_recv().is_ok() {
                present = true;
            }
        }

        let outcome = self
            .updates
            .borrow()
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(outcome) = outcome {
            *self.updates.borrow_mut() = None;
            let before = self.update.borrow().clone();
            let after = next_update_state(&before, &outcome);
            if let Some(message) = describe_update(&before, &after, updates::can_install_elevated())
            {
                println!("instacache: {message}");
                if matches!(after, UpdateState::Available(_)) {
                    self.send_toast(message, String::new(), false);
                }
            }
            *self.update.borrow_mut() = after;
        }

        self.recheck_updates();

        let (update, version) = match &*self.update.borrow() {
            UpdateState::Idle => ("none", String::new()),
            UpdateState::Available(v) if updates::can_install_elevated() => {
                ("installable", v.clone())
            }
            UpdateState::Available(v) => ("available", v.clone()),
            UpdateState::Installing(v) => ("installing", v.clone()),
            UpdateState::Ready(v) => ("ready", v.clone()),
        };

        serde_json::json!({
            "present": present,
            "urls": urls,
            "quit": SHUTDOWN.load(Ordering::SeqCst),
            "update": update,
            "update_version": version,
        })
        .to_string()
    }
}

/// Whether a saved position is worth anything.
///
/// A Wayland client is not told where its window is and cannot ask to be put
/// back there, so Qt reports a number that means nothing — storing it would
/// place the window wrong the moment the same profile is opened under X11.
/// A maximized window's position is not worth restoring either: the size that
/// matters is the one it had before being maximized.
fn keep_position(maximized: bool, wayland: bool) -> bool {
    !maximized && !wayland
}

fn on_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t.eq_ignore_ascii_case("wayland"))
}

/// Where an update stands in this process.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum UpdateState {
    #[default]
    Idle,
    /// Newer, and not installed: a system-wide install.
    Available(String),
    /// Being installed as root, on the user's request.
    Installing(String),
    /// Installed on disk. The next start runs it.
    Ready(String),
}

/// What `main` starts after the scene quits for an update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restart {
    /// Start with the window hidden, as it was.
    pub hidden: bool,
    /// The page to reopen. Only ever an internal one.
    pub url: Option<String>,
}

impl Restart {
    /// Arguments for the new process.
    pub fn arguments(&self, profile: &str) -> Vec<String> {
        let mut args = Vec::new();
        if profile != crate::paths::DEFAULT_PROFILE {
            args.push("--profile".to_string());
            args.push(profile.to_string());
        }
        if self.hidden {
            args.push("--background".to_string());
        }
        if let Some(url) = &self.url {
            args.push(url.clone());
        }
        args
    }
}

/// A finished check or install, applied to where things stood. An update that
/// is already on disk is not forgotten because a later check found nothing.
fn next_update_state(before: &UpdateState, outcome: &updates::Outcome) -> UpdateState {
    match (before, outcome) {
        (_, updates::Outcome::Installed { version }) => UpdateState::Ready(version.clone()),
        (UpdateState::Ready(_), _) => before.clone(),
        (_, updates::Outcome::Available { version }) => UpdateState::Available(version.clone()),
        (UpdateState::Installing(version), updates::Outcome::UpToDate) => {
            UpdateState::Available(version.clone())
        }
        (_, updates::Outcome::UpToDate) => before.clone(),
    }
}

/// The line logged, and for an update this copy cannot install on its own
/// also shown, when the state changes. `None` when nothing new happened.
fn describe_update(before: &UpdateState, after: &UpdateState, can_elevate: bool) -> Option<String> {
    if before == after {
        return None;
    }
    match after {
        UpdateState::Ready(version) => Some(format!(
            "{APP_NAME} {version} is installed and takes over at the next restart"
        )),
        UpdateState::Available(version) if can_elevate => Some(format!(
            "{APP_NAME} {version} is available. Install it from the window."
        )),
        UpdateState::Available(version) => Some(format!(
            "{APP_NAME} {version} is available. Run `sudo instacache --update` to install it."
        )),
        UpdateState::Idle | UpdateState::Installing(_) => None,
    }
}

/// Whether an update may take over without the user seeing it happen.
///
/// Only when it is on disk, the window is closed and no call is running: a
/// restart ends a call, and restarting a window somebody is looking at is not
/// quiet. A visible window offers a button instead, and closing it counts.
fn quiet_restart_allowed(state: &UpdateState, window_visible: bool, in_call: bool) -> bool {
    matches!(state, UpdateState::Ready(_)) && !window_visible && !in_call
}

/// Shows notifications and waits for clicks on a thread of its own.
///
/// `notify-rust` blocks while waiting for the user to click, and the handle it
/// returns is bound to the connection that produced it, so both the showing
/// and the waiting stay on this one thread and only plain messages cross.
fn spawn_notifier() -> (Sender<Toast>, Receiver<()>) {
    let (toast_tx, toast_rx) = std::sync::mpsc::channel::<Toast>();
    let (activated_tx, activated_rx) = std::sync::mpsc::channel::<()>();

    std::thread::spawn(move || {
        while let Ok(toast) = toast_rx.recv() {
            let mut notification = notify_rust::Notification::new();
            notification
                .summary(&toast.title)
                .body(&toast.body)
                .icon(ICON_NAME)
                .appname(APP_NAME);

            if toast.clickable {
                notification.action("default", "Open");
            }

            match notification.show() {
                Ok(handle) => {
                    if toast.clickable {
                        let activated = activated_tx.clone();
                        handle.wait_for_action(|action| {
                            if action == "default" {
                                let _ = activated.send(());
                            }
                        });
                    }
                }
                Err(error) => eprintln!("instacache: could not post a notification: {error}"),
            }
        }
    });

    (toast_tx, activated_rx)
}

/// The permission policy, apart from the Qt types so it can be tested.
///
/// Anything not named here is refused: geolocation, screen capture, pointer
/// lock, and whatever a newer Qt adds. A request from a host outside the
/// window's allow-list is refused whatever it asks for.
fn permission_granted(config: &Config, internal: bool, feature: &str) -> bool {
    if !internal {
        return false;
    }
    match feature {
        "Notifications" => config.notifications,
        // Backs "paste an image into a DM".
        "ClipboardReadWrite" => true,
        // Voice and video calls in Direct, which cannot work without them.
        "MediaAudioCapture" | "MediaVideoCapture" | "MediaAudioVideoCapture" => config.calls,
        _ => false,
    }
}

/// Whether a running instance should ask again if an update check is due.
///
/// Never once an update has been installed: the running binary is still the
/// old version, so every later check would find the same release "newer" and
/// install it again. And never for an interval of `0`, which means "at every
/// launch" and would otherwise mean "every hour".
fn update_recheck_due(
    asked: Option<Instant>,
    now: Instant,
    installed: bool,
    interval_hours: u64,
) -> bool {
    if installed || interval_hours == 0 {
        return false;
    }
    asked.is_none_or(|asked| now.saturating_duration_since(asked) >= UPDATE_RECHECK)
}

/// The unread count a page title announces: `(3) Instagram` is 3, and a
/// title with no count in front is 0.
///
/// Only a count at the very start is read. A number in brackets further along
/// is part of whatever the page is showing, not a count of anything.
fn unread_count(title: &str) -> u32 {
    let Some(rest) = title.trim_start().strip_prefix('(') else {
        return 0;
    };
    let Some((inside, _)) = rest.split_once(')') else {
        return 0;
    };
    let digits = inside.trim().trim_end_matches('+');
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return 0;
    }
    digits.parse().unwrap_or(u32::MAX)
}

/// Hands a URL to the system browser, without a shell in between.
pub fn open_externally(uri: &str) {
    if urls::scheme_of(uri).is_none() {
        return;
    }
    if let Err(error) = std::process::Command::new("xdg-open").arg(uri).spawn() {
        eprintln!("instacache: could not open {uri} externally: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell() -> Shell {
        Shell {
            config: Some(Rc::new(Config::default())),
            ..Default::default()
        }
    }

    #[test]
    fn a_crashing_renderer_is_reloaded_a_few_times_then_left_alone() {
        let shell = shell();
        for attempt in 1..=MAX_CRASH_RELOADS {
            assert!(shell.note_crash(), "attempt {attempt} should reload");
        }
        assert!(
            !shell.note_crash(),
            "past the limit the crash page is shown instead"
        );
    }

    #[test]
    fn calls_get_the_microphone_and_camera_on_instagram_only() {
        let config = Config::default();
        for feature in [
            "MediaAudioCapture",
            "MediaVideoCapture",
            "MediaAudioVideoCapture",
        ] {
            assert!(permission_granted(&config, true, feature), "{feature}");
            assert!(!permission_granted(&config, false, feature), "{feature}");
        }
        let off = Config {
            calls: false,
            ..Config::default()
        };
        assert!(!permission_granted(&off, true, "MediaAudioCapture"));
    }

    #[test]
    fn what_calls_do_not_need_stays_refused() {
        let config = Config::default();
        for feature in [
            "Geolocation",
            "DesktopVideoCapture",
            "DesktopAudioVideoCapture",
            "MouseLock",
            "",
        ] {
            assert!(!permission_granted(&config, true, feature), "{feature}");
        }
        assert!(!permission_granted(&config, false, "Notifications"));
    }

    #[test]
    fn the_unread_count_is_read_from_the_front_of_the_title() {
        assert_eq!(unread_count("(3) Instagram"), 3);
        assert_eq!(unread_count("(12) Inbox • Direct"), 12);
        assert_eq!(unread_count("(99+) Instagram"), 99);
        assert_eq!(unread_count("  (2) Instagram"), 2);
        assert_eq!(unread_count("Instagram"), 0);
        assert_eq!(unread_count(""), 0);
        assert_eq!(unread_count("(0) Instagram"), 0);
        assert_eq!(unread_count("Photo (3) by someone"), 0);
        assert_eq!(unread_count("(new) Instagram"), 0);
        assert_eq!(unread_count("(+) Instagram"), 0);
        assert_eq!(unread_count("(4 Instagram"), 0);
        assert_eq!(unread_count("(99999999999) Instagram"), u32::MAX);
    }

    #[test]
    fn a_background_instance_asks_about_updates_hourly_until_one_is_installed() {
        let start = Instant::now();
        let later = start + UPDATE_RECHECK;
        assert!(!update_recheck_due(Some(start), start, false, 24));
        assert!(update_recheck_due(Some(start), later, false, 24));
        assert!(!update_recheck_due(Some(start), later, true, 24));
        assert!(!update_recheck_due(Some(start), later, false, 0));
    }

    #[test]
    fn an_update_restarts_quietly_only_when_nobody_is_looking() {
        let ready = UpdateState::Ready("9.0.0".into());
        assert!(quiet_restart_allowed(&ready, false, false));
        assert!(!quiet_restart_allowed(&ready, true, false), "window open");
        assert!(!quiet_restart_allowed(&ready, false, true), "call running");
        assert!(!quiet_restart_allowed(&UpdateState::Idle, false, false));
        assert!(!quiet_restart_allowed(
            &UpdateState::Available("9.0.0".into()),
            false,
            false
        ));
    }

    #[test]
    fn an_installed_update_is_not_forgotten_by_a_later_check() {
        use updates::Outcome;
        let ready = UpdateState::Ready("9.0.0".into());
        assert_eq!(next_update_state(&ready, &Outcome::UpToDate), ready);
        assert_eq!(
            next_update_state(
                &ready,
                &Outcome::Available {
                    version: "9.0.1".into()
                }
            ),
            ready
        );
        let installing = UpdateState::Installing("9.0.0".into());
        assert_eq!(
            next_update_state(&installing, &Outcome::UpToDate),
            UpdateState::Available("9.0.0".into()),
            "a refused password leaves the update on offer"
        );
        assert_eq!(
            next_update_state(
                &UpdateState::Idle,
                &Outcome::Installed {
                    version: "9.0.0".into()
                }
            ),
            ready
        );
    }

    #[test]
    fn a_restart_only_happens_for_an_installed_update_and_keeps_the_page() {
        let shell = shell();
        assert!(!shell.request_restart(false, "https://www.instagram.com/direct/".into()));
        assert_eq!(shell.take_restart(), None);

        *shell.update.borrow_mut() = UpdateState::Ready("9.0.0".into());
        assert!(shell.request_restart(false, "https://www.instagram.com/direct/".into()));
        let restart = shell.take_restart().unwrap();
        assert_eq!(
            restart.arguments("default"),
            vec!["https://www.instagram.com/direct/".to_string()]
        );

        assert!(shell.request_restart(true, "https://evil.example/".into()));
        let restart = shell.take_restart().unwrap();
        assert_eq!(restart.url, None, "an external page is not reopened");
        assert_eq!(
            restart.arguments("work"),
            vec!["--profile", "work", "--background"]
        );
    }

    #[test]
    fn only_a_change_is_announced() {
        let idle = UpdateState::Idle;
        let ready = UpdateState::Ready("9.0.0".into());
        let available = UpdateState::Available("9.0.0".into());
        assert!(describe_update(&idle, &ready, false).is_some());
        assert!(describe_update(&ready, &ready, false).is_none());
        assert!(describe_update(&idle, &available, false)
            .unwrap()
            .contains("sudo instacache --update"));
        assert!(describe_update(&idle, &available, true)
            .unwrap()
            .contains("from the window"));
    }

    #[test]
    fn a_count_that_did_not_change_is_not_sent_again() {
        let (tx, rx) = std::sync::mpsc::channel();
        let shell = Shell {
            badge: Some(tx),
            ..shell()
        };
        shell.update_badge(0);
        shell.update_badge(2);
        shell.update_badge(2);
        shell.update_badge(0);
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![2, 0]);
    }

    #[test]
    fn instagram_shortcuts_are_for_instagram_only() {
        assert!(shell().instagram_shortcuts());
        let x = Shell {
            config: Some(Rc::new(Config {
                home_url: "https://x.com/".into(),
                ..Config::default()
            })),
            ..Default::default()
        };
        assert!(!x.instagram_shortcuts());
    }

    #[test]
    fn a_first_run_is_sized_from_the_screen() {
        let shell = shell();
        let json: serde_json::Value =
            serde_json::from_str(&shell.initial_geometry_json(1920, 1080)).unwrap();
        assert!(json["width"].as_i64().unwrap() > 0);
        assert!(json["height"].as_i64().unwrap() <= 1080);
        assert_eq!(json["zoom"].as_f64().unwrap(), 1.0);
    }

    #[test]
    fn a_window_position_is_only_kept_when_it_means_something() {
        assert!(keep_position(false, false), "X11, windowed: keep it");
        assert!(!keep_position(false, true), "Wayland reports nonsense");
        assert!(!keep_position(true, false), "maximized: nothing to restore");
    }

    #[test]
    fn a_command_line_url_wins_once_and_then_the_home_page_returns() {
        let shell = Shell {
            pending_url: RefCell::new(Some("https://www.instagram.com/reels/".into())),
            ..shell()
        };
        assert_eq!(
            shell.home_url().to_string(),
            "https://www.instagram.com/reels/"
        );
        assert_eq!(shell.home_url().to_string(), Config::default().home_url);
    }
}
