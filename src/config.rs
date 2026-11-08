//! User configuration (`config.json`) and persisted window geometry
//! (`window-state.json`).
//!
//! Both files are optional and every field falls back to a sane default, so a
//! truncated, hand-edited or older file never prevents the app from starting.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::paths::Paths;

pub const DEFAULT_HOME_URL: &str = "https://www.instagram.com/";

/// `auto`: Chrome on Linux, at the version of Chromium the engine really is —
/// see [`user_agent_for`].
pub const DEFAULT_USER_AGENT: &str = AUTO_USER_AGENT;

/// The `user_agent` value that means "say what the engine is, as Chrome does".
pub const AUTO_USER_AGENT: &str = "auto";

/// What the page is told it runs in, from the configured `user_agent` and the
/// engine's own string (`QtWebEngine/6.4.2 Chrome/102.0.5005.177 …`).
///
/// `auto` says Chrome on Linux at the engine's real Chromium version, in the
/// reduced form Chrome itself sends (`Chrome/102.0.0.0`). Every part is the
/// truth, and that is the point:
///
/// - **The engine.** Instagram serves different code to different engines;
///   claiming Safari would put Chromium on Safari's code path.
/// - **The platform.** Instagram names the user agent's operating system in
///   its login alerts: a Linux machine announcing macOS reads as somebody
///   else signing in.
/// - **The version.** Until 2.10 this was a fixed `Chrome/140`, and the engine
///   went on saying its real version everywhere else — in the features it has
///   and, on newer Qt, in `navigator.userAgentData`. A browser contradicting
///   itself is what an account's risk checks look for; one that drifts with
///   its distribution's Qt looks like a browser being updated, which it is.
///
/// Qt's own string is not sent as it is because of its `QtWebEngine/6.x`
/// token, which no site has heard of. An empty `user_agent` still sends it;
/// any other value is sent verbatim.
pub fn user_agent_for(configured: &str, engine: &str) -> String {
    match configured.trim() {
        "" => engine.to_string(),
        AUTO_USER_AGENT => chrome_major(engine)
            .map(|major| {
                format!(
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                     (KHTML, like Gecko) Chrome/{major}.0.0.0 Safari/537.36"
                )
            })
            .unwrap_or_else(|| engine.to_string()),
        other => other.to_string(),
    }
}

fn chrome_major(engine: &str) -> Option<u32> {
    engine
        .split("Chrome/")
        .nth(1)?
        .split('.')
        .next()?
        .parse()
        .ok()
}

/// The `Accept-Language` a browser in this session's language sends, from
/// `LC_ALL`, `LC_MESSAGES` or `LANG` and the `LANGUAGE` list, the way Chrome
/// builds it: `fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7`.
///
/// Qt WebEngine 6.4 sent no `Accept-Language` at all — measured, with `LANG`
/// set — and every real browser sends one. Its absence is the kind of detail
/// that sets a client apart from a person's browser.
pub fn accept_language(env: impl Fn(&str) -> Option<String>) -> String {
    let set = |name: &str| env(name).filter(|value| !value.trim().is_empty());
    let primary = set("LC_ALL")
        .or_else(|| set("LC_MESSAGES"))
        .or_else(|| set("LANG"));
    let mut tags: Vec<String> = Vec::new();
    let mut push = |tag: String| {
        if !tag.is_empty() && !tags.contains(&tag) {
            tags.push(tag);
        }
    };
    let named = |locale: &str| {
        // `fr_FR.UTF-8@euro` -> `fr-FR`; `C` and `POSIX` name no language.
        let bare = locale.split(['.', '@']).next().unwrap_or_default();
        if bare.is_empty() || bare == "C" || bare == "POSIX" {
            None
        } else {
            Some(bare.replace('_', "-"))
        }
    };
    let mut languages: Vec<String> = primary.as_deref().and_then(named).into_iter().collect();
    if !languages.is_empty() {
        if let Some(list) = set("LANGUAGE") {
            languages.extend(list.split(':').filter_map(named));
        }
    }
    languages.extend(["en-US".to_string()]);
    for tag in languages {
        let base = tag.split('-').next().unwrap_or_default().to_string();
        push(tag);
        push(base);
    }
    tags.iter()
        .enumerate()
        .map(|(i, tag)| {
            if i == 0 {
                tag.clone()
            } else {
                let q = (10 - i.min(9)) as f32 / 10.0;
                format!("{tag};q={q:.1}")
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Defaults shipped by earlier versions. A config file still carrying one of
/// these was never a deliberate choice by the user, only the old default
/// written out on first run, so they are migrated rather than preserved.
///
/// The macOS string was the default up to 1.1.1; the Linux Safari one until
/// the engine became Chromium, at which point it stopped being true.
const SUPERSEDED_USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (X11; Linux x86_64) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.3 Safari/605.1.15",
    "Mozilla/5.0 (X11; Linux x86_64) \
AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.3 Safari/605.1.15",
];

/// How often a running instaCache asks whether a newer release exists: a few
/// times a day, the way Chrome and Firefox do, rather than once a day at
/// launch. The request is one small JSON document from GitHub.
pub const DEFAULT_UPDATE_CHECK_HOURS: u64 = 6;

/// How long a page waits in the background before it is discarded. Long
/// enough that closing the window for a coffee costs no reload, short enough
/// that a page left behind overnight is not holding its memory all night.
pub const DEFAULT_DISCARD_MINUTES: u64 = 30;

/// The HTTP cache's size: room for Instagram's scripts and a long evening of
/// media together.
pub const DEFAULT_HTTP_CACHE_MB: u32 = 1024;

/// The largest cache Qt can be told about: its size is a C `int` of bytes.
const MAX_HTTP_CACHE_MB: u32 = 2047;

/// What `WebEngineProfile.httpCacheMaximumSize` is given: bytes, with `0`
/// meaning the engine's own choice, and never more than an `int` holds.
pub fn http_cache_bytes(megabytes: u32) -> i32 {
    let bytes = u64::from(megabytes.min(MAX_HTTP_CACHE_MB)) * 1024 * 1024;
    i32::try_from(bytes).unwrap_or(i32::MAX)
}

/// The volume `videos_with_sound` starts a video at until the user sets one
/// of their own, which the page then remembers: loud enough to be heard
/// over a room, short of full.
pub const DEFAULT_VIDEO_VOLUME: u8 = 75;

/// The default until 2.11.0, found too quiet. Written out by every first run
/// since the setting existed, so it says nothing about what the user wants.
const SUPERSEDED_VIDEO_VOLUME: u8 = 40;

/// The default until 2.4.0. See `DEFAULT_USER_AGENT` for why an old default
/// found in a config file is migrated rather than kept.
const SUPERSEDED_UPDATE_CHECK_HOURS: u64 = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Page loaded at startup and by the "home" shortcut.
    pub home_url: String,
    /// `auto`, the default, says Chrome on Linux at the engine's real Chromium
    /// version. An empty string keeps Qt WebEngine's built-in user agent, with
    /// its `QtWebEngine/6.x` token; anything else is sent as it is. See
    /// [`user_agent_for`].
    pub user_agent: String,
    /// `auto` | `always` | `never`.
    ///
    /// `always` and `auto` both leave Chromium's own decision alone — unlike
    /// WebKit it does not switch compositing modes mid-page, which is the
    /// reason this setting had three meanings rather than two. `never` turns
    /// GPU compositing off entirely and is a last resort for a window that
    /// renders wrong. Carried out in `chromium.rs`.
    pub hardware_acceleration: String,
    /// Let videos start with their sound on.
    ///
    /// The engine's default is the web's default: a video that starts playing
    /// without the user having clicked something is forced to be silent. In a
    /// dedicated Instagram window that reads as "the app muted itself", since
    /// Instagram keeps its own mute button anyway.
    pub allow_autoplay_with_sound: bool,
    /// The mod that starts every video with its sound on, at `video_volume`.
    ///
    /// `allow_autoplay_with_sound` lifts the engine's rule; this one deals
    /// with Instagram's, which mutes every video and only unmutes it from its
    /// own speaker button. Muting a video by hand switches it off until the
    /// page is loaded again. See `mods.rs`.
    ///
    /// On for Instagram and off for another site by default.
    pub videos_with_sound: bool,
    /// The volume `videos_with_sound` starts a video at, from 0 to 100, until
    /// the user picks one with `+` and `-`; the page remembers that one.
    pub video_volume: u8,
    /// The mod that tints Instagram's dark theme plum, in its own colours,
    /// instead of black. Its light theme is left as it is.
    ///
    /// On for Instagram and off for another site by default.
    pub instagram_colors: bool,
    /// With `instagram_colors`, the background takes on the colour of the
    /// picture or video in view, the way a video player's ambient mode does,
    /// and drifts slowly towards a neighbouring colour while the page rests.
    ///
    /// On for Instagram and off for another site by default.
    pub ambient_colors: bool,
    /// The mod that brings the phone's gestures and a few keys to the page:
    /// a double-click likes, as a double tap does, and a right-click reposts;
    /// on Reels, Space twice likes the reel and stays on it while one Space goes to
    /// the next; Enter twice likes; `L` likes, `R` reposts, `M` turns the
    /// sound on or off, `C` opens the comments, `S` saves or unsaves, `O`
    /// turns a video sideways; a click on a story pauses it or plays it
    /// again, and the wheel over it moves to the next story or back; the
    /// mouse wheel scrolls the row of stories at the top of the feed.
    ///
    /// On for Instagram and off for another site by default.
    pub shortcuts: bool,
    /// The mod that hides "Also from Meta", a menu of Meta's other apps, from
    /// Instagram's side bar.
    ///
    /// On for Instagram and off for another site by default.
    pub hide_also_from_meta: bool,
    /// The mod that adds small motion to the page: dialogs and pictures fade
    /// in. Off by itself wherever the desktop asks for reduced motion.
    ///
    /// On for Instagram and off for another site by default.
    pub page_animations: bool,
    /// Which decoder to reach for: `gpu`, `software` or `auto`.
    ///
    /// `gpu` is the default and switches on the VA-API decoding that Chromium
    /// leaves off on Linux. `software` leaves decoding to the CPU without
    /// switching off compositing as well. `auto` takes Chromium's own
    /// decision. Carried out in `chromium.rs`, which is where the flags and
    /// their tests live.
    ///
    /// The name and the meaning survived the change of engine; the mechanism
    /// did not. Under WebKit this reordered GStreamer's plugin ranks, which
    /// Chromium does not use at all.
    ///
    /// Note what this setting does *not* decide. What makes a Reels feed
    /// stutter is building and tearing down a video per clip, not decoding
    /// one frame — which is why the engine changed and this setting did not.
    /// On the reference machine it is worth 2 or 3 late frames either way.
    pub video_decoding: String,
    /// Show the engine's context menu on right-click.
    ///
    /// Off by default. In a window that exists to be one application, the
    /// menu is browser chrome: it offers Back, Forward, Reload, View Source
    /// and Inspect, none of which belong in it, and it covers the page while
    /// Instagram's own long-press and right-click handling stops working.
    ///
    /// It is a real trade-off rather than a free win, so it is a switch:
    /// turning the menu off also removes "Save image as", "Copy image address"
    /// and "Copy link address". Set it to `true` to get them back.
    pub context_menu: bool,
    /// Enables the Web Inspector (Ctrl+Shift+I / right-click → Inspect).
    pub developer_tools: bool,
    /// Forward web notifications to the desktop notification daemon.
    pub notifications: bool,
    /// Register with the browser's push service, so a message or a call
    /// reaches the desktop when nobody is looking at the window.
    ///
    /// Instagram delivers both over Web Push, from a service worker, and Qt
    /// WebEngine's push service is off unless it is asked for: without it
    /// `PushManager.subscribe()` fails with "push service not available",
    /// Instagram never registers, no push ever arrives, and the service
    /// worker that would post the notification never runs. That is the whole
    /// reason the unread badge could move while nothing rang.
    ///
    /// Turning it on holds one connection to Google's push servers, the same
    /// one Chrome holds. Turning it off is the setting for someone who would
    /// rather not have it, at the cost of hearing nothing while the window is
    /// closed.
    ///
    /// Needs Qt 6.5. On 6.4 the property does not exist and this does nothing.
    pub push_notifications: bool,
    /// A short sound with a message's notification, the way a chat
    /// application has one. The notification server plays it where it can,
    /// and Do Not Disturb silences it.
    pub notification_sounds: bool,
    /// Ring, repeatedly, for an incoming call's notification, until it is
    /// answered, dismissed or the window is opened. See `alerts.rs`.
    ///
    /// On for Instagram and off for another site by default: see
    /// [`Config::for_home`].
    pub ring_for_calls: bool,
    /// Let Instagram use the microphone and camera, which is what voice and
    /// video calls in Direct need. Only a host in `internal_domains` is ever
    /// granted them; `false` refuses them everywhere.
    ///
    /// On for Instagram and off for another site by default.
    pub calls: bool,
    /// Keep running when the window is closed, so opening it again is instant
    /// and notifications keep arriving. `Ctrl+Q` still quits for real.
    ///
    /// Most of a launch is spent before instaCache has any say in it: loading
    /// Qt WebEngine, starting Chromium's processes and running Instagram's
    /// own JavaScript. Keeping the window instead of rebuilding it is the one
    /// thing that removes that time rather than shaving a little off it.
    ///
    /// On for Instagram and off for another site by default.
    pub run_in_background: bool,
    /// Freeze the page as soon as the window is closed to the background:
    /// its JavaScript stops, so it costs no processor time at all, and it
    /// picks up where it was the moment the window opens again.
    ///
    /// A frozen page does not read what arrives on its connection, so the
    /// unread badge stops moving and a message is heard only through Web
    /// Push. Whether Instagram still pushes to a client whose connection is
    /// open but frozen is not established; `false` keeps the page running,
    /// throttled by Chromium, as before 2.8.0.
    pub freeze_in_background: bool,
    /// Discard the page after this many minutes in the background, which
    /// gives back the memory its renderer holds. Opening the window reloads
    /// it from the cache, behind the snapshot of how it was left. `0` never
    /// discards it.
    ///
    /// A discarded page has no connection, so Instagram delivers what arrives
    /// through Web Push, and the unread badge waits for the reload.
    pub discard_in_background_after_minutes: u64,
    /// How much of the disk the HTTP cache may use, in megabytes; `0` leaves
    /// it to Chromium. Its own choice was measured near 300 MB, which an
    /// evening of Reels fills with video and then pays for by evicting
    /// Instagram's scripts, fetched again at the next start. See
    /// `http_cache_bytes`.
    pub http_cache_size_mb: u32,
    /// Show the number of unread items on the task bar icon, read from the
    /// `(3)` Instagram puts in front of its page title.
    pub unread_badge: bool,
    /// Start with the desktop session, in the background, the way Discord and
    /// Telegram do — so a message that arrives before you have opened
    /// anything still reaches you.
    ///
    /// This decides what the **first** run of a profile puts in
    /// `~/.config/autostart`, and nothing after that: from then on the entry
    /// itself is the truth, so turning instaCache off in your desktop's own
    /// startup panel stays turned off. See `autostart.rs`.
    ///
    /// On for Instagram and off for another site by default: a site you open
    /// and close has no reason to be running before you ask for it.
    pub start_with_session: bool,
    /// An icon in the system tray for as long as instaCache runs, the way
    /// Spotify or Discord have one: it shows that a closed window is still
    /// running, and opens or quits it. Where the desktop has no tray, the
    /// setting has no effect and nothing breaks.
    ///
    /// On for Instagram and off for another site by default.
    pub tray_icon: bool,
    /// Hosts allowed to render inside the window, as an allow-list.
    ///
    /// This is the security boundary of the app: the window holds a logged-in
    /// session, so a host that is not named here — or a sub-domain of one — is
    /// handed to the system browser instead of being rendered next to it.
    ///
    /// It is per profile so a second profile can be a dedicated window for a
    /// different site: point `home_url` at it and name its domains here. The
    /// list stays an allow-list either way — adding `x.com` lets in `x.com`
    /// and its sub-domains, not the web.
    ///
    /// The Meta hosts in the default are not decoration: Instagram's login,
    /// two-factor and Accounts Center flows redirect through `facebook.com`
    /// and `meta.com`, and removing them breaks signing in. `threads.com` and
    /// `threads.net` are there by choice rather than necessity, and are the
    /// entries to drop for a window that stays on Instagram alone.
    ///
    /// An empty list restores the default rather than locking every
    /// navigation out of the window, which would make the app unusable and is
    /// never what an empty value in a config file means.
    pub internal_domains: Vec<String>,
    /// Open non-Meta links in the system browser instead of inside the app.
    pub open_external_links_in_browser: bool,
    /// Hunspell dictionaries to load, e.g. `["en_US", "fr_FR"]`. Empty disables
    /// spell checking entirely.
    pub spell_checking_languages: Vec<String>,
    /// Initial zoom when no window state has been saved yet.
    pub default_zoom: f64,
    /// Persist window size/position/zoom between runs.
    pub remember_window_state: bool,
    /// Show the thin loading bar at the top of the window.
    pub show_loading_indicator: bool,
    /// Open maximized the first time, and whenever no geometry has been saved.
    pub start_maximized: bool,
    /// Check GitHub for a newer release and install it. Nothing else updates
    /// instaCache: it is installed from an archive, not by a package manager.
    pub auto_update: bool,
    /// How long to wait between checks. `0` checks on every launch only.
    pub update_check_interval_hours: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            home_url: DEFAULT_HOME_URL.to_string(),
            user_agent: DEFAULT_USER_AGENT.to_string(),
            hardware_acceleration: "always".to_string(),
            video_decoding: "gpu".to_string(),
            allow_autoplay_with_sound: true,
            videos_with_sound: true,
            video_volume: DEFAULT_VIDEO_VOLUME,
            instagram_colors: true,
            ambient_colors: true,
            page_animations: true,
            shortcuts: true,
            hide_also_from_meta: true,
            context_menu: false,
            developer_tools: false,
            notifications: true,
            push_notifications: true,
            notification_sounds: true,
            ring_for_calls: true,
            calls: true,
            run_in_background: true,
            freeze_in_background: true,
            discard_in_background_after_minutes: DEFAULT_DISCARD_MINUTES,
            http_cache_size_mb: DEFAULT_HTTP_CACHE_MB,
            unread_badge: true,
            start_with_session: true,
            tray_icon: true,
            internal_domains: crate::urls::INTERNAL_DOMAINS
                .iter()
                .map(|domain| (*domain).to_string())
                .collect(),
            open_external_links_in_browser: true,
            spell_checking_languages: Vec::new(),
            default_zoom: 1.0,
            remember_window_state: true,
            show_loading_indicator: true,
            start_maximized: false,
            auto_update: true,
            update_check_interval_hours: DEFAULT_UPDATE_CHECK_HOURS,
        }
    }
}

impl Config {
    /// The defaults for a window whose home page is `home_url`.
    ///
    /// Instagram is a messenger as well as a feed, and gets what a messenger
    /// needs: calls, ringing, a tray icon, and staying alive when closed.
    /// Another site added with `--add-site` — X, say — is a feed you open and
    /// close, and gets none of those unless its config asks. Everything else,
    /// the fixes and the rest of the features, is the same for both.
    pub fn for_home(home_url: &str) -> Self {
        let mut config = Config {
            home_url: home_url.to_string(),
            ..Config::default()
        };
        if !is_instagram(home_url) {
            config.calls = false;
            config.ring_for_calls = false;
            config.run_in_background = false;
            config.tray_icon = false;
            config.start_with_session = false;
            // The mods are written for Instagram's page.
            config.videos_with_sound = false;
            config.instagram_colors = false;
            config.ambient_colors = false;
            config.page_animations = false;
            config.shortcuts = false;
            config.hide_also_from_meta = false;
            // A window that stops running when it is closed has nothing to
            // deliver a push to, so the connection to the push service would
            // be held for nothing.
            config.push_notifications = false;
        }
        config
    }

    /// Reads a config file's contents. A key the file leaves out takes the
    /// default for the file's own home page, not Instagram's: a site's config
    /// written before a setting existed must not inherit a messenger's
    /// behaviour just because it never mentioned it.
    pub fn from_json(raw: &str) -> Result<Self, serde_json::Error> {
        let written: serde_json::Value = serde_json::from_str(raw)?;
        if !written.is_object() {
            // Not a config at all; let serde say so in its own words.
            return serde_json::from_value(written);
        }
        let home = written
            .get("home_url")
            .and_then(serde_json::Value::as_str)
            .filter(|home| !home.trim().is_empty())
            .unwrap_or(DEFAULT_HOME_URL);
        let mut merged = serde_json::to_value(Config::for_home(home))?;
        if let (Some(merged), Some(written)) = (merged.as_object_mut(), written.as_object()) {
            for (key, value) in written {
                merged.insert(key.clone(), value.clone());
            }
        }
        serde_json::from_value(merged)
    }

    /// Reads `config.json`, falling back to defaults on any problem. A missing
    /// file is written out with the defaults so the knobs are discoverable.
    pub fn load_or_create(paths: &Paths) -> Self {
        let file = paths.config_file();
        match std::fs::read_to_string(&file) {
            Ok(raw) => match Config::from_json(&raw) {
                Ok(cfg) => cfg.normalized(),
                Err(err) => {
                    eprintln!(
                        "instacache: {} is not valid JSON ({err}); using defaults",
                        file.display()
                    );
                    Config::default()
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let cfg = Config::default();
                if let Err(err) = write_json(&file, &cfg) {
                    eprintln!("instacache: could not write {}: {err}", file.display());
                }
                cfg
            }
            Err(err) => {
                eprintln!("instacache: could not read {}: {err}", file.display());
                Config::default()
            }
        }
    }

    fn normalized(mut self) -> Self {
        if self.home_url.trim().is_empty() {
            self.home_url = DEFAULT_HOME_URL.to_string();
        }
        if SUPERSEDED_USER_AGENTS.contains(&self.user_agent.as_str()) {
            self.user_agent = DEFAULT_USER_AGENT.to_string();
        }
        self.default_zoom = self.default_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.video_volume = self.video_volume.min(100);
        if self.video_volume == SUPERSEDED_VIDEO_VOLUME {
            self.video_volume = DEFAULT_VIDEO_VOLUME;
        }
        // Written out by every first run before 2.4.0, so it says nothing
        // about what the user wants, only what the default was.
        if self.update_check_interval_hours == SUPERSEDED_UPDATE_CHECK_HOURS {
            self.update_check_interval_hours = DEFAULT_UPDATE_CHECK_HOURS;
        }

        // A host is compared lower-cased, and an empty entry would turn the
        // sub-domain check into a wildcard, so both are dealt with here rather
        // than at every comparison.
        self.internal_domains = self
            .internal_domains
            .iter()
            .map(|domain| domain.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|domain| !domain.is_empty())
            .collect();
        if self.internal_domains.is_empty() {
            self.internal_domains = Config::default().internal_domains;
        }

        self
    }
}

pub const MIN_ZOOM: f64 = 0.3;
pub const MAX_ZOOM: f64 = 5.0;

const DEFAULT_WIDTH: i32 = 1440;
const DEFAULT_HEIGHT: i32 = 900;

/// Bounds for the automatically chosen first-run size. The lower bounds keep
/// Instagram's three-column layout intact; the upper bounds stop the window
/// from spanning an entire ultrawide, where the site would just letterbox
/// itself in the middle anyway.
const MIN_START_WIDTH: i32 = 1280;
const MAX_START_WIDTH: i32 = 1920;
const MIN_START_HEIGHT: i32 = 860;
const MAX_START_HEIGHT: i32 = 1240;

fn scale(available: i32, factor: f64, min: i32, max: i32) -> i32 {
    let scaled = (f64::from(available) * factor).round() as i32;
    scaled.clamp(min.min(available.max(1)), max)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    pub width: i32,
    pub height: i32,
    /// `None` on Wayland, where clients cannot position their own windows.
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub maximized: bool,
    pub zoom: f64,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            x: None,
            y: None,
            maximized: false,
            zoom: 1.0,
        }
    }
}

impl WindowState {
    /// Returns `None` on a first run, so the caller can pick a size from the
    /// monitor instead of imposing a fixed one.
    pub fn load(paths: &Paths) -> Option<Self> {
        let raw = std::fs::read_to_string(paths.state_file()).ok()?;
        let state = serde_json::from_str::<WindowState>(&raw).ok()?;
        Some(state.sanitized())
    }

    /// First-run geometry: a large share of the monitor's usable area, kept
    /// within bounds that stay comfortable on both a laptop and an ultrawide.
    ///
    /// `work_area` is the monitor rectangle minus panels and docks.
    pub fn from_work_area(work_area: (i32, i32)) -> Self {
        let (available_width, available_height) = work_area;

        let width = scale(available_width, 0.90, MIN_START_WIDTH, MAX_START_WIDTH);
        let height = scale(available_height, 0.92, MIN_START_HEIGHT, MAX_START_HEIGHT);

        // On a screen too small to host the preferred size, a maximized window
        // beats one that overflows the work area.
        let maximized = width >= available_width || height >= available_height;

        Self {
            width,
            height,
            x: None,
            y: None,
            maximized,
            zoom: 1.0,
        }
    }

    pub fn save(&self, paths: &Paths) {
        if let Err(err) = write_json(&paths.state_file(), self) {
            eprintln!("instacache: could not save window state: {err}");
        }
    }

    /// Guards against a corrupt or stale state file producing an unusable
    /// window (zero-sized, or placed off every connected monitor).
    fn sanitized(mut self) -> Self {
        if self.width < 360 {
            self.width = DEFAULT_WIDTH;
        }
        if self.height < 360 {
            self.height = DEFAULT_HEIGHT;
        }
        if self.zoom.is_nan() {
            self.zoom = 1.0;
        }
        self.zoom = self.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self
    }
}

/// Atomic write: a crash mid-save leaves the previous file intact rather than
/// a half-written one.
/// Writes a configuration file, creating its directory. Used when a site is
/// added, which is the only path that writes a config the user has not seen.
pub fn write(path: &Path, config: &Config) -> std::io::Result<()> {
    write_json(path, config)
}

/// Whether a home page is Instagram's.
pub fn is_instagram(home_url: &str) -> bool {
    crate::urls::is_internal_in(&["instagram.com"], home_url)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let cfg: Config = serde_json::from_str(r#"{"developer_tools": true}"#).unwrap();
        assert!(cfg.developer_tools);
        assert_eq!(cfg.home_url, DEFAULT_HOME_URL);
        assert_eq!(cfg.user_agent, DEFAULT_USER_AGENT);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let cfg: Config = serde_json::from_str(r#"{"from_a_future_version": 42}"#).unwrap();
        assert_eq!(cfg.home_url, DEFAULT_HOME_URL);
    }

    #[test]
    fn the_context_menu_is_off_but_can_be_asked_for() {
        assert!(!Config::default().context_menu);
        let cfg: Config = serde_json::from_str(r#"{"context_menu": true}"#).unwrap();
        assert!(cfg.context_menu);
    }

    #[test]
    fn threads_is_not_in_the_default_allow_list() {
        let cfg = Config::default();
        assert!(!cfg.internal_domains.iter().any(|d| d.contains("threads")));
    }

    #[test]
    fn the_default_allow_list_is_the_built_in_one() {
        let cfg = Config::default();
        assert!(cfg.internal_domains.iter().any(|d| d == "instagram.com"));
        assert!(
            cfg.internal_domains.iter().any(|d| d == "facebook.com"),
            "the login flow redirects through Meta hosts"
        );
    }

    #[test]
    fn a_configured_allow_list_replaces_the_default() {
        let cfg: Config =
            serde_json::from_str(r#"{"internal_domains": ["  X.com ", ".news.ycombinator.com"]}"#)
                .unwrap();
        let cfg = cfg.normalized();
        assert_eq!(cfg.internal_domains, ["x.com", "news.ycombinator.com"]);
    }

    #[test]
    fn an_empty_allow_list_falls_back_rather_than_locking_the_window() {
        let cfg: Config = serde_json::from_str(r#"{"internal_domains": ["", "   "]}"#).unwrap();
        let cfg = cfg.normalized();
        assert_eq!(cfg.internal_domains, Config::default().internal_domains);
    }

    #[test]
    fn video_decoding_defaults_to_the_gpu() {
        assert_eq!(Config::default().video_decoding, "gpu");
    }

    #[test]
    fn the_superseded_decoding_key_is_ignored() {
        // 1.1.x wrote `hardware_video_decoding`; the replacement key wins.
        let cfg: Config = serde_json::from_str(r#"{"hardware_video_decoding": false}"#).unwrap();
        assert_eq!(cfg.video_decoding, "gpu");
    }

    const QT_64: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
        (KHTML, like Gecko) QtWebEngine/6.4.2 Chrome/102.0.5005.177 Safari/537.36";

    #[test]
    fn the_default_user_agent_is_chrome_on_linux_at_the_engines_version() {
        let sent = user_agent_for(DEFAULT_USER_AGENT, QT_64);
        assert_eq!(
            sent,
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/102.0.0.0 Safari/537.36"
        );
        // No token no site knows, no other platform, no Safari code path.
        assert!(!sent.contains("QtWebEngine") && !sent.contains("Mac OS X"));
        assert!(!sent.contains("Version/"));
    }

    #[test]
    fn a_configured_user_agent_is_sent_as_it_is_and_empty_is_the_engines() {
        assert_eq!(user_agent_for("Custom/1.0", QT_64), "Custom/1.0");
        assert_eq!(user_agent_for("", QT_64), QT_64);
        // An engine string with no version to read is sent as it is.
        assert_eq!(user_agent_for("auto", "Odd/1"), "Odd/1");
    }

    #[test]
    fn accept_language_follows_the_session_like_chrome() {
        let env = |vars: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                vars.iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert_eq!(
            accept_language(env(&[("LANG", "fr_FR.UTF-8")])),
            "fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7"
        );
        // LC_ALL wins over LANG; LANGUAGE adds after the primary.
        assert_eq!(
            accept_language(env(&[
                ("LANG", "en_GB.UTF-8"),
                ("LC_ALL", "de_DE.UTF-8"),
                ("LANGUAGE", "de:fr")
            ])),
            "de-DE,de;q=0.9,fr;q=0.8,en-US;q=0.7,en;q=0.6"
        );
        // No language set, or C: English, never nothing.
        assert_eq!(accept_language(env(&[])), "en-US,en;q=0.9");
        assert_eq!(
            accept_language(env(&[("LANG", "C.UTF-8")])),
            "en-US,en;q=0.9"
        );
    }

    #[test]
    fn the_cache_size_reaches_qt_as_an_int_of_bytes() {
        assert_eq!(http_cache_bytes(0), 0);
        assert_eq!(http_cache_bytes(1024), 1 << 30);
        assert_eq!(http_cache_bytes(2047), 2047 * 1024 * 1024);
        assert_eq!(http_cache_bytes(u32::MAX), 2047 * 1024 * 1024);
    }

    #[test]
    fn the_old_quiet_volume_is_raised_and_a_chosen_one_kept() {
        let old: Config = serde_json::from_str(r#"{"video_volume": 40}"#).unwrap();
        assert_eq!(old.normalized().video_volume, DEFAULT_VIDEO_VOLUME);
        let chosen: Config = serde_json::from_str(r#"{"video_volume": 55}"#).unwrap();
        assert_eq!(chosen.normalized().video_volume, 55);
    }

    #[test]
    fn every_superseded_default_is_migrated() {
        for old in SUPERSEDED_USER_AGENTS {
            let raw = serde_json::json!({ "user_agent": old }).to_string();
            let cfg: Config = serde_json::from_str(&raw).unwrap();
            assert_eq!(
                cfg.normalized().user_agent,
                DEFAULT_USER_AGENT,
                "{old} should have been migrated"
            );
        }
    }

    #[test]
    fn a_site_is_not_a_messenger_unless_it_says_so() {
        // XCache's config as 2.2 wrote it: none of the messenger settings.
        let x =
            Config::from_json(r#"{"home_url": "https://x.com/", "internal_domains": ["x.com"]}"#)
                .unwrap();
        assert!(!x.run_in_background && !x.tray_icon && !x.calls && !x.ring_for_calls);
        assert!(
            !x.videos_with_sound
                && !x.instagram_colors
                && !x.ambient_colors
                && !x.page_animations
                && !x.shortcuts
                && !x.hide_also_from_meta,
            "the mods are written for Instagram"
        );
        assert!(
            !x.start_with_session,
            "a site you open and close has no reason to run before you ask"
        );
        assert!(
            !x.push_notifications,
            "a window that stops running when it is closed has nothing to push to"
        );
        // What every window shares.
        assert!(x.notification_sounds && x.unread_badge && x.notifications);

        let instagram = Config::from_json(r#"{"home_url": "https://www.instagram.com/"}"#).unwrap();
        assert!(instagram.run_in_background && instagram.tray_icon && instagram.calls);
        assert!(instagram.ring_for_calls && instagram.start_with_session);
        assert!(instagram.videos_with_sound && instagram.instagram_colors);
        assert!(instagram.page_animations && instagram.ambient_colors);
        assert!(instagram.shortcuts && instagram.hide_also_from_meta);
        assert!(
            instagram.push_notifications,
            "a config written before the key existed still gets the push service"
        );

        let chosen =
            Config::from_json(r#"{"home_url": "https://x.com/", "tray_icon": true}"#).unwrap();
        assert!(chosen.tray_icon, "a value in the file always wins");

        let no_home = Config::from_json("{}").unwrap();
        assert!(no_home.run_in_background, "no home page means Instagram's");
    }

    #[test]
    fn a_config_that_is_not_an_object_is_an_error() {
        assert!(Config::from_json("[1, 2]").is_err());
        assert!(Config::from_json("not json").is_err());
    }

    #[test]
    fn the_old_daily_update_check_becomes_the_new_default() {
        let cfg: Config = serde_json::from_str(r#"{"update_check_interval_hours": 24}"#).unwrap();
        assert_eq!(
            cfg.normalized().update_check_interval_hours,
            DEFAULT_UPDATE_CHECK_HOURS
        );
        for chosen in [0, 1, 12, 48] {
            let raw = serde_json::json!({ "update_check_interval_hours": chosen }).to_string();
            let cfg: Config = serde_json::from_str(&raw).unwrap();
            assert_eq!(cfg.normalized().update_check_interval_hours, chosen);
        }
    }

    #[test]
    fn a_user_chosen_agent_is_left_alone() {
        let cfg: Config = serde_json::from_str(r#"{"user_agent": "MyBrowser/1.0"}"#).unwrap();
        assert_eq!(cfg.normalized().user_agent, "MyBrowser/1.0");
    }

    #[test]
    fn empty_home_url_is_replaced() {
        let cfg: Config = serde_json::from_str(r#"{"home_url": "  "}"#).unwrap();
        assert_eq!(cfg.normalized().home_url, DEFAULT_HOME_URL);
    }

    #[test]
    fn first_run_geometry_fills_a_laptop_screen() {
        let state = WindowState::from_work_area((1920, 1080));
        assert_eq!(state.width, 1728);
        assert_eq!(state.height, 994);
        assert!(!state.maximized);
    }

    #[test]
    fn first_run_geometry_is_capped_on_an_ultrawide() {
        let state = WindowState::from_work_area((5120, 1440));
        assert_eq!(state.width, MAX_START_WIDTH);
        assert_eq!(state.height, MAX_START_HEIGHT);
        assert!(!state.maximized);
    }

    #[test]
    fn tiny_screens_start_maximized_instead_of_overflowing() {
        let state = WindowState::from_work_area((1366, 768));
        assert!(state.maximized);
        assert!(state.width <= 1366);
        assert!(state.height <= 768);
    }

    #[test]
    fn degenerate_window_state_is_repaired() {
        let state = WindowState {
            width: 0,
            height: -10,
            zoom: f64::NAN,
            ..WindowState::default()
        };
        let fixed = state.sanitized();
        assert_eq!(fixed.width, DEFAULT_WIDTH);
        assert_eq!(fixed.height, DEFAULT_HEIGHT);
        assert_eq!(fixed.zoom, 1.0);
    }
}
