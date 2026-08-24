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
| `src/progress.rs` | The loading bar, including in-app navigation. |
| `src/updates.rs` | Checking for and installing a newer release. |
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

`install.sh` runs this check and installs the packages itself, so it must not
be dropped.

Hardware decoding matters as much as having a decoder at all. WebKit hands
video to GStreamer, which ranks `vah264dec` (GPU) barely above `avdec_h264`
(CPU); `web.rs` raises the GPU decoders to MAX through
`GST_PLUGIN_FEATURE_RANK` so the choice is deterministic. Names GStreamer does
not know are ignored, and a decoder that fails to negotiate still falls back,
so the list is safe to extend.

## The loading bar looks dead

Instagram is a single-page application. Opening a profile or the inbox does not
trigger a page load, so `load-changed` never fires and
`estimated-load-progress` never moves — a bar driven only by those lights up
once at startup and never again. `progress.rs` therefore also starts on a URI
change and finishes when the network has been quiet for `QUIET_PERIOD`. Test
both paths: a cold start *and* clicking through the app.

## Never cancel a GLib source by id

`SourceId::remove` panics if the id is gone, and a panic inside a GTK callback
aborts the process because the release profile sets `panic = "abort"`. A
one-shot timeout removes itself when it fires, so any id kept from
`timeout_add_local_once` is stale the moment it runs — and GLib recycles ids,
so cancelling later can destroy an unrelated source, including one of WebKit's.

This is not hypothetical: it is what made 1.1.0 abort while scrolling.
`progress.rs` now cancels nothing and uses a generation counter instead; a
pending callback checks whether it is still the current generation and returns
if not. Do the same for anything new.

## Reproducing a crash you cannot click your way to

The reference machine is reached over a remote desktop: injected keyboard and
mouse events never arrive and screenshots come back entirely white. Driving the
page from inside is the way around both.

```sh
cargo run --example stress            # local page, touches no server
cargo run --example stress -- 90 https://www.instagram.com/
```

Use the real site sparingly. Automated navigation there looks like a bot and
risks the account.

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

## The installer's option contract

`instacache --update` downloads the newest archive and runs the `install.sh`
*inside it*, which is a script this version has never seen. `--prefix`, `--yes`
and `--no-deps` are therefore a stable interface: rename or remove one and
every existing install fails to update. `updates.rs` retries once with no
options at all as a safety net, but do not spend it.

Test an update by lowering the version in `Cargo.toml`, building, installing to
a scratch prefix and running `--update` against the real published release. It
is the only way to exercise the download, the checksum and the hand-off
together.

## Releasing

`scripts/release.sh <patch|minor|major>` does everything: runs the checks, bumps
`Cargo.toml` and `Cargo.lock`, writes the changelog,
commits, tags and pushes. Pushing the tag is what triggers the release workflow.
Never tag by hand — the workflow refuses a tag that disagrees with `Cargo.toml`.

Two traps the script now handles, both found the hard way:

- It rebuilds after bumping. The checks run against the *old* version, so
  without a rebuild `./install.sh` straight after a release installs a binary
  that reports the previous number.
- It leaves a hand-written changelog section alone. Generating one on top of it
  produced two `## [1.1.0]` headings in the same file.
