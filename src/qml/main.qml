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
    color: "#000000"

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

    function bringToFront() {
        if (!root.visible) {
            // The snapshot first, so the window opens on the page as it was
            // left rather than on nothing while Chromium draws again.
            snapshot.visible = snapshot.status === Image.Ready;
            if (root.hiddenMaximized)
                root.showMaximized();
            else
                root.show();
            root.backgrounded = false;
            root.waitForFrames();
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

    function restartForUpdate() {
        // Back on the same Instagram page, in a window that shows.
        if (shell.restart_for_update(false, view.url.toString()))
            root.quit();
    }

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
    }
    // Bumped so the menu's bindings re-read `shell.setting`, which is a plain
    // method and notifies nothing on its own.
    property int settingsRevision: 0
    function settingIsOn(name) {
        return root.settingsRevision >= 0 && shell.setting(name);
    }

    Component.onCompleted: {
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
        var blank = target === "" || target === "about:blank";
        if (!blank && shell.external_links_in_browser && !shell.is_internal(target)) {
            shell.open_externally(target);
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

    // Keeps a logged-in Instagram session away from arbitrary sites: anything
    // that is not Meta's goes to the system browser instead. Returns whether
    // the navigation was sent away.
    function routeNavigation(request) {
        var target = request.url.toString();
        if (!shell.external_links_in_browser
            || shell.is_engine_scheme(target)
            || shell.is_internal(target))
            return false;
        request.action = WebEngineNavigationRequest.IgnoreRequest;
        shell.open_externally(target);
        return true;
    }

    function handleLoad(target, info) {
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
            lastNotification = notification;
            // A message, or a call, which rings until somebody acts on it.
            shell.notify_page(notification.title, notification.message,
                              notification.tag, root.inFront);
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

    // Held so a click on a desktop notification can be handed back to the
    // page, which is what makes Instagram open the right thread.
    property var lastNotification: null

    // --- What is shown ---------------------------------------------------

    Item {
        id: stage
        anchors.fill: parent

        WebEngineView {
            id: view
            anchors.fill: parent
            // By an id no property shares. Written `profile: profile`, a view
            // created from a component got a private profile with no session,
            // and adopting a request into it killed the renderer.
            profile: session
            url: shell.home_url
            zoomFactor: root.geometry.zoom
            backgroundColor: "#000000"
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
            backgroundColor: "#000000"
            visible: root.current === page && !root.backgrounded

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
            onClicked: root.current = root.callPage
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
    // this window, a notification being clicked, an update check finishing.
    Timer {
        // Faster while hidden: that is when a launch from the menu is waiting
        // on this timer to show the window, and nothing else is going on.
        interval: root.visible ? 250 : 80
        running: true
        repeat: true
        onTriggered: {
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

            if (events.present) {
                root.bringToFront();
                if (root.lastNotification) {
                    // Tells the page the notification was clicked.
                    root.showInstagram();
                    root.lastNotification.click();
                    root.lastNotification = null;
                }
            }
            for (var i = 0; i < events.urls.length; i++) {
                root.showInstagram();
                view.url = events.urls[i];
            }

            root.updateState = events.update;
            root.updateVersion = events.update_version;
            // The way a browser updates itself: the new version is already on
            // disk, so while nobody is looking it simply takes over, staying
            // hidden. A visible window gets the pill instead, and closing it
            // brings this path round on the next tick.
            if (events.update === "ready"
                && shell.may_restart_quietly(root.visible, root.callPage !== null)
                && shell.restart_for_update(true, view.url.toString())) {
                root.quit();
            }
        }
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
