//! The unread count on the task bar icon.
//!
//! There is no freedesktop standard for this. What task bars actually read is
//! Unity's `com.canonical.Unity.LauncherEntry` signal, which outlived Unity:
//! KDE Plasma's task manager, Dash to Dock and Plank all honour it. The signal
//! names the application by its desktop entry, so it has to carry the same
//! name the window announces — `sites::window_class` — or the count lands on
//! nobody's icon.
//!
//! It is sent over the shared session connection in `bus.rs`, which stays
//! open for as long as the app runs: some task bars drop an application's
//! badge when the connection that set it goes away.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};

use zbus::zvariant::Value;

const INTERFACE: &str = "com.canonical.Unity.LauncherEntry";

/// Starts the badge thread for the desktop entry `<desktop_id>.desktop` and
/// returns where to send counts. `0` hides the badge.
pub fn spawn(desktop_id: String) -> Sender<u32> {
    let (sender, counts) = channel::<u32>();
    std::thread::spawn(move || {
        let uri = format!("application://{desktop_id}.desktop");
        let path = object_path(&desktop_id);
        while let Ok(mut count) = counts.recv() {
            // A burst of title changes only needs its last count shown.
            while let Ok(newer) = counts.try_recv() {
                count = newer;
            }
            // Asked for at the first count, not at startup, and again after
            // the bus went away.
            let Some(connection) = crate::bus::session() else {
                continue;
            };
            let mut properties: HashMap<&str, Value> = HashMap::new();
            properties.insert("count", Value::from(i64::from(count)));
            properties.insert("count-visible", Value::from(count > 0));
            if let Err(error) = connection.emit_signal(
                None::<&str>,
                path.as_str(),
                INTERFACE,
                "Update",
                &(uri.as_str(), properties),
            ) {
                eprintln!("instacache: could not update the unread badge: {error}");
            }
        }
    });
    sender
}

/// A D-Bus object path allows letters, digits and underscores in each
/// element, and a profile name may also contain `-` and `.`.
fn object_path(desktop_id: &str) -> String {
    let element: String = desktop_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("/io/github/lightzirconite/instaCache/{element}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_object_path_is_valid_for_any_profile() {
        assert_eq!(
            object_path("instacache-x.site"),
            "/io/github/lightzirconite/instaCache/instacache_x_site"
        );
        assert!(zbus::zvariant::ObjectPath::try_from(object_path("instacache-a.b-c")).is_ok());
    }
}
