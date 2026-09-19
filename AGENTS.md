# Working on instaCache

Conventions and hard-won facts for anyone — human or agent — changing this
repository. Keep it short; if a rule can be enforced by CI instead, enforce it
there.

## What this project is

One Qt Quick window hosting one Qt WebEngine view that displays Instagram,
with the cache and session pinned to persistent XDG directories. The value is
in the persistence and the desktop integration, not in any UI of our own.
Resist adding chrome: what exists is the loading bar, the back button shown
over a page, the pills for a call and an update, and the tray icon, and each
of those is there because the window has no other way to offer it.

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
| `src/qml/main.qml` | The window, its pages, the loading bar, the shortcuts. |
| `src/badge.rs` | The unread count on the task bar icon, over D-Bus. |
| `src/alerts.rs` | Notifications, their sounds, and ringing for a call. |
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
| `tests/scene/` | The QML scene under qmltestrunner, with a mock `shell`. |
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

## A live process, no window and no error

qmetaobject does not report QML component errors. A scene that fails to
instantiate leaves a process that runs happily for as long as you let it,
prints nothing at all — not one line — and never shows a window. It looks like
a hang, or like the page failing to load, and it is neither.

If a change to `main.qml` produces silence, assume the scene did not build.
Bisect by removing what you added rather than by adding logging, because your
logging will not run either.

What produced this the first time: `WebEngineScript { … }` inside
`userScripts.collection`. Qt's own type registry says why —

```sh
grep -A 12 '"QtWebEngine/WebEngineScript' /usr/lib/qt6/qml/QtWebEngine/plugins.qmltypes
```

— `isCreatable: false`. User scripts cannot be declared from QML in this Qt, so
running the user's JavaScript at document creation, the way an extension's
content script does, needs `QWebEngineProfile::scripts()` from C++. Until
somebody does that, `user.js` runs after the load finishes, which is fine for
changing a page and useless for stopping something appearing.

## Pages, calls and permissions

The window shows one page at a time. `view` is Instagram and lives as long as
the window. Anything opened with `target="_blank"` or `window.open` is adopted
with `openIn` into a page laid over it, and opening another page closes the
previous one, so the window never collects tabs. A page granted the
microphone or camera is a call: going back only hides it, the pill brings it
back, and it closes itself when the call's own `window.close()` arrives.

Two things here are easy to break without noticing:

- **Adopt before closing.** The request may come from the page about to be
  replaced, and a page is destroyed with `Qt.callLater`, never from inside one
  of its own handlers.
- **Name the profile by an id no property shares.** `profile: profile` inside a
  view created from a component gave it a private, session-less profile, and
  `openIn` then killed the renderer without a word. The id is `session`.

What a page may use is decided by `permission_granted` in `bridge.rs`: the
microphone and camera for calls, notifications, the clipboard, and only for a
host in `internal_domains`. Everything else is refused. Refusing the
microphone to every page is what once broke calls in Direct.

## Testing the scene

`bridge.rs` has unit tests; the scene has `tests/scene/run.sh`. It builds a
copy of `main.qml` with `shell` replaced by `MockShell.qml`, serves stand-in
pages from `tests/scene/www` on `127.0.0.1:8791`, and drives the real scene
with qmltestrunner: pages opening and closing, a call surviving the back
button, the pill, the mouse's back button, `Ctrl+W`, `Escape`, the Direct
shortcut and closing to the background. It runs offscreen with a fake camera
and microphone, so nothing appears on screen and no device is opened.

```sh
tests/scene/run.sh
```

A new `shell` property or method has to be added to `MockShell.qml` too. A
missing one does not stop the scene loading; it throws only when called.

To check the real binary and the real bridge together, still off screen:

```sh
QT_QPA_PLATFORM=offscreen \
QTWEBENGINE_CHROMIUM_FLAGS=--use-fake-device-for-media-stream \
INSTACACHE_CONFIG_HOME=… INSTACACHE_DATA_HOME=… INSTACACHE_CACHE_HOME=… \
./target/debug/instacache --profile scenetest
```

with that profile's `home_url` and `internal_domains` pointing at a local
server. Leave `XDG_RUNTIME_DIR` alone: the instance socket lives there, and a
long scratch path exceeds the socket path limit. Pages served without cache
headers are cached, so clear the profile's cache directory when the pages
change.

The unread badge can be watched without any task bar:

```sh
dbus-monitor --session "type='signal',interface='com.canonical.Unity.LauncherEntry'"
```

## Closing to the background

With `run_in_background`, closing the window hides it and the process keeps
running; `Ctrl+Q`, a termination signal and the session ending still quit.
Two consequences to keep in mind:

- `Qt.quit()` sends a close event to the window first, and a close the scene
  refuses cancels the quit. `root.quit()` sets `quitting` before calling it for
  exactly that reason; quit through it, never through `Qt.quit()` directly.
- The update check used to run only at startup, and an instance that is never
  restarted would never check again. `bridge.rs` asks hourly whether a check is
  due, and stops once an update is installed, because the running binary would
  otherwise keep finding the same release newer and install it again.

## Updates take over by restarting

An update is installed long before it runs: replacing the binary under a
running copy is safe on Linux, and the running copy keeps the file it started
from. `bridge.rs` then holds the update as `Ready`, and the new version takes
over by a restart that `main.rs` performs after the scene has quit:

- **Quietly**, when `quiet_restart_allowed` says so — window closed, no call.
  The scene asks on every poll and quits; `main` starts the new binary with
  `--background`, so nothing appears.
- **On request**, from the pill in a visible window, reopening its page.

Three details that each cost something to get right:

- The path to restart is read with `current_exe()` **at startup**. Once the
  file has been replaced, `/proc/self/exe` names a deleted file.
- `main` drops the engine and releases the instance socket **before** starting
  the new copy, which would otherwise find this one still holding the profile
  and hand itself over to it.
- `--background` also has to set the window's `visibility` to `Hidden`: in Qt
  any other visibility shows the window, whatever `visible` says.

`--background` is new in 2.4.0. A quiet restart only ever starts a newer
binary, which knows it, but a test that "updates" a build to an *older*
published release sees that release reject the option. That is the test, not
the updater.

## The tray icon

It is `SystemTrayIcon` from `Qt.labs.platform`, created with
`Qt.createQmlObject` from a string in `Component.onCompleted` rather than
declared in the scene. That is deliberate: a missing module in a declared
import stops the whole scene loading, silently (see "A live process, no window
and no error"), and a tray icon is not worth that risk. A failure is caught
and logged, and the window works without it.

The string cannot see `shell` in the test harness, where `shell` is a property
of the root window rather than a context property, so everything it shows is
read through `root` (`root.trayIconName`, `root.unread`, …). Keep it that way.

With plain `QGuiApplication` the icon comes from the platform theme: on KDE,
`KDEPlasmaPlatformTheme6` exports a StatusNotifierItem with a D-Bus menu. It
can be checked without looking at the panel, with a profile of its own:

```sh
qdbus6 org.kde.StatusNotifierWatcher /StatusNotifierWatcher \
    org.kde.StatusNotifierWatcher.RegisteredStatusNotifierItems
```

then `org.freedesktop.DBus.Properties.GetAll` on the new item and
`com.canonical.dbusmenu.GetLayout` on its `Menu` path. A site profile without
an installed icon still shows instaCache's: the icon theme falls back from
`instacache-<profile>` to `instacache` by itself.

## Closing to the background, measured

Hiding a Qt window does **not** hide the page from Chromium. Measured off
screen, twelve seconds after `hide()`: `document.visibilityState` was still
`visible`, timers ran at full rate, and two videos kept playing, one with its
sound. And a window shown again came back blank — black with a test page,
Instagram's grey for a user — for seconds, or until a click, because its
graphics went with the hide and Chromium did not know.

So closing to the background does three things, in `hideToBackground`:

1. **Snapshot** the stage with `grabToImage`, while it is still on screen.
2. **Pause** every `video` and `audio` that is not a `MediaStream` — a call's
   own streams are left alone — in Instagram and in ordinary pages.
3. **Hide the views** (`backgrounded`), which is what tells Chromium the page
   is hidden: measured, `visibilityState` becomes `hidden`, timers throttle,
   videos stay paused.

`bringToFront` shows the snapshot, shows the window, unhides the views, nudges
a two-pixel square in the page for a few frames — Chromium draws only what
changed, and a page unchanged since it was hidden would otherwise draw nothing
and keep a frame whose GPU image is gone, which is the blank page — and waits
for two animation frames to run in the page — which only happens once
Chromium composites again — before fading the snapshot out, with a two-second
cap. On the reference machine, closed for 20 seconds: the live page took over
78 and 308 ms after the window showed, and the screen was never blank.

An earlier fix hid the view for 30 ms after a show to force a new frame. It
worked on a plain test page and still left Instagram black for seconds while
Chromium redrew; do not go back to it.

Reproduce with a window that was shown, closed and left hidden for 20 seconds
or more, a solid-colour page and a profile of its own — never by starting with
`--background`, which does not show the problem.

## Per-site defaults

A profile whose `home_url` is Instagram gets `calls`, `ring_for_calls`,
`run_in_background` and `tray_icon` on; any other site gets them off.
`Config::from_json` fills a key the file leaves out with the default for *that
file's* home page, so XCache's config, written before those keys existed, does
not inherit a messenger's behaviour. A key present in the file always wins.
New settings that only make sense for a messenger belong in `Config::for_home`.

## Notifications and ringing

`alerts.rs` shows each notification on a thread of its own. Waiting for a click
blocks, and a call's notification stays up until acted on; on one shared
thread, every notification after it would wait.

A message names `message-new-instant` in the notification when the server has
the `sound` capability, so the server plays it and honours Do Not Disturb, and
plays it locally otherwise. A call rings locally, `phone-incoming-call` on
repeat, because no server repeats a sound; it checks the server's `Inhibited`
property first. Ringing stops when the notification is clicked or dismissed,
when the window becomes active, when any view is granted the microphone or
camera, or after thirty seconds — and it never starts while the window is
already active, where the page rings by itself and ringing over it is heard as
a noise during the call (`should_sound`).

Instagram does not mark a call notification, so `alerts::classify` reads its
words. Extend `CALL_PHRASES` rather than loosening the match: a message that
merely mentions calling must not ring. Nothing about a notification's content
is logged, only that a call rang.

Check it without Instagram, on a local page that posts `new Notification(…)`,
by watching `Notify` with `dbus-monitor` and the player with `pgrep`.

## One arrival, one notification

Two rules, both learned from a real inbox rather than from reading the code.

**The tag is not decoration.** The Notification API says two notifications
sharing a `tag` are the same notification and the second replaces the first.
Instagram uses it the way a phone does: a ringing call is re-posted every few
seconds under one tag. instaCache ignored `QWebEngineNotification::tag`, so a
single call filled the screen with copies of itself. `alerts.rs` now keeps a
`tag -> notification id` map and passes the id as `replaces_id`, which is the
whole fix. Verified on D-Bus: the second and third notification under one tag
carry the first one's id, and a different tag gets its own.

**The unread count must not repeat what push already said.** The count-derived
alert exists for an account where push never delivers; once the page has
posted a notification of its own, push works and the count is a second telling
of the same arrival — which arrives as "and by the way, you have an unread
message" a moment after the real one. So `page_notified_ever` latches, and
after that the count only ever *pings* over a window in front, never posts.
The twenty-second echo window is not enough on its own: a backgrounded page is
throttled by Chromium and can update its title minutes after the push landed.

## Instagram does not say "call" in any language you guessed

`alerts::classify` reads `CALL_PHRASES` out of the notification's words
because Instagram publishes no marker for a call. **The list is not known to
match what Instagram actually sends** — on a French-language account, a real
incoming call produced a screen of notifications and no ring at all, and the
journal carried no `instacache: incoming call` line, which is how you know
`classify` never returned `Kind::Call`.

Do not extend the list by guessing. Capture what arrives first — from
instaCache itself, which is the shortest route:

```sh
INSTACACHE_LOG_NOTIFICATIONS=1 instacache
journalctl --user -f | grep 'instacache: notification'
```

That prints the kind, tag, title and body of every notification a page posts.
It is **off by default and must stay off**: what a notification says is
nobody's business, this log included. It exists because the only way to learn
why a real call did not ring is to see the words Instagram actually sent.

`dbus-monitor --session "interface='org.freedesktop.Notifications',member='Notify'"`
shows the same thing from the other side. Beware of reading back your own
fixtures there: a local test page posting `showNotification` looks identical
on D-Bus to the real thing, so clear the log before the real attempt. That
mistake was made here once already.

A repeated notification under one tag is tempting as a language-independent
"this is a call" signal, and it is what a ringing call looks like. It is not
safe on its own: Instagram coalesces a thread's messages under a tag too, so a
chatty friend would ring. Get the text.

## Nothing is notified unless the push service is on

This is where the whole feature lived or died for two releases, and the
symptom was misleading: the unread badge moved and nothing was ever heard, so
`alerts.rs` looked broken when it was simply never reached.

Instagram sends a message or a call over **Web Push**, from its service
worker. Qt WebEngine registers with no push service unless the profile asks:
`WebEngineProfile.isPushServiceEnabled` is **off** by default. Without it
`PushManager.subscribe()` rejects with `AbortError: Registration failed - push
service not available`, Instagram never registers, no push arrives, its
service worker never runs, `onPresentNotification` never fires. The badge kept
working the whole time because it is read from the page title, which needs no
push at all.

Two traps in one property:

- The QML name is **`isPushServiceEnabled`**, not `pushServiceEnabled`.
  Assigning the latter throws "Cannot assign to non-existent property" and an
  otherwise correct fix does nothing.
- It arrived in **Qt 6.5** and the baseline is 6.4, so it must be *assigned*
  from `Component.onCompleted` inside a `try`, never declared. A declared
  property Qt 6.4 does not know stops the entire scene loading — see "A live
  process, no window and no error".

Measure it rather than reasoning about it. A local page with a service worker
and any VAPID key tells you in one run which side of the line you are on:

```sh
# with the property set, subscribe() resolves with an fcm/send endpoint;
# without it, it rejects with "push service not available"
QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=offscreen ./target/debug/instacache --profile pushcheck
```

`QT_FORCE_STDERR_LOGGING=1` is not optional: Qt sends `qDebug` and the page's
console to journald otherwise, and a run that proves nothing looks like a run
that printed nothing. Only warnings and errors from the page appear at all, so
make a test page report with `console.error`.

`user_prefs.json` in the profile's data directory is the other witness:
`gcm.push_messaging_application_id_map` stays `{}` for as long as no
subscription exists.

## The unread count is the second half of it

A message that arrives while the page is open is *not* pushed — the page is
already connected, and Instagram only changes its title. So the count is the
one signal an open window gets, and `unread_changed` in `bridge.rs` turns a
rise in it into an alert. Two rules keep that honest, and both are tested:

- Nothing is announced for `UNREAD_SETTLE` after a load. Instagram's title
  says `Instagram` while it loads and grows its `(3)` a second or two later;
  without the wait, every launch announces messages from last week.
- A rise within `PAGE_NOTIFICATION_ECHO` of the page's own notification is
  that same arrival counted twice, and is dropped. The page's version names
  the sender; this one can only count.

Over a window that is really in front the count plays the sound and shows
nothing, because the page already shows the message — the same reasoning that
stopped instaCache ringing over an answered call in 2.6.1. "Really in front"
is `root.inFront` in the scene: shown, focused, and not closed to the
background. `root.active` alone is not enough.

## Starting with the session, and the tray's settings

`autostart.rs` writes one file into `~/.config/autostart`. Every desktop reads
the same XDG entry, so there is nothing per-desktop here and nothing to detect.

The rule that is easy to get wrong: **the entry is the truth, not the
setting.** `start_with_session` in `config.json` decides only what the *first*
run of a profile puts there, guarded by an `autostart-applied` marker beside
it. After that, `is_enabled()` reads the file. A user who turns instaCache off
in their desktop's own startup panel must stay off, and a setting that
re-created the file at every launch would silently undo them. Plasma and GNOME
disable an entry by writing `Hidden=true` or
`X-GNOME-Autostart-enabled=false` into it rather than deleting it, which is
why the contents are read and not just the file's existence.

The entry uses `--background` on purpose: logging in should leave a warm page
behind a tray icon, not a window over whatever you opened the machine to do.

The tray menu also switches settings, through `setting` / `set_setting` in
`bridge.rs`. `SWITCHABLE` is an allow-list and is load-bearing — both the
getter and the setter refuse a name that is not in it. `internal_domains` is
the security boundary of the app and `developer_tools` opens the inspector;
neither belongs one click away in a menu. `Shell` holds its `Config` behind a
`RefCell` for this, and `update_config` rewrites the whole `config.json`,
which is what `load_or_create` already does on a first run.

The menu itself is built from a **QML string**, because `Qt.labs.platform` may
not be installed and a failed import in the scene would stop the window
appearing at all. Nothing checks that string at compile time, so a typo costs
the entire tray, silently, with a missing icon as the only symptom.
`test_01c_the_tray_menu_is_valid_qml` in the scene tests exists for exactly
that: it builds the object and fails if `createQmlObject` throws. Run the
scene tests after touching it.

A method is not a property: a binding on `shell.setting(name)` never
re-evaluates on its own. That is what `settingsRevision` is for — bump it and
the bindings re-read. A binding on `trayState()` *does* track `root.visible`
and `root.unread`, because QML captures property reads through a function
call; only the `shell` method needs the manual nudge.

## Packaging for the AUR

`packaging/aur/PKGBUILD.in` is a template; `render.sh` fills in the version
and the two checksums and writes `PKGBUILD` and `.SRCINFO`. The Release
workflow runs it after the GitHub Release exists, because the checksums are
**read from the published `.sha256` files** rather than recomputed — the
package then verifies exactly what was released, and a missing archive fails
the job instead of producing a PKGBUILD with a placeholder in it.

`.SRCINFO` is written by hand because the runner has no `makepkg`. It is
verified to be byte-identical to `makepkg --printsrcinfo` for a real release;
if you change `PKGBUILD.in`, check that again:

```sh
./packaging/aur/render.sh 2.7.0 /tmp/aur && cd /tmp/aur
diff <(cat .SRCINFO) <(makepkg --printsrcinfo)
makepkg --nodeps --noconfirm && bsdtar -tf *.pkg.tar.zst
```

The push step is skipped when `AUR_SSH_PRIVATE_KEY` is not configured, so a
fork still releases. On this repository it is already set, along with the
`AUR_USERNAME` and `AUR_EMAIL` variables; the key pair lives in
`~/.ssh/aur_instacache` and exists for nothing but this.

### Nobody reviews an AUR package

There is no review queue and no approval. A push to
`ssh://aur@aur.archlinux.org/<name>.git` publishes, and the first push creates
the repository if the name is free. The Package Maintainers only step in after
the fact, through orphan, deletion and merge requests. That is exactly why the
checks in this pipeline are worth having: nothing else is going to catch a
PKGBUILD that installs the wrong files or verifies the wrong archive.

The conventions that are actually enforced, socially: a package built from a
released binary must end in `-bin`, it must `provides`/`conflicts` the name it
stands in for, and it must not duplicate something already in the official
repositories.

### The push does not use a third-party action

It is plain `git` in the workflow. The AUR key is the one credential here that
can publish under somebody's name, and handing it to an action nobody in this
repository reviews is a larger risk than writing eight lines of shell.

The host keys are fetched with `ssh-keyscan` and then **verified against the
fingerprints the AUR publishes**, and all three must be present — a bare
keyscan trusts whatever answers on the network, which is the whole attack. The
fingerprints are in the workflow. If the AUR rotates a key this fails loudly
rather than trusting the new one; update the list from
<https://aur.archlinux.org/> and satisfy yourself about why it changed.

Both halves are tested: the real keys pass, a foreign key injected among them
is rejected, and a downgrade to a single key is rejected.

**A packaged copy must not update itself.** `updates::is_package_managed()`
is how it knows: `install.sh` puts `update.sh` under the same prefix as the
binary and a distribution package does not, so "no updater beside it, in a
prefix the user cannot write" means pacman owns this copy. Without that check
an AUR user is offered an update whose only route is `sudo instacache
--update`, which overwrites files pacman owns and is reverted by the next
`-Syu`.

## A binary replaced under a running copy

`/proc/self/exe` ends in ` (deleted)` once the file was replaced. `bridge.rs`
looks every twenty seconds and treats it as an installed update, so a window
closed to the background takes it over by its quiet restart. Without that, an
`instacache --update` run by hand left the old copy running for as long as it
stayed in the background, and every launch from the menu was handed to it.

## A crash when quitting after a video played

Quitting — a window closed with `run_in_background` off, or `Ctrl+Q` — shortly
after a video played occasionally dies in Chromium's shutdown: `SIGTRAP` from
a Chromium thread, or glibc's `__pthread_tpp_change_priority` assertion. It is
not new: a build from before the background changes did it too, 1 time in 11,
on the reference machine, where CachyOS's `foreground_booster` changes process
priorities on every focus change. The window state is already saved by then.
Not fixed; if you look into it, start from the priority assertion.

## A test launch that does nothing

`instance.rs` gives one window per profile, and a second launch of the same
profile hands its URL over and exits 0 immediately. That is correct behaviour
and a trap while testing: a stray instance left from an earlier run swallows
every launch after it, so the app appears to do nothing and the bench records
nothing.

It cost a wrong conclusion here — a change was blamed for breaking rendering
when it had simply never run. A window closed to the background is exactly
such an instance, and it looks like nothing is running at all — and a test
launched with the default profile while the user's own instaCache runs goes to
*their* window and raises it. Give each test its own profile name, and check:

```sh
pgrep -ax instacache; ls "$XDG_RUNTIME_DIR"/instacache-*.sock
```

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

The automatic path is tested the same way, without a terminal command: set
`XDG_DATA_HOME`, `XDG_CONFIG_HOME` and `XDG_CACHE_HOME` to a scratch directory
— the installer adds menu entries through them, and must not touch yours —
give that config `"update_check_interval_hours": 0`, and start the scratch
binary with `QT_QPA_PLATFORM=offscreen` and `--background`. The log should show
the install, `takes over at the next restart`, and `restarted into the updated
version`, and the first process should exit on its own.

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
