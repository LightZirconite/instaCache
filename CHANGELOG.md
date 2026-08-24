# Changelog

All notable changes to instaCache are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

## [1.1.0] - 2026-08-24

### Added
- Self-updating. instaCache is installed from an archive, so nothing else would
  ever update it. It now asks GitHub once a day for a newer release and, when
  the install is one the user can write to, downloads it, verifies its SHA-256
  and installs it in the background, then says to restart. `instacache
  --update` does it on demand. Controlled by `auto_update` and
  `update_check_interval_hours`.
- One-line installer: `curl -fsSL .../get.sh | sh` downloads the release for
  the current architecture, verifies its published SHA-256 and runs the
  installer inside it.
- `install.sh` now installs the missing system packages itself instead of only
  naming them, through the distribution's package manager, after showing the
  exact command.
- The installer leaves `uninstall.sh` in `<prefix>/share/instacache/`, so a
  one-line install can still be undone.
- A loading bar that follows Instagram's in-app navigation, not just real page
  loads — those never fire `load-changed`, so the previous bar only ever moved
  at startup.
- Automatic recovery when WebKit's rendering process dies, replacing the grey
  unresponsive area with a reload and, after repeated crashes, an explanation.

### Changed
- `hardware_acceleration` defaults to `always`. Under `auto`, WebKit switched
  compositing modes mid-page, which appeared as single-frame freezes during
  video playback.
- GPU video decoders are preferred over the software ones through
  `GST_PLUGIN_FEATURE_RANK`, controlled by the new `hardware_video_decoding`
  setting.
- `enable_media_capabilities` is on, so Instagram can query what this machine
  decodes well before choosing a stream.

### Removed
- The Arch `PKGBUILD` and the AUR submission path. The one-line installer
  covers every distribution, including Arch.

## [1.0.0] - 2026-08-24

First release.

### Added
- Native GTK 3 + WebKitGTK 4.1 window hosting Instagram, in a 517 KB binary.
- Persistent, aggressive disk cache (`CacheModel::WebBrowser`) rooted in
  `~/.cache/instacache`, so a warm start reuses everything already downloaded.
- Persistent session: cookies in SQLite, local storage, IndexedDB and service
  workers all survive a restart, so you stay signed in.
- Window geometry, maximized state and zoom level remembered between runs,
  including when the session ends via `SIGTERM`.
- First-run window sized from the monitor's usable area instead of a fixed
  default.
- Keyboard navigation: reload, reload-without-cache, back, forward, home,
  zoom, fullscreen and quit.
- Links outside Instagram and the Meta hosts its login needs open in the
  system browser instead of inside the app.
- Web notifications forwarded to the desktop notification daemon, with a
  click routed back into the page.
- Downloads saved to the XDG download directory under a non-colliding name.
- Branded offline page shown instead of WebKit's default error page.
- Named profiles (`--profile work`) for running several accounts side by side.
- Optional user stylesheet at `~/.config/instacache/user.css`.
- `install.sh` / `uninstall.sh` for any distribution.
- Tag-triggered GitHub/Gitea Actions workflow producing x86_64 and aarch64
  archives.
