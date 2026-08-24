//! Renders a page to PNG through the real engine configuration.
//!
//! On this project's reference machine every display-server screenshot comes
//! back entirely white, for every window, including unrelated applications —
//! a broken capture pipeline, not a broken app. Two blank captures cost an
//! hour before that was established, so "does it actually render?" is answered
//! here instead, from inside the engine, without touching the compositor.
//!
//!     cargo run --example snapshot -- https://www.instagram.com/ shot.png
//!
//! It builds the same `Shell` the app builds, so the profile, the user agent
//! and the Chromium flags under test are the ones that ship.

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

        onLoadingChanged: function (info) {
            if (info.status === WebEngineView.LoadStartedStatus)
                return;
            // Give the page a moment to paint what it just finished loading;
            // grabbing the instant a load reports success captures a blank
            // frame often enough to be useless.
            settle.start();
        }
    }

    Timer {
        id: settle
        interval: 2500
        onTriggered: view.grabToImage(function (result) {
            if (result.saveToFile(OUTPUT))
                shell.log("wrote " + OUTPUT);
            else
                shell.log("could not write " + OUTPUT);
            Qt.quit();
        })
    }

    // Never hang a scripted run for ever.
    Timer { interval: 30000; running: true; onTriggered: { shell.log("timed out"); Qt.quit(); } }
}
"#;

fn main() {
    let mut args = std::env::args().skip(1);
    let url = args
        .next()
        .unwrap_or_else(|| "https://www.instagram.com/".to_string());
    let output = args.next().unwrap_or_else(|| "shot.png".to_string());
    let output = std::path::absolute(&output)
        .unwrap_or_else(|_| output.into())
        .to_string_lossy()
        .into_owned();

    let paths = paths::Paths::for_profile(paths::DEFAULT_PROFILE);
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
        .replace("OUTPUT", &format!("{output:?}"));
    engine.load_data(scene.into());
    engine.exec();
}
