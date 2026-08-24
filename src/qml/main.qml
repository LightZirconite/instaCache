// The whole visible application.
//
// This scene owns widgets and nothing else. Every decision — whether a URL
// stays in the window, where a download goes, what the offline page says,
// whether a dead renderer should be reloaded again — is asked of `shell`,
// which is Rust. Keep it that way: policy in QML cannot be unit tested.

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

    title: shell.window_title
    width: geometry.width
    height: geometry.height
    x: geometry.x !== null && geometry.x !== undefined ? geometry.x : x
    y: geometry.y !== null && geometry.y !== undefined ? geometry.y : y
    visibility: geometry.maximized ? Window.Maximized : Window.Windowed
    visible: true
    color: "#000000"

    // Written on the way out, and again from a termination signal, because a
    // session ending never delivers a close event.
    function saveState() {
        if (stateSaved)
            return;
        stateSaved = true;
        var maximized = root.visibility === Window.Maximized
                     || root.visibility === Window.FullScreen;
        // Reported as-is. Whether a position is worth storing at all is Rust's
        // decision, not the scene's -- see `keep_position`.
        shell.save_window_state(root.width, root.height, root.x, root.y,
                                maximized, view.zoomFactor);
    }

    onClosing: saveState()

    WebEngineProfile {
        id: profile
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

    WebEngineView {
        id: view
        anchors.fill: parent
        profile: profile
        url: shell.home_url
        zoomFactor: root.geometry.zoom
        backgroundColor: "#000000"

        settings.playbackRequiresUserGesture: !shell.autoplay_without_gesture
        settings.fullScreenSupportEnabled: true
        settings.localStorageEnabled: true
        settings.javascriptCanAccessClipboard: true
        settings.javascriptCanPaste: true
        settings.screenCaptureEnabled: false
        settings.showScrollBars: false

        onTitleChanged: root.title = title !== "" ? title : shell.window_title

        // Keeps a logged-in Instagram session away from arbitrary sites:
        // anything that is not Meta's goes to the system browser instead.
        onNavigationRequested: function (request) {
            var target = request.url.toString();
            if (!shell.external_links_in_browser
                || shell.is_engine_scheme(target)
                || shell.is_internal(target))
                return;
            request.action = WebEngineNavigationRequest.IgnoreRequest;
            shell.open_externally(target);
        }

        // target="_blank" and window.open. A single-window app has nowhere to
        // put a second view, so an internal one replaces the current page.
        onNewWindowRequested: function (request) {
            var target = request.requestedUrl.toString();
            if (!shell.external_links_in_browser || shell.is_internal(target))
                view.url = request.requestedUrl;
            else
                shell.open_externally(target);
        }

        onLoadingChanged: function (info) {
            if (info.status === WebEngineView.LoadStartedStatus) {
                bar.begin();
            } else if (info.status === WebEngineView.LoadFailedStatus) {
                bar.finish();
                // A navigation we cancelled ourselves is not a failure worth
                // an error page, and neither is one the user interrupted.
                if (info.errorCode !== 0 && !info.errorString.includes("Aborted")) {
                    view.loadHtml(shell.error_page(info.url.toString(),
                                                   info.errorString),
                                  info.url);
                }
            } else {
                bar.finish();
                if (info.status === WebEngineView.LoadSucceededStatus)
                    view.applyUserStyle();
            }
        }

        // `~/.config/instacache/user.css`, injected rather than installed as a
        // user script: `WebEngineScript` is not instantiable from QML, and a
        // style element added to the head survives in-app navigation anyway,
        // which is all Instagram ever does after the first load.
        function applyUserStyle() {
            var css = shell.user_stylesheet;
            if (css === "")
                return;
            view.runJavaScript(
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

        // Instagram is a single-page application: opening a profile or the
        // inbox changes the URL without loading anything, so a bar driven by
        // load events alone would light up once at startup and never again.
        onUrlChanged: if (!view.loading) bar.sweep()

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

        // Instagram needs none of geolocation, camera or microphone.
        // Notifications follow the user's setting; clipboard reading backs
        // "paste an image into a DM".
        //
        // This is the pre-6.8 permission API on purpose. `permissionRequested`
        // replaced it, but only from Qt 6.8, and instaCache targets 6.4 so
        // that Debian 12 can run it -- the same reasoning that pinned the
        // WebKitGTK build to 4.1 rather than 6.0. A `Feature` value a older Qt
        // does not define simply fails to match, and an unmatched feature is
        // denied, so the fallback is the safe one.
        onFeaturePermissionRequested: function (securityOrigin, feature) {
            var granted = false;
            if (feature === WebEngineView.Notifications)
                granted = shell.notifications_enabled;
            else if (feature === WebEngineView.ClipboardReadWrite)
                granted = true;
            view.grantFeaturePermission(securityOrigin, feature, granted);
        }

        onFullScreenRequested: function (request) {
            root.visibility = request.toggleOn ? Window.FullScreen : Window.Windowed;
            request.accept();
        }

        onJavaScriptDialogRequested: function (request) {
            // The page must never be able to block the UI thread with a modal.
            request.dialogAccept();
        }
    }

    // The only chrome instaCache adds to the page: a 3px gradient line.
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
            onTriggered: bar.fraction = Math.max(bar.fraction, view.loadProgress / 100)
        }
        Timer { id: sweepDone; interval: 450; onTriggered: bar.finish() }
        Timer {
            id: hide
            interval: 220
            onTriggered: { bar.opacity = 0; bar.fraction = 0; }
        }
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
                root.saveState();
                Qt.quit();
                return;
            }

            if (events.present) {
                root.raise();
                root.requestActivate();
                if (root.lastNotification) {
                    // Tells the page the notification was clicked.
                    root.lastNotification.click();
                    root.lastNotification = null;
                }
            }
            for (var i = 0; i < events.urls.length; i++)
                view.url = events.urls[i];
        }
    }

    // Bound on the window, so the page keeps receiving every other key it
    // needs for its own shortcuts (`/` to search, arrows in Stories).
    property real zoomStep: 1.1
    function zoomBy(factor) {
        view.zoomFactor = Math.min(5.0, Math.max(0.25, view.zoomFactor * factor));
    }

    Shortcut { sequences: ["Ctrl+R", "F5"];             onActivated: view.reload() }
    Shortcut { sequences: ["Ctrl+Shift+R", "Shift+F5"]; onActivated: view.triggerWebAction(WebEngineView.ReloadAndBypassCache) }
    Shortcut { sequences: ["Alt+Left", "Back"];         onActivated: view.goBack() }
    Shortcut { sequences: ["Alt+Right", "Forward"];     onActivated: view.goForward() }
    Shortcut { sequences: ["Ctrl+H", "Alt+Home"];       onActivated: view.url = shell.home_url }
    Shortcut { sequences: ["Ctrl++", "Ctrl+=" ];        onActivated: root.zoomBy(root.zoomStep) }
    Shortcut { sequence: "Ctrl+-";                      onActivated: root.zoomBy(1 / root.zoomStep) }
    Shortcut { sequence: "Ctrl+0";                      onActivated: view.zoomFactor = 1.0 }
    Shortcut { sequences: ["Ctrl+Q", "Ctrl+W"];         onActivated: { root.saveState(); Qt.quit(); } }
    Shortcut {
        sequence: "F11"
        onActivated: root.visibility = root.visibility === Window.FullScreen
                                     ? Window.Windowed : Window.FullScreen
    }
    Shortcut {
        sequence: "Escape"
        onActivated: if (root.visibility === Window.FullScreen) root.visibility = Window.Windowed
    }
    Shortcut {
        sequences: ["Ctrl+Shift+I", "F12"]
        enabled: shell.developer_tools
        onActivated: view.triggerWebAction(WebEngineView.InspectElement)
    }
}
