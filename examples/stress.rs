//! Drives the page hard from inside, without touching the GUI.
//!
//! The reference machine is reached over a remote desktop, where injected
//! keyboard and mouse events never arrive and screenshots come back blank.
//! Reproducing a crash that only happens while scrolling therefore needs the
//! page driven from within, which is what this does: it builds the same
//! `Shell` the app builds, then scrolls and navigates through JavaScript on a
//! timer.
//!
//!     cargo run --example stress                  # local page, hits nothing
//!     cargo run --example stress -- 120           # for two minutes
//!     cargo run --example stress -- 120 <url>     # against a real site
//!
//! Point it at Instagram only when the bug genuinely needs the real site:
//! automated navigation there looks like a bot and risks the account. For
//! measuring video smoothness use `bench/` instead, which drives the shipped
//! binary rather than a stand-in.

use std::rc::Rc;

use instacache::bridge::Shell;
use instacache::{chromium, config, paths};
use qmetaobject::{QObjectPinned, QmlEngine};

const SCENE: &str = r#"
import QtQuick
import QtQuick.Window
import QtWebEngine

Window {
    id: root
    width: 1280; height: 900; visible: true

    // Enough churn to exercise in-app navigation and a steady stream of
    // resource loads several times over.
    property var actions: [
        "window.scrollBy(0, 900);",
        "window.scrollBy(0, 1400);",
        "window.scrollBy(0, -600);",
        "history.pushState({}, '', '/explore/'); window.dispatchEvent(new PopStateEvent('popstate'));",
        "history.pushState({}, '', '/reels/'); window.dispatchEvent(new PopStateEvent('popstate'));",
        "history.back();",
        "document.querySelectorAll('video').forEach(v => { try { v.play(); } catch (e) {} });"
    ]
    property int step: 0

    WebEngineProfile {
        id: profile
        offTheRecord: false
        storageName: "instacache"
        persistentStoragePath: shell.storage_path
        cachePath: shell.cache_path
        httpCacheType: WebEngineProfile.DiskHttpCache
        persistentCookiesPolicy: WebEngineProfile.ForcePersistentCookies
        httpUserAgent: shell.user_agent
    }

    WebEngineView {
        id: view
        anchors.fill: parent
        profile: profile
        url: TARGET
        settings.playbackRequiresUserGesture: false

        onRenderProcessTerminated: function (status, exitCode) {
            shell.log("rendering process terminated: status " + status
                      + ", exit " + exitCode);
        }
    }

    // Deliberately unhurried: a page needs a couple of seconds to settle
    // before the next action means anything, and a faster cadence measures
    // the harness rather than the browser.
    Timer {
        interval: INTERVAL
        running: true
        repeat: true
        onTriggered: {
            view.runJavaScript(root.actions[root.step % root.actions.length]);
            root.step++;
        }
    }

    Timer {
        interval: DURATION
        running: true
        onTriggered: {
            shell.log("survived " + (DURATION / 1000) + "s and " + root.step + " actions");
            Qt.quit();
        }
    }
}
"#;

/// A page that changes its URL and loads resources continuously, the two
/// things a single-page application does. Written to a temporary file so the
/// default run touches nobody's servers.
fn local_page() -> String {
    let path = std::env::temp_dir().join(format!("instacache-stress-{}.html", std::process::id()));
    std::fs::write(
        &path,
        r#"<!doctype html>
<meta charset="utf-8">
<title>instaCache stress</title>
<body style="font:14px system-ui;padding:2rem;height:400vh">
<h1>Stress page</h1>
<p id="log"></p>
<script>
let n = 0;
setInterval(() => {
  n++;
  history.pushState({}, '', '/fake/' + n);
  fetch(location.pathname + '?probe=' + n).catch(() => {});
  document.getElementById('log').textContent = n + ' navigations';
}, 300);
</script>
</body>"#,
    )
    .expect("could not write the stress page");
    format!("file://{}", path.display())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(120);
    let url = args.next().unwrap_or_else(local_page);

    let interval_ms: u64 = std::env::var("STRESS_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);

    // A throwaway profile unless a real site was named, so the default run
    // cannot disturb a live session.
    let profile = if url.starts_with("file://") {
        format!("stress-{}", std::process::id())
    } else {
        paths::DEFAULT_PROFILE.to_string()
    };
    let paths = paths::Paths::for_profile(&profile);
    paths.ensure().expect("could not open the profile");
    let config = Rc::new(config::Config::load_or_create(&paths));

    chromium::apply(&config);
    qmetaobject::webengine::initialize();

    let mut engine = QmlEngine::new();
    let shell = std::cell::RefCell::new(Shell::new(config, Rc::new(paths), None, None));
    let pinned = unsafe { QObjectPinned::new(&shell) };
    engine.set_object_property("shell".into(), pinned);

    let scene = SCENE
        .replace("TARGET", &format!("{url:?}"))
        .replace("INTERVAL", &interval_ms.to_string())
        .replace("DURATION", &(seconds * 1000).to_string());
    engine.load_data(scene.into());
    engine.exec();
}
