//! instaCache — a native, ultra-light Instagram client for Linux.
//!
//! Copyright (C) 2026 LightZirconite.
//!
//! This program is free software: you can redistribute it and/or modify it
//! under the terms of the GNU Affero General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or (at your
//! option) any later version. It is distributed in the hope that it will be
//! useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero
//! General Public License in `LICENSE` for more details.
//!
//! One Qt Quick window hosting a single Chromium view through Qt WebEngine,
//! with the browser session and the HTTP cache pinned to persistent
//! per-profile directories.
//!
//! The engine is the distribution's `qt6-webengine`, linked dynamically and
//! never vendored — the same contract the WebKitGTK build kept, for the same
//! reason. What changed is which engine: WebKit builds a GStreamer pipeline
//! per `<video>` element and stalls the page while it does, which no setting
//! removed. See `bench/README.md` for the measurements that decided it.
//!
//! Everything that can be decided without a toolkit is decided in Rust —
//! which URL is internal, where a download goes, what the offline page says,
//! when to check for an update. The QML scene owns only the widgets.

pub mod alerts;
pub mod autostart;
pub mod badge;
pub mod bridge;
pub mod chromium;
pub mod config;
pub mod downloads;
pub mod errorpage;
pub mod http;
pub mod instance;
pub mod paths;
pub mod sites;
pub mod updates;
pub mod urls;

pub const APP_NAME: &str = "instaCache";
pub const APP_ID: &str = "io.github.lightzirconite.instaCache";
pub const ICON_NAME: &str = "instacache";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Shown by `--version`, because the AGPL asks an interactive program to say
/// so where a user can see it. Kept to three lines: a licence notice nobody
/// reads is worse than none.
pub const LICENSE_NOTICE: &str = "\
Copyright (C) 2026 LightZirconite. Licence AGPL-3.0-or-later.
This is free software: you are free to change and redistribute it, and to run
it for any purpose. There is NO WARRANTY. Source: <https://git.justw.tf/LightZirconite/instaCache>";

/// Wayland compositors and X11 window managers match a window to its
/// `.desktop` file through the application name, which is also what
/// `StartupWMClass=instacache` refers to. Keep the three in sync or the app
/// shows up with a generic icon in the dock.
pub const PROGRAM_NAME: &str = "instacache";
