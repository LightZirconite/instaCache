<div align="center">

<img src="assets/instacache.svg" width="112" alt="instaCache">

# instaCache

**A native, ultra-light Instagram client for Linux.**

A single 517 KB binary. No Electron, no Node, no Python.

[Install](#install) · [Why](#why) · [Shortcuts](#keyboard-shortcuts) · [Configuration](#configuration) · [Build](#build-from-source)

*[Lire ce document en français](docs/README.fr.md)*

</div>

---

## What it is

instaCache puts Instagram in a proper desktop window: it opens in your dock,
remembers where you left it, keeps you signed in, and stays out of the way.

Underneath, it is one GTK 3 window hosting one WebKitGTK view — the same engine
Safari and GNOME Web use — with its cache and session pinned to persistent
directories. Nothing is bundled, nothing is duplicated: the browser engine
already on your system does the rendering.

## Why

| | instaCache | An Electron wrapper | A browser tab |
|---|---|---|---|
| Download size | **517 KB** | 80–150 MB | — |
| Bundled browser engine | none (uses system WebKitGTK) | a full Chromium | — |
| Own dock icon and window | yes | yes | no |
| Survives closing the browser | yes | yes | no |
| Session kept between runs | yes | yes | yes |
| Disk cache reused on restart | yes, aggressively | usually | yes |

Measured on the reference machine, adding up proportional memory across every
process: about **300 MB** on a fresh window, and **800 MB** on a signed-in feed
after a minute of scrolling through video. The second number is the honest one
for daily use.

That is what a browser engine costs to render a heavy web app; instaCache is
small, the web app is not. Where it wins is the 517 KB download, the absence of
a second browser engine on your disk, and a cache that makes the next launch
instant. It is not a lightweight way to *view* Instagram — it is a lightweight
*wrapper* around it.

## Features

- **Aggressive persistent cache.** WebKit's largest cache budget
  (`CacheModel::WebBrowser`) plus the back/forward page cache, written to
  `~/.cache/instacache` and reused on every launch. A warm start does not
  re-download the interface.
- **You stay signed in.** Cookies, local storage, IndexedDB and service workers
  live in `~/.local/share/instacache` and survive restarts, reboots and cache
  clears.
- **The window remembers itself.** Size, position, maximized state and zoom are
  restored — including when the desktop session ends and the app is terminated
  rather than closed.
- **Sized for your screen.** The first launch takes 90% of your monitor's usable
  area instead of a fixed default, so nothing is cut off.
- **A loading bar that actually tracks Instagram.** A thin gradient line across
  the top, the way YouTube does it. It follows real page loads *and* in-app
  navigation, which produces no page load at all and would otherwise leave the
  bar dead.
- **Hardware video decoding.** WebKit hands video to GStreamer, which by
  default may pick the CPU decoder over the GPU one; instaCache asks for the
  GPU decoders explicitly, which is what stops Reels from stuttering.
- **Real keyboard navigation.** Reload, hard reload, back, forward, home, zoom,
  fullscreen.
- **External links leave.** Anything that is not Instagram — or one of the Meta
  hosts its login flow needs — opens in your default browser.
- **Desktop notifications.** Web notifications become real notifications;
  clicking one focuses the window and tells the page, so the right conversation
  opens.
- **Several accounts at once.** `instacache --profile work` gets its own session,
  cache and window, running alongside your main one.
- **A proper offline page** instead of WebKit's default error screen.

## Install

One command. Paste it into a terminal:

```sh
curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh | sh
```

It downloads the release for your architecture, checks it against the published
SHA-256, installs instaCache into `~/.local` — **no root needed for the app
itself** — and adds it to your application menu.

It also checks the two system libraries instaCache needs and offers to install
the missing ones with your distribution's package manager. That step asks for
your password, because installing system packages requires it. Answer `y` and
it is done; the exact command is printed first so you can see what will run.

Then launch **instaCache** from your application menu.

<details>
<summary>Options</summary>

```sh
# Install for every user instead of just yours
curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh | sh -s -- --system

# Never ask anything, install missing packages automatically
curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh | sh -s -- --yes

# Only install the app, never touch system packages
curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh | sh -s -- --no-deps

# A specific version
INSTACACHE_VERSION=v1.0.0 sh -c "$(curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh)"
```

</details>

<details>
<summary>Prefer not to pipe a script into a shell?</summary>

Download the archive yourself from the
[releases page](https://github.com/LightZirconite/instaCache/releases), then:

```sh
tar -xzf instacache-*-linux-x86_64.tar.gz
cd instacache-*-linux-x86_64
./install.sh
```

Same installer, same result. `get.sh` only automates the download and verifies
the checksum for you.

</details>

## Updates

Nothing else updates instaCache — it is installed from an archive, not by a
package manager — so it updates itself.

On startup it asks GitHub once a day whether a newer release exists. If there
is one, and your install is in `~/.local` where no root is needed, it is
downloaded, checked against its SHA-256 and installed in the background. You
get a notification saying to restart. Nothing is ever replaced while you are
looking at it.

To check right now:

```sh
instacache --update
```

A system-wide install cannot update itself without root, so it only tells you
that a new version exists. Turn the whole thing off with `"auto_update": false`
in `config.json`.

## Uninstall

The installer leaves the uninstaller next to the app, so this works even if you
installed with the one-liner and no longer have the archive:

```sh
~/.local/share/instacache/uninstall.sh            # removes the app, keeps your session
~/.local/share/instacache/uninstall.sh --purge    # also deletes session, cache and settings
```

For a `--system` install the path is `/usr/local/share/instacache/uninstall.sh`.

## What it needs on your system

instaCache does not bundle a browser. It uses the WebKitGTK and GStreamer
libraries your distribution already packages, and the installer sets them up
for you. For reference:

| | Package (Arch) | Package (Debian/Ubuntu) | Without it |
|---|---|---|---|
| Rendering | `webkit2gtk-4.1 gtk3` | `libwebkit2gtk-4.1-0 libgtk-3-0` | Does not start |
| Video | `gst-plugins-good gst-plugins-bad gst-libav` | `gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav` | Photos load, every video stays blank |

If videos never play, that second row is almost always the reason. Run
`./install.sh` again and it will detect and fix it.

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
instacache [OPTIONS] [URL]

  <URL>                  Open this Instagram URL instead of your feed.
  -p, --profile <NAME>   Use a separate session, cache and window.
      --update           Check for a newer release and install it.
      --clear-cache      Delete cached resources, stay signed in.
      --clear-session    Delete cookies and site storage (signs you out).
  -h, --help             Full help.
  -V, --version          Version.
```

Launching instaCache twice with the same profile focuses the existing window
instead of starting a second copy.

## Configuration

`~/.config/instacache/config.json` is created on first run with every option at
its default. Edit it and restart.

| Key | Default | What it does |
|---|---|---|
| `home_url` | `https://www.instagram.com/` | Page opened at startup and by `Ctrl+H`. |
| `user_agent` | a macOS Safari string | Sent to Instagram. Empty string keeps WebKitGTK's own. |
| `hardware_acceleration` | `always` | `always`, `auto` or `never`. `auto` lets WebKit switch compositing modes mid-page, which shows up as one-frame freezes during video. Set `never` only if the window renders wrong. |
| `hardware_video_decoding` | `true` | Ask GStreamer to prefer the GPU video decoders. Set `false` if video breaks entirely after an update. |
| `developer_tools` | `false` | Enables the Web Inspector and console output. |
| `notifications` | `true` | Forward web notifications to your desktop. |
| `open_external_links_in_browser` | `true` | Send non-Instagram links to your browser. |
| `spell_checking_languages` | `[]` | e.g. `["en_US", "fr_FR"]`. Empty disables spell checking. |
| `default_zoom` | `1.0` | Zoom used when no window state has been saved. |
| `remember_window_state` | `true` | Restore size, position and zoom. |
| `show_loading_indicator` | `true` | The thin gradient bar at the top of the window. |
| `start_maximized` | `false` | Always open maximized. |
| `auto_update` | `true` | Check GitHub for a newer release and install it. |
| `update_check_interval_hours` | `24` | Hours between checks. `0` checks every launch. |

### Custom styling

Drop CSS into `~/.config/instacache/user.css` and it is applied to every page.

```css
/* Widen the feed on a large screen */
main[role="main"] { max-width: 1100px; }
```

### Where your data lives

| Path | Contents | Safe to delete |
|---|---|---|
| `~/.config/instacache/` | `config.json`, `user.css`, window geometry | yes, resets settings |
| `~/.local/share/instacache/` | cookies, local storage, IndexedDB — your session | yes, signs you out |
| `~/.cache/instacache/` | the resource cache | yes, always |

Every path honours `XDG_*_HOME`, and can be redirected with
`INSTACACHE_DATA_HOME`, `INSTACACHE_CACHE_HOME` and `INSTACACHE_CONFIG_HOME` for a
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
and aarch64 archives and publishes the release with notes and checksums — which
is what the one-line installer downloads. The workflow runs on GitHub Actions
and on Gitea Actions.

## Architecture

```
src/
  main.rs        argument parsing and process startup
  lib.rs         module wiring and the application constants
  ui.rs          window assembly, signals, notifications, downloads
  web.rs         WebKit context, persistent storage, settings, link routing
  progress.rs    the loading bar, including in-app navigation
  updates.rs     checking for and installing a newer release
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

- instaCache renders Instagram's own website. If Instagram changes something,
  instaCache follows automatically — but it also inherits any feature Instagram
  does not offer on the web.
- Camera, microphone, geolocation and pointer-lock permission requests are
  refused outright. The web app does not need them.
- This is an unofficial client. It is not affiliated with, endorsed by, or
  connected to Instagram or Meta. Instagram is a trademark of Meta Platforms, Inc.

## License

[MIT](LICENSE).
