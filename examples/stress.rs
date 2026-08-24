//! Development harness: drive the browser hard without touching the GUI.
//!
//! The reference machine is reached over a remote desktop, where injected
//! keyboard and mouse events never arrive and screenshots come back blank.
//! Reproducing a crash that only happens while scrolling therefore needs the
//! page to be driven from inside, which is what this does: it runs the same
//! `web::build` the app runs, installs the same loading bar, and then scrolls
//! and navigates through JavaScript on a timer.
//!
//!     cargo run --example stress                 # local page, hits nothing
//!     cargo run --example stress -- 120          # for two minutes
//!     cargo run --example stress -- 120 <url>    # against a real site
//!
//! By default it drives a generated local page, which exercises the same
//! signals — a URI changing without a page load, and a steady stream of
//! resource loads — without touching anyone's servers. Point it at Instagram
//! only when the bug genuinely needs the real site: automated navigation there
//! looks like a bot and risks the account.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use instacache::config::Config;
use instacache::paths::Paths;
use webkit2gtk::WebViewExt;

/// Enough churn to exercise the resource-load storm and the in-app navigation
/// path several times over, without hammering Instagram.
const ACTIONS: &[&str] = &[
    "window.scrollBy(0, 900);",
    "window.scrollBy(0, 1400);",
    "window.scrollBy(0, -600);",
    "history.pushState({}, '', '/explore/'); window.dispatchEvent(new PopStateEvent('popstate'));",
    "history.pushState({}, '', '/reels/'); window.dispatchEvent(new PopStateEvent('popstate'));",
    "history.back();",
    "document.querySelectorAll('video').forEach(v => { try { v.play(); } catch (e) {} });",
];

/// Writes a page that changes its URL and loads resources continuously, the
/// two things a single-page application does that the loading bar reacts to.
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
  // A URI change with no page load, exactly like an in-app navigation.
  history.pushState({}, '', '/fake/' + n);
  // A resource load, so the bar sees network activity start and stop.
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
    let url = match args.next() {
        Some(url) => url,
        None => local_page(),
    };

    gtk::init().expect("GTK could not be initialised");

    // A throwaway profile unless a real site was named, so the default run
    // cannot disturb a live session.
    let profile = if url.starts_with("file://") {
        format!("stress-{}", std::process::id())
    } else {
        instacache::paths::DEFAULT_PROFILE.to_string()
    };
    let paths = Rc::new(Paths::for_profile(&profile));
    paths.ensure().expect("could not open the profile");
    let config = Config::load_or_create(&paths);

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_default_size(1280, 900);

    let browser = instacache::web::build(&config, &paths);
    let view = browser.view.clone();

    // The loading bar is the code under test, so it has to be wired exactly
    // the way ui.rs wires it.
    let overlay = gtk::Overlay::new();
    overlay.add(&view);
    let bar = gtk::ProgressBar::new();
    bar.set_valign(gtk::Align::Start);
    bar.set_no_show_all(true);
    overlay.add_overlay(&bar);
    window.add(&overlay);
    instacache::progress::install(&view, &bar);

    window.show_all();
    bar.hide();
    view.load_uri(&url);

    let step = Cell::new(0usize);
    let elapsed = Cell::new(0u64);
    let driver_view = view.clone();

    // The interval is deliberately unhurried: a page needs a couple of seconds
    // to settle before the next action means anything, and a faster cadence
    // measures the harness rather than the browser.
    let interval_ms: u64 = std::env::var("STRESS_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);

    gtk::glib::timeout_add_local(Duration::from_millis(interval_ms), move || {
        let index = step.get();
        step.set(index + 1);
        elapsed.set(elapsed.get() + interval_ms);

        let script = ACTIONS[index % ACTIONS.len()];
        driver_view.evaluate_javascript(
            script,
            None,
            None,
            None::<&gtk::gio::Cancellable>,
            |result| {
                if let Err(error) = result {
                    eprintln!("stress: script failed: {error}");
                }
            },
        );

        // Read the page's own jank counter, when it exposes one.
        if std::env::var_os("JANK_REPORT").is_some() {
            driver_view.evaluate_javascript(
                "JSON.stringify(window.__jank || {})",
                None,
                None,
                None::<&gtk::gio::Cancellable>,
                |result| {
                    if let Ok(value) = result {
                        use javascriptcore::ValueExt;
                        let text = value.to_str();
                        if text.len() > 2 {
                            println!("jank: {text}");
                        }
                    }
                },
            );
        }

        if elapsed.get() >= seconds * 1000 {
            println!("stress: survived {seconds}s and {index} actions");
            gtk::main_quit();
            gtk::glib::ControlFlow::Break
        } else {
            gtk::glib::ControlFlow::Continue
        }
    });

    gtk::main();
}
