// Drives the real scene: pages, calls, the back button and pill, the mouse
// buttons, shortcuts and closing to the background. Run through run.sh, which
// builds Harness.qml from src/qml/main.qml.

import QtQuick
import QtQuick.Window
import QtTest
import QtWebEngine

Item {
    id: host
    property var win: null
    TestCase {
        name: "Scene"
        when: true

        function findByLabel(item, label) {
            if (item.label !== undefined && item.label === label) return item;
            for (var i = 0; i < item.children.length; i++) {
                var f = findByLabel(item.children[i], label);
                if (f) return f;
            }
            return null;
        }
        function waitFor(fn, ms, what) {
            var t = 0;
            while (!fn() && t < ms) { wait(100); t += 100; }
            verify(fn(), what);
        }
        function initTestCase() {
            var c = Qt.createComponent("Harness.qml");
            if (c.status !== Component.Ready) console.log("component error: " + c.errorString());
            compare(c.status, Component.Ready, "scene compiles");
            win = c.createObject(null);
            verify(win !== null, "scene instantiates");
            var main = win.activeView();
            waitFor(function () { return !main.loading && main.title === "Instagram"; }, 15000, "home loads");
        }

        function test_01_unread_title_reaches_shell() {
            var main = win.activeView();
            main.runJavaScript("document.title = '(3) Instagram'");
            waitFor(function () { return win.shell.titles.indexOf("(3) Instagram") >= 0; }, 5000, "title_changed called");
            compare(win.title, "(3) Instagram");
            compare(win.unread, 3, "the tray tooltip's count");
            main.runJavaScript("document.title = 'Instagram'");
        }

        // The count Instagram announces just after a load is what was already
        // waiting, and bridge.rs only knows that if the scene says a load
        // started. Without this the app would announce old messages at every
        // launch.
        function test_01b_a_load_of_the_main_page_is_reported() {
            verify(win.shell.mainPageLoads > 0, "main_page_loading called for the home page");
            var before = win.shell.mainPageLoads;
            var main = win.activeView();
            main.reload();
            waitFor(function () { return win.shell.mainPageLoads > before; }, 8000,
                    "a reload is reported too");
            waitFor(function () { return !main.loading && main.title === "Instagram"; }, 15000,
                    "home loads again");
        }

        // The tray menu is built from a QML string, so nothing checks it at
        // compile time: a typo in it costs the whole tray, silently, and the
        // only symptom is a missing icon. Building it is the test.
        function test_01c_the_tray_menu_is_valid_qml() {
            verify(win.tray !== null, "the tray object was created");
            verify(win.tray.menu !== null, "it has a menu");
            verify(win.trayState().length > 0, "the state line says something");

            // The settings in it reach the shell and come back changed.
            var before = win.shell.setting("notification_sounds");
            win.toggleSetting("notification_sounds");
            compare(win.shell.setting("notification_sounds"), !before,
                    "the menu writes the setting");
            verify(win.settingIsOn("notification_sounds") === !before,
                   "and the menu shows what it wrote");
            win.toggleSetting("notification_sounds");

            compare(win.startsWithSession, false);
            win.shell.set_start_with_session(true);
            win.refreshStartsWithSession();
            compare(win.startsWithSession, true, "start at login is read back");
            win.shell.set_start_with_session(false);
            win.refreshStartsWithSession();
        }

        // The mods are a user script on the profile, and the point of that is
        // running before the page's own code: at document creation the
        // document is still loading. Switching one off takes effect at the
        // next load.
        function test_01d_mods_run_before_the_page() {
            var main = win.activeView();
            function mark() {
                var seen = null;
                main.runJavaScript("window.__instacacheMod || ''", function (r) { seen = r; });
                waitFor(function () { return seen !== null; }, 3000, "the page answers");
                return seen;
            }
            function reload() {
                main.reload();
                wait(200);
                waitFor(function () { return !main.loading && main.title === "Instagram"; },
                        15000, "home loads again");
            }
            compare(mark(), "loading", "the mod ran at document creation");

            win.toggleSetting("videos_with_sound");
            reload();
            compare(mark(), "", "a mod switched off is gone at the next load");

            win.toggleSetting("videos_with_sound");
            reload();
            compare(mark(), "loading", "and back once switched on again");
        }

        function test_02_call_opens_as_page_and_survives_back() {
            var main = win.activeView();
            main.runJavaScript("window.open('/call.html', 'call', 'width=640,height=480')");
            waitFor(function () { return win.current !== null; }, 8000, "call page shown");
            compare(win.pages.length, 1);
            waitFor(function () { return win.current.title.indexOf("Call live") === 0; }, 10000, "call page got mic+camera: " + (win.current ? win.current.title : ""));
            verify(win.current.inCall);
            verify(!main.visible, "Instagram hidden under the page");
            var back = findByLabel(win.contentItem, "‹  Instagram");
            verify(back !== null && back.visible, "back button labelled for a call");
            mouseClick(back);
            waitFor(function () { return win.current === null; }, 3000, "back returns to Instagram");
            verify(win.callPage !== null, "call kept alive");
            verify(main.visible);
            var pill = findByLabel(win.contentItem, "●  Call in progress — return");
            waitFor(function () { return pill.visible; }, 2000, "call pill shown");
            mouseClick(pill);
            waitFor(function () { return win.current === win.callPage; }, 3000, "pill returns to the call");
            win.current = null;
        }

        function test_03_link_page_and_mouse_back_button() {
            var main = win.activeView();
            main.runJavaScript("document.getElementById('tab').click()");
            waitFor(function () { return win.current !== null && !win.current.inCall; }, 8000, "link opened as a page");
            compare(win.pages.length, 2, "call kept, one ordinary page");
            waitFor(function () { return win.current.title === "A page"; }, 8000, "page loaded");
            var back = findByLabel(win.contentItem, "‹  Back");
            verify(back.visible);
            mousePress(win.contentItem, 500, 400, Qt.BackButton);
            mouseRelease(win.contentItem, 500, 400, Qt.BackButton);
            waitFor(function () { return win.current === null; }, 3000, "mouse back closes the page");
            wait(300);
            compare(win.pages.length, 1, "ordinary page destroyed, call kept");
        }

        function test_04_second_page_replaces_the_first() {
            var main = win.activeView();
            main.runJavaScript("document.getElementById('tab').click()");
            waitFor(function () { return win.current !== null && !win.current.inCall; }, 8000, "first page");
            var first = win.current;
            main.runJavaScript("document.getElementById('tab').click()");
            waitFor(function () { return win.current !== null && win.current !== first; }, 8000, "second page");
            wait(300);
            compare(win.pages.length, 2, "one ordinary page at a time, plus the call");
            keyClick(Qt.Key_W, Qt.ControlModifier);
            waitFor(function () { return win.current === null; }, 3000, "Ctrl+W closes the page");
            verify(win.visible, "and not the window");
        }

        function test_05_popup_leaving_for_the_browser_closes() {
            var before = win.pages.length;
            win.activeView().runJavaScript("var w = window.open('', 'x', 'width=300,height=300'); setTimeout(function(){ w.location.href = 'https://example.invalid/share'; }, 300);");
            waitFor(function () { return win.shell.externals.indexOf("https://example.invalid/share") >= 0; }, 8000, "external address handed to the browser: " + JSON.stringify(win.shell.externals));
            waitFor(function () { return win.pages.length === before && win.current === null; }, 3000, "empty page closed");
        }

        function test_06_call_ending_closes_its_page() {
            win.current = win.callPage;
            win.callPage.runJavaScript("window.close()");
            waitFor(function () { return win.callPage === null && win.current === null; }, 5000, "call page gone");
            compare(win.pages.length, 0);
        }

        function test_07_escape_reaches_the_page_when_not_fullscreen() {
            var main = win.activeView();
            main.runJavaScript("document.title = 'Instagram'");
            wait(300);
            mouseClick(win.contentItem, 600, 500);
            keyClick(Qt.Key_Escape);
            waitFor(function () { return main.title === "escape-reached"; }, 3000, "Escape not swallowed: " + main.title);
        }

        function test_08_shortcut_moves_inside_instagram() {
            var main = win.activeView();
            keyClick(Qt.Key_4, Qt.ControlModifier);
            waitFor(function () { return main.title === "inbox-link"; }, 3000, "Ctrl+4 clicked the Direct link: " + main.title);
        }

        function test_10_update_ready_offers_a_restart_on_the_same_page() {
            win.shell.update = "ready";
            var pill = null;
            waitFor(function () { pill = findByLabel(win.contentItem, "Update ready — Restart"); return pill !== null && pill.parent.opacity === 1; }, 3000, "update pill shown");
            mouseClick(pill);
            waitFor(function () { return win.shell.restarts.length === 1; }, 2000, "restart asked for");
            compare(win.shell.restarts[0].hidden, false, "a visible window restarts visible");
            verify(win.shell.restarts[0].url.indexOf("127.0.0.1") > 0, "on the same page");
            var later = findByLabel(win.contentItem, "Later");
            mouseClick(later);
            waitFor(function () { return pill.parent.opacity === 0; }, 2000, "Later hides the offer");
        }

        function test_11_a_system_install_asks_before_installing() {
            win.shell.update = "installable";
            var pill = null;
            waitFor(function () { pill = findByLabel(win.contentItem, "instaCache 9.9.9 available — Install"); return pill !== null && pill.parent.opacity === 1; }, 3000, "install offer shown again for a new state");
            mouseClick(pill);
            compare(win.shell.installs, 1);
            waitFor(function () { return findByLabel(win.contentItem, "Installing instaCache 9.9.9…") !== null; }, 2000, "installing shown");
            win.shell.update = "none";
        }

        function test_09_close_hides_instead_of_quitting() {
            win.activeView().runJavaScript("document.body.insertAdjacentHTML('beforeend', '<video id=v loop muted autoplay src=\"data:,\"></video>')");
            win.close();
            wait(500);
            verify(!win.visible, "hidden");
            verify(!win.activeView().visible, "Chromium is told the page is hidden");
            compare(win.shell.hidden, 1);
            win.shell.pollResult = '{"present":true,"urls":[],"quit":false}';
            waitFor(function () { return win.visible; }, 3000, "a second launch shows it again");
            waitFor(function () { return win.activeView().visible && !win.backgrounded; }, 2000, "the page is live again");
        }

        // Each desktop notification reaches the page notification it came
        // from: a click on the first opens the first, not whichever arrived
        // last, and the page withdrawing one takes it off the desktop.
        function test_13_a_click_reaches_the_notification_that_was_clicked() {
            win.bringToFront();
            win.shell.notifications_enabled = true;
            var main = win.activeView();
            main.url = "http://127.0.0.1:8791/notify.html";
            waitFor(function () { return win.shell.notificationKeys.length >= 2; }, 8000,
                    "both notifications reached shell: " + main.title);
            var keys = win.shell.notificationKeys;
            verify(keys[0] !== keys[1], "each has its own key");

            win.shell.pollResult = JSON.stringify({clicked: [keys[0]]});
            waitFor(function () { return main.title === "clicked-a"; }, 3000,
                    "the first was clicked, not the latest: " + main.title);

            main.runJavaScript("window.second.close()");
            waitFor(function () { return win.shell.withdrawn.indexOf(keys[1]) >= 0; }, 3000,
                    "the page closing it withdraws it from the desktop");
            verify(win.pageNotifications[keys[1]] === undefined, "and lets go of it");

            win.shell.pollResult = JSON.stringify({closed: [keys[0]]});
            waitFor(function () { return main.title === "closed-a"; }, 3000,
                    "dismissed on the desktop, the page hears it closed: " + main.title);
            verify(win.pageNotifications[keys[0]] === undefined, "and it is let go of");
            compare(win.shell.withdrawn.indexOf(keys[0]), -1,
                    "a close the desktop made is not sent back to it");
            win.shell.notifications_enabled = false;
        }

        // Nothing runs while nothing happens: the scene drains on `wake`,
        // not on a timer that fires whether or not there is news.
        function test_14_an_idle_window_does_not_poll() {
            win.close();
            wait(300);
            var before = win.shell.polls;
            wait(2000);
            compare(win.shell.polls, before, "no poll without a wake, even hidden");
            win.shell.pollResult = '{"present":true}';
            waitFor(function () { return win.visible; }, 2000, "a wake is answered at once");
        }

        // A closed window's page is frozen once settled, discarded later, and
        // comes back live -- reloaded, behind its snapshot -- when opened.
        function test_15_a_closed_window_freezes_then_discards_its_page() {
            var main = win.activeView();
            main.url = win.shell.home_url;
            waitFor(function () { return !main.loading && main.title === "Instagram"; }, 8000, "home again");
            win.restDelay = 200;
            win.shell.discard_after_ms = 1500;
            win.close();
            waitFor(function () { return main.lifecycleState === WebEngineView.LifecycleState.Frozen; }, 3000,
                    "frozen once settled: " + main.lifecycleState);
            waitFor(function () { return main.lifecycleState === WebEngineView.LifecycleState.Discarded; }, 4000,
                    "discarded after the delay: " + main.lifecycleState);
            win.shell.pollResult = '{"present":true}';
            waitFor(function () { return win.visible; }, 2000, "shown again");
            compare(main.lifecycleState, WebEngineView.LifecycleState.Active, "active before it shows");
            waitFor(function () { return !main.loading && main.title === "Instagram"; }, 8000, "reloaded");
            waitFor(function () { return !win.awaitingReload; }, 3000, "the snapshot waits for the reload, then goes");
            win.shell.discard_after_ms = 0;
            win.restDelay = 10000;
        }

        // Opening the window with an update waiting says so and restarts into
        // it after a count; "Not now" puts it off.
        function test_16_opening_the_window_shows_the_update_and_restarts() {
            // A "Later" from an earlier test still holds, as it should.
            win.updateDismissed = "";
            win.shell.update = "ready";
            win.close();
            wait(300);
            win.shell.restarts = [];
            win.shell.pollResult = '{"present":true}';
            waitFor(function () { return win.visible && win.updateCountdown > 0; }, 2000, "the update card shows");
            verify(findByLabel(win.contentItem, "Restart now") !== null, "with a way to restart now");
            var notNow = findByLabel(win.contentItem, "Not now");
            waitFor(function () { return notNow.visible; }, 2000, "the card has faded in");
            mouseClick(notNow);
            waitFor(function () { return win.updateCountdown === 0; }, 1000, "Not now puts it off");
            wait(1500);
            compare(win.shell.restarts.length, 0, "and nothing restarts");

            win.updateDismissed = "";
            win.close();
            wait(300);
            win.shell.restarts = [];
            win.shell.pollResult = '{"present":true}';
            waitFor(function () { return win.updateCountdown > 0; }, 2000, "shown again for a new opening");
            waitFor(function () { return win.shell.restarts.length > 0; }, 7000, "the count ends in a restart");
            compare(win.shell.restarts[win.shell.restarts.length - 1].hidden, false, "into a window that shows");
            win.shell.update = "none";
        }

        function test_12_a_closed_window_takes_an_update_quietly() {
            win.shell.restarts = [];
            win.shell.update = "ready";
            wait(600);
            compare(win.shell.restarts.length, 0, "not while the window is open");
            win.close();
            waitFor(function () { return win.shell.restarts.length > 0; }, 3000, "quiet restart once closed");
            compare(win.shell.restarts[0].hidden, true, "and it stays hidden");
            win.shell.update = "none";
        }
    }
}
