<div align="center">

<img src="assets/gramcache.svg" width="112" alt="GramCache">

# GramCache

**A native, ultra-light Instagram client for Linux.**

A single 517 KB binary. No Electron, no Node, no Python.

[Install](#install) · [Why](#why) · [Shortcuts](#keyboard-shortcuts) · [Configuration](#configuration) · [Build](#build-from-source)

*[Lire ce document en français](docs/README.fr.md)*

</div>

---

## What it is

GramCache puts Instagram in a proper desktop window: it opens in your dock,
remembers where you left it, keeps you signed in, and stays out of the way.

Underneath, it is one GTK 3 window hosting one WebKitGTK view — the same engine
Safari and GNOME Web use — with its cache and session pinned to persistent
directories. Nothing is bundled, nothing is duplicated: the browser engine
already on your system does the rendering.

## Why

| | GramCache | An Electron wrapper | A browser tab |
|---|---|---|---|
| Download size | **517 KB** | 80–150 MB | — |
| Bundled browser engine | none (uses system WebKitGTK) | a full Chromium | — |
| Own dock icon and window | yes | yes | no |
| Survives closing the browser | yes | yes | no |
| Session kept between runs | yes | yes | yes |
| Disk cache reused on restart | yes, aggressively | usually | yes |

Measured on the reference machine: **433 MB** of proportional memory with the
feed open, across the three WebKit processes. That is roughly what a single
Instagram tab costs in Chrome, and well under a typical Electron wrapper — but
it is still a full browser engine rendering a heavy web app, not a toy.

## Features

- **Aggressive persistent cache.** WebKit's largest cache budget
  (`CacheModel::WebBrowser`) plus the back/forward page cache, written to
  `~/.cache/gramcache` and reused on every launch. A warm start does not
  re-download the interface.
- **You stay signed in.** Cookies, local storage, IndexedDB and service workers
  live in `~/.local/share/gramcache` and survive restarts, reboots and cache
  clears.
- **The window remembers itself.** Size, position, maximized state and zoom are
  restored — including when the desktop session ends and the app is terminated
  rather than closed.
- **Sized for your screen.** The first launch takes 90% of your monitor's usable
  area instead of a fixed default, so nothing is cut off.
- **Real keyboard navigation.** Reload, hard reload, back, forward, home, zoom,
  fullscreen.
- **External links leave.** Anything that is not Instagram — or one of the Meta
  hosts its login flow needs — opens in your default browser.
- **Desktop notifications.** Web notifications become real notifications;
  clicking one focuses the window and tells the page, so the right conversation
  opens.
- **Several accounts at once.** `gramcache --profile work` gets its own session,
  cache and window, running alongside your main one.
- **A proper offline page** instead of WebKit's default error screen.

## Install

### Any distribution — the installer

Download the archive for your architecture from the
[releases page](https://git.justw.tf/LightZirconite/instaCache/releases),
then:

```sh
tar -xzf gramcache-1.0.0-linux-x86_64.tar.gz
cd gramcache-1.0.0-linux-x86_64
./install.sh
```

That is the whole thing. The installer:

- installs into `~/.local` — **no root required**;
- adds GramCache to your application menu with its icon;
- checks that the WebKitGTK 4.1 runtime is present and, if it is not, prints the
  exact package to install for your distribution.

Then launch **GramCache** from your application menu.

Other options:

```sh
./install.sh --system            # /usr/local, for every user
./install.sh --prefix ~/apps     # anywhere you like
./install.sh --build             # build from source instead of using the binary
./install.sh --help
```

To remove it:

```sh
./uninstall.sh            # removes the app, keeps your session
./uninstall.sh --purge    # also deletes session, cache and settings
```

### Arch, CachyOS, Manjaro, EndeavourOS

```sh
git clone https://git.justw.tf/LightZirconite/instaCache.git
cd instaCache/packaging
makepkg -si
```

### Runtime requirement

GramCache links against the WebKitGTK 4.1 and GTK 3 libraries already packaged
by your distribution. It does not bundle a browser.

| Distribution | Command |
|---|---|
| Arch, CachyOS, Manjaro | `sudo pacman -S --needed webkit2gtk-4.1 gtk3` |
| Debian, Ubuntu, Mint | `sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0` |
| Fedora, RHEL | `sudo dnf install webkit2gtk4.1 gtk3` |
| openSUSE | `sudo zypper install libwebkit2gtk-4_1-0 gtk3` |
| Alpine | `sudo apk add webkit2gtk-4.1 gtk+3.0` |
| Void | `sudo xbps-install -S webkit2gtk gtk+3` |

`install.sh` detects this for you and tells you if something is missing.

## Keyboard shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+R` · `F5` | Reload (the cache is used — this is the fast path) |
| `Ctrl+Shift+R` · `Shift+F5` | Reload, bypassing the cache |
| `Alt+←` · `Alt+→` | Back · Forward |
| `Ctrl+H` · `Alt+Home` | Go to your feed |
| `Ctrl+=` · `Ctrl+-` · `Ctrl+0` | Zoom in · out · reset |
| `F11` · `Esc` | Enter · leave fullscreen |
| `Ctrl+W` · `Ctrl+Q` | Quit |
| `Ctrl+Shift+I` · `F12` | Web Inspector (when enabled in the config) |

Two-finger swipe on a touchpad also goes back and forward.

## Command line

```
gramcache [OPTIONS] [URL]

  <URL>                  Open this Instagram URL instead of your feed.
  -p, --profile <NAME>   Use a separate session, cache and window.
      --clear-cache      Delete cached resources, stay signed in.
      --clear-session    Delete cookies and site storage (signs you out).
  -h, --help             Full help.
  -V, --version          Version.
```

Launching GramCache twice with the same profile focuses the existing window
instead of starting a second copy.

## Configuration

`~/.config/gramcache/config.json` is created on first run with every option at
its default. Edit it and restart.

| Key | Default | What it does |
|---|---|---|
| `home_url` | `https://www.instagram.com/` | Page opened at startup and by `Ctrl+H`. |
| `user_agent` | a macOS Safari string | Sent to Instagram. Empty string keeps WebKitGTK's own. |
| `hardware_acceleration` | `auto` | `auto`, `always` or `never`. Set `never` if you see rendering glitches. |
| `developer_tools` | `false` | Enables the Web Inspector and console output. |
| `notifications` | `true` | Forward web notifications to your desktop. |
| `open_external_links_in_browser` | `true` | Send non-Instagram links to your browser. |
| `spell_checking_languages` | `[]` | e.g. `["en_US", "fr_FR"]`. Empty disables spell checking. |
| `default_zoom` | `1.0` | Zoom used when no window state has been saved. |
| `remember_window_state` | `true` | Restore size, position and zoom. |
| `show_loading_indicator` | `true` | The thin gradient bar at the top of the window. |
| `start_maximized` | `false` | Always open maximized. |

### Custom styling

Drop CSS into `~/.config/gramcache/user.css` and it is applied to every page.

```css
/* Widen the feed on a large screen */
main[role="main"] { max-width: 1100px; }
```

### Where your data lives

| Path | Contents | Safe to delete |
|---|---|---|
| `~/.config/gramcache/` | `config.json`, `user.css`, window geometry | yes, resets settings |
| `~/.local/share/gramcache/` | cookies, local storage, IndexedDB — your session | yes, signs you out |
| `~/.cache/gramcache/` | the resource cache | yes, always |

Every path honours `XDG_*_HOME`, and can be redirected with
`GRAMCACHE_DATA_HOME`, `GRAMCACHE_CACHE_HOME` and `GRAMCACHE_CONFIG_HOME` for a
portable install.

## Build from source

```sh
sudo pacman -S --needed rust webkit2gtk-4.1 gtk3 pkgconf   # or your equivalent
git clone https://git.justw.tf/LightZirconite/instaCache.git
cd instaCache
cargo build --release
./install.sh
```

You need the `-dev` / `-devel` packages of GTK 3 and WebKitGTK 4.1 to compile —
`libgtk-3-dev` and `libwebkit2gtk-4.1-dev` on Debian and Ubuntu.

### Verifying that a page actually renders

Display-server screenshots are unreliable on some Wayland and XWayland setups.
This helper renders through WebKit itself and writes a PNG, so it works anywhere:

```sh
cargo run --example snapshot -- https://www.instagram.com/ shot.png
```

It uses the real application configuration and a throwaway profile, so your
session is never touched.

## Releasing

```sh
scripts/release.sh patch --dry-run   # preview
scripts/release.sh patch             # bump, changelog, commit, tag, push
```

Pushing the tag triggers `.github/workflows/release.yml`, which builds x86_64
and aarch64 archives and publishes the release with notes and checksums. The
workflow runs on GitHub Actions and on Gitea Actions.

## Architecture

```
src/
  main.rs        argument parsing and process startup
  lib.rs         module wiring and the application constants
  ui.rs          window assembly, signals, notifications, downloads
  web.rs         WebKit context, persistent storage, settings, link routing
  config.rs      config.json and window geometry
  paths.rs       XDG locations and profiles
  shortcuts.rs   keyboard navigation
  urls.rs        which hosts stay inside the app
  errorpage.rs   the offline page
examples/
  snapshot.rs    render a page to PNG through WebKit, for verification
```

The crate is split into a library and a thin binary so the snapshot helper
exercises exactly the configuration the app ships with.

## Project status and limits

- GramCache renders Instagram's own website. If Instagram changes something,
  GramCache follows automatically — but it also inherits any feature Instagram
  does not offer on the web.
- Camera, microphone, geolocation and pointer-lock permission requests are
  refused outright. The web app does not need them.
- This is an unofficial client. It is not affiliated with, endorsed by, or
  connected to Instagram or Meta. Instagram is a trademark of Meta Platforms, Inc.

## License

[MIT](LICENSE).
