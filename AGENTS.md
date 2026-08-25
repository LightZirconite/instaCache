# Working on instaCache

Conventions and hard-won facts for anyone — human or agent — changing this
repository. Keep it short; if a rule can be enforced by CI instead, enforce it
there.

## What this project is

One Qt Quick window hosting one Qt WebEngine view that displays Instagram,
with the cache and session pinned to persistent XDG directories. The value is
in the persistence and the desktop integration, not in any UI of our own.
Resist adding chrome.

## Non-negotiables

- **No Electron, no Node, no Python, no bundled browser engine.** The whole
  point is a small binary that uses the system's Chromium through
  `qt6-webengine`, shared with every other Qt application on the machine.
- **The release binary must stay dynamically linked** against the
  distribution's Qt. Never vendor it.
- **Qt 6.4 is the baseline**, because it is what Debian 12 ships. Newer API is
  tempting and mostly off limits: `permissionRequested` and
  `persistentPermissionsPolicy` are 6.8, and using them would lock out the
  distributions this is meant to run on. This is the same reasoning that once
  pinned the project to WebKitGTK 4.1 rather than 6.0.
- **Policy lives in Rust, widgets live in QML.** Which URL stays in the window,
  where a download goes, whether a dead renderer is reloaded again — all of it
  is decided in `bridge.rs`, where it is unit tested. QML that decides
  something cannot be tested at all.
- **Nothing may write outside the XDG directories** resolved in `paths.rs`.

## Why not WebKitGTK

It was the engine until 1.2.0, and it was replaced for one measured reason:
WebKit builds a GStreamer pipeline per `<video>` element, on the thread that
also runs the page, and a feed builds one about twice a second. On the
reference machine that cost 78 frames over 50 ms in a 40-second run where
Chromium cost 2. Nothing in WebKit's settings closed the gap — the full list of
what was tried and rejected, with numbers, is in `bench/README.md`. Do not
reopen this without a measurement.

The engine change also cost something, and it is fair to say so: the binary
grew from 522 KB to a few megabytes of Rust-to-Qt glue, and `config.json` keys
that named GStreamer concepts now mean Chromium ones.

## Layout

| File | Responsibility |
|---|---|
| `src/main.rs` | Argument parsing, process startup, termination signals. |
| `src/lib.rs` | Module list and application constants. |
| `src/bridge.rs` | Everything QML may ask Rust. The policy lives here. |
| `src/qml/main.qml` | The window, the view, the loading bar, the shortcuts. |
| `src/chromium.rs` | Config settings translated into Chromium flags. |
| `src/config.rs` | `config.json` and window geometry, both fault-tolerant. |
| `src/paths.rs` | XDG locations and named profiles. |
| `src/downloads.rs` | Where a download goes and under what name. |
| `src/instance.rs` | One window per profile, over a Unix socket. |
| `src/sites.rs` | Turning a site into its own menu entry, icon and window class. |
| `src/http.rs` | Fetching a URL through curl or wget, never a linked stack. |
| `build.rs` | Compiles the single C++ call Qt exposes no binding for. |
| `src/urls.rs` | Which hosts stay inside the app. Security-relevant. |
| `src/errorpage.rs` | The offline page. Escapes everything it embeds. |
| `src/updates.rs` | Checking for and installing a newer release. |
| `examples/snapshot.rs` | Renders a page to PNG, for verification. |
| `examples/stress.rs` | Drives a page from inside, for reproducing crashes. |
| `bench/` | The video-smoothness harness. Read its README first. |

The library/binary split exists so the examples exercise the real `Shell`
configuration. Do not collapse it.

The QML scene is compiled into the binary with `include_str!` rather than
installed beside it, so there is still one file to ship and no way for the two
to drift apart across an update.

## Three constants that must stay in sync

`PROGRAM_NAME` in `src/lib.rs`, `StartupWMClass` in `instacache.desktop`, and
the installed icon name `instacache`. Qt derives the Wayland `app_id` and the
X11 `WM_CLASS` from the application name, which `main.rs` sets before the first
window exists. Break the chain and the app shows a generic icon in the dock —
which looks like a packaging bug and is not.

There is a fourth link now: a site added with `--add-site` gets its own class,
`instacache-<profile>`, from `sites::window_class()`, and its generated entry
carries the matching `StartupWMClass`. Both sides come from that one function
on purpose — if they ever disagree, the task bar cannot tell which application
the window belongs to. The default profile still returns plain `instacache`,
which is what keeps the shipped entry correct.

### The name the compositor actually reads

`QCoreApplication::setApplicationName` is **not** what sets the Wayland
`app_id`, and believing otherwise cost an afternoon here. Qt's Wayland plugin
calls `QGuiApplication::desktopFileName()` and passes the result straight to
`xdg_toplevel::set_app_id`; it never looks at the application name. You can
confirm it without reading Qt's source:

```sh
strings /usr/lib/libQt6WaylandClient.so.6 | grep -E 'desktopFileName|set_app_id'
```

Setting only the application name leaves every window announcing `instacache`,
so a site's window matched `instacache.desktop` and the task bar drew
instaCache's icon over the site's — while the menu, which reads the entry
directly, showed the right one. A wrong icon in one place and the right icon in
the other is the signature of this bug.

`sites::announce_desktop_file()` makes the real call, through the only C++ in
this codebase. That is why `build.rs`, `cpp` and `qttypes` exist: no binding
exposes the function, and nothing else in Qt's API stands in for it. It must
run before the first window is created.

To check a window's class without trusting anything:

```sh
kdotool search --classname . | while read w; do kdotool getwindowclassname "$w"; done
```

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

Use the snapshot helper instead; it grabs the view from inside the engine and
never touches the compositor:

```sh
cargo run --example snapshot -- https://www.instagram.com/ shot.png
```

Other checks that need no screenshot at all:

- `xdotool getwindowname <id>` — if it reports the page title, the page loaded
  and its JavaScript ran.
- `du -sh ~/.cache/instacache` — proves the disk cache is being written.
- `ls ~/.local/share/instacache/` — `cookies.sqlite` and `localstorage/` prove
  the session is persisting.
- `qml6 bench/runners/bench.qml -- <url>` — a bare Qt WebEngine view with none
  of our configuration, the reference for "is this our bug or the engine's?".

## Testing shortcuts and window closing

`xdotool windowclose` destroys the window outright, so `onClosing` never runs
and the geometry is never written. Test the state-saving path with `kill -TERM`
instead.

That path is deliberately indirect. The signal handler in `main.rs` sets an
atomic flag and does nothing else, because a signal handler may safely do
almost nothing — writing a JSON file from inside one means allocating on a
thread it interrupted mid-allocation. The scene's own 250 ms timer notices the
flag and shuts down through the ordinary close path.

## Adding a setting

A setting reaches the engine by one of three routes, and picking the wrong one
is the usual way a change compiles and does nothing:

| what it affects | where it goes |
|---|---|
| the page or the view | `settings.*` on `WebEngineView` in the QML scene |
| the session or storage | a `WebEngineProfile` property in the same scene |
| Chromium itself | `src/chromium.rs`, as a command-line flag |

Chromium flags are the trap. They are read from `QTWEBENGINE_CHROMIUM_FLAGS`
exactly once, when Qt WebEngine initialises, which happens before any Qt
application object exists — so `chromium::apply()` has to run before
`webengine::initialize()` in `main.rs`, and a flag set anywhere later is
silently ignored. That is why the flags are a pure function of the config, with
tests: it is the only part of the wiring that can be checked without starting a
browser.

Check the QML side against the installed types rather than against
documentation for a newer Qt:

```sh
grep -n '"yourProperty"' /usr/lib/qt6/qml/QtWebEngine/plugins.qmltypes
```

That file also tells you which Qt version introduced a signal — the baseline is
6.4, and anything newer is off limits. See the non-negotiables.

## Video does not play

Chromium decodes video itself, so unlike the WebKitGTK builds of instaCache
there is no GStreamer plugin set to get wrong. There is one exception, and it
produces the same very specific symptom: photos and avatars render, every video
stays blank.

Fedora builds Qt WebEngine without the patent-encumbered codecs. Instagram is
H.264 throughout, so on Fedora the video is blank until
`qt6-qtwebengine-freeworld` is installed alongside it. `install.sh` checks for
this and installs it, so that check must not be dropped.

Hardware decoding is a separate question from having a decoder at all.
Chromium disables VA-API on Linux by default; `chromium.rs` turns it back on
for `video_decoding: "gpu"`, which is the default. It is worth roughly nothing
in stutter on the reference machine — 2 or 3 late frames either way — and is
kept because it does reduce the CPU cost, and because `software` has to mean
something.

## The loading bar looks dead

Instagram is a single-page application. Opening a profile or the inbox does not
trigger a page load, so `loadingChanged` never fires and `loadProgress` never
moves — a bar driven only by those lights up once at startup and never again.
The scene therefore also reacts to `onUrlChanged` while the view is not
loading, with a short sweep rather than real progress, because an in-app
navigation has no progress to report. Test both paths: a cold start *and*
clicking through the app.

## Timers, and the hazard that is now gone

The WebKitGTK build had a rule here: never cancel a GLib source by id.
`SourceId::remove` panics if the id is gone, a panic inside a GTK callback
aborts the process because the release profile sets `panic = "abort"`, and
GLib recycles ids — so cancelling late could destroy an unrelated source,
including one of WebKit's. It is what made 1.1.0 abort while scrolling.

QML `Timer` has no such hazard: `stop()` and `restart()` on an already-stopped
timer are both defined and harmless, and the loading bar uses them freely. The
rule is recorded because the class of bug is worth remembering, not because it
still applies.

What does still apply: `panic = "abort"` is still set, and a panic inside a
method QML calls still takes the process with it. Nothing in `bridge.rs` may
unwrap something a page can influence.

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

## Measuring video smoothness

CPU averages do not measure stutter, and a single run against Instagram
measures whichever clips happened to be in the feed — two samples there
disagreed by 3x and led to the wrong conclusion once already. Use `bench/`,
which is in this repository for exactly that reason, and read
[`bench/README.md`](bench/README.md) before trusting any number it prints.

```sh
./bench/make-clips.sh && cargo build
python3 bench/serve.py &
./bench/run.sh mine app 50 churn file
```

Every configuration needs **two runs that agree**. Four counters in the report
exist because each of them once turned an apparent win into a measured loss:
`presented` (frames actually shown, engine-independent), `playedSec` (smooth
because nothing was playing), `ttffMed` (smooth because every video started
late) and `errors` (a fix that was really a breakage).

Where it stands, on the reference machine, churning progressive video:

| | frames over 50 ms | p99 | first frame |
|---|---|---|---|
| WebKitGTK 4.1, as shipped in 1.2.0 | 78 | 70 ms | 264 ms |
| Qt WebEngine, as shipped now | 1-6 over five runs | 33-50 ms | 48-64 ms |

Two conclusions worth keeping, both of which correct what this project
believed before the bench could tell the two video paths apart:

- **MediaSource was never the problem.** WebKit handled the `mse` path at 4
  late frames, near Chromium. The path that stalled was the plain
  `<video src="…mp4">` one, which is what a Reels feed uses. A year of blaming
  MSE came from a bench that only ever exercised the other path.
- **No WebKit setting closed the gap**, and several looked as though they had.
  `WEBKIT_GST_USE_PLAYBIN3` is the cautionary one: it halved the stalls and
  stopped 22-28 videos out of 99 from ever playing. That is why the report
  carries `errors`, `presented`, `playedSec` and `ttffMed` at all.

### The bench is not a Meta host

`urls.rs` calls `127.0.0.1` external, so the app hands the bench to the system
browser and leaves its own window blank — and the system browser then quietly
produces the numbers. That is not hypothetical; it happened, and the readings
looked wonderful. `bench/run.sh` disables the routing for the run, and the page
reports which engine actually rendered it. Check that field.

## A grey, unresponsive page

That is the renderer process having died, not a frozen UI. The scene handles
`onRenderProcessTerminated` and asks `bridge.rs` whether to reload; it says yes
at most `MAX_CRASH_RELOADS` times inside `CRASH_WINDOW`, and then the crash
page is shown instead of looping forever. That decision is in Rust, and tested,
precisely so it cannot quietly become "always reload". Anything that makes the
process die on every load must not be "fixed" by raising the limit.

## Touching `urls.rs`

`is_internal_in()` decides what renders inside a window holding a logged-in
session. It is an allow-list and must stay one: the suffix check has to keep
rejecting `notinstagram.com` and `instagram.com.evil.example`, and an empty
entry must never match, because `host.ends_with(".")` would otherwise let in
the entire web. There are tests for all three; extend them rather than
replacing them.

The list itself now comes from the profile's `internal_domains`, so a second
profile can be a dedicated window for another site. `INTERNAL_DOMAINS` remains
as the default and as what `is_internal()` uses when there is no configuration
to hand. Widening the list is the user's decision; widening what *counts* as a
match is not, and is the thing to be careful about here.

`facebook.com` and `meta.com` are in the default on purpose: Instagram's login,
two-factor and Accounts Center flows redirect through them. Removing them
breaks signing in. `threads.com` and `threads.net` are there by choice, not by
necessity.

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
