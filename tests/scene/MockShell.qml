// A stand-in for `shell`, the Rust bridge, so the real scene can run under
// qmltestrunner. It records what the scene asks of Rust instead of doing it:
// nothing here opens a browser, posts a notification or writes a file.
//
// Keep it in step with the `qt_property!` and `qt_method!` list in bridge.rs.
// A name missing here fails only when the scene calls it, not when it loads.

import QtQuick

QtObject {
    property string home_url: "http://127.0.0.1:8791/home.html"
    function user_agent_for(engine) { return engine; }
    property string accept_language: "en-US,en;q=0.9"
    property string storage_path: "@TMP@/data"
    property string cache_path: "@TMP@/cache"
    property int http_cache_size: 0
    property string window_title: "instaCache"
    property bool show_loading_indicator: true
    property bool developer_tools: false
    property bool remember_window_state: true
    property bool autoplay_without_gesture: true
    property bool notifications_enabled: false
    property bool push_service_enabled: false
    property bool external_links_in_browser: true
    property bool context_menu: false
    property bool run_in_background: true
    property bool instagram_shortcuts: true
    property bool start_hidden: false
    property bool tray_icon: true
    property bool own_title_bar: true
    property string icon_name: "instacache"
    property string icon_source: ""
    property string user_stylesheet: ""
    property string user_script: ""
    property string spell_check_languages: ""
    property int discard_after_ms: 0
    property string app_version: "9.9.9"
    property string updated_from: ""
    // The real policy, less its settings: frozen when closed, discarded
    // when closed for long, never past what the view recommends, never a
    // call.
    property var lifecycles: []
    function lifecycle_when_hidden(forLong, recommended, inCall) {
        var order = ["Active", "Frozen", "Discarded"];
        var wanted = inCall ? 0 : (forLong ? 2 : 1);
        var limit = Math.max(0, order.indexOf(recommended));
        var name = order[Math.min(wanted, limit)];
        lifecycles = lifecycles.concat([name]);
        return name;
    }

    // What the scene did, for the tests to read.
    property var externals: []
    property var titles: []
    property int hidden: 0
    property string pollResult: ""
    // Reported by every poll, the way bridge.rs reports it.
    property string update: "none"
    property var restarts: []
    property int installs: 0

    function unwrap_link(u) {
        var m = /^https:\/\/l\.instagram\.com\/\?u=([^&#]*)/.exec(u);
        return m ? decodeURIComponent(m[1]) : "";
    }
    function is_internal(u) { return /^https?:\/\/127\.0\.0\.1(:|\/)/.test(u); }
    function grants_permission(origin, feature) {
        return is_internal(origin + "/")
            && (feature.indexOf("Media") === 0 || feature === "Notifications");
    }
    function is_engine_scheme(u) {
        return /^(about|blob|data|javascript|file):/.test(u) || !/^[a-z]+:/.test(u);
    }
    // Records what would leave, and opens nothing.
    function external_target(u) { externals = externals.concat([u]); return ""; }
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
    // The real bridge emits this when a thread hands something over; here a
    // test changing what the next poll returns is that thread.
    signal wake()
    onPollResultChanged: if (pollResult !== "") wake()
    onUpdateChanged: wake()
    property int polls: 0
    function poll() {
        polls++;
        var result = pollResult !== "" ? JSON.parse(pollResult) : {};
        pollResult = "";
        var defaults = {present: false, urls: [], quit: false,
                        clicked: [], closed: [], forgotten: [], show: []};
        for (var name in defaults) {
            if (result[name] === undefined)
                result[name] = defaults[name];
        }
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
    property var notifications: []
    property var notificationKeys: []
    property var withdrawn: []
    property int ringStops: 0
    property var ringing: []
    function notify_page(title, body, tag, key, windowActive) {
        notifications = notifications.concat([title]);
        notificationKeys = notificationKeys.concat([key]);
        ringing = ringing.concat([windowActive]);
        return true;
    }
    function page_notification_closed(key) { withdrawn = withdrawn.concat([key]); }
    property var trayClicks: []
    function notification_clicked(id) { trayClicks = trayClicks.concat([id]); }
    function stop_ringing() { ringStops++; }
    function title_changed(title, windowActive) {
        titles = titles.concat([title]);
        var match = /^\((\d+)\+?\)/.exec(title.trim());
        return match ? parseInt(match[1]) : 0;
    }
    property int mainPageLoads: 0
    function main_page_loading() { mainPageLoads++; }

    // The tray's settings. The real ones live in config.json and in
    // ~/.config/autostart; here they are just remembered.
    property bool autostart: false
    property var settings: ({
        notifications: true, notification_sounds: true, ring_for_calls: true,
        unread_badge: true, run_in_background: true, videos_with_sound: true,
        instagram_colors: true, ambient_colors: true, page_animations: true,
        shortcuts: true, hide_also_from_meta: true
    })
    function starts_with_session() { return autostart; }
    function set_start_with_session(on) { autostart = on; return on; }
    function setting(name) { return settings[name] === true; }
    function set_setting(name, value) { settings[name] = value; }
    function reloads_page(name) { return name === "shortcuts" || name === "videos_with_sound"; }

    // A stand-in for the built-in mods that leaves a mark, so a test can see
    // whether it ran, and how early.
    function mods_script() {
        return settings.videos_with_sound
            ? "window.__instacacheMod = document.readyState;" : "";
    }
}
