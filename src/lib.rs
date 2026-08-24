//! instaCache — a native, ultra-light Instagram client for Linux.
//!
//! One GTK window hosting a single WebKitGTK view, with the browser session
//! and the HTTP cache pinned to persistent per-profile directories.
//!
//! The crate is split into a library and a thin binary so the rendering
//! verification helper (`cargo run --example snapshot`) exercises exactly the
//! same WebKit configuration the app ships with.

pub mod config;
pub mod errorpage;
pub mod paths;
pub mod progress;
pub mod shortcuts;
pub mod ui;
pub mod updates;
pub mod urls;
pub mod web;

pub const APP_NAME: &str = "instaCache";
pub const APP_ID: &str = "io.github.lightzirconite.instaCache";
pub const ICON_NAME: &str = "instacache";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Wayland compositors and X11 window managers match a window to its
/// `.desktop` file through GTK3's program name, which is also what
/// `StartupWMClass=instacache` refers to. Keep the three in sync or the app
/// shows up with a generic icon in the dock.
pub const PROGRAM_NAME: &str = "instacache";
