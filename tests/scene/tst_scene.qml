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

        function test_1_unread_title_reaches_shell() {
            var main = win.activeView();
            main.runJavaScript("document.title = '(3) Instagram'");
            waitFor(function () { return win.shell.titles.indexOf("(3) Instagram") >= 0; }, 5000, "title_changed called");
            compare(win.title, "(3) Instagram");
            main.runJavaScript("document.title = 'Instagram'");
        }

        function test_2_call_opens_as_page_and_survives_back() {
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

        function test_3_link_page_and_mouse_back_button() {
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

        function test_4_second_page_replaces_the_first() {
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

        function test_5_popup_leaving_for_the_browser_closes() {
            var before = win.pages.length;
            win.activeView().runJavaScript("var w = window.open('', 'x', 'width=300,height=300'); setTimeout(function(){ w.location.href = 'https://example.invalid/share'; }, 300);");
            waitFor(function () { return win.shell.externals.indexOf("https://example.invalid/share") >= 0; }, 8000, "external address handed to the browser: " + JSON.stringify(win.shell.externals));
            waitFor(function () { return win.pages.length === before && win.current === null; }, 3000, "empty page closed");
        }

        function test_6_call_ending_closes_its_page() {
            win.current = win.callPage;
            win.callPage.runJavaScript("window.close()");
            waitFor(function () { return win.callPage === null && win.current === null; }, 5000, "call page gone");
            compare(win.pages.length, 0);
        }

        function test_7_escape_reaches_the_page_when_not_fullscreen() {
            var main = win.activeView();
            main.runJavaScript("document.title = 'Instagram'");
            wait(300);
            mouseClick(win.contentItem, 600, 500);
            keyClick(Qt.Key_Escape);
            waitFor(function () { return main.title === "escape-reached"; }, 3000, "Escape not swallowed: " + main.title);
        }

        function test_8_shortcut_moves_inside_instagram() {
            var main = win.activeView();
            keyClick(Qt.Key_4, Qt.ControlModifier);
            waitFor(function () { return main.title === "inbox-link"; }, 3000, "Ctrl+4 clicked the Direct link: " + main.title);
        }

        function test_9_close_hides_instead_of_quitting() {
            win.close();
            wait(500);
            verify(!win.visible, "hidden");
            compare(win.shell.hidden, 1);
            win.shell.pollResult = '{"present":true,"urls":[],"quit":false}';
            waitFor(function () { return win.visible; }, 3000, "a second launch shows it again");
        }
    }
}
