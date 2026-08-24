//! Window assembly and signal wiring.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::glib::Propagation;
use gtk::prelude::*;
use webkit2gtk::{
    DownloadExt, NetworkError, NotificationExt, NotificationPermissionRequest,
    PermissionRequestExt, PolicyError, WebContextExt, WebProcessTerminationReason, WebViewExt,
    WebsiteDataAccessPermissionRequest,
};

use crate::config::{Config, WindowState};
use crate::errorpage;
use crate::paths::Paths;
use crate::web;
use crate::{APP_NAME, ICON_NAME};

const EMBEDDED_ICON: &[u8] = include_bytes!("../assets/instacache.svg");

/// A 3px gradient bar pinned to the top of the window; the only chrome
/// instaCache adds to the page.
const LOADING_BAR_CSS: &str = "
progressbar.instacache-loading,
progressbar.instacache-loading trough {
    min-height: 3px;
    background-color: transparent;
    background-image: none;
    border: 0;
    padding: 0;
    box-shadow: none;
}
progressbar.instacache-loading progress {
    min-height: 3px;
    border: 0;
    border-radius: 0;
    background-image: linear-gradient(to right, #FFDD55, #E1306C 55%, #5B4BE0);
}
";

pub fn build_window(
    app: &gtk::Application,
    config: Rc<Config>,
    paths: Rc<Paths>,
    start_url: Option<String>,
) -> gtk::ApplicationWindow {
    // A saved geometry always wins; otherwise the window is sized from the
    // monitor so the first launch is not cramped.
    let state = WindowState::load(&paths)
        .filter(|_| config.remember_window_state)
        .unwrap_or_else(|| WindowState::from_work_area(primary_work_area()));

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title(window_title(&paths))
        .default_width(state.width)
        .default_height(state.height)
        .build();

    install_icon(&window);

    if state.maximized || config.start_maximized {
        window.maximize();
    } else {
        // `default_width`/`default_height` are only a hint, and some window
        // managers substitute a remembered or heuristic geometry instead. An
        // explicit resize marks the size as user-specified, which they do
        // honour, and it is repeated once the window is mapped because that is
        // the point at which the manager has committed to a geometry.
        window.resize(state.width, state.height);
        apply_geometry_on_map(&window, state.width, state.height);

        if let (Some(x), Some(y)) = (state.x, state.y) {
            // No-op under Wayland, where clients cannot place their own windows.
            window.move_(x, y);
        }
    }

    let browser = web::build(&config, &paths);
    let view = browser.view.clone();

    view.set_zoom_level(if config.remember_window_state {
        state.zoom
    } else {
        config.default_zoom
    });

    let overlay = gtk::Overlay::new();
    overlay.add(&view);

    let progress = build_loading_bar();
    if config.show_loading_indicator {
        overlay.add_overlay(&progress);
    }
    window.add(&overlay);

    wire_loading_feedback(&view, &window, &progress, config.show_loading_indicator);
    wire_error_page(&view);
    wire_crash_recovery(&view);
    wire_notifications(app, &window, &view, &config);
    wire_downloads(app, &browser.context);
    wire_state_persistence(&window, &view, &config, &paths);

    crate::shortcuts::install(&window, &view, &config);
    wire_update_check(app, &config, &paths);

    window.show_all();
    progress.hide();

    let target = start_url.unwrap_or_else(|| config.home_url.clone());
    view.load_uri(&target);
    view.grab_focus();

    window
}

/// Re-applies the requested size the first time the window is mapped.
fn apply_geometry_on_map(window: &gtk::ApplicationWindow, width: i32, height: i32) {
    let applied = Cell::new(false);
    window.connect_map_event(move |window, _| {
        if !applied.replace(true) && !window.is_maximized() {
            window.resize(width, height);
        }
        Propagation::Proceed
    });
}

/// Looks for a newer release in the background and reports the result through
/// the notification daemon. Never blocks startup, and stays quiet when there
/// is nothing to say.
fn wire_update_check(app: &gtk::Application, config: &Rc<Config>, paths: &Rc<Paths>) {
    let app = app.clone();
    crate::updates::check_in_background(config, paths, move |outcome| {
        let (title, body) = match outcome {
            crate::updates::Outcome::Installed { version } => (
                format!("{APP_NAME} {version} installed"),
                "Restart instaCache to start using it.".to_string(),
            ),
            crate::updates::Outcome::Available { version } => (
                format!("{APP_NAME} {version} is available"),
                "Run `instacache --update` in a terminal to install it.".to_string(),
            ),
            crate::updates::Outcome::UpToDate => return,
        };

        let notification = gio::Notification::new(&title);
        notification.set_body(Some(&body));
        notification.set_icon(&gio::ThemedIcon::new(ICON_NAME));
        app.send_notification(Some("instacache-update"), &notification);
        println!("instacache: {title} — {body}");
    });
}

/// Usable area of the primary monitor (the screen minus panels and docks).
/// Falls back to a common laptop resolution when GDK cannot report one, which
/// only happens before a display is attached.
fn primary_work_area() -> (i32, i32) {
    gdk::Display::default()
        .and_then(|display| display.primary_monitor().or_else(|| display.monitor(0)))
        .map(|monitor| {
            let area = monitor.workarea();
            (area.width(), area.height())
        })
        .unwrap_or((1920, 1080))
}

fn window_title(paths: &Paths) -> String {
    if paths.is_default_profile() {
        APP_NAME.to_string()
    } else {
        format!("{APP_NAME} — {}", paths.profile)
    }
}

fn build_loading_bar() -> gtk::ProgressBar {
    let provider = gtk::CssProvider::new();
    if provider.load_from_data(LOADING_BAR_CSS.as_bytes()).is_ok() {
        if let Some(screen) = gdk::Screen::default() {
            gtk::StyleContext::add_provider_for_screen(
                &screen,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    let progress = gtk::ProgressBar::new();
    progress.style_context().add_class("instacache-loading");
    progress.set_valign(gtk::Align::Start);
    progress.set_halign(gtk::Align::Fill);
    progress.set_show_text(false);
    // Keep `show_all()` from un-hiding it while nothing is loading.
    progress.set_no_show_all(true);
    progress
}

fn wire_loading_feedback(
    view: &webkit2gtk::WebView,
    window: &gtk::ApplicationWindow,
    progress: &gtk::ProgressBar,
    enabled: bool,
) {
    {
        let window = window.clone();
        view.connect_title_notify(move |view| {
            let title = view.title().unwrap_or_default();
            let title = title.trim();
            window.set_title(if title.is_empty() { APP_NAME } else { title });
        });
    }

    if enabled {
        crate::progress::install(view, progress);
    }
}

/// Replaces WebKit's default error page with a branded offline screen.
fn wire_error_page(view: &webkit2gtk::WebView) {
    view.connect_load_failed(move |view, _event, failing_uri, error| {
        // A load we cancelled ourselves (external-link routing) and ordinary
        // user-cancelled navigations are not failures worth reporting.
        if error.matches(NetworkError::Cancelled) || error.kind::<PolicyError>().is_some() {
            return false;
        }
        view.load_alternate_html(
            &errorpage::render(failing_uri, error.message()),
            failing_uri,
            None,
        );
        true
    });
}

/// WebKit runs the page in a separate process. When that process dies the view
/// is left showing a blank grey area with no way back, so it is reloaded
/// automatically — but only a few times, because a page that crashes on every
/// load would otherwise reload forever.
fn wire_crash_recovery(view: &webkit2gtk::WebView) {
    let attempts = Cell::new(0u32);
    let window_started = Cell::new(Instant::now());

    view.connect_web_process_terminated(move |view, reason| {
        // A termination we asked for ourselves is not a crash.
        if matches!(reason, WebProcessTerminationReason::TerminatedByApi) {
            return;
        }

        let uri = view.uri().map(Into::into).unwrap_or_else(String::new);

        if window_started.get().elapsed() > CRASH_WINDOW {
            window_started.set(Instant::now());
            attempts.set(0);
        }

        let attempt = attempts.get() + 1;
        attempts.set(attempt);

        if attempt > MAX_CRASH_RELOADS {
            eprintln!("instacache: rendering process died {attempt} times; giving up on reloading");
            view.load_alternate_html(
                &errorpage::render_crash(&uri, &describe(reason)),
                &uri,
                None,
            );
            return;
        }

        eprintln!(
            "instacache: rendering process {} — reloading ({attempt}/{MAX_CRASH_RELOADS})",
            describe(reason)
        );
        if uri.is_empty() {
            view.reload();
        } else {
            view.load_uri(&uri);
        }
    });
}

const MAX_CRASH_RELOADS: u32 = 3;
const CRASH_WINDOW: Duration = Duration::from_secs(120);

fn describe(reason: WebProcessTerminationReason) -> String {
    match reason {
        WebProcessTerminationReason::Crashed => "crashed".to_string(),
        WebProcessTerminationReason::ExceededMemoryLimit => "ran out of memory".to_string(),
        WebProcessTerminationReason::TerminatedByApi => {
            "was stopped by the application".to_string()
        }
        other => format!("stopped unexpectedly ({other:?})"),
    }
}

/// Bridges web notifications to the desktop notification daemon, and routes a
/// click on one back into the page.
fn wire_notifications(
    app: &gtk::Application,
    window: &gtk::ApplicationWindow,
    view: &webkit2gtk::WebView,
    config: &Rc<Config>,
) {
    let latest: Rc<RefCell<Option<webkit2gtk::Notification>>> = Rc::new(RefCell::new(None));

    {
        let window = window.clone();
        let latest = latest.clone();
        let action = gio::SimpleAction::new("present", None);
        action.connect_activate(move |_, _| {
            window.present();
            // Tells the page the notification was clicked, so Instagram opens
            // the relevant thread or post.
            //
            // The clone matters: `clicked()` runs page JavaScript, which can
            // post another notification, which writes to this same cell. With
            // a borrow still held that is a panic, and a panic in a GTK
            // callback aborts the process.
            let notification = latest.borrow().clone();
            if let Some(notification) = notification {
                notification.clicked();
            }
        });
        app.add_action(&action);
    }

    let notifications_enabled = config.notifications;

    view.connect_permission_request(move |_, request| {
        if request.is::<NotificationPermissionRequest>() {
            if notifications_enabled {
                request.allow();
            } else {
                request.deny();
            }
            return true;
        }

        // Instagram's Meta-hosted login needs cross-site storage access.
        if request.is::<WebsiteDataAccessPermissionRequest>() {
            request.allow();
            return true;
        }

        // Reading the clipboard backs "paste an image into a DM". WebKitGTK
        // 2.42 added a dedicated request type that this binding release does
        // not expose yet, so it is matched by GType name.
        if request.type_().name().contains("Clipboard") {
            request.allow();
            return true;
        }

        // Everything else — geolocation, camera, microphone, pointer lock — is
        // refused: the Instagram web app does not need it.
        request.deny();
        true
    });

    if !notifications_enabled {
        return;
    }

    let app = app.clone();
    view.connect_show_notification(move |_, notification| {
        let title = notification.title().unwrap_or_default();
        let desktop = gio::Notification::new(if title.is_empty() { APP_NAME } else { &title });
        if let Some(body) = notification.body() {
            desktop.set_body(Some(&body));
        }
        desktop.set_icon(&gio::ThemedIcon::new(ICON_NAME));
        desktop.set_default_action("app.present");

        latest.replace(Some(notification.clone()));
        app.send_notification(Some(&format!("instacache-{}", notification.id())), &desktop);
        // Tell WebKit we displayed it, so it does not draw its own.
        true
    });
}

/// Sends downloads to the XDG download directory under a non-colliding name,
/// then reports the outcome through the notification daemon.
fn wire_downloads(app: &gtk::Application, context: &webkit2gtk::WebContext) {
    let app = app.clone();
    context.connect_download_started(move |_, download| {
        download.connect_decide_destination(|download, suggested_name| {
            let directory = dirs::download_dir()
                .or_else(dirs::home_dir)
                .unwrap_or_else(|| PathBuf::from("."));
            if std::fs::create_dir_all(&directory).is_err() {
                return false;
            }
            let target = unique_destination(&directory, suggested_name);
            download.set_allow_overwrite(false);
            download.set_destination(&target.to_string_lossy());
            true
        });

        let app = app.clone();
        download.connect_finished(move |download| {
            let name = download
                .destination()
                .map(|d| file_name_of(d.as_str()))
                .unwrap_or_else(|| "File".to_string());
            let notification = gio::Notification::new("Download finished");
            notification.set_body(Some(&name));
            notification.set_icon(&gio::ThemedIcon::new(ICON_NAME));
            app.send_notification(None, &notification);
        });

        download.connect_failed(|_, error| {
            eprintln!("instacache: download failed: {error}");
        });
    });
}

fn wire_state_persistence(
    window: &gtk::ApplicationWindow,
    view: &webkit2gtk::WebView,
    config: &Rc<Config>,
    paths: &Rc<Paths>,
) {
    if !config.remember_window_state {
        return;
    }

    let save = {
        let window = window.clone();
        let view = view.clone();
        let paths = paths.clone();
        move || capture_state(&window, &view).save(&paths)
    };

    {
        let save = save.clone();
        window.connect_delete_event(move |_, _| {
            save();
            Propagation::Proceed
        });
    }

    // A desktop session ending, `systemctl --user stop`, or a plain `kill`
    // never produces a delete-event, so the same state is written from the
    // termination signals and the window is then closed the normal way.
    for signal in [SIGINT, SIGTERM] {
        let save = save.clone();
        let window = window.clone();
        let mut done = false;
        glib::unix_signal_add_local(signal, move || {
            if !done {
                done = true;
                save();
                window.close();
            }
            glib::ControlFlow::Break
        });
    }
}

/// libc constants, inlined to avoid a dependency for two integers. Both values
/// are the same on every Linux architecture instaCache targets.
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;

fn capture_state(window: &gtk::ApplicationWindow, view: &webkit2gtk::WebView) -> WindowState {
    let maximized = window.is_maximized();
    let (width, height) = window.size();
    // Position is only meaningful on X11; Wayland reports (0, 0).
    let (x, y) = window.position();
    let on_wayland = window.display().type_().name().contains("Wayland");

    WindowState {
        width,
        height,
        x: (!on_wayland && !maximized).then_some(x),
        y: (!on_wayland && !maximized).then_some(y),
        maximized,
        zoom: view.zoom_level(),
    }
}

/// Uses the installed hicolor icon when available and falls back to the copy
/// compiled into the binary, so a `cargo run` from a checkout still shows it.
fn install_icon(window: &gtk::ApplicationWindow) {
    gtk::Window::set_default_icon_name(ICON_NAME);

    let has_installed_icon =
        gtk::IconTheme::default().is_some_and(|theme| theme.has_icon(ICON_NAME));
    if has_installed_icon {
        return;
    }

    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
    if loader.write(EMBEDDED_ICON).is_err() || loader.close().is_err() {
        return;
    }
    if let Some(pixbuf) = loader.pixbuf() {
        window.set_icon(Some(&pixbuf));
        gtk::Window::set_default_icon(&pixbuf);
    }
}

/// `foo.jpg` -> `foo.jpg`, `foo (1).jpg`, `foo (2).jpg`, …
fn unique_destination(directory: &Path, suggested: &str) -> PathBuf {
    let name = sanitize_file_name(suggested);
    let candidate = directory.join(&name);
    if !candidate.exists() {
        return candidate;
    }

    let path = Path::new(&name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".to_string());
    let extension = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    for index in 1..10_000 {
        let candidate = directory.join(format!("{stem} ({index}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(name)
}

/// The suggested name comes from the remote server, so path separators and
/// traversal segments are stripped before it touches the filesystem.
fn sanitize_file_name(suggested: &str) -> String {
    let base = file_name_of(suggested);
    let cleaned: String = base
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | '\0'))
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
    if cleaned.is_empty() {
        "download".to_string()
    } else {
        cleaned.chars().take(200).collect()
    }
}

fn file_name_of(value: &str) -> String {
    value
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty() && *part != "." && *part != "..")
        .unwrap_or(value)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_path_traversal_from_suggested_names() {
        assert_eq!(sanitize_file_name("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_file_name("/tmp/a.jpg"), "a.jpg");
        assert_eq!(sanitize_file_name(""), "download");
        assert_eq!(sanitize_file_name("   "), "download");
        assert_eq!(sanitize_file_name(".bashrc"), "bashrc");
    }

    #[test]
    fn deduplicates_download_names() {
        let dir = std::env::temp_dir().join(format!("instacache-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("photo.jpg"), b"x").unwrap();

        assert_eq!(
            unique_destination(&dir, "photo.jpg"),
            dir.join("photo (1).jpg")
        );
        assert_eq!(unique_destination(&dir, "other.jpg"), dir.join("other.jpg"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
