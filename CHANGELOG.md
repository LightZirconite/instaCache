# Changelog

All notable changes to GramCache are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

## [1.0.0] - 2026-08-24

First release.

### Added
- Native GTK 3 + WebKitGTK 4.1 window hosting Instagram, in a 517 KB binary.
- Persistent, aggressive disk cache (`CacheModel::WebBrowser`) rooted in
  `~/.cache/gramcache`, so a warm start reuses everything already downloaded.
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
- Optional user stylesheet at `~/.config/gramcache/user.css`.
- `install.sh` / `uninstall.sh` for any distribution, and a `PKGBUILD` for
  Arch-based systems.
- Tag-triggered GitHub/Gitea Actions workflow producing x86_64 and aarch64
  archives.
