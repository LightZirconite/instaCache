<div align="center">

<img src="assets/instacache.svg" width="120" alt="">

# instaCache

**Instagram as a real Linux app.** Messages ring, calls ring, and it starts
with your session — in a 2 MB binary that borrows the browser engine you
already have.

[![AUR](https://img.shields.io/aur/version/instacache-bin?style=flat-square&color=1793d1&label=AUR)](https://aur.archlinux.org/packages/instacache-bin)
[![Release](https://img.shields.io/github/v/release/LightZirconite/instaCache?style=flat-square&color=e4405f)](https://github.com/LightZirconite/instaCache/releases)
[![Licence](https://img.shields.io/badge/licence-AGPL--3.0-brightgreen?style=flat-square)](LICENSE)

</div>

---

## Install

**Arch, CachyOS, EndeavourOS**

```sh
yay -S instacache-bin
```

**Anything else**

```sh
curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh | sh
```

No root, nothing bundled. It installs into `~/.local`, adds itself to your
application menu, and offers to install the Qt packages it needs. Remove it
with `~/.local/share/instacache/uninstall.sh`.

## What you get

**It behaves like a messenger, not like a tab.**
A new message plays a sound and posts a real desktop notification — through
Web Push, so it reaches you with the window closed, minimised, or behind
everything else. An incoming call rings until you answer it or thirty seconds
pass. Clicking the notification opens the right conversation.

**It is already running when you log in.**
Starts with your session, in the background, behind a tray icon that says so.
Closing the window does not quit it; `Ctrl+Q` does.

**It opens instantly.**
Your session, cookies and cache are pinned to disk and reused. The window
comes back the size, position and zoom you left it, on the page you left it.

**It costs 2 MB.**
No Electron, no Node, no second browser on your disk. One Qt Quick window over
the Qt WebEngine your distribution already ships, shared with every other Qt
app. The engine still costs what an engine costs — about 280 MB resident — but
you only pay for it once, and you already were.

**Video does not stutter.**
Chromium reuses its decoders instead of building a pipeline per clip, which is
what a Reels feed asks for twice a second. Measured: **1 to 6** late frames in
a 40-second churn, against 78 for the WebKitGTK build this replaced. The
numbers and everything that was tried and rejected are in
[`bench/`](bench/README.md).

**It stays one application.**
A link, a popup or a call opens as a page over Instagram with a back button,
never as a tab. Anything that is not Meta's goes to your real browser, so a
signed-in session never sits next to an arbitrary site.

**Several accounts, and other sites.**
`instacache --profile work` is a separate session, cache, window and dock
icon. `instacache --add-site x https://x.com/` gives any site the same
treatment, with its own menu entry and icon.

## Shortcuts

| | |
|---|---|
| `Ctrl+1` `Ctrl+2` `Ctrl+3` `Ctrl+4` | Feed · Explore · Reels · Direct |
| `Ctrl+R` · `Ctrl+Shift+R` | Reload · reload ignoring the cache |
| `Alt+←` · `Alt+→` | Back · forward, including out of a call |
| `Ctrl+=` `Ctrl+-` `Ctrl+0` | Zoom in · out · reset |
| `F11` | Fullscreen |
| `Ctrl+W` · `Ctrl+Q` | Close the page on top · quit for real |

Your mouse's back and forward buttons work, and so does a two-finger swipe.

## Settings

Most of it is in the tray menu: start at login, notifications, sounds,
ringing, whether closing keeps it running.

Everything else is `~/.config/instacache/config.json`, written with the
defaults on first run so the knobs are visible. Drop a `user.css` or a
`user.js` beside it and they are applied to every page.

<details>
<summary><b>Every setting</b></summary>

<br>

| Key | Default | What it does |
|---|---|---|
| `home_url` | `https://www.instagram.com/` | Page opened at startup and by `Ctrl+H`. |
| `user_agent` | a Linux Chrome string | Sent to Instagram. Honest about both the system and the engine — claiming Safari would put a Chromium engine on Safari's code path. Empty keeps Qt WebEngine's own. |
| `hardware_acceleration` | `always` | `always`, `auto` or `never`. The first two both leave Chromium's own decision alone. Set `never` only if the window renders wrong — it turns off GPU compositing entirely. |
| `video_decoding` | `gpu` | `gpu`, `software` or `auto`. `gpu` turns on VA-API, which Chromium disables on Linux. Only change this if playback misbehaves. |
| `allow_autoplay_with_sound` | `true` | Let a video start with its sound on. The engine otherwise silences anything that plays without a click, which reads as the app muting itself. |
| `context_menu` | `false` | Show the engine's right-click menu. Off, because in a one-application window it is browser chrome — Back, Forward, View Source — and it covers the page. Turning it off also removes "Save image as" and "Copy link address"; set `true` to get them back. |
| `developer_tools` | `false` | Enables the Web Inspector and console output. |
| `notifications` | `true` | Forward web notifications to your desktop. |
| `push_notifications` | `true` for Instagram, `false` for other sites | Register with the browser's push service, which is how Instagram delivers a message or a call while nobody is looking at the window. Without it `PushManager.subscribe()` fails, Instagram never registers, and nothing is ever pushed. It holds one connection to Google's push servers, the same one Chrome holds; `false` gives that up and leaves only what arrives while the page is open. Needs Qt 6.5. |
| `notification_sounds` | `true` | A short sound with a message's notification. Where the notification server plays sounds itself (KDE Plasma does), it honours Do Not Disturb and its own sound settings. |
| `ring_for_calls` | `true` for Instagram, `false` for other sites | Ring for an incoming call until it is answered or dismissed. A call is recognised from the notification's words, in English, French, Spanish, German, Portuguese and Italian. |
| `calls` | `true` for Instagram, `false` for other sites | Let Instagram use the microphone and camera, for voice and video calls in Direct. Only hosts in `internal_domains` are ever granted them; `false` refuses them. |
| `run_in_background` | `true` for Instagram, `false` for other sites | Closing the window hides it instead of quitting, so it opens again instantly and notifications keep arriving. `Ctrl+Q` quits. You are told once, the first time. |
| `start_with_session` | `true` for Instagram, `false` for other sites | Start with your desktop session, in the background. Only the **first** run writes `~/.config/autostart`; after that your desktop's own startup panel wins, and so does the tray's **Start at login**. |
| `unread_badge` | `true` | Show the unread count from Instagram's title on the task bar icon. |
| `tray_icon` | `true` for Instagram, `false` for other sites | An icon in the system tray while instaCache runs. Needs a desktop with a tray: KDE Plasma, Cinnamon, XFCE and most others have one; GNOME needs the AppIndicator extension. Without a tray nothing breaks, there is simply no icon. |
| `open_external_links_in_browser` | `true` | Send non-Instagram links to your browser. |
| `internal_domains` | Instagram + the Meta hosts its login needs | Hosts allowed to render inside the window, as an allow-list — a host matches only exactly or as a sub-domain. Per profile, so a second profile can be a dedicated window for another site: point `home_url` at it and name its domains here. Threads is deliberately not in the default; add `threads.com` to keep it inside the window. An empty list restores the default rather than locking the window. |
| `spell_checking_languages` | `[]` | e.g. `["en_US", "fr_FR"]`. Empty disables spell checking. |
| `default_zoom` | `1.0` | Zoom used when no window state has been saved. |
| `remember_window_state` | `true` | Restore size, position and zoom. |
| `show_loading_indicator` | `true` | The thin gradient bar at the top of the window. |
| `start_maximized` | `false` | Always open maximized. |
| `auto_update` | `true` | Check GitHub for a newer release and install it. |
| `update_check_interval_hours` | `6` | Hours between checks. `0` checks at every launch and never in between. A `24` written by an older version is read as the new default. |

`~/.config/instacache/user.css` is injected into every page, and `user.js`
runs once per load in the page's own world — the nearest thing to an
extension, since Qt WebEngine implements no extension API.

</details>

<details>
<summary><b>Command line</b></summary>

```
instacache [OPTIONS] [URL]

  <URL>                  Open this Instagram URL instead of your feed.
  -p, --profile <NAME>   Use a separate session, cache and window.
      --add-site <NAME> <URL>
                         Add a site to your application menu.
      --icon <PATH>      Use this image instead of the site's own logo.
      --remove-site <NAME>
                         Take it back out. Its data is kept.
      --list-sites       Show the sites you have added.
      --update           Check for a newer release and install it.
      --background       Start with the window hidden.
      --clear-cache      Delete cached resources, stay signed in.
      --clear-session    Delete cookies and site storage (signs you out).
  -h, --help             Full help.
  -V, --version          Version.
```

Launching it twice with the same profile focuses the window that already
exists rather than starting a second one over the same cookie jar.

</details>

<details>
<summary><b>A notification did not arrive, or did not ring</b></summary>

<br>

Instagram publishes no marker for an incoming call, so instaCache reads it
from the words — and the words differ by language and change without notice.
If a call does not ring, show what actually arrived:

```sh
INSTACACHE_LOG_NOTIFICATIONS=1 instacache
journalctl --user -f | grep 'instacache: notification'
```

Off by default, and it stays off unless you ask: what a notification says is
nobody's business, this log included. Send the line for a call that did not
ring and it is a one-line fix.

</details>

<details>
<summary><b>Where your data lives</b></summary>

<br>

| Path | Contents | Safe to delete |
|---|---|---|
| `~/.config/instacache/` | `config.json`, `user.css`, window geometry | yes, resets settings |
| `~/.local/share/instacache/` | cookies, local storage, IndexedDB — your session | yes, signs you out |
| `~/.cache/instacache/` | the resource cache | yes, always |

Every path honours `XDG_*_HOME`, and can be redirected with
`INSTACACHE_DATA_HOME`, `INSTACACHE_CACHE_HOME` and `INSTACACHE_CONFIG_HOME`
for a portable install. Nothing is ever written outside them.

</details>

<details>
<summary><b>Updates</b></summary>

<br>

Installed from the AUR, updating is `pacman -Syu` — instaCache knows it is
packaged and does not touch itself.

Installed with `get.sh`, it updates the way a browser does: it checks a few
times a day, downloads in the background, and takes over at the next quiet
moment. An open window gets an **Update ready — Restart** button instead, and
comes back on the same page. Turn it off with `auto_update: false`.

</details>

## Requirements

Qt 6.4 or newer, with `qt6-webengine`. `get.sh` installs it for you on Arch,
Debian, Ubuntu, Fedora, openSUSE and their derivatives.

On Fedora, video needs `qt6-qtwebengine-freeworld` — Fedora builds Qt
WebEngine without H.264, and Instagram is H.264 throughout. The installer
handles it.

## Build from source

```sh
git clone https://git.justw.tf/LightZirconite/instaCache
cd instaCache
cargo build --release
```

You need Rust 1.82+ and the Qt 6 development packages
(`qt6-base`, `qt6-declarative`, `qt6-webengine`). The result is
`target/release/instacache`, dynamically linked against your system's Qt —
that is deliberate and not negotiable.

Working on it? [`AGENTS.md`](AGENTS.md) is the real documentation: the
conventions, the traps, and the reasons behind every decision that cost an
afternoon.

## Licence

[GNU AGPL-3.0-or-later](LICENSE). Use it, change it, share it, run it for
anything you like — but if you distribute it, or run a modified version as a
network service, your changes have to come back under the same licence.

Copyright © 2026 LightZirconite. The copyright is held by one person on
purpose, so a commercial licence remains possible for anyone the AGPL does not
suit. Ask.

<div align="center">
<br>
<sub>Not affiliated with, endorsed by, or connected to Instagram or Meta.</sub>
</div>
