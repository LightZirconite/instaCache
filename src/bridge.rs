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
use crate::{downloads, errorpage, instance, updates, urls};
use crate::{APP_NAME, ICON_NAME};

/// Set by the termination signal handler, which may do nothing more than
/// this. The scene polls it and shuts down through the ordinary path, so the
/// window geometry is written by normal code rather than from inside a signal.
pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// A renderer that dies on every load must not be reloaded forever.
const MAX_CRASH_RELOADS: u32 = 3;
const CRASH_WINDOW: Duration = Duration::from_secs(120);

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

    // --- Not visible to QML ----------------------------------------------
    config: Option<Rc<Config>>,
    paths: Option<Rc<Paths>>,
    listener: Option<UnixListener>,
    updates: RefCell<Option<Receiver<updates::Outcome>>>,
    toasts: Option<Sender<Toast>>,
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
    ) -> Self {
        let update_check = updates::check_in_background(&config, &paths);
        let (toasts, activations) = spawn_notifier();

        Self {
            config: Some(config),
            paths: Some(paths),
            listener,
            updates: RefCell::new(update_check),
            toasts: Some(toasts),
            activations: Some(activations),
            pending_url: RefCell::new(start_url),
            ..Default::default()
        }
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
            if let Some((title, body)) = describe_update(&outcome) {
                println!("instacache: {title} — {body}");
                self.send_toast(title, body, false);
            }
        }

        serde_json::json!({
            "present": present,
            "urls": urls,
            "quit": SHUTDOWN.load(Ordering::SeqCst),
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

fn describe_update(outcome: &updates::Outcome) -> Option<(String, String)> {
    match outcome {
        updates::Outcome::Installed { version } => Some((
            format!("{APP_NAME} {version} installed"),
            "Restart instaCache to start using it.".to_string(),
        )),
        updates::Outcome::Available { version } => Some((
            format!("{APP_NAME} {version} is available"),
            "Run `instacache --update` in a terminal to install it.".to_string(),
        )),
        updates::Outcome::UpToDate => None,
    }
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
