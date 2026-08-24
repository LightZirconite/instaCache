# Working on instaCache

Conventions and hard-won facts for anyone — human or agent — changing this
repository. Keep it short; if a rule can be enforced by CI instead, enforce it
there.

## What this project is

One GTK 3 window hosting one WebKitGTK 4.1 view that displays Instagram, with
the cache and session pinned to persistent XDG directories. The value is in the
persistence and the desktop integration, not in any UI of our own. Resist
adding chrome.

## Non-negotiables

- **No Electron, no Node, no Python, no bundled browser engine.** The whole
  point is a small binary that uses the system WebKitGTK.
- **WebKitGTK 4.1 (GTK 3), not 6.0 (GTK 4).** The 4.1 API is what Debian 12,
  Ubuntu 22.04 and every current distribution actually ship. Note that
  `WebKitNetworkSession` does *not* exist in 4.1 — `WebKitWebsiteDataManager`
  is the supported API there, not a deprecated one.
- **The release binary must stay dynamically linked** against the distribution's
  WebKitGTK. Never vendor it.
- **Nothing may write outside the XDG directories** resolved in `paths.rs`.

## Layout

| File | Responsibility |
|---|---|
| `src/main.rs` | Argument parsing, process startup. Nothing else. |
| `src/lib.rs` | Module list and application constants. |
| `src/ui.rs` | Window, signals, notifications, downloads, state persistence. |
| `src/web.rs` | WebKit context, storage, settings, external-link routing. |
| `src/config.rs` | `config.json` and window geometry, both fault-tolerant. |
| `src/paths.rs` | XDG locations and named profiles. |
| `src/shortcuts.rs` | Keyboard navigation. |
| `src/urls.rs` | Which hosts stay inside the app. Security-relevant. |
| `src/errorpage.rs` | The offline page. Escapes everything it embeds. |
| `examples/snapshot.rs` | Renders a page to PNG through WebKit, for verification. |

The library/binary split exists so `examples/snapshot.rs` exercises the real
`web::build` configuration. Do not collapse it.

## Three constants that must stay in sync

`PROGRAM_NAME` in `src/lib.rs`, `StartupWMClass` in `instacache.desktop`, and
the installed icon name `instacache`. GTK 3 derives both the Wayland `app_id` and the
X11 `WM_CLASS` from `g_set_prgname()`. Break the chain and the app shows a
generic icon in the dock — which looks like a packaging bug and is not.

## Before you commit

```sh
cargo fmt --all
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

CI runs exactly these, plus `desktop-file-validate`, `shellcheck` on the three
shell scripts, and an XML well-formedness check on the SVG.

The runners carry an older `desktop-file-utils` than a current desktop does, and
it emits warnings for valid keys. The CI step therefore fails on `error:` lines
only. Keep `Version=1.0` in the desktop entry: newer spec versions are rejected
outright by the version on the runners.

## Verifying that something actually renders

Do not trust a display-server screenshot. On this project's reference machine,
`spectacle` and `import` both return an entirely white image for *every* window,
including unrelated applications — a broken capture pipeline, not a broken app.
Two blank captures in a row cost an hour before that was established.

Use the snapshot helper instead; it renders inside the WebProcess and never
touches the compositor:

```sh
cargo run --example snapshot -- https://www.instagram.com/ shot.png
```

Other checks that need no screenshot at all:

- `xdotool getwindowname <id>` — if it reports the page title, the page loaded
  and its JavaScript ran.
- `du -sh ~/.cache/instacache` — proves the disk cache is being written.
- `ls ~/.local/share/instacache/` — `cookies.sqlite` and `localstorage/` prove
  the session is persisting.
- `/usr/lib/webkit2gtk-4.1/MiniBrowser <url>` — vanilla WebKitGTK, the reference
  for "is this our bug or the engine's?".

## Testing shortcuts and window closing

`xdotool windowclose` destroys the X window outright; GTK never emits
`delete-event` and you get `BadDrawable` instead of a graceful shutdown. Test the
state-saving path with `kill -TERM` instead — `ui.rs` handles `SIGINT`/`SIGTERM`
precisely so this path is both robust and testable.

## Adding a WebKit setting

Check the installed headers first, because the Rust bindings expose calls that
the underlying library has since turned into no-ops:

```sh
grep -rn webkit_settings_set_your_thing /usr/include/webkitgtk-4.1/webkit/
```

`webkit_settings_set_enable_dns_prefetching` is the cautionary example: it
compiles, it links, and it prints a deprecation warning at every startup while
doing nothing. It was removed for that reason.

## Video does not play

WebKit decodes media through GStreamer, not through anything this project
controls. `gst-plugins-good` supplies `qtdemux` (MP4), `souphttpsrc` and
`autoaudiosink`; `gst-libav` supplies `avdec_h264`. Miss either and Instagram
looks broken in a very specific way: photos and avatars render, every video
stays blank. Check before assuming a bug in the app:

```sh
for e in qtdemux souphttpsrc autoaudiosink avdec_h264; do
    printf '%-16s ' "$e"; gst-inspect-1.0 "$e" >/dev/null 2>&1 && echo ok || echo MISSING
done
```

`install.sh` runs this check and `packaging/PKGBUILD` declares the packages, so
neither should be dropped.

## A grey, unresponsive page

That is WebKit's rendering process having died, not a frozen UI. `ui.rs`
handles `web-process-terminated` and reloads, at most `MAX_CRASH_RELOADS` times
inside `CRASH_WINDOW`, then shows the crash page instead of looping forever.
Anything that makes the process die on every load must not be "fixed" by
raising that limit.

## Touching `urls.rs`

`is_internal()` decides what renders inside a window holding a logged-in
Instagram session. The suffix check must keep rejecting `notinstagram.com` and
`instagram.com.evil.example`. If you extend `INTERNAL_DOMAINS`, add a test for
the lookalike form too.

`facebook.com` and `meta.com` are in that list on purpose: Instagram's login,
two-factor and Accounts Center flows redirect through them. Removing them breaks
signing in.

## Releasing

`scripts/release.sh <patch|minor|major>` does everything: runs the checks, bumps
`Cargo.toml`, `Cargo.lock` and `packaging/PKGBUILD`, writes the changelog,
commits, tags and pushes. Pushing the tag is what triggers the release workflow.
Never tag by hand — the workflow refuses a tag that disagrees with `Cargo.toml`.
