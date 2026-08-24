//! Development helper: render a page through WebKit's own snapshot API and
//! write it to a PNG.
//!
//! Useful when a display-server screenshot is unavailable or unreliable
//! (headless CI, XWayland, screenshot portals), because the snapshot is
//! produced inside the WebProcess and never touches the compositor.
//!
//! It builds the web view through `gramcache::web::build`, so what it captures
//! is the real application configuration — user agent, cache model, settings
//! and all.
//!
//!     cargo run --example snapshot -- <url> <output.png> [seconds]

use std::rc::Rc;

use gramcache::config::Config;
use gramcache::paths::Paths;
use gtk::cairo;
use gtk::prelude::*;
use webkit2gtk::{LoadEvent, SnapshotOptions, SnapshotRegion, WebViewExt};

fn main() {
    let mut args = std::env::args().skip(1);
    let url = args
        .next()
        .unwrap_or_else(|| "https://www.instagram.com/".into());
    let output = args.next().unwrap_or_else(|| "snapshot.png".into());
    let settle: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(8);

    // Redirect every storage root into a temporary directory so the
    // developer's real session is never touched.
    let sandbox = std::env::temp_dir().join(format!("gramcache-snapshot-{}", std::process::id()));
    for key in [
        "GRAMCACHE_DATA_HOME",
        "GRAMCACHE_CACHE_HOME",
        "GRAMCACHE_CONFIG_HOME",
    ] {
        std::env::set_var(key, &sandbox);
    }

    gtk::init().expect("GTK could not be initialised");

    let paths = Rc::new(Paths::for_profile("snapshot"));
    paths
        .ensure()
        .expect("could not create the snapshot sandbox");
    let config = Config::default();

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_default_size(1180, 820);
    let view = gramcache::web::build(&config, &paths).view;
    window.add(&view);
    window.show_all();

    view.connect_load_changed(move |view, event| {
        if event != LoadEvent::Finished {
            return;
        }
        let view = view.clone();
        let output = output.clone();
        // Give the client-side app time to hydrate before snapshotting.
        gtk::glib::timeout_add_seconds_local_once(settle as u32, move || {
            view.snapshot(
                SnapshotRegion::Visible,
                SnapshotOptions::NONE,
                None::<&gtk::gio::Cancellable>,
                move |result| {
                    match result {
                        Ok(surface) => write_png(&surface, &output),
                        Err(err) => eprintln!("snapshot failed: {err}"),
                    }
                    gtk::main_quit();
                },
            );
        });
    });

    view.load_uri(&url);
    gtk::main();

    std::fs::remove_dir_all(&sandbox).ok();
}

fn write_png(surface: &cairo::Surface, output: &str) {
    let image =
        cairo::ImageSurface::try_from(surface.clone()).expect("snapshot is not an image surface");
    println!("snapshot: {}x{} -> {output}", image.width(), image.height());
    match std::fs::File::create(output) {
        Ok(mut file) => {
            if let Err(err) = image.write_to_png(&mut file) {
                eprintln!("could not encode PNG: {err}");
            }
        }
        Err(err) => eprintln!("could not create {output}: {err}"),
    }
}
