# Changelog

All notable changes to instaCache are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

## [2.6.0] - 2026-09-16

### Added
- **Sounds like a messenger.** A new message plays a short sound with its
  notification, and an incoming call rings on repeat until the window is
  opened, the notification is clicked or dismissed, or thirty seconds pass.
  The notification server plays message sounds where it can, so Do Not Disturb
  silences them; ringing checks it too. Settings: `notification_sounds`,
  `ring_for_calls`.
- Notifications are filed under the right application, with its icon — a
  site's own for a site's window.

### Changed
- **A site is not a messenger.** XCache, and any site added with `--add-site`,
  no longer keeps running when closed, has no tray icon, is not given the
  microphone or camera and does not ring. Closing it frees its memory. It keeps
  everything else Instagram's window has. A value written in a site's own
  config still wins.

### Fixed
- **Closing the window now really puts the page to rest.** Measured, a hidden
  window's page still thought it was visible: its timers ran at full speed and
  its videos kept playing, sound included. Videos and Reels are now paused, and
  Chromium is told the page is hidden. A call keeps going.
- **No blank screen when opening a closed window.** The page as it was left is
  shown at once, and the live page takes over when Chromium has drawn it — 78
  to 308 ms later on the reference machine. The 2.5.0 fix forced a redraw but
  still showed black while Instagram redrew.
- An update installed from outside the app — `instacache --update` in a
  terminal — is now noticed by a copy running in the background, which restarts
  into it instead of running the old version indefinitely.

## [2.5.0] - 2026-09-16

### Added
- **A tray icon** while instaCache runs, open or closed to the background:
  click to open or hide the window, right-click to quit or to restart into a
  downloaded update. The tooltip shows the unread count. Off with `tray_icon`;
  a desktop without a tray simply has no icon.

### Fixed
- A window opened again after being closed to the background came back with
  a blank page until something was clicked. It now draws at once.
- Opening a closed window from the menu answers faster: the running copy looks
  for a second launch every 80 ms while hidden, instead of every 250 ms.
- The notice shown the first time the window is closed says what to do in
  plain words, and points at the tray icon when there is one, instead of
  naming a setting in backticks.

## [2.4.0] - 2026-09-16

### Changed
- **Updates work the way a browser's do.** instaCache checks at startup and
  every 6 hours while it runs, installs a new release in the background, and
  lets it take over by restarting — on its own and invisibly when the window is
  closed and no call is running, or from an **Update ready — Restart** pill in
  an open window, which reopens the same page. Before, an update waited for you
  to quit and relaunch after a notification, which in the background could mean
  weeks.
- A system-wide install can be updated from the window: the pill asks for the
  administrator password through `pkexec`.
- The default `update_check_interval_hours` is 6. A config file still holding
  the old default of 24 is read as the new one.

### Added
- `--background` starts instaCache with its window hidden.

## [2.3.0] - 2026-09-16

Withdrawn within the hour, before anyone had downloaded it; everything in it
ships in 2.4.0.

### Added
- **Pages instead of tabs.** Whatever Instagram opens in a new tab or window —
  a call above all — appears in the same window over Instagram, with a small
  back button. Opening another page replaces it, so the window stays one
  application.
- **Calls keep running while you browse.** Going back from a call hides it
  rather than ending it; a pill at the top of the window returns to it or ends
  it, and the page closes by itself when the call does.
- **Closing the window keeps instaCache running**, so opening it again is
  instant and notifications keep arriving. `Ctrl+Q` quits. Measured with a warm
  cache, a cold launch spends 2 to 2.5 seconds before the feed shows, almost all
  of it in Qt WebEngine, Chromium and Instagram's own loading; this is the part
  a reopened window skips. Off with `run_in_background`; you are told once.
- **Unread count on the task bar icon**, read from Instagram's title and sent
  through Unity's launcher API. Off with `unread_badge`.
- `Ctrl+1` to `Ctrl+4` jump to the feed, Explore, Reels and Direct through
  Instagram's own links, without reloading the page.
- The mouse's back and forward buttons also leave a page or a call.
- `tests/scene/run.sh` runs the QML scene under qmltestrunner, off screen,
  with a stand-in for the Rust bridge.

### Changed
- `Ctrl+W` closes the page on top, and the window when there is none; it no
  longer quits. `Ctrl+Q` still does.
- An instance left running in the background asks every hour whether an update
  check is due, instead of only at startup.
- The French README is gone; the documentation is English only.

### Fixed
- Calls in Direct can use the microphone and camera. Both were refused to
  every page, Instagram included. They are now granted to the hosts in
  `internal_domains`, and the new `calls` setting turns that off.
- `Esc` now reaches Instagram outside fullscreen. The shortcut took the key
  even when it had nothing to do with it, so Instagram's dialogs did not close
  on it.

## [2.2.1] - 2026-08-25

### Fixed
- A menu entry now names the installed binary rather than whichever copy
  created it. Running `--add-site` from a build tree wrote an entry pointing
  into `target/debug`, so the site stopped opening as soon as that binary was
  rebuilt or removed. The installed copy is preferred, the running one is the
  fallback for an install that is not on `PATH`.

## [2.2.0] - 2026-08-25

### Added
- **XCache**, X in its own window, is set up by the installer. It is the only
  site instaCache ships, and it is added once: the rule is "this profile has
  never existed", not "this entry is missing", so a site somebody removes stays
  removed instead of coming back with the next update. `--setup-sites` runs the
  same step by hand.

### Fixed
- A site's icon is installed into the icon theme and named there, instead of
  being pointed at by absolute path from the entry. `Icon=` accepts a path, and
  the application menu honoured it, but a task bar resolving a window to an
  application goes through the icon theme — which is why the menu showed the
  site's logo while the task bar still showed instaCache's. Removing a site
  removes its icon again; `uninstall.sh` clears any that are left.
- Adding a site now refreshes KDE's cache as well as the freedesktop ones. A
  new entry was otherwise invisible to the task bar until the next login, which
  made the icon look wrong when it was merely not yet known.

## [2.1.1] - 2026-08-25

### Fixed
- A site added to the menu now keeps its own icon **in the task bar**, not just
  in the application menu. Its window announced `instacache` like every other,
  so the task bar matched it to `instacache.desktop` and drew instaCache's icon
  over the site's.

  The cause is worth writing down: `QCoreApplication::setApplicationName` does
  not set the Wayland `app_id`. Qt's Wayland plugin calls
  `QGuiApplication::desktopFileName()` and passes that to
  `xdg_toplevel::set_app_id`, ignoring the application name entirely. No Rust
  binding exposes `setDesktopFileName`, so this release adds a `build.rs` and
  the one C++ call in the codebase to make it. A wrong icon in the task bar and
  the right one in the menu is the signature of exactly this mistake.

## [2.1.0] - 2026-08-25

### Changed
- **Threads is no longer part of the app.** It was in the allowed-host list by
  choice rather than necessity — Instagram works without it — so a click on
  Instagram's Threads button now leaves for the system browser like any other
  link to a different application. Name `threads.com` in `internal_domains` to
  put it back.
- **The right-click menu is off.** In a window that exists to be one
  application it is browser chrome — Back, Forward, View Source, Inspect — and
  it covers the page while Instagram's own right-click handling stops working.
  This is a trade-off rather than a free win: it also removes "Save image as"
  and "Copy link address", so `context_menu: true` brings the menu back.

### Added
- **`--add-site`: another site, in its own window, with its own icon.**
  `instacache --add-site X https://x.com/ --domains x.com,twimg.com` writes a
  profile and a `.desktop` entry, and X appears in the application menu as its
  own application — own window, own session, own cache, own cookies. Nothing in
  the window was ever Instagram-specific; only the allowed-host list was, and
  that became configurable in the same release.

  `--domains` defaults to the URL's own host, which is right for a site that
  serves everything itself and wrong for one whose images come from a separate
  CDN, so it can be stated. `--list-sites` shows what has been added and
  `--remove-site` takes an entry back out without touching the session behind
  it.

  **A site keeps its own logo.** It is taken from the site: the largest icon it
  declares in its markup, or its `/favicon.ico`. GitHub publishes a 512x512 one
  and gets that; X declares none, so its favicon is used. The format is decided
  by the bytes rather than the file name, because `x.com/favicon.ico` is
  actually a PNG and writing it as `icon.ico` produces a file some icon loaders
  refuse. The image lands in the profile's own directory and the entry points
  at it by absolute path, which the specification allows -- so nothing is
  installed into the shared icon theme, and clearing a session or a cache never
  takes the icon with it. `--icon` overrides the choice; a site that publishes
  nothing usable keeps instaCache's own icon.

  Each site gets its own window class, `instacache-<profile>`, so the desktop
  gives it a separate icon instead of piling every window under one. The class
  and the entry's `StartupWMClass` come from the same function, because a
  disagreement between them shows the window as a second, unnamed item next to
  its own launcher. `uninstall.sh` removes the entries it finds.
- `user.js`: JavaScript in `~/.config/instacache/user.js` runs on every page
  once it has loaded, in the page's own world. Qt WebEngine implements no
  extension API — no Chrome Web Store, no `.crx`, no uBlock — so this is the
  nearest thing to one. It is trusted completely; a mistake in it is reported
  to the console rather than breaking the page, but it is your file running
  with the page's privileges.
- `internal_domains`, per profile: which hosts are allowed to render inside the
  window. It is an allow-list — naming `x.com` lets in `x.com` and its
  sub-domains, not the web — which means a second profile can now be a
  dedicated window for another site: point `home_url` at it and name its
  domains. An empty list restores the default rather than locking the window.
- The bench can now show whether the disk cache is doing anything, by counting
  the requests that reach the server rather than timing a transfer that is too
  fast over loopback to time. Measured: a cold start fetches 12 assets, a warm
  start after a restart fetches none, and still loads them with the files
  deleted from the server.

## [2.0.2] - 2026-08-24

### Fixed
- The release job states its shell instead of relying on the container's
  default. 2.0.1 was tagged but never published: inside a container the shell
  falls back to `sh` when bash is not found, and the version check uses
  `set -o pipefail`, which dash rejects with an exit code that reads as a
  build failure. Nothing in the application changed between 2.0.0 and 2.0.2.

## [2.0.1] - 2026-08-24

### Fixed
- Release builds are produced inside Debian 12 rather than on the runner's own
  Ubuntu. 2.0.0 was tagged but never published: its build failed on both
  architectures because the runner carried Qt 6.2, below the Qt 6.4 baseline
  this project documents. Debian 12 is the one environment with both the right
  Qt and a glibc old enough for the binary to start there — the newer Ubuntu
  that has Qt 6.4 would have locked Debian 12 out. Nothing in the application
  changed between 2.0.0 and 2.0.1.

## [2.0.0] - 2026-08-24

### Changed
- **The rendering engine is now Qt WebEngine instead of WebKitGTK.** A Reels
  feed builds and throws away a video about twice a second, and WebKit builds a
  fresh GStreamer pipeline for each one on the thread that also runs the page.
  On the reference machine that cost 78 frames over 50 ms in a 40-second run;
  the same run on Qt WebEngine costs 1 to 6 across five runs, and video shows
  its first frame in about 50 ms instead of 264. Chromium reuses its decoders; WebKit does not, and no
  WebKit setting closed the gap — `bench/README.md` lists everything that was
  tried, with numbers.

  The engine is still the distribution's, still linked dynamically and still
  never vendored. What it costs: the binary grows from 517 KB to 2.1 MB of
  Rust-to-Qt glue, and the dependency becomes `qt6-webengine` rather than
  `webkit2gtk-4.1` plus a GStreamer plugin set. Qt 6.4 is the baseline, which
  is what Debian 12 ships.
- `video_decoding` and `hardware_acceleration` keep their names and their
  meaning, but are now carried out as Chromium command-line flags rather than
  GStreamer plugin ranks. `gpu`, the default, switches on the VA-API decoding
  Chromium disables on Linux. An existing `config.json` needs no change.
- The default user agent reports Chrome on Linux instead of Safari on Linux.
  Instagram serves different code to different engines, and claiming Safari on
  a Chromium engine is a bug waiting to happen rather than a useful disguise.
  The system is still reported honestly, because Instagram puts it in its
  login-alert emails. A config still holding either old default is migrated; a
  hand-picked one is left alone.
- Desktop notifications go out over D-Bus directly instead of through GIO, and
  a click on one still raises the window and reaches the page.
- Only one window per profile still opens, now over a Unix socket in
  `XDG_RUNTIME_DIR` rather than a GtkApplication D-Bus name. A socket left
  behind by a process that died is cleaned up rather than treated as a running
  instance.

### Migration
- **You will be signed in again on the first launch.** A Chromium engine cannot
  read WebKit's cookie jar or local storage, so the session does not carry
  across. This is the one thing the update cannot do for you.
- The old engine's files stay behind, unused, in
  `~/.local/share/instacache`: `cookies.sqlite`, `localstorage/`,
  `serviceworkers/`, `storage/` and `mediakeys/`. Deleting them is safe and
  frees a few hundred kilobytes; keeping them costs nothing either.

### Added
- `bench/`, the measurement harness itself: the churn page, a collector, a
  runner for six engines and every number established so far. It used to live
  outside the repository, which meant claims about smoothness had to be taken
  on trust. Its clips are generated by a script, so nothing binary is
  committed.

### Fixed
- The stated cause of the stutter was wrong, and had been for a while.
  WebKit's MediaSource path handled a churning feed at 4 late frames; the path
  that stalled is the plain `<video src="…mp4">` one, which is what a Reels
  feed uses. The old bench only ever exercised the other path.

### Removed
- `video_gl_sink`, which existed only for a WebKit GStreamer sink. The measured
  improvement it carried — 78 late frames down to 20-30 — is superseded by the
  engine change and kept in the history.

## [1.2.0] - 2026-08-24

### Changed
- The user agent reports Linux instead of macOS. Instagram puts the user
  agent's operating system in its login-alert emails, so a Linux machine
  announcing macOS made those alerts read as though somebody else had signed
  in. A config still holding the old macOS string is migrated; a
  hand-picked one is left alone.
- `hardware_video_decoding` becomes `video_decoding`, with `gpu`, `software`
  and `auto`. `gpu` stays the default and is now backed by measurement rather
  than by reading GStreamer's rank table.

### Added
- `allow_autoplay_with_sound`, on by default. WebKit silences any video that
  starts without a click, which in a dedicated Instagram window reads as the
  app muting itself.
- A jank meter in the stress harness. It counts frames over 50 ms from inside
  the page, which is what a person perceives; CPU averages do not measure
  stutter at all.

### Known
- A Reels feed stutters, and the decoder is not the cause. WebKit gives every
  `<video>` its own GStreamer pipeline and a feed builds one about twice a
  second; the same streams playing without that churn produce 2 late frames
  instead of 27. The cost is in WebKit's Media Source implementation. See
  "Video performance" in the README for the numbers.

## [1.1.1] - 2026-08-24

### Fixed
- The app aborted while browsing. The loading bar cancelled a one-shot GLib
  timeout that had already fired and removed itself, so `SourceId::remove`
  panicked on an id GLib had since handed to somebody else — and with
  `panic = "abort"` the panic took the whole process down. No timeout source is
  cancelled any more; a generation counter decides whether a pending action is
  still wanted. Reproduced with `cargo run --example stress` before the fix and
  survived 83 navigations after it.
- Clicking a desktop notification could abort the same way. The handler held a
  `RefCell` borrow while running page JavaScript that can post another
  notification, which writes to that same cell.

### Added
- `allow_autoplay_with_sound`, on by default. WebKit follows the web's rule
  that a video starting without a click must be silent, which in a dedicated
  Instagram window reads as the app muting itself.
- `cargo run --example stress`, a harness that drives the browser from
  JavaScript. It reproduces crashes that only appear while scrolling, on
  machines where injected input and screenshots do not work — a remote desktop,
  for instance. It drives a local page by default and never touches a real site
  unless asked.

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
