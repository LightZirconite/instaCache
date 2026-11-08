// The whole visible application.
//
// This scene owns widgets and nothing else. Every decision — whether a URL
// stays in the window, where a download goes, what the offline page says,
// whether a dead renderer should be reloaded again — is asked of `shell`,
// which is Rust. Keep it that way: policy in QML cannot be unit tested.
//
// One window, and one page in it at a time. Instagram itself is `view`, which
// lives as long as the window. Anything a page opens — a link in a new tab, a
// `window.open`, a call — becomes a page laid over it, with a back button to
// return. Opening another page closes the one before, so the window never
// turns into a browser full of tabs. The one exception is a call: leaving it
// only hides it, it keeps running, and a pill brings it back.

import QtQuick
import QtQuick.Window
import QtWebEngine

Window {
    id: root

    // Filled in from the saved geometry, or from the screen on a first run.
    // Read before the window is shown so it never appears at one size and
    // jumps to another.
    property var geometry: JSON.parse(
        shell.initial_geometry(Screen.desktopAvailableWidth,
                               Screen.desktopAvailableHeight))
    property bool stateSaved: false
    property bool quitting: false
    // `--background` starts hidden; showing it later restores what the saved
    // geometry asked for.
    property bool hiddenMaximized: geometry.maximized

    // Where an update stands, from `shell.poll`: "none", "available",
    // "installable", "installing" or "ready". See bridge.rs.
    property string updateState: "none"
    property string updateVersion: ""
    // "Later" hides the offer until something about the update changes.
    property string updateDismissed: ""

    // The page laid over Instagram, or null when Instagram itself is shown.
    property WebEngineView current: null
    // Every page that exists, shown or not. Reassigned rather than mutated,
    // so that bindings on it notice.
    property var pages: []
    // A page that holds a call, which is kept when it is not shown.
    property WebEngineView callPage: {
        for (var i = 0; i < pages.length; i++) {
            if (pages[i].inCall)
                return pages[i];
        }
        return null;
    }

    title: {
        var shown = root.current !== null ? root.current.title : view.title;
        return shown !== "" ? shown : shell.window_title;
    }
    width: geometry.width
    height: geometry.height
    x: geometry.x !== null && geometry.x !== undefined ? geometry.x : x
    y: geometry.y !== null && geometry.y !== undefined ? geometry.y : y
    // Setting a visibility other than Hidden shows a window, so a hidden
    // start has to say Hidden here as well as in `visible`.
    visibility: shell.start_hidden ? Window.Hidden
              : geometry.maximized ? Window.Maximized : Window.Windowed
    visible: !shell.start_hidden
    // Instagram's dark background as the `instagram_colors` mod tints it, so
    // nothing changes colour between the splash, the window and the page.
    color: "#120c1a"

    function writeState() {
        var maximized = root.visibility === Window.Maximized
                     || root.visibility === Window.FullScreen;
        // Reported as-is. Whether a position is worth storing at all is Rust's
        // decision, not the scene's -- see `keep_position`.
        shell.save_window_state(root.width, root.height, root.x, root.y,
                                maximized, view.zoomFactor);
    }

    // Written on the way out, and again from a termination signal, because a
    // session ending never delivers a close event. A hidden window already
    // wrote its state when it was hidden, and reads as unmaximized now.
    function saveState() {
        if (stateSaved)
            return;
        stateSaved = true;
        if (root.visible)
            writeState();
    }

    function quit() {
        quitting = true;
        saveState();
        Qt.quit();
    }

    // Closing the window keeps instaCache running when `run_in_background`
    // says so, which is what makes the next launch instant: most of a cold
    // start is Chromium and Instagram, not anything this scene could skip.
    // Opening the window is answering the ring, whatever it was for.
    onActiveChanged: if (active) shell.stop_ringing()

    onClosing: function (close) {
        if (!root.quitting && shell.run_in_background) {
            close.accepted = false;
            root.hideToBackground();
            return;
        }
        root.saveState();
    }

    // Closed to the background. Hiding a Qt window does not hide the page
    // from Chromium: measured, the page still reports itself visible, its
    // timers run at full speed and its videos keep playing, sound included.
    // So the views are hidden explicitly, which is what makes Chromium
    // throttle the page, and whatever is playing is paused first.
    // Started with `--background`, the window is hidden from the first frame,
    // and the page has to be hidden with it or Chromium keeps drawing and
    // playing for a window nobody can see.
    property bool backgrounded: shell.start_hidden

    function hideToBackground() {
        root.writeState();
        root.hiddenMaximized = root.visibility === Window.Maximized;
        root.pauseMedia();
        // The snapshot is taken while the page is still on screen, and the
        // window hides once it exists, or at once if it cannot be taken.
        var hidden = false;
        function finish() {
            if (hidden)
                return;
            hidden = true;
            root.backgrounded = true;
            root.hide();
            shell.window_hidden(root.trayShown);
            root.beginResting();
            // An update waiting for the window to close takes over now.
            root.handleEvents();
        }
        if (!stage.grabToImage(function (result) {
                snapshot.source = result.url;
                finish();
            }))
            finish();
    }

    // Everything playing in Instagram and in ordinary pages. A call's own
    // streams are left alone: they are what makes it a call.
    function pauseMedia() {
        var script =
            "document.querySelectorAll('video, audio').forEach(function (m) {" +
            "  if (!m.srcObject && !m.paused) m.pause();" +
            "});";
        view.runJavaScript(script);
        for (var i = 0; i < pages.length; i++) {
            if (!pages[i].inCall)
                pages[i].runJavaScript(script);
        }
    }

    // A window opened again settles into place: the page, and the snapshot
    // over it, grow the last few percent to full size, the way the splash
    // fades in at a cold start.
    property real revealScale: 1
    NumberAnimation {
        id: reveal
        target: root; property: "revealScale"
        from: 0.965; to: 1; duration: 260; easing.type: Easing.OutCubic
    }

    function bringToFront() {
        if (!root.visible) {
            // The snapshot first, so the window opens on the page as it was
            // left rather than on nothing while Chromium draws again.
            snapshot.visible = snapshot.status === Image.Ready;
            reveal.restart();
            // Before the views are shown: a visible page must be Active.
            var reloading = root.rouse();
            if (root.hiddenMaximized)
                root.showMaximized();
            else
                root.show();
            root.backgrounded = false;
            root.offerUpdateOnOpen();
            // A discarded page reloads; its snapshot stays up until the
            // reload is done, then waits for frames like any other.
            if (reloading) {
                root.awaitingReload = true;
                reloadWatch.restart();
            } else {
                root.waitForFrames();
            }
        }
        root.raise();
        root.requestActivate();
    }

    // The snapshot goes once the live page has drawn: two animation frames
    // have run in it, which only happens once Chromium is compositing again.
    // A page that never answers still gets its snapshot removed.
    property int frameWaits: 0
    function waitForFrames() {
        root.frameWaits = 0;
        // Counting frames is not enough on its own: Chromium only draws when
        // something changed, and a page that has not changed since it was
        // hidden draws nothing — the window then keeps showing a frame whose
        // GPU image Chromium has already dropped, which is the blank page
        // users saw until they clicked. So a two-pixel square in the corner
        // is nudged for a few frames, which is a change, and each change is a
        // frame Qt can show.
        root.activeView().runJavaScript(
            "(function () {" +
            "  window.__instacacheFrames = 0;" +
            "  var mark = document.createElement('div');" +
            "  mark.style.cssText = 'position:fixed;left:0;top:0;width:2px;height:2px;" +
            "z-index:2147483647;pointer-events:none;background:rgba(127,127,127,0.01)';" +
            "  document.documentElement.appendChild(mark);" +
            "  var n = 0;" +
            "  (function step() {" +
            "    mark.style.opacity = (n % 2) ? '0.99' : '0.98';" +
            "    window.__instacacheFrames = n;" +
            "    if (++n < 8) requestAnimationFrame(step); else mark.remove();" +
            "  })();" +
            "})();");
        frameWatch.restart();
    }
    Timer {
        id: frameWatch
        interval: 50
        repeat: true
        onTriggered: {
            root.frameWaits++;
            if (root.frameWaits * interval >= 2000) {
                stop();
                root.dropSnapshot();
                return;
            }
            root.activeView().runJavaScript("window.__instacacheFrames || 0", function (frames) {
                if (frames >= 2 && frameWatch.running) {
                    frameWatch.stop();
                    root.dropSnapshot();
                }
            });
        }
    }
    function dropSnapshot() {
        snapshotFade.restart();
    }

    // --- Resting in the background ---------------------------------------

    // A closed window's pages are frozen once they have settled, and
    // discarded after `shell.discard_after_ms`, so a window left in the
    // background costs no processor time and, later, none of the memory its
    // renderer holds. How far each view goes is Rust's decision -- see
    // `background_lifecycle` in bridge.rs -- and never further than the
    // view's own `recommendedState`, which keeps sound and calls running.
    // Settling first lets the page finish what closing interrupted: a load,
    // a read receipt, and at startup its push subscription, without which
    // nothing would ever arrive.
    property int restDelay: 10000
    property bool awaitingReload: false

    function lifecycleName(state) {
        if (state === WebEngineView.LifecycleState.Discarded)
            return "Discarded";
        if (state === WebEngineView.LifecycleState.Frozen)
            return "Frozen";
        return "Active";
    }
    function lifecycleValue(name) {
        if (name === "Discarded")
            return WebEngineView.LifecycleState.Discarded;
        if (name === "Frozen")
            return WebEngineView.LifecycleState.Frozen;
        return WebEngineView.LifecycleState.Active;
    }
    function allViews() {
        return [view].concat(root.pages);
    }

    function beginResting() {
        restTimer.restart();
        if (shell.discard_after_ms > 0) {
            discardTimer.interval = shell.discard_after_ms;
            discardTimer.restart();
        }
    }

    // `forLong`: closed for long enough to discard. Not `long`, which Qt 6.4's
    // JavaScript still reserves: the scene would not load at all there.
    function rest(forLong) {
        if (!root.backgrounded)
            return;
        var views = root.allViews();
        for (var i = 0; i < views.length; i++) {
            var v = views[i];
            // Out of Discarded, the only way is back to Active.
            if (v.lifecycleState === WebEngineView.LifecycleState.Discarded)
                continue;
            var wanted = root.lifecycleValue(shell.lifecycle_when_hidden(
                forLong, root.lifecycleName(v.recommendedState),
                v !== view && v.inCall));
            if (v.lifecycleState !== wanted)
                v.lifecycleState = wanted;
        }
    }

    // Back to Active, for a window about to show. Returns whether Instagram
    // itself was discarded and is reloading.
    function rouse() {
        restTimer.stop();
        discardTimer.stop();
        var reloading = view.lifecycleState === WebEngineView.LifecycleState.Discarded;
        var views = root.allViews();
        for (var i = 0; i < views.length; i++) {
            if (views[i].lifecycleState !== WebEngineView.LifecycleState.Active)
                views[i].lifecycleState = WebEngineView.LifecycleState.Active;
        }
        return reloading;
    }

    Timer {
        id: restTimer
        interval: root.restDelay
        onTriggered: {
            // Still loading, or not started yet: give it the time it needs,
            // then rest.
            if (view.loading || view.loadProgress < 100)
                restart();
            else
                root.rest(false);
        }
    }
    Timer {
        id: discardTimer
        onTriggered: root.rest(true)
    }
    // A reload that never finishes still gets its snapshot removed.
    Timer {
        id: reloadWatch
        interval: 8000
        onTriggered: {
            if (root.awaitingReload) {
                root.awaitingReload = false;
                root.dropSnapshot();
            }
        }
    }

    function restartForUpdate() {
        updateTick.stop();
        // Back on the same Instagram page, in a window that shows.
        if (shell.restart_for_update(false, view.url.toString()))
            root.quit();
        else
            root.updateCountdown = 0;
    }

    // --- Updating, where it can be seen ----------------------------------

    // Opening the window while an update waits on disk says so, plainly,
    // and restarts into it after a short count -- the user sees that updates
    // happen rather than finding a new version by accident. "Later" keeps
    // the pill at the bottom instead. Never over a call, which a restart
    // would end.
    property int updateCountdown: 0
    readonly property int updateCountFrom: 5

    function offerUpdateOnOpen() {
        if (root.updateState !== "ready" || root.callPage !== null
            || root.updateDismissed === updateOffer.key)
            return;
        root.updateCountdown = root.updateCountFrom;
        updateTick.restart();
    }

    function postponeUpdate() {
        updateTick.stop();
        root.updateCountdown = 0;
        root.updateDismissed = updateOffer.key;
    }

    Timer {
        id: updateTick
        interval: 1000
        repeat: true
        onTriggered: {
            // A call that began meanwhile wins.
            if (root.callPage !== null) {
                root.postponeUpdate();
                return;
            }
            root.updateCountdown--;
            if (root.updateCountdown <= 0)
                root.restartForUpdate();
        }
    }

    // The first start after an update says what it became, once. Started
    // hidden, it waits for the window to open; Rust has already posted a
    // notification for the meantime.
    property bool updatedNoticePending: shell.updated_from !== ""
    function showUpdatedNotice() {
        if (!root.updatedNoticePending || !root.visible)
            return;
        root.updatedNoticePending = false;
        updatedNotice.shown = true;
        updatedNoticeHide.restart();
    }
    onVisibleChanged: if (visible) root.showUpdatedNotice()
    Timer { id: updatedNoticeHide; interval: 6000; onTriggered: updatedNotice.shown = false }

    // --- The tray icon ---------------------------------------------------

    // Created from a string rather than declared, so that a system without
    // the Qt.labs.platform module loses the tray icon and nothing else: a
    // failed import in the scene itself would stop the window appearing at
    // all, silently. See "A live process, no window and no error" in AGENTS.md.
    property var tray: null
    property bool trayShown: tray !== null && tray.available
    property int unread: 0
    readonly property string trayIconName: shell.icon_name
    readonly property string trayTitle: shell.window_title

    // Whether the user is really looking at the window: shown, not closed to
    // the background, and focused. What decides whether an alert is something
    // they can already see -- so it must not be true of a hidden window, which
    // some compositors still report as active.
    readonly property bool inFront: root.visible && root.active && !root.backgrounded

    function toggleFromTray() {
        if (root.inFront)
            root.hideToBackground();
        else
            root.bringToFront();
    }

    // What the tray icon says instaCache is doing. The point of the icon is
    // that a closed window is not a closed application: without a line saying
    // so, a user who closed the window and still hears a notification has no
    // way to tell whether it is running or haunted.
    function trayState() {
        var where = root.visible ? "Window open" : "Running in the background";
        return root.unread > 0 ? where + " — " + root.unread + " unread" : where;
    }

    // Read once when the menu is built and after every change, because the
    // autostart entry lives on disk and a desktop's own startup panel can
    // turn it off behind us. See `autostart.rs`.
    property bool startsWithSession: false
    function refreshStartsWithSession() {
        root.startsWithSession = shell.starts_with_session();
    }

    // Settings the tray may switch, by the names `bridge.rs` accepts.
    function toggleSetting(name) {
        shell.set_setting(name, !shell.setting(name));
        root.settingsRevision++;
        root.installMods();
    }
    // Bumped so the menu's bindings re-read `shell.setting`, which is a plain
    // method and notifies nothing on its own.
    property int settingsRevision: 0
    function settingIsOn(name) {
        return root.settingsRevision >= 0 && shell.setting(name);
    }

    Component.onCompleted: {
        root.showUpdatedNotice();
        // Started with `--background`, as at login: the page rests once it
        // has loaded and settled, like a window closed by hand.
        if (shell.start_hidden)
            root.beginResting();
        if (!shell.tray_icon)
            return;
        root.refreshStartsWithSession();
        try {
            root.tray = Qt.createQmlObject(
                "import QtQuick\n" +
                "import Qt.labs.platform\n" +
                "SystemTrayIcon {\n" +
                "    visible: available\n" +
                "    icon.name: root.trayIconName\n" +
                "    tooltip: root.trayTitle + ' — ' + root.trayState()\n" +
                "    onActivated: function (reason) {\n" +
                "        if (reason !== SystemTrayIcon.Context) root.toggleFromTray();\n" +
                "    }\n" +
                "    menu: Menu {\n" +
                // Not a title: Qt.labs.platform's Menu has no title item that
                // every tray implementation draws, and a disabled entry is
                // shown greyed by all of them.
                "        MenuItem { text: root.trayState(); enabled: false }\n" +
                "        MenuSeparator {}\n" +
                "        MenuItem { text: root.visible ? 'Hide window' : 'Open window'; onTriggered: root.toggleFromTray() }\n" +
                "        MenuItem { text: 'Restart to update'; visible: root.updateState === 'ready'; onTriggered: root.restartForUpdate() }\n" +
                "        MenuSeparator {}\n" +
                "        MenuItem {\n" +
                "            text: 'Start at login'; checkable: true\n" +
                "            checked: root.startsWithSession\n" +
                "            onTriggered: { shell.set_start_with_session(!root.startsWithSession); root.refreshStartsWithSession(); }\n" +
                "        }\n" +
                "        MenuItem {\n" +
                "            text: 'Notifications'; checkable: true\n" +
                "            checked: root.settingIsOn('notifications')\n" +
                "            onTriggered: root.toggleSetting('notifications')\n" +
                "        }\n" +
                "        MenuItem {\n" +
                "            text: 'Notification sounds'; checkable: true\n" +
                "            checked: root.settingIsOn('notification_sounds')\n" +
                "            onTriggered: root.toggleSetting('notification_sounds')\n" +
                "        }\n" +
                "        MenuItem {\n" +
                "            text: 'Ring for calls'; checkable: true\n" +
                "            checked: root.settingIsOn('ring_for_calls')\n" +
                "            onTriggered: root.toggleSetting('ring_for_calls')\n" +
                "        }\n" +
                "        Menu {\n" +
                "            title: 'Mods'\n" +
                "            MenuItem {\n" +
                "                text: 'Videos with sound'; checkable: true\n" +
                "                checked: root.settingIsOn('videos_with_sound')\n" +
                "                onTriggered: root.toggleSetting('videos_with_sound')\n" +
                "            }\n" +
                "            MenuItem {\n" +
                "                text: 'Instagram colours'; checkable: true\n" +
                "                checked: root.settingIsOn('instagram_colors')\n" +
                "                onTriggered: root.toggleSetting('instagram_colors')\n" +
                "            }\n" +
                "            MenuItem {\n" +
                "                text: 'Colours follow the post'; checkable: true\n" +
                "                checked: root.settingIsOn('ambient_colors')\n" +
                "                onTriggered: root.toggleSetting('ambient_colors')\n" +
                "            }\n" +
                "            MenuItem {\n" +
                "                text: 'Animations'; checkable: true\n" +
                "                checked: root.settingIsOn('page_animations')\n" +
                "                onTriggered: root.toggleSetting('page_animations')\n" +
                "            }\n" +
                "            MenuItem {\n" +
                "                text: 'Shortcuts (double-click, Space Space, L, R, M, C, S)'; checkable: true\n" +
                "                checked: root.settingIsOn('shortcuts')\n" +
                "                onTriggered: root.toggleSetting('shortcuts')\n" +
                "            }\n" +
                "            MenuItem {\n" +
                "                text: 'Hide \\'Also from Meta\\''; checkable: true\n" +
                "                checked: root.settingIsOn('hide_also_from_meta')\n" +
                "                onTriggered: root.toggleSetting('hide_also_from_meta')\n" +
                "            }\n" +
                "            MenuItem { text: 'Changes apply at the next load'; enabled: false }\n" +
                "        }\n" +
                "        MenuItem {\n" +
                "            text: 'Keep running when closed'; checkable: true\n" +
                "            checked: root.settingIsOn('run_in_background')\n" +
                "            onTriggered: root.toggleSetting('run_in_background')\n" +
                "        }\n" +
                "        MenuSeparator {}\n" +
                "        MenuItem { text: 'Quit'; onTriggered: root.quit() }\n" +
                "    }\n" +
                "}\n",
                root, "tray");
        } catch (error) {
            shell.log("no tray icon: " + error);
        }
    }

    function activeView() {
        return root.current !== null ? root.current : view;
    }

    // --- Pages -----------------------------------------------------------

    function showInstagram() {
        root.closeOrdinaryPages();
        root.current = null;
    }

    function closeOrdinaryPages() {
        var kept = [];
        for (var i = 0; i < pages.length; i++) {
            if (pages[i].inCall)
                kept.push(pages[i]);
            else
                root.discard(pages[i]);
        }
        pages = kept;
        if (root.current !== null && !root.current.inCall)
            root.current = null;
    }

    function closePage(page) {
        pages = pages.filter(function (p) { return p !== page; });
        if (root.current === page)
            root.current = null;
        root.discard(page);
    }

    // Destroyed later, never from inside one of the page's own handlers.
    function discard(page) {
        page.visible = false;
        Qt.callLater(function () { page.destroy(); });
    }

    // target="_blank", window.open and calls, from Instagram or from a page.
    function openRequest(request) {
        var target = request.requestedUrl.toString();
        if (root.leaveThroughShim(target))
            return;
        var blank = target === "" || target === "about:blank";
        if (!blank && shell.external_links_in_browser && !shell.is_internal(target)) {
            root.openExternally(target);
            return;
        }
        var page = pageComponent.createObject(stage);
        if (page === null) {
            shell.log("could not create a page; opening in place instead");
            view.url = request.requestedUrl;
            root.current = null;
            return;
        }
        // Adopted before anything is closed: the request may come from the
        // very page this replaces.
        request.openIn(page);
        root.closeOrdinaryPages();
        pages = pages.concat([page]);
        root.current = page;
        page.enter();
    }

    // Back means: out of a call without ending it, back through a page's own
    // history, out of a page, and only then back through Instagram's.
    function goBack() {
        var page = root.current;
        if (page === null) {
            if (view.canGoBack)
                view.goBack();
        } else if (page.inCall) {
            root.current = null;
        } else if (page.canGoBack) {
            page.goBack();
        } else {
            root.closePage(page);
        }
    }

    function goForward() {
        var target = root.activeView();
        if (target.canGoForward)
            target.goForward();
    }

    // Instagram's own navigation link when it is on the page, which moves
    // inside the app without reloading it; a full load when it is not.
    function goTo(path) {
        root.showInstagram();
        view.runJavaScript(
            "(function (path) {" +
            "  var link = document.querySelector('a[href=\"' + path + '\"]');" +
            "  if (link) { link.click(); return; }" +
            "  location.assign(path);" +
            "})(" + JSON.stringify(path) + ");");
    }

    // --- Shared by every view --------------------------------------------

    // `WebEngineView.Feature` by name, because Rust decides and an enum value
    // means nothing on that side. An enumerator this Qt does not define is
    // left out and falls through to "refused".
    function featureName(feature) {
        var names = ["MediaAudioCapture", "MediaVideoCapture",
                     "MediaAudioVideoCapture", "Geolocation",
                     "DesktopVideoCapture", "DesktopAudioVideoCapture",
                     "Notifications", "ClipboardReadWrite"];
        for (var i = 0; i < names.length; i++) {
            if (WebEngineView[names[i]] !== undefined
                && feature === WebEngineView[names[i]])
                return names[i];
        }
        return "";
    }

    // This is the pre-6.8 permission API on purpose. `permissionRequested`
    // replaced it, but only from Qt 6.8, and instaCache targets 6.4 so that
    // Debian 12 can run it. What is granted is decided by `grants_permission`.
    // Returns the name of what was granted, or "" when it was refused.
    function answerPermission(target, securityOrigin, feature) {
        var name = featureName(feature);
        var granted = shell.grants_permission(securityOrigin.toString(), name);
        target.grantFeaturePermission(securityOrigin, feature, granted);
        return granted ? name : "";
    }

    // Out to the browser, or whatever handles the scheme, once Rust agrees.
    // Through Qt rather than a process of our own, so on Qt 6.5 and later the
    // browser is handed an activation token and comes to the front.
    function openExternally(url) {
        var target = shell.external_target(url);
        if (target !== "")
            Qt.openUrlExternally(target);
    }

    // A link through Meta's link shim (`l.instagram.com/?u=…`) goes straight
    // to its destination in the browser: no detour through the shim in the
    // window, no click recorded, no empty page left to go back from.
    // Returns whether it was sent.
    function leaveThroughShim(target) {
        if (!shell.external_links_in_browser)
            return false;
        var destination = shell.unwrap_link(target);
        if (destination === "" || shell.is_internal(destination))
            return false;
        root.openExternally(destination);
        return true;
    }

    // Keeps a logged-in Instagram session away from arbitrary sites: anything
    // that is not Meta's goes to the system browser instead. Returns whether
    // the navigation was sent away.
    function routeNavigation(request) {
        var target = request.url.toString();
        if (root.leaveThroughShim(target)) {
            request.action = WebEngineNavigationRequest.IgnoreRequest;
            return true;
        }
        if (!shell.external_links_in_browser
            || shell.is_engine_scheme(target)
            || shell.is_internal(target))
            return false;
        request.action = WebEngineNavigationRequest.IgnoreRequest;
        root.openExternally(target);
        return true;
    }

    function handleLoad(target, info) {
        if (target === view && info.status !== WebEngineView.LoadStartedStatus)
            splash.done();
        var shown = target === root.activeView();
        if (info.status === WebEngineView.LoadStartedStatus) {
            if (shown)
                bar.begin();
        } else if (info.status === WebEngineView.LoadFailedStatus) {
            if (shown)
                bar.finish();
            // A navigation we cancelled ourselves is not a failure worth an
            // error page, and neither is one the user interrupted.
            if (info.errorCode !== 0 && !info.errorString.includes("Aborted")) {
                target.loadHtml(shell.error_page(info.url.toString(),
                                                 info.errorString),
                                info.url);
            }
        } else {
            if (shown)
                bar.finish();
            if (info.status === WebEngineView.LoadSucceededStatus) {
                root.applyUserStyle(target);
                root.applyUserScript(target);
            }
        }
    }

    // The built-in mods (`mods.rs`), as one user script on the profile, run
    // at document creation in the page's own world -- before Instagram's
    // code, which is what lets a mod change how the page starts rather than
    // fix it up afterwards. Every page of the window gets it from the
    // profile. Rebuilt when a mod is switched, which takes effect at the next
    // load. `WebEngine.script()` is how Qt 6 makes a WebEngineScript, which
    // cannot be declared (see "A live process, no window and no error").
    function installMods() {
        try {
            var scripts = session.userScripts;
            var old = scripts.find("instacache-mods");
            for (var i = 0; i < old.length; i++)
                scripts.remove(old[i]);
            var source = shell.mods_script();
            if (source === "")
                return;
            var script = WebEngine.script();
            script.name = "instacache-mods";
            script.sourceCode = source;
            script.injectionPoint = WebEngineScript.DocumentCreation;
            script.worldId = WebEngineScript.MainWorld;
            scripts.insert(script);
        } catch (error) {
            shell.log("mods not installed: " + error);
        }
    }

    // `~/.config/instacache/user.js`. Qt WebEngine implements no extension
    // API, so this is the nearest thing: the user's own script, run once per
    // real page load, in the page's own world so it can see what the page
    // sees. Wrapped so a mistake in it cannot take the page with it.
    function applyUserScript(target) {
        var js = shell.user_script;
        if (js === "")
            return;
        target.runJavaScript(
            "(function () { try {\n" + js + "\n} catch (e) {" +
            "  console.error('instacache: user.js failed:', e);" +
            "} })();");
    }

    // `~/.config/instacache/user.css`, injected rather than installed as a
    // user script: `WebEngineScript` is not instantiable from QML, and a
    // style element added to the head survives in-app navigation anyway,
    // which is all Instagram ever does after the first load.
    function applyUserStyle(target) {
        var css = shell.user_stylesheet;
        if (css === "")
            return;
        target.runJavaScript(
            "(function () {" +
            "  var id = 'instacache-user-style';" +
            "  var el = document.getElementById(id);" +
            "  if (!el) {" +
            "    el = document.createElement('style');" +
            "    el.id = id;" +
            "    document.head.appendChild(el);" +
            "  }" +
            "  el.textContent = " + JSON.stringify(css) + ";" +
            "})();");
    }

    // In a window that exists to be one application, the engine's menu is
    // browser chrome -- Back, Forward, View Source, Inspect -- and it covers
    // the page while Instagram's own right-click handling stops working.
    // Accepting the request without showing anything is how Qt says
    // "handled".
    function handleContextMenu(request) {
        if (!shell.context_menu)
            request.accepted = true;
    }

    function handleFullScreen(request) {
        root.visibility = request.toggleOn ? Window.FullScreen : Window.Windowed;
        request.accept();
    }

    // --- The session -----------------------------------------------------

    WebEngineProfile {
        id: session
        offTheRecord: false
        storageName: "instacache"
        // Everything that makes a session survive a restart: cookies,
        // localStorage, IndexedDB and service workers all live under this
        // path, and the HTTP cache under the other one.
        persistentStoragePath: shell.storage_path
        cachePath: shell.cache_path
        httpCacheType: WebEngineProfile.DiskHttpCache
        persistentCookiesPolicy: WebEngineProfile.ForcePersistentCookies
        httpUserAgent: shell.user_agent
        spellCheckEnabled: shell.spell_check_languages !== ""
        spellCheckLanguages: shell.spell_check_languages === ""
                             ? [] : shell.spell_check_languages.split(",")

        // Instagram delivers a message or a call through Web Push, from its
        // service worker, and Qt WebEngine refuses to register with any push
        // service unless it is asked to: without this,
        // `PushManager.subscribe()` fails with "push service not available",
        // so nothing is ever pushed and the notification below never happens.
        //
        // Assigned rather than declared because the property arrived in Qt
        // 6.5 and the baseline is 6.4 -- see the non-negotiables in AGENTS.md.
        // Declaring it would stop the whole scene loading on 6.4; assigning it
        // there throws, and an older Qt simply has no push service.
        Component.onCompleted: {
            root.installMods();
            if (!shell.push_service_enabled)
                return;
            try {
                session.isPushServiceEnabled = true;
            } catch (e) {
                shell.log("this Qt has no push service; "
                          + "notifications arrive only while the page is open");
            }
        }

        // Instagram posts web notifications; without a presenter Qt drops
        // them silently.
        onPresentNotification: function (notification) {
            if (!shell.notifications_enabled)
                return;
            var key = ++root.notificationSerial;
            // A message, or a call, which rings until somebody acts on it.
            if (!shell.notify_page(notification.title, notification.message,
                                   notification.tag, key, root.inFront))
                return;
            root.pageNotifications[key] = notification;
            // The page closing it itself takes it off the desktop and stops a
            // ring. Whether Instagram does so when a call is given up on is
            // not verified yet; a test page does it, and the path works.
            notification.closed.connect(function () {
                if (root.pageNotifications[key] === undefined)
                    return;
                delete root.pageNotifications[key];
                shell.page_notification_closed(key);
            });
            notification.show();
        }

        onDownloadRequested: function (download) {
            var target = shell.download_destination(download.downloadFileName);
            if (target === "") {
                shell.log("could not prepare a download directory");
                return;
            }
            var cut = target.lastIndexOf("/");
            download.downloadDirectory = target.substring(0, cut);
            download.downloadFileName = target.substring(cut + 1);
            download.accept();
        }

        onDownloadFinished: function (download) {
            shell.notify("Download finished", download.downloadFileName);
        }
    }

    // Every page notification still on the desktop, by the key Rust was
    // given for it, so a click is handed back to the notification that was
    // clicked -- which is what makes Instagram open the right thread. Rust
    // reports each key as clicked, closed or forgotten, and nothing stays here
    // longer than the desktop shows it.
    property var pageNotifications: ({})
    property int notificationSerial: 0

    function takeNotification(key) {
        var notification = root.pageNotifications[key];
        if (notification === undefined)
            return null;
        delete root.pageNotifications[key];
        return notification;
    }

    // --- What is shown ---------------------------------------------------

    Item {
        id: stage
        anchors.fill: parent
        scale: root.revealScale

        WebEngineView {
            id: view
            anchors.fill: parent
            // By an id no property shares. Written `profile: profile`, a view
            // created from a component got a private profile with no session,
            // and adopting a request into it killed the renderer.
            profile: session
            url: shell.home_url
            zoomFactor: root.geometry.zoom
            backgroundColor: root.color
            // Hidden under a page, so Chromium treats it as a background tab.
            visible: root.current === null && !root.backgrounded

            settings.playbackRequiresUserGesture: !shell.autoplay_without_gesture
            settings.fullScreenSupportEnabled: true
            settings.localStorageEnabled: true
            settings.javascriptCanAccessClipboard: true
            settings.javascriptCanPaste: true
            settings.screenCaptureEnabled: false
            settings.showScrollBars: false

            // The unread badge reads Instagram's title, whatever is shown --
            // and so does the alert for a message that arrived while the page
            // was already open, which is never pushed because the page is
            // connected and only changes its title.
            onTitleChanged: root.unread = shell.title_changed(title, root.inFront)

            onNavigationRequested: function (request) {
                root.routeNavigation(request);
            }
            onNewWindowRequested: function (request) {
                root.openRequest(request);
            }
            onLoadingChanged: function (info) {
                // The count Instagram is about to announce is what was
                // already waiting, so it must not be heard as an arrival.
                if (info.status === WebEngineView.LoadStartedStatus)
                    shell.main_page_loading();
                root.handleLoad(view, info);
                if (root.awaitingReload
                    && info.status !== WebEngineView.LoadStartedStatus) {
                    root.awaitingReload = false;
                    reloadWatch.stop();
                    root.waitForFrames();
                }
            }

            // Instagram is a single-page application: opening a profile or the
            // inbox changes the URL without loading anything, so a bar driven
            // by load events alone would light up once at startup and never
            // again.
            onUrlChanged: if (!view.loading && root.current === null) bar.sweep()

            onRenderProcessTerminated: function (status, exitCode) {
                if (status === WebEngineView.NormalTerminationStatus)
                    return;
                var where = view.url.toString();
                if (shell.should_reload_after_crash()) {
                    shell.log("rendering process died (" + status + "); reloading");
                    view.reload();
                } else {
                    shell.log("rendering process died repeatedly; giving up");
                    view.loadHtml(shell.crash_page(where, "stopped unexpectedly"),
                                  view.url);
                }
            }

            // The microphone and camera, notifications per the user's setting,
            // the clipboard for pasting an image into a DM, nothing else. The
            // microphone here is a voice message, not a call: calls open a
            // page of their own. See `permission_granted` in bridge.rs.
            onFeaturePermissionRequested: function (securityOrigin, feature) {
                // The microphone here means a call answered in place, or a
                // voice message: either way nothing should still be ringing.
                if (root.answerPermission(view, securityOrigin, feature).indexOf("Media") === 0)
                    shell.stop_ringing();
            }

            onFullScreenRequested: function (request) {
                root.handleFullScreen(request);
            }
            onContextMenuRequested: function (request) {
                root.handleContextMenu(request);
            }
            onJavaScriptDialogRequested: function (request) {
                // The page must never be able to block the UI thread with a modal.
                request.dialogAccept();
            }
        }
    }

    Component {
        id: pageComponent

        WebEngineView {
            id: page
            anchors.fill: parent
            profile: session
            zoomFactor: view.zoomFactor
            backgroundColor: root.color
            visible: root.current === page && !root.backgrounded

            // A page rises into place over Instagram rather than appearing
            // in one frame. Only when it is opened or brought back from the
            // pill: a window shown again comes back as it was, without it.
            transform: Translate { id: shift }
            function enter() { entrance.restart(); }
            ParallelAnimation {
                id: entrance
                NumberAnimation {
                    target: page; property: "opacity"
                    from: 0; to: 1; duration: 170; easing.type: Easing.OutCubic
                }
                NumberAnimation {
                    target: shift; property: "y"
                    from: 14; to: 0; duration: 220; easing.type: Easing.OutCubic
                }
            }

            // Set once the page is granted the microphone or camera, which
            // for a page of its own means a call. A call is hidden rather
            // than closed when the user goes back.
            property bool inCall: false

            settings.playbackRequiresUserGesture: !shell.autoplay_without_gesture
            settings.fullScreenSupportEnabled: true
            settings.localStorageEnabled: true
            settings.javascriptCanAccessClipboard: true
            settings.javascriptCanPaste: true
            settings.screenCaptureEnabled: false
            settings.showScrollBars: false

            onNavigationRequested: function (request) {
                // A page whose very first address leaves for the browser
                // would otherwise stay behind, empty.
                var empty = page.url.toString() === ""
                         || page.url.toString() === "about:blank";
                if (root.routeNavigation(request) && empty && request.isMainFrame)
                    root.closePage(page);
            }
            onNewWindowRequested: function (request) {
                root.openRequest(request);
            }
            onLoadingChanged: function (info) {
                root.handleLoad(page, info);
            }
            onFeaturePermissionRequested: function (securityOrigin, feature) {
                if (root.answerPermission(page, securityOrigin, feature).indexOf("Media") === 0) {
                    page.inCall = true;
                    shell.stop_ringing();
                }
            }
            // A call ending closes its own window, which here is this page.
            onWindowCloseRequested: root.closePage(page)
            onRenderProcessTerminated: function (status, exitCode) {
                if (status !== WebEngineView.NormalTerminationStatus)
                    shell.log("a page's rendering process died (" + status + "); closing it");
                root.closePage(page);
            }
            onFullScreenRequested: function (request) {
                root.handleFullScreen(request);
            }
            onContextMenuRequested: function (request) {
                root.handleContextMenu(request);
            }
            onJavaScriptDialogRequested: function (request) {
                request.dialogAccept();
            }
        }
    }

    // The mouse's back and forward buttons. Qt WebEngine already follows them
    // through a page's history, but not out of a page and back to Instagram,
    // so they are taken here. Only those two buttons: a click, a scroll or a
    // hover still reaches the page underneath.
    Item {
        anchors.fill: parent
        TapHandler {
            acceptedButtons: Qt.BackButton | Qt.ForwardButton
            gesturePolicy: TapHandler.WithinBounds
            onTapped: function (eventPoint, button) {
                if (button === Qt.BackButton)
                    root.goBack();
                else
                    root.goForward();
            }
        }
    }

    // The page as it was when the window closed, shown while it draws again.
    Image {
        id: snapshot
        anchors.fill: stage
        scale: root.revealScale
        visible: false
        cache: false
        smooth: false
        onVisibleChanged: if (visible) opacity = 1
        NumberAnimation on opacity {
            id: snapshotFade
            running: false
            to: 0
            duration: 120
            onFinished: {
                snapshot.visible = false;
                snapshot.source = "";
            }
        }
    }

    // The splash: instaCache's mark over the window from the first frame
    // until the first load of the main page has finished, so a cold start
    // shows something other than black while Chromium starts and fetches.
    // Instagram's own boot screen takes over from there. Drawn with Canvas
    // rather than loaded from the SVG: QtQuick has Canvas everywhere, while
    // an SVG image needs a Qt module a distribution may not have installed.
    // Not shown when started hidden, where nobody is watching the load.
    Rectangle {
        id: splash
        anchors.fill: parent
        color: root.color
        visible: opacity > 0
        opacity: shell.start_hidden ? 0 : 1
        Behavior on opacity { NumberAnimation { duration: 220 } }

        function done() {
            splash.opacity = 0;
            splashCap.stop();
        }

        // Instagram's colours as light from the corners, the way its own
        // gradient runs: purple above, orange and pink below.
        Canvas {
            anchors.fill: parent
            onWidthChanged: requestPaint()
            onHeightChanged: requestPaint()
            onPaint: {
                var c = getContext("2d");
                c.reset();
                function glow(x, y, radius, colour) {
                    var g = c.createRadialGradient(x, y, 0, x, y, radius);
                    g.addColorStop(0, colour);
                    g.addColorStop(1, "rgba(0, 0, 0, 0)");
                    c.fillStyle = g;
                    c.fillRect(0, 0, width, height);
                }
                var r = Math.max(width, height);
                glow(width, 0, r * 0.75, "rgba(150, 47, 191, 0.30)");
                glow(0, height, r * 0.65, "rgba(250, 126, 30, 0.18)");
                glow(width * 0.8, height, r * 0.5, "rgba(214, 41, 118, 0.18)");
            }
        }

        // A load that never finishes still gets the splash out of the way.
        Timer {
            id: splashCap
            interval: 15000
            running: splash.opacity > 0
            onTriggered: splash.done()
        }

        Canvas {
            id: mark
            width: 96
            height: 96
            anchors.centerIn: parent
            // The same drawing as assets/instacache.svg, on its 256 grid.
            onPaint: {
                var c = getContext("2d");
                c.reset();
                c.scale(width / 256, height / 256);
                function body() {
                    var x = 8, y = 8, w = 240, h = 240, r = 62;
                    c.beginPath();
                    c.moveTo(x + r, y);
                    c.arcTo(x + w, y, x + w, y + h, r);
                    c.arcTo(x + w, y + h, x, y + h, r);
                    c.arcTo(x, y + h, x, y, r);
                    c.arcTo(x, y, x + w, y, r);
                    c.closePath();
                }
                function bolt() {
                    c.beginPath();
                    c.moveTo(150, 44);
                    c.lineTo(86, 140);
                    c.lineTo(124, 140);
                    c.lineTo(106, 212);
                    c.lineTo(174, 110);
                    c.lineTo(134, 110);
                    c.closePath();
                }
                var brand = c.createRadialGradient(61, 258, 0, 61, 258, 312);
                brand.addColorStop(0.00, "#FFD25A");
                brand.addColorStop(0.20, "#FF9A3C");
                brand.addColorStop(0.42, "#F4503E");
                brand.addColorStop(0.62, "#DD2A7B");
                brand.addColorStop(0.82, "#9B30B8");
                brand.addColorStop(1.00, "#5145D8");
                body();
                c.fillStyle = brand;
                c.fill();

                // The lens, then the bolt's outline in the body's own
                // gradient over it: that is the cut in the ring.
                c.lineJoin = "round";
                c.strokeStyle = "#ffffff";
                c.lineWidth = 20;
                c.beginPath();
                c.arc(128, 130, 62, 0, 2 * Math.PI);
                c.stroke();
                bolt();
                c.fillStyle = brand;
                c.strokeStyle = brand;
                c.lineWidth = 26;
                c.fill();
                c.stroke();

                bolt();
                c.fillStyle = "#ffffff";
                c.strokeStyle = "#ffffff";
                c.lineWidth = 6;
                c.fill();
                c.stroke();

                var sheen = c.createLinearGradient(0, 8, 0, 248);
                sheen.addColorStop(0.00, "rgba(255, 255, 255, 0.22)");
                sheen.addColorStop(0.48, "rgba(255, 255, 255, 0)");
                sheen.addColorStop(1.00, "rgba(0, 0, 0, 0.12)");
                body();
                c.fillStyle = sheen;
                c.fill();
            }

            // A slow breath, so the window reads as busy rather than stuck.
            SequentialAnimation on opacity {
                running: splash.visible
                loops: Animation.Infinite
                NumberAnimation { to: 0.55; duration: 900; easing.type: Easing.InOutSine }
                NumberAnimation { to: 1.0; duration: 900; easing.type: Easing.InOutSine }
            }
        }
    }

    // The loading bar: a 3px gradient line.
    Rectangle {
        id: bar
        anchors.top: parent.top
        anchors.left: parent.left
        height: 3
        visible: shell.show_loading_indicator && opacity > 0
        opacity: 0
        width: parent.width * fraction

        property real fraction: 0

        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0;  color: "#FFDD55" }
            GradientStop { position: 0.55; color: "#E1306C" }
            GradientStop { position: 1.0;  color: "#5B4BE0" }
        }

        Behavior on fraction { NumberAnimation { duration: 180 } }
        Behavior on opacity  { NumberAnimation { duration: 150 } }

        function begin() {
            fraction = 0.08;
            opacity = 1;
            follow.start();
        }

        // An in-app navigation loads nothing, so there is no progress to
        // follow: sweep once and get out of the way.
        function sweep() {
            if (opacity > 0)
                return;
            opacity = 1;
            fraction = 0.75;
            sweepDone.restart();
        }

        function finish() {
            follow.stop();
            fraction = 1;
            hide.restart();
        }

        Timer {
            id: follow
            interval: 120
            repeat: true
            onTriggered: bar.fraction = Math.max(bar.fraction,
                                                 root.activeView().loadProgress / 100)
        }
        Timer { id: sweepDone; interval: 450; onTriggered: bar.finish() }
        Timer {
            id: hide
            interval: 220
            onTriggered: { bar.opacity = 0; bar.fraction = 0; }
        }
    }

    // Out of a page and back to Instagram. Shown only over a page, never over
    // Instagram itself, which has its own navigation.
    Pill {
        id: backButton
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.margins: 12
        shown: root.current !== null && root.visibility !== Window.FullScreen
        label: root.current !== null && root.current.inCall ? "‹  Instagram" : "‹  Back"
        onClicked: root.goBack()
    }

    // A call keeps running while the user is elsewhere; this is the way back.
    Row {
        anchors.top: parent.top
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.topMargin: 12
        spacing: 6
        visible: opacity > 0
        opacity: root.callPage !== null && root.current !== root.callPage
                 && root.visibility !== Window.FullScreen ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 150 } }

        Pill {
            shown: true
            accent: true
            label: "●  Call in progress — return"
            onClicked: {
                root.current = root.callPage;
                root.callPage.enter();
            }
        }
        Pill {
            shown: true
            label: "End"
            onClicked: if (root.callPage !== null) root.closePage(root.callPage)
        }
    }

    // An update: ready to take over, or waiting for the password a
    // system-wide install needs. At the bottom, where Instagram keeps nothing
    // that a small pill would hide for long, and never over a call.
    Row {
        id: updateOffer
        anchors.bottom: parent.bottom
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottomMargin: 16
        spacing: 6
        property string key: root.updateState + " " + root.updateVersion
        visible: opacity > 0
        opacity: (root.updateState === "ready" || root.updateState === "installable"
                  || root.updateState === "installing")
                 && root.updateDismissed !== key
                 && root.updateCountdown === 0
                 && root.callPage === null
                 && root.visibility !== Window.FullScreen ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 150 } }

        Pill {
            shown: true
            accent: root.updateState !== "installing"
            label: root.updateState === "ready"
                   ? "Update ready — Restart"
                   : root.updateState === "installable"
                     ? "instaCache " + root.updateVersion + " available — Install"
                     : "Installing instaCache " + root.updateVersion + "…"
            onClicked: {
                if (root.updateState === "ready") {
                    root.restartForUpdate();
                } else if (root.updateState === "installable") {
                    shell.install_update();
                }
            }
        }
        Pill {
            shown: root.updateState !== "installing"
            label: "Later"
            onClicked: root.updateDismissed = updateOffer.key
        }
    }

    Pill {
        id: updatedNotice
        anchors.bottom: parent.bottom
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottomMargin: 16
        accent: true
        label: "✓  Updated to instaCache " + shell.app_version
        onClicked: shown = false
    }

    // The update, announced: a card over a dimmed page, the version, the
    // count, and the two ways out of it.
    Rectangle {
        id: updateCard
        anchors.fill: parent
        color: "#99000000"
        visible: opacity > 0
        opacity: root.updateCountdown > 0 ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 180 } }
        // Keeps clicks on the dimmed page from reaching it.
        MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons; hoverEnabled: true }

        Rectangle {
            anchors.centerIn: parent
            width: Math.min(parent.width - 48, 380)
            height: cardColumn.implicitHeight + 48
            radius: 18
            color: "#1c1c1e"
            border.color: "#3a3a3c"

            Column {
                id: cardColumn
                anchors.centerIn: parent
                width: parent.width - 48
                spacing: 14

                Text {
                    width: parent.width
                    text: "Updating instaCache"
                    color: "#ffffff"
                    font.pixelSize: 20
                    font.weight: Font.Bold
                    horizontalAlignment: Text.AlignHCenter
                }
                Text {
                    width: parent.width
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                    color: "#c7c7cc"
                    font.pixelSize: 14
                    text: (root.updateVersion !== ""
                           ? "Version " + root.updateVersion + " is installed."
                           : "A new version is installed.")
                          + "\nRestarting in " + root.updateCountdown + "…"
                }
                // The count, drawn: it empties as the restart comes.
                Rectangle {
                    width: parent.width
                    height: 4
                    radius: 2
                    color: "#3a3a3c"
                    Rectangle {
                        height: parent.height
                        radius: 2
                        color: "#1f8f4e"
                        width: parent.width * root.updateCountdown / root.updateCountFrom
                        Behavior on width { NumberAnimation { duration: 950 } }
                    }
                }
                Row {
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 8
                    Pill { shown: true; accent: true; label: "Restart now"; onClicked: root.restartForUpdate() }
                    Pill { shown: true; label: "Not now"; onClicked: root.postponeUpdate() }
                }
            }
        }
    }

    component Pill: Rectangle {
        id: pill
        property string label
        property bool shown: false
        property bool accent: false
        signal clicked()

        height: 34
        width: text.implicitWidth + 28
        radius: height / 2
        color: accent ? "#1f8f4e" : "#000000"
        opacity: shown ? (hover.hovered ? 0.95 : 0.78) : 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 150 } }

        Text {
            id: text
            anchors.centerIn: parent
            text: pill.label
            color: "#ffffff"
            font.pixelSize: 14
            font.weight: Font.DemiBold
        }
        HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
        TapHandler { onTapped: pill.clicked() }
    }

    // Anything that happened off the UI thread: a second launch asking for
    // this window, a notification being clicked, an update check finishing, a
    // termination signal. Rust says when there is something, with `wake`;
    // nothing runs while nothing happens. It used to be a timer firing four
    // times a second, and twelve and a half with the window closed.
    function handleEvents() {
        var events = JSON.parse(shell.poll());

        // A termination signal only sets a flag -- see main.rs for why --
        // so this is where the session actually ends, through the same
        // path a window close takes. Without it a desktop session ending
        // loses the window geometry, which is the whole reason the signal
        // is caught at all.
        if (events.quit) {
            root.quit();
            return;
        }

        if (events.present)
            root.bringToFront();
        var i;
        for (i = 0; i < events.clicked.length; i++) {
            // Kept, not taken: a call's notification stays on the desktop
            // after a click, and the page withdrawing it later still has to
            // reach it. The server's own close, which usually follows a
            // click, is what lets go of it.
            var clicked = root.pageNotifications[events.clicked[i]];
            if (clicked !== undefined) {
                // Tells the page which notification was clicked.
                root.showInstagram();
                clicked.click();
            }
        }
        // Dismissed on the desktop: the page is told, the way a browser
        // tells it.
        for (i = 0; i < events.closed.length; i++) {
            var closed = root.takeNotification(events.closed[i]);
            if (closed !== null)
                closed.close();
        }
        for (i = 0; i < events.forgotten.length; i++)
            root.takeNotification(events.forgotten[i]);
        for (i = 0; i < events.urls.length; i++) {
            root.showInstagram();
            view.url = events.urls[i];
        }

        root.updateState = events.update;
        root.updateVersion = events.update_version;
        // The way a browser updates itself: the new version is already on
        // disk, so while nobody is looking it simply takes over, staying
        // hidden. A visible window gets the pill instead, and closing it
        // brings this path round again.
        if (events.update === "ready"
            && shell.may_restart_quietly(root.visible, root.callPage !== null)
            && shell.restart_for_update(true, view.url.toString())) {
            root.quit();
        }
    }
    Connections {
        target: shell
        function onWake() { root.handleEvents(); }
    }
    // What nobody announces: the hourly question whether an update check is
    // due, and a binary replaced on disk by hand. Both are cheap and neither
    // is urgent, so twenty seconds is plenty.
    Timer {
        interval: 20000
        running: true
        repeat: true
        onTriggered: root.handleEvents()
    }

    // Bound on the window, so the page keeps receiving every other key it
    // needs for its own shortcuts (`/` to search, arrows in Stories).
    property real zoomStep: 1.1
    function zoomBy(factor) {
        view.zoomFactor = Math.min(5.0, Math.max(0.25, view.zoomFactor * factor));
    }

    Shortcut { sequences: ["Ctrl+R", "F5"];             onActivated: root.activeView().reload() }
    Shortcut { sequences: ["Ctrl+Shift+R", "Shift+F5"]; onActivated: root.activeView().triggerWebAction(WebEngineView.ReloadAndBypassCache) }
    Shortcut { sequences: ["Alt+Left", "Back"];         onActivated: root.goBack() }
    Shortcut { sequences: ["Alt+Right", "Forward"];     onActivated: root.goForward() }
    Shortcut { sequences: ["Ctrl+H", "Alt+Home"];       onActivated: { root.showInstagram(); view.url = shell.home_url; } }
    Shortcut { sequences: ["Ctrl++", "Ctrl+=" ];        onActivated: root.zoomBy(root.zoomStep) }
    Shortcut { sequence: "Ctrl+-";                      onActivated: root.zoomBy(1 / root.zoomStep) }
    Shortcut { sequence: "Ctrl+0";                      onActivated: view.zoomFactor = 1.0 }
    Shortcut { sequence: "Ctrl+Q";                      onActivated: root.quit() }
    // Closes the page on top, and the window when there is none.
    Shortcut {
        sequence: "Ctrl+W"
        onActivated: {
            if (root.current !== null && !root.current.inCall)
                root.closePage(root.current);
            else
                root.close();
        }
    }
    Shortcut { sequence: "Ctrl+1"; enabled: shell.instagram_shortcuts; onActivated: root.goTo("/") }
    Shortcut { sequence: "Ctrl+2"; enabled: shell.instagram_shortcuts; onActivated: root.goTo("/explore/") }
    Shortcut { sequence: "Ctrl+3"; enabled: shell.instagram_shortcuts; onActivated: root.goTo("/reels/") }
    Shortcut { sequence: "Ctrl+4"; enabled: shell.instagram_shortcuts; onActivated: root.goTo("/direct/inbox/") }
    Shortcut {
        sequence: "F11"
        onActivated: root.visibility = root.visibility === Window.FullScreen
                                     ? Window.Windowed : Window.FullScreen
    }
    // Enabled only in fullscreen: a shortcut takes its key whether or not it
    // does anything with it, and Instagram closes its own dialogs on Escape.
    Shortcut {
        sequence: "Escape"
        enabled: root.visibility === Window.FullScreen
        onActivated: root.visibility = Window.Windowed
    }
    Shortcut {
        sequences: ["Ctrl+Shift+I", "F12"]
        enabled: shell.developer_tools
        onActivated: root.activeView().triggerWebAction(WebEngineView.InspectElement)
    }
}
