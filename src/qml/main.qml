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
    property bool hiddenMaximized: false

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
    visibility: geometry.maximized ? Window.Maximized : Window.Windowed
    visible: true
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
    onClosing: function (close) {
        if (!root.quitting && shell.run_in_background) {
            close.accepted = false;
            root.writeState();
            root.hiddenMaximized = root.visibility === Window.Maximized;
            root.hide();
            shell.window_hidden();
            return;
        }
        root.saveState();
    }

    function bringToFront() {
        if (!root.visible) {
            if (root.hiddenMaximized)
                root.showMaximized();
            else
                root.show();
        }
        root.raise();
        root.requestActivate();
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

        // Instagram posts web notifications; without a presenter Qt drops
        // them silently.
        onPresentNotification: function (notification) {
            if (!shell.notifications_enabled)
                return;
            lastNotification = notification;
            shell.notify(notification.title, notification.message);
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
            visible: root.current === null

            settings.playbackRequiresUserGesture: !shell.autoplay_without_gesture
            settings.fullScreenSupportEnabled: true
            settings.localStorageEnabled: true
            settings.javascriptCanAccessClipboard: true
            settings.javascriptCanPaste: true
            settings.screenCaptureEnabled: false
            settings.showScrollBars: false

            // The unread badge reads Instagram's title, whatever is shown.
            onTitleChanged: shell.title_changed(title)

            onNavigationRequested: function (request) {
                root.routeNavigation(request);
            }
            onNewWindowRequested: function (request) {
                root.openRequest(request);
            }
            onLoadingChanged: function (info) {
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
                root.answerPermission(view, securityOrigin, feature);
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
            visible: root.current === page

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
                if (root.answerPermission(page, securityOrigin, feature).indexOf("Media") === 0)
                    page.inCall = true;
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
        interval: 250
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
