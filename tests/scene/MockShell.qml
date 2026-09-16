// A stand-in for `shell`, the Rust bridge, so the real scene can run under
// qmltestrunner. It records what the scene asks of Rust instead of doing it:
// nothing here opens a browser, posts a notification or writes a file.
//
// Keep it in step with the `qt_property!` and `qt_method!` list in bridge.rs.
// A name missing here fails only when the scene calls it, not when it loads.

import QtQuick

QtObject {
    property string home_url: "http://127.0.0.1:8791/home.html"
    property string user_agent: ""
    property string storage_path: "@TMP@/data"
    property string cache_path: "@TMP@/cache"
    property string window_title: "instaCache"
    property bool show_loading_indicator: true
    property bool developer_tools: false
    property bool remember_window_state: true
    property bool autoplay_without_gesture: true
    property bool notifications_enabled: false
    property bool external_links_in_browser: true
    property bool context_menu: false
    property bool run_in_background: true
    property bool instagram_shortcuts: true
    property bool start_hidden: false
    property bool tray_icon: false
    property string icon_name: "instacache"
    property string user_stylesheet: ""
    property string user_script: ""
    property string spell_check_languages: ""

    // What the scene did, for the tests to read.
    property var externals: []
    property var titles: []
    property int hidden: 0
    property string pollResult: ""
    // Reported by every poll, the way bridge.rs reports it.
    property string update: "none"
    property var restarts: []
    property int installs: 0

    function is_internal(u) { return /^https?:\/\/127\.0\.0\.1(:|\/)/.test(u); }
    function grants_permission(origin, feature) {
        return is_internal(origin + "/") && feature.indexOf("Media") === 0;
    }
    function is_engine_scheme(u) {
        return /^(about|blob|data|javascript|file):/.test(u) || !/^[a-z]+:/.test(u);
    }
    function open_externally(u) { externals = externals.concat([u]); }
    function initial_geometry(w, h) {
        return JSON.stringify({width: 1000, height: 700, x: null, y: null,
                               maximized: false, zoom: 1.0});
    }
    function save_window_state() {}
    function download_destination(name) { return ""; }
    function error_page(u, m) { return "<title>error</title>"; }
    function crash_page(u, m) { return "<title>crash</title>"; }
    function should_reload_after_crash() { return false; }
    function notify(title, body) {}
    function poll() {
        var result = pollResult !== "" ? JSON.parse(pollResult)
                                       : {present: false, urls: [], quit: false};
        pollResult = "";
        result.update = update;
        result.update_version = update === "none" ? "" : "9.9.9";
        return JSON.stringify(result);
    }
    function may_restart_quietly(visible, inCall) {
        return update === "ready" && !visible && !inCall;
    }
    // Recorded and refused, so the scene never quits the test runner.
    function restart_for_update(hidden, url) {
        restarts = restarts.concat([{hidden: hidden, url: url}]);
        return false;
    }
    function install_update() { installs++; update = "installing"; }
    function log(message) { console.log("shell.log: " + message); }
    function window_hidden(tray) { hidden++; }
    function title_changed(title) {
        titles = titles.concat([title]);
        var match = /^\((\d+)\+?\)/.exec(title.trim());
        return match ? parseInt(match[1]) : 0;
    }
}
