//! User configuration (`config.json`) and persisted window geometry
//! (`window-state.json`).
//!
//! Both files are optional and every field falls back to a sane default, so a
//! truncated, hand-edited or older file never prevents the app from starting.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::paths::Paths;

pub const DEFAULT_HOME_URL: &str = "https://www.instagram.com/";

/// Safari on macOS. instaCache renders with WebKit, so a Safari user agent is
/// the honest match for the engine actually in use and lands on the code path
/// Instagram tests against WebKit. Override it in `config.json` if you prefer
/// to be served the Chrome/Blink variant.
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.3 Safari/605.1.15";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Page loaded at startup and by the "home" shortcut.
    pub home_url: String,
    /// Empty string keeps WebKitGTK's built-in user agent.
    pub user_agent: String,
    /// `auto` | `always` | `never`.
    pub hardware_acceleration: String,
    /// Enables the Web Inspector (Ctrl+Shift+I / right-click → Inspect).
    pub developer_tools: bool,
    /// Forward web notifications to the desktop notification daemon.
    pub notifications: bool,
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            home_url: DEFAULT_HOME_URL.to_string(),
            user_agent: DEFAULT_USER_AGENT.to_string(),
            hardware_acceleration: "auto".to_string(),
            developer_tools: false,
            notifications: true,
            open_external_links_in_browser: true,
            spell_checking_languages: Vec::new(),
            default_zoom: 1.0,
            remember_window_state: true,
            show_loading_indicator: true,
            start_maximized: false,
        }
    }
}

impl Config {
    /// Reads `config.json`, falling back to defaults on any problem. A missing
    /// file is written out with the defaults so the knobs are discoverable.
    pub fn load_or_create(paths: &Paths) -> Self {
        let file = paths.config_file();
        match std::fs::read_to_string(&file) {
            Ok(raw) => match serde_json::from_str::<Config>(&raw) {
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
        self.default_zoom = self.default_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
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
