# Working on instaCache

Conventions and hard-won facts for anyone — human or agent — changing this
repository. It is also the project's memory between sessions, and it is kept
that way by whoever works here; see the first rule below. If a rule can be
enforced by CI instead, enforce it there.

This is the only instruction file. Claude Code reads `AGENTS.md` by itself when
no `CLAUDE.md` exists, and ignores it when one does unless that file imports
it — so do not add a `CLAUDE.md`, and do not split rules into a second file.

## How to work here

These come from the owner and apply to every session, before anything else in
this file.

### This file updates itself

The owner does not say "write that down". When they state a preference, a
correction, a decision or a way of working — in passing, in any session, in any
words — it goes into this file in the same piece of work, without being asked.
Being told to edit `AGENTS.md` is the exception, not the trigger.

Keeping it a memory without letting it turn into a log:

- **Record what will still matter next session**: a lasting preference, a
  rule, a fact that cost time to learn. Not a one-off instruction for the task
  at hand, and not an account of what was done — that is `git log` and
  `CHANGELOG.md`.
- **Write it where it belongs**, in the section it concerns, merged into what
  is already said. A rule is stated once. A new section only for a new subject.
- **Rewrite, don't append.** When something new contradicts or supersedes what
  is here, change or delete the old text in the same edit. A stale rule is
  worse than none, because it is believed.
- **Record the intent, not the sentence.** A remark made in frustration about
  one bug is usually a rule about a whole class of them; write the rule.
- Commit it with the change it came from, or on its own as `docs:` if there was
  no other change.

### Finish the job

A task is done when it is complete, checked ("Before you commit"), committed
and pushed — not when the code compiles, and not with work left uncommitted
in the tree. Commit each part as soon as it is validated, with Conventional
Commits (`feat:`, `fix:`, `refactor:`, `docs:`, `style:`). No `Co-authored-by`
trailer and no line naming an AI tool in a commit message.

**The owner works on `main`, with no pull requests.** The project is small
enough that a branch and a review add nothing: an agent working for the owner
commits on `main` and pushes to `main` directly — no feature branch, no pull
request, even if a session's tooling suggests one. The checks above are the
gate, and CI runs on every push. Anyone without push access to this repository
has no such route, and contributes through a pull request as usual.

**A release is cut only when the owner asks for one**, with
`scripts/release.sh` (see "Releasing"). Pushing to `main` does not release.

### Every point gets an answer

A request with several points — the owner's usually has a dozen — is kept
as a numbered list for the whole of the work, and the report at the end
takes them one by one: done, and how it was checked; not done, and why; or
a question only the owner can answer. No point disappears in silence, and
nothing is reported as done that was only written. Doing half of a request
without saying which half is the failure this rule exists for.

### Not a yes-man

If a request is mistaken, breaks a non-negotiable, duplicates something that
already exists or has a better route, say so with the reason before doing it.
Agreeing is not the goal; a project that works is.

### English in the repository, the owner's language in conversation

Everything committed is English: code, comments, documentation, this file,
commit messages, release notes.

**Every word addressed to the owner is in the language they write in** —
French, so far. Not only the final summary: every progress note between two
steps, every question, every explanation of a command. Thinking in English
and writing to them in English is the mistake to avoid; it happened, and they
could not follow the work. Translate when it goes into the repository, never
when it comes back to them.

### Intent over wording

The owner often dictates. Take what they mean, not the exact words: a request
to "add" something that already exists means improve the existing one, and a
detail they did not mention is decided by what the rest of the request is
clearly for. Ask only when two readings lead to different work and the code
cannot settle it.

### Clean and coherent, always

No dead code: a change removes whatever it makes unused — functions, config
keys, flags, tests of removed behaviour, documentation of it. The code, this
file, `README.md` and `CHANGELOG.md` say the same thing at every commit; a
change that makes one of them wrong fixes it in the same commit.

## What this project is

One Qt Quick window hosting one Qt WebEngine view that displays Instagram,
with the cache and session pinned to persistent XDG directories. The value is
in the persistence and the desktop integration, not in any UI of our own.
Resist adding chrome: what exists is the splash over a cold start, the
loading bar, the back button shown over a page, the pills for a call and an
update, the tray icon, the card of shortcuts and tips with its dim `?` in
the bottom corner, and the window's own title bar, and each of those is
there because the window has no other way to offer it. The last two are the
owner's requests: gestures nobody can see are gestures nobody finds (the
card is also on `F1` and in the tray menu), and the desktop's frame made the
window look like a browser rather than one application, as Discord's does
not. The title bar sits above the page, never over it, in the page's colour
— read from `--ig-primary-background`, or the body's for another site, and
faded to, so the two meet without a seam;
moving and resizing are handed to the compositor (`startSystemMove`,
`startSystemResize`, both in Qt 6.4), the only way a Wayland client may do
either. `own_title_bar: false` gives the desktop's frame back. The splash is drawn with `Canvas` from the same
shapes as `assets/instacache.svg`, because an SVG image needs a Qt module a
distribution may not install, and a missing one costs the whole scene. Change
one and change the other.

The mark is instaCache's own: a lens cut by a lightning bolt, on Instagram's
gradient. Until 2.8 it was Instagram's camera glyph with a bolt added, which
is Meta's trademark with a sticker on it; do not drift back towards it.

Where it is going: what Vencord is to Discord, for Instagram — the page itself
changed by built-in mods (sound on by default, styles, small features), not a
browser wrapped around it. That is a change to the page, which is welcome; it
is not a licence for window chrome, which the paragraph above still governs.
The mods are built in: no marketplace, no mod loader for other people's code.

Instagram first. XCache ships as the one extra site and `--add-site` stays a
one-line command, but instaCache is not a launcher or a store for wrapping
arbitrary sites, and work goes into Instagram before it goes anywhere else.

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

## The account comes first

instaCache holds somebody's real Instagram account, and nothing it does may
make that account look like anything but a person in a browser. Everything
below was checked against the real engine, not assumed; keep it true.

- **Nothing reaches Instagram's servers without a gesture.** A mod may change
  what the page looks like and press local controls — the speaker, a story's
  pause — on its own, but a like, a repost, a save or a comment is pressed
  only for a click or a key, one for one, at most once every 800 ms
  (`paced` in `shortcuts.js`). No timers that act, no queues, no loops over
  posts, no private API calls. Moving to the next story, which tells the
  author it was seen, is one story for one turn of the wheel. That is why
  "mark every story as seen" is not built: it can only be done by acting
  without the user.
- **A feature that puts the account at any risk is not built, even when it
  is asked for.** The owner's own words, when the risks of marking every
  story as seen were put to them: an account is a whole life, nobody wants
  it banned, and what is strange or cannot be done safely is not done. Say
  so when such a feature is requested, with the risk, and leave it out.
- **A coherent browser, never a disguised one.** The user agent says Chrome
  on Linux at the engine's real Chromium version (`config::user_agent_for`),
  and `Accept-Language` is the session's language
  (`config::accept_language`) — Qt WebEngine 6.4 sent none at all, measured.
  What the engine itself reports — `navigator.userAgentData`, which is empty
  on 6.4, and its features — is left as it is: faking it is detectable in
  itself, and an honest Qt WebEngine is what qutebrowser and Falkon users
  sign in to Instagram with every day. `navigator.webdriver` is `false`.
- **One device, steadily.** The session and cookies persist, so Instagram
  sees the same browser every time; the user agent changes only when the
  distribution's Qt does, as a browser updating would.
- **Never automate the real site from tests** — see "Reproducing a crash".
  A logged-out page load to read its structure is fine; anything that clicks
  through a logged-in account is not.

To see what a page receives, serve one locally that logs its request headers
and reports `navigator.userAgent`, `userAgentData` and `languages`, and point
a scratch profile's `home_url` at it.

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
| `src/mods.rs` | The built-in mods, and the one script that carries them. |
| `src/qml/main.qml` | The window, its pages, the loading bar, the shortcuts. |
| `src/badge.rs` | The unread count on the task bar icon, over D-Bus. |
| `src/alerts.rs` | Notifications, their sounds, and ringing for a call. |
| `src/bus.rs` | The one session bus connection everything shares. |
| `src/wake.rs` | How a thread with news wakes the scene. |
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

A site's entry is written once, and was never rewritten: a site added by an
older version kept that version's entry and icon through every fix, and its
window sat in the task bar under instaCache's icon — the icon theme falls back
from `instacache-xcache` to `instacache` by itself when the first is missing.
`--setup-sites`, which `install.sh` runs at every install and update, now
rewrites every site's entry in the current format and puts its icon back in
the theme, fetching it again if it never arrived (`sites::refresh_existing`).
A change to `desktop_entry` therefore reaches every existing site.

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
thread it interrupted mid-allocation. It also writes one byte to a socket pair,
which is the other thing a signal handler may do; a thread blocked on the other
end wakes the scene, which notices the flag and shuts down through the ordinary
close path.

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

The cheapest way to cause it: a word Qt 6.4's JavaScript still reserves, used
as a name. `function rest(long)` shipped in 2.8.0, parsed on the newer Qt of
the reference machine, and left 6.4 — Debian 12, Ubuntu 24.04, the baseline —
with no window at all. `long`, `int`, `short`, `byte`, `char`, `float`,
`double`, `final`, `native`, `synchronized`, `throws`, `transient`,
`volatile`, `goto`, `abstract`, `boolean`: none of them as a name. Running
the scene tests on 6.4 is what catches it.

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

## Mods

A mod is a script, a stylesheet or both in `src/mods/`, compiled in,
switched by a setting of its own, listed in `BUILT_IN` in `mods.rs`.
`mods::script` wraps every enabled one in its own `try`, adds a stylesheet
through `addStyle` as soon as the document has somewhere to put it, and
passes `options` from the config; the scene installs the result as one user
script on the profile (`installMods`), at `DocumentCreation`, in the main
world. `examples/snapshot.rs` installs it the same way, so a snapshot shows
the page with its mods. That is the difference from `user.js`,
which runs after the load: a mod is there before Instagram's own code, so it
can change how the page starts instead of repairing it afterwards.

- **Find Instagram's controls by their labels, never guess the labels.**
  `page.js` comes first in the script and is shared by every mod: `LABELS`
  holds the `aria-label`s of the controls the mods press — speaker, Like,
  Comment, Save, "Also from Meta" — each read off instagram.com in English
  and French, and `postOf` finds the post a picture or video belongs to as
  the nearest ancestor holding a Comment button, which every post has, liked
  or not. A label in no list means the control is not found and nothing is
  pressed, which is the failure to want. Labels are compared through
  `plain`: any run of spaces as one, either apostrophe as U+2019. A label
  that reads "Autres applications Meta" in the console may hold
  non-breaking spaces, and 2.12.0's exact comparison — script and CSS —
  missed exactly that case on a test page; CSS matches the first and last
  words (`^=` and `$=`) for the same reason. To capture more, run in a browser
  console on the page: every `svg[aria-label]` with the role of its
  clickable ancestor, and every element that overflows sideways. Logged-out
  post pages (`/p/<id>/`) show the post's controls; stories and the side bar
  need an account, so the owner runs it. A feed post logged in, in French,
  carries `Plus d’options`, `Le son est coupé`, `J’aime`, `Commenter`,
  `Republier`, `Partager` and `Enregistrer`, each a `div[role=button]`, and
  the side bar's links include `Paramètres`. That is how the French "Also from
  Meta" turned out to be "Autres applications Meta", which the mod had been
  missing, and how the stories row turned out to be a plain sideways scroll
  container — so the wheel moves it, where Instagram's arrow misses every
  other click. Serve a test page with `<meta charset="utf-8">`, or its
  accented labels arrive mangled and look like a bug in the mod.
- **Press Instagram's own switch rather than working around it.**
  `videos_with_sound` once set `muted = false` on the element: the first video
  had sound under an icon that said muted, and the next video started muted.
  Instagram keeps one sound switch for every video; the mod presses its
  speaker button once (a synthetic `click()` works — measured on the real
  page), and the element is unmuted directly only when no button is found.
  The shortcuts likewise press Like, Repost, Comment, Save and a story's
  pause button.
- **A double gesture must not also do what its first half does.** On Reels,
  Space is the way to the next reel; letting the first Space through and
  liking on the second liked the reel and skipped it. So on Reels the mod
  holds every Space for 250 ms (in the capture phase on `window`, ahead of
  Instagram's listeners) and either likes, on a second one, or scrolls the
  reels itself (`scrollerOf`); where it finds nothing to scroll, Space stays
  Instagram's. A double-click is two clicks, and a click pauses a reel: the
  state at the first `pointerdown` is put back once the like is done.
- **A smooth scroll asked for by the page is not smooth here.** Qt WebEngine
  turns Chromium's scroll animator off (`ScrollAnimatorEnabled`, which QML
  in 6.4 cannot reach), so `scrollBy({behavior: "smooth"})` lands in one
  frame — measured, 0 to 796 pixels between two frames — and the next reel
  appeared with no movement. `glide` in `shortcuts.js` eases it over
  frames instead, with the scroller's snapping suspended for the move.
- **A right-click reaches the page.** Chromium hands `contextmenu` to the
  page first and shows its menu only if the page lets it, so the repost on
  a right-click works with `context_menu` off, and steps aside with it on.
  Shift+double-click, which it replaces, did nothing on the owner's
  account.
- **Full screen refuses a transform**, and Instagram's containers clip a
  video turned inside them. `O` therefore shows a copy — the video's own
  frames through `captureStream()`, in a layer of the mod's own — which
  costs no second decoding and is silent; its source keeps playing. Escape
  and F11 in the scene ask the page to leave its full screen
  (`leaveFullScreen`): setting the window alone left the page believing it
  was still in it.
- **The wheel over an open story presses Instagram's arrows**, "Suivant"
  and "Précédente" (`div[role=button]`, captured on the owner's open story,
  beside "Mettre en pause", "Le son est coupé", "J’aime", "Direct", "Menu"
  and "Fermer"). Where no button is found — a language not captured yet —
  it sends the arrow key Instagram answers on a story instead.
- **The volume is the page's to remember.** `+` and `-` set it, and it is
  kept in the page's `localStorage` (`instacache:video-volume`), which is
  the profile's: no channel back into Rust, which a page could also write
  through, and it takes effect in the very page that set it.
  `video_volume` in the config only says where a profile starts.
- **`WebEngineScript` is a value type in Qt 6.** `WebEngine.script()` makes
  one and `userScripts.insert()` takes it; declaring one in QML is what once
  stopped the scene loading. Measured on 6.4: a script inserted this way runs
  while `document.readyState` is `loading`, before the page's first script.
- **A mod listens; it does not replace.** Patching a prototype the page relies
  on breaks the page in ways that look like Instagram's bugs. Listening in
  the capture phase on `document` hears every media event, which do not
  bubble — but only from elements in the document.
- **A mod must give up rather than fight.** `videos_with_sound` unmutes a video
  at most three times, and stops for the page's life once the user mutes a
  video that had its sound, with a real click (`navigator.userActivation`).
  "Had its sound" matters: transient activation lasts seconds, and Instagram
  creates the next video muted a moment after the click on "Reels" — read as
  the user muting, it switched the mod off for the session, and the owner's
  videos arrived silent. The speaker is found from the video itself
  (`speakerOf`), not through the post: logged in, the video and its Like
  button need not share a block.
- **A call is never touched**: a `MediaStream` source is left alone.
- **Never key on Instagram's class names.** They are hashed and change from
  build to build: its dark theme was `_aa4d` and `__fb-dark-mode` on one
  load, `__ig-dark-mode` in its inline CSS. Roles, attributes and its CSS
  variables are what stays. `instagram_colors` reads the colour of
  `--ig-primary-background` to know the page is dark, marks `<html>` with a
  class of its own, and overrides the `--ig-*` variables — as `R, G, B`,
  which is how Instagram writes them — under that class.
- **"Also from Meta" stayed visible for the owner through 2.12.0**, whose
  exact rule should have hidden their one copy (`a[role=link]` around the
  labelled icon). The likeliest cause is a non-breaking space in the label;
  2.12.1 compares loosely. If it still shows, capture from inside
  instaCache (`developer_tools: true`, `F12`), with the label's character
  codes, not from a browser.
- **Restyling the page is the expensive thing.** `--ig-*` live on `<html>`, so
  changing one restyles every element — measured at 55 to 90 ms for ten
  thousand elements in a container with no GPU. `instagram_colors` writes them from one
  tint — per kind of page, and with `ambient_colors` the colour of the media
  in view mixed in — only when the media in view changes, after it has a
  frame, once a scroll has come to rest, in four steps. Reading the colour of
  a playing reel every two seconds made the next reel stutter in. Pictures
  carry `crossorigin="anonymous"` and Reels are `blob:` sources of the page's
  own, so both can be read into a canvas; weigh pixels by their alpha, or a
  video with no picture reads as black.
- **The background stays dark, a tint per page, eased.** The owner's rule:
  Instagram's colours on, a tint for each kind of page as before, changes
  as a light fade, and nothing garish. 2.12.0 made the tints brighter and
  let them drift slowly, and the owner's logged-in page came up bright red —
  every surface painted with `--ig-primary-background` — which no test page
  reproduced and no arithmetic in the mod explains. 2.12.1 went back to
  2.11's tints and easing, and `write` refuses anything but finite numbers
  and bounds every channel to 0–48. Do not brighten the tints or bring the
  drift back without the owner's own window to check it on.
- **Light over the page, not under it.** The halos the owner asked for —
  visible, different for each kind of page, moving — are a layer of the
  mod's own (`glow` in `instagram_colors.js`), appended to `<html>` beside
  `<body>`, `pointer-events: none`, three radial gradients at most a third
  opaque at their heart, drifting by `transform` and crossfading by
  `opacity` when the page changes. Nothing in it touches `--ig-*`, so it
  never restyles the page. It is over the page because nothing can be
  under it: Instagram's surfaces are opaque, by the next rule.
- **Opaque colours, not transparency.** A tinted background that let a
  gradient through looked right on one layer and disappeared on the page:
  Instagram stacks several containers painting the same background, and
  their alpha adds up. The gradient lives where instaCache owns the pixels —
  the splash — and the window's own colour matches the mod's background.
- **Animations are opacity and transform only**, which the compositor runs
  without laying the page out again, and nothing moves under
  `prefers-reduced-motion`. **Nothing reacts to a press**: a `:active`
  shrink broke Reels, whose whole video is a `role="button"` — the reel
  shrank and showed a white edge, and the speaker button shrank away from
  the pointer, so the click landed on the video and paused it. The engine is Chromium 102 on Qt 6.4: nothing
  newer than that, such as `@starting-style`.
- Mods are on for Instagram and off for another site by default, like the
  messenger settings below: they are written for Instagram's page.
- Switching a mod in the tray reinstalls the script and loads Instagram
  again, so it takes effect at once (`reloads_page`). A switch that changed
  nothing visible until the next load read as a menu that did nothing.

Check a mod against a local page with the real binary rather than against
Instagram: a page that creates the element the way Instagram does, attached
to the document, and reports what it sees with `fetch` to a local server
whose log you read. The scene tests check only that the mods are installed
early, with a stand-in script.

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

In a container with no GPU — a cloud session, say — `qmltestrunner` on Qt
6.4 dies with a segmentation fault as Qt WebEngine starts, on an untouched
tree too, because it initialises the engine from a plugin without
`Qt::AA_ShareOpenGLContexts`. Setting that attribute before `main` runs is
enough: preload a two-line library whose constructor calls
`QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts)`, built with
`g++ -shared -fPIC … $(pkg-config --cflags --libs Qt6Core)`, and run the
suite with `LD_PRELOAD` and `QTWEBENGINE_DISABLE_SANDBOX=1` when running as
root. On Qt 6.4.2 in such a container, `test_05` fails and the run dies
after `test_14`, on an untouched tree as well; compare against one before
blaming a change. Clear the cache directory between runs, or a page served
slowly on purpose comes back from the cache at once.

An overlay that fades in is invisible for its first frame if `visible` is
`opacity > 0`, and a click in that frame goes to the page underneath — a
test clicking straight after opening the tips opened a link instead. One
that must catch clicks is visible from the moment it is shown.

A new `shell` property or method has to be added to `MockShell.qml` too. A
missing one does not stop the scene loading; it throws only when called. The
mock's `wake` fires whenever a test changes `pollResult` or `update`, which is
how a test stands in for a thread with news.

The scene tests run the mock, so they cannot see anything the real bridge does
wrong — the binding trap above passed every one of them. After changing
`bridge.rs` or a signature the scene calls, run the real binary once as well:
a QML call with the wrong number of arguments only prints `Insufficient
arguments` to the log, and does nothing. And rebuild the binary itself:
`cargo test` builds the test harness, not `target/debug/instacache`, which
keeps the scene it was last built with.

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
  The scene asks whenever it drains (see "The scene is woken, not polled")
  and when the window closes, and quits; `main` starts the new binary with
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

### Updates that are seen

Opening the window while an update is `ready` shows the update card and
restarts into it after `updateCountFrom` seconds, unless there is a call or
the user has said "Later" or "Not now" to that version. It is deliberately
on *opening*, not whenever an update lands: a window in use gets the pill,
because a restart in the middle of typing a message is a lost message.

The new version knows it is new from `last-version` in the profile's config
directory (`updates::note_running_version`), and says so once: a pill when
its window shows, and a notification when it starts hidden, which is what a
quiet restart is. A first run, a restart of the same version and a downgrade
say nothing. To see it without releasing anything, write an older version
into that file and start the binary.

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

## Resting in the background: frozen, then discarded

Hidden views are already throttled by Chromium, so freezing buys little
processor time: measured on a heavy local page closed to the background, 1
tick in ten seconds hidden, 0 frozen. Discarding is what gives memory back —
the renderer process goes: 314 MiB of PSS across the process tree, then 127
MiB once discarded, measured with `/proc/*/smaps_rollup` over every
descendant of the main process (the renderer is a great-grandchild, under
the zygotes; a two-level walk misses it and reads 166 MiB for everything).

The scene does it, Rust decides how far (`background_lifecycle`):

- `restTimer` freezes once the page has loaded and settled for ten seconds,
  so a load, a read receipt — and, at a `--background` start, Instagram's
  push subscription — finish first. Freezing before the subscription would
  leave nothing to deliver a message at all.
- `discardTimer` discards after `discard_in_background_after_minutes`.
- Never further than the view's `recommendedState`, and never past `Active`
  for a call.
- `rouse()` sets every view back to `Active` *before* the window shows: the
  setter refuses anything else for a visible view. A discarded Instagram
  reloads, and the snapshot stays up until the reload has finished, then
  waits for frames as usual, with an eight-second cap.

A discarded page did not close the desktop notification it had posted, on
Qt 6.12; check that again if the notification handling changes.

What is **not** established: whether Instagram still pushes to a client whose
connection is open but frozen. A frozen page does not read its socket, so if
Instagram's server counts that client as online and skips the push, a message
arriving in the first thirty minutes is only heard when the window opens.
Discarding closes the socket, which is unambiguous. `freeze_in_background:
false` is the switch if a real account shows the problem; test it with
`INSTACACHE_LOG_NOTIFICATIONS=1` and a message sent from a phone a few minutes
after closing the window.

## The HTTP cache

Chromium sizes the disk cache itself unless told, and kept about 300 MB
here: 272 of 300 one-megabyte responses survived. An evening of Reels is
more than that, and what it evicts is Instagram's scripts, fetched again at
the next start. `http_cache_size_mb` (1 GB by default) is passed as
`httpCacheMaximumSize`, which is an `int` of bytes, so it stops at 2047.
Nothing about it is visible to Instagram. Prefetching pages or media is
not done: requests the user did not ask for are what a bot makes.
`dnsPrefetchEnabled` is on, as in every Chrome; it asks the resolver, never
the site.

## Per-site defaults

A profile whose `home_url` is Instagram gets `calls`, `ring_for_calls`,
`run_in_background` and `tray_icon` on; any other site gets them off.
`Config::from_json` fills a key the file leaves out with the default for *that
file's* home page, so XCache's config, written before those keys existed, does
not inherit a messenger's behaviour. A key present in the file always wins.
New settings that only make sense for a messenger belong in `Config::for_home`.

## Notifications and ringing

`alerts.rs` keeps every notification's state on one thread and listens for the
server's `ActionInvoked` and `NotificationClosed` on one more, both over the
connection in `bus.rs`. Do not go back to a thread per notification: that is
what `notify-rust` did, opening a D-Bus connection — a socket and a zbus
executor thread — for each one and holding it until the notification closed.
Measured, thirty notifications left on screen took 2.7.0 from 48 threads to
108; the same thirty now leave it at 50. A desktop that keeps notifications in
a history may never close them, so that was a leak by the hour. It also
`unwrap`ped D-Bus errors inside its threads, and `panic = "abort"` turned a
hiccup on the bus into the whole application quitting.

The listener is subscribed *before* the first `Notify` goes out, or a click
quicker than the subscription is lost. The bus carries every application's
notifications: an id that is not in the registry is somebody else's, and is
ignored.

Every page notification has a key the scene chooses, and each key comes back
from `poll` exactly once as `clicked`, `closed` or `forgotten` — the scene
holds the `WebEngineNotification` until then, and a key that never came back
would hold it for ever. `Registry` in `alerts.rs` is where that promise is
kept, and tested: replaced under a tag, forgotten; past `TRACKED`, the oldest
forgotten; a `Notify` that failed, forgotten at once. In the other direction,
the page closing its own notification (`closed` on the notification) reaches
`page_notification_closed`, which takes it off the desktop and stops a ring.
Whether Instagram closes a call's notification when the caller gives up is not
yet verified against a real call; if it does, the ring stops with it instead
of running on for thirty seconds.

A server with `body-markup` reads the body as markup, and `<3` is the start of
a tag to it: the body is escaped for such a server and only for one.

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
by watching `Notify` with `dbus-monitor` and the player with `pgrep`. To click
and dismiss without touching your own desktop, run the binary under
`dbus-run-session` with a stand-in notification server on that private bus — a
few lines of Python with `Gio` that answers `Notify` and emits `ActionInvoked`
and `NotificationClosed` on request — and stub `xdg-open` on `PATH` so no link
reaches your browser. Kill the app on the way out: a `timeout` around
`dbus-run-session` does not, and a leftover instance writes into the next run's
log.

## The scene is woken, not polled

The scene used to ask Rust what happened on a timer: every 250 ms with the
window open and every 80 ms with it closed, so a launch from the menu would
show the window quickly. Measured on the main thread of a window closed to the
background, ten seconds: 125 wake-ups. Now a thread that hands something over
calls `Wake::wake`, which emits `shell.wake` through
`qmetaobject::queued_callback`, and the scene drains everything with one
`poll`: 2 wake-ups in the same ten seconds. A twenty-second timer remains, for
what nobody announces — whether the hourly update check is due, and a binary
replaced on disk.

Hand the news over *before* calling `wake`, and call `drained` at the *start*
of `poll`: that order is what makes it impossible for something to arrive
between a drain and the next wake-up.

### The first signal of `Shell` must never fire

qmetaobject 0.2.10, for Qt 6, writes index 0 as the notify signal of every
property that has no `NOTIFY`, where moc writes "none". So QML hears the first
signal declared on `Shell` as "every property changed" and re-runs every
binding on one. When `wake` was the first signal, `url: shell.home_url` re-ran
at each wake-up, set the view's address and reloaded the page — which posted a
notification, which woke the scene: a load every few milliseconds. That is why
`unchanging` is declared first and never emitted. Keep it first; a new signal
goes after it.

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

**Instagram repeats itself too.** A real inbox showed two notifications for
one arrival, both from Instagram: "You have unseen notifications." beside the
message. A notification that says only that something arrived is a `Nudge` —
Instagram's, and instaCache's own count-derived "New message" — and
`Coalescer` in `alerts.rs` tells each arrival once: a nudge waits four seconds
for the notification that names the sender and is dropped when it comes; a
nudge within twenty seconds after one is an echo; a nudge still on screen is
taken off when a detailed one follows, however late — on a real inbox,
Instagram's came well before the message it announced. All nudges share one tag, so they
replace each other rather than stack.

**What was read leaves the desktop.** When the unread count falls to zero —
read in the window, on the phone, or answered — `unread_changed` asks
`alerts.rs` to take every message notification and nudge off the desktop
(`Registry::messages`); a call's notification stays, and so do instaCache's
own. Only zero is trusted: a count that merely falls says some thread was
read, not which. A frozen page reads no count, so a notification read on the
phone while the window is closed goes when the window opens.

**An unsent message leaves nothing behind.** Instagram re-posts the message's
notification under the same tag as "This message is no longer available
because it was unsent by the sender." That is `Kind::Unsent`: the notification
under that tag is closed, and nothing replaces it.

Both are recognised by their words, captured from a real account, in English
only so far; the rule in the next section applies to extending them.

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

The menu is **flat**, the mods under a disabled "Mods" header. A `Menu`
inside the tray's menu is exported over `com.canonical.dbusmenu` as a
sub-menu with no children at all — measured on Qt 6.4 with `GetLayout`,
before and after `AboutToShow` — so on KDE it opened empty and the owner's
clicks on it did nothing. Do not nest menus there again.

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

The host keys are fetched with `ssh-keyscan`, one type at a time with
retries — asked for all three at once, the AUR twice answered for only two —
and then **verified against the fingerprints the AUR publishes**, and all
three must be present — a bare
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

Meta's link shim (`l.instagram.com/?u=…`) is the exception that is handled
before the allow-list: every outside link in Instagram points at it, and as an
Instagram host it used to open in the window, record the click, redirect, and
leave an empty page behind once the browser took the destination.
`unwrap_link_shim` reads the destination out of it — exact shim hosts only,
`http`/`https` destinations only, Meta's `fbclid`/`igshid`/`igsh` removed —
and the scene sends that straight to the browser, from a navigation and from a
new window alike. A destination that is itself internal is left to the shim.

Whatever leaves goes through `external_target` in `bridge.rs`, which refuses
the engine's own schemes — a page cannot have a local `file:` opened — and
then `Qt.openUrlExternally` in the scene. Not a spawned `xdg-open`: on
Wayland a window comes to the front only with an activation token, and from
Qt 6.5 `QDesktopServices::openUrl` requests one from the focused window and
hands it to the portal or to `xdg-open` (`runWithXdgActivationToken` in
`qgenericunixservices.cpp`). Spawned without one, the browser opened the link
behind instaCache's window. Qt 6.4 has no token to give, and behaves as before.

`instacache:` links come the other way, from a browser:
`instacache.desktop` declares `x-scheme-handler/instacache`, and
`urls::from_app_link` turns `instacache:https://…` — or `instacache://host/…`,
which a browser may normalise it to — into the web address, and nothing but a
web address. The page then goes through the same allow-list as any other.
`extras/open-in-instacache.user.js` is the browser half, as a userscript
rather than an extension: nothing to publish to a store, nothing to review.

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

A cloud session's git proxy pushes branches and refuses tags (`HTTP 403` on
the tag, after `main` went through). There, run `release.sh` as usual, then
start the Release workflow by hand on `main` with the tag as its input: its
first job creates the missing tag on that commit, and only when `Cargo.toml`
there says the same version. Delete the local tag the script made; the real
one is the workflow's. The jobs after it state `!failure() && !cancelled()`
because a skipped job up the chain — this one, on a pushed tag — otherwise
skips everything below it.

Two traps the script now handles, both found the hard way:

- It rebuilds after bumping. The checks run against the *old* version, so
  without a rebuild `./install.sh` straight after a release installs a binary
  that reports the previous number.
- It leaves a hand-written changelog section alone. Generating one on top of it
  produced two `## [1.1.0]` headings in the same file.
