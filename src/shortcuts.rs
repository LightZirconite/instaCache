//! Keyboard navigation.
//!
//! Bound on the window rather than through a `GtkAccelGroup` so the web view
//! keeps receiving every other key event untouched — the page needs its own
//! shortcuts (`/` to search, arrow keys in Stories, …) to keep working.

use gtk::gdk;
use gtk::glib::Propagation;
use gtk::prelude::*;
use webkit2gtk::{WebInspectorExt, WebView, WebViewExt};

use crate::config::{Config, MAX_ZOOM, MIN_ZOOM};

const ZOOM_STEP: f64 = 1.1;

pub fn install(window: &gtk::ApplicationWindow, view: &WebView, config: &Config) {
    let view = view.clone();
    let home_url = config.home_url.clone();
    let developer_tools = config.developer_tools;

    window.connect_key_press_event(move |window, event| {
        let state = event.state();
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let alt = state.contains(gdk::ModifierType::MOD1_MASK);
        // Normalise so Ctrl+Shift+R arrives as `r`, not `R`.
        let key = event.keyval().to_lower();
        let key = *key;

        use gdk::keys::constants as k;

        match (ctrl, shift, alt, key) {
            // Reload. Shift bypasses the disk cache; plain reload is allowed to
            // serve from it, which is the whole point of GramCache.
            (true, true, false, key) if key == *k::r => view.reload_bypass_cache(),
            (true, false, false, key) if key == *k::r => view.reload(),
            (false, false, false, key) if key == *k::F5 => view.reload(),
            (false, true, false, key) if key == *k::F5 => view.reload_bypass_cache(),

            // History.
            (false, false, true, key) if key == *k::Left => view.go_back(),
            (false, false, true, key) if key == *k::Right => view.go_forward(),
            (false, false, false, key) if key == *k::Back => view.go_back(),
            (false, false, false, key) if key == *k::Forward => view.go_forward(),

            // Home.
            (true, false, false, key) if key == *k::h => view.load_uri(&home_url),
            (false, false, true, key) if key == *k::Home => view.load_uri(&home_url),

            // Zoom. `equal` covers Ctrl+= on layouts where `+` needs Shift.
            (true, _, false, key) if key == *k::plus || key == *k::equal || key == *k::KP_Add => {
                set_zoom(&view, view.zoom_level() * ZOOM_STEP)
            }
            (true, false, false, key) if key == *k::minus || key == *k::KP_Subtract => {
                set_zoom(&view, view.zoom_level() / ZOOM_STEP)
            }
            (true, false, false, key) if key == *k::_0 || key == *k::KP_0 => set_zoom(&view, 1.0),

            // Window.
            (true, false, false, key) if key == *k::q || key == *k::w => window.close(),
            (false, false, false, key) if key == *k::F11 => toggle_fullscreen(window),
            (false, false, false, key) if key == *k::Escape && is_fullscreen(window) => {
                window.unfullscreen()
            }

            // Web Inspector, only when the user opted in.
            (true, true, false, key) if developer_tools && key == *k::i => {
                if let Some(inspector) = view.inspector() {
                    inspector.show();
                }
            }
            (false, false, false, key) if developer_tools && key == *k::F12 => {
                if let Some(inspector) = view.inspector() {
                    inspector.show();
                }
            }

            // Not ours: hand it to the web view.
            _ => return Propagation::Proceed,
        }

        Propagation::Stop
    });
}

fn set_zoom(view: &WebView, level: f64) {
    view.set_zoom_level(level.clamp(MIN_ZOOM, MAX_ZOOM));
}

fn is_fullscreen(window: &gtk::ApplicationWindow) -> bool {
    window
        .window()
        .is_some_and(|w| w.state().contains(gdk::WindowState::FULLSCREEN))
}

fn toggle_fullscreen(window: &gtk::ApplicationWindow) {
    if is_fullscreen(window) {
        window.unfullscreen();
    } else {
        window.fullscreen();
    }
}
