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
//!
//! On Windows the count is an overlay on the window's task bar button,
//! through `ITaskbarList3`, drawn by Qt — the one place outside `sites.rs`
//! where instaCache needs C++. Windows forgets an overlay whenever it makes
//! the button again, which it does each time a window closed to the tray is
//! shown, and says so with a `TaskbarButtonCreated` message; a native event
//! filter hears it and puts the count back.

#[cfg(not(windows))]
use std::collections::HashMap;
#[cfg(not(windows))]
use std::sync::mpsc::{channel, Sender};

use cpp::cpp;
#[cfg(not(windows))]
use zbus::zvariant::Value;

#[cfg(not(windows))]
const INTERFACE: &str = "com.canonical.Unity.LauncherEntry";

/// The unread count on this profile's icon.
#[cfg(not(windows))]
pub struct Badge(Sender<u32>);

#[cfg(not(windows))]
impl Badge {
    /// For the desktop entry `<desktop_id>.desktop`.
    pub fn new(desktop_id: String) -> Self {
        Self(spawn(desktop_id))
    }

    /// `0` hides the badge.
    pub fn show(&self, count: u32) {
        let _ = self.0.send(count);
    }

    /// A badge whose counts go to `sender` rather than to the bus.
    #[cfg(test)]
    pub(crate) fn sending_to(sender: Sender<u32>) -> Self {
        Self(sender)
    }
}

/// Starts the badge thread for the desktop entry `<desktop_id>.desktop` and
/// returns where to send counts. `0` hides the badge.
#[cfg(not(windows))]
fn spawn(desktop_id: String) -> Sender<u32> {
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
#[cfg(not(windows))]
fn object_path(desktop_id: &str) -> String {
    let element: String = desktop_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("/io/github/lightzirconite/instaCache/{element}")
}

cpp! {{
    #ifdef _WIN32
    #include <windows.h>
    #include <shobjidl.h>
    #include <QtCore/QAbstractNativeEventFilter>
    #include <QtCore/QCoreApplication>
    #include <QtGui/QColor>
    #include <QtGui/QFont>
    #include <QtGui/QGuiApplication>
    #include <QtGui/QImage>
    #include <QtGui/QPainter>
    #include <QtGui/QWindow>

    namespace instacache_badge {
        static unsigned int count = 0;
        static ITaskbarList3 *taskbar = nullptr;
        static UINT button_created = 0;

        // Draws the count on a red disc, as Discord and Telegram do, and
        // lays it over every window's button. Runs on the GUI thread only.
        static void apply() {
            if (!taskbar) {
                CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
                if (FAILED(CoCreateInstance(CLSID_TaskbarList, nullptr, CLSCTX_INPROC_SERVER,
                                            IID_ITaskbarList3, reinterpret_cast<void **>(&taskbar)))) {
                    taskbar = nullptr;
                    return;
                }
                if (FAILED(taskbar->HrInit())) {
                    taskbar->Release();
                    taskbar = nullptr;
                    return;
                }
            }
            HICON icon = nullptr;
            if (count > 0) {
                QImage image(32, 32, QImage::Format_ARGB32_Premultiplied);
                image.fill(Qt::transparent);
                QPainter painter(&image);
                painter.setRenderHint(QPainter::Antialiasing);
                painter.setPen(Qt::NoPen);
                painter.setBrush(QColor(0xed, 0x1c, 0x4b));
                painter.drawEllipse(0, 0, 32, 32);
                QFont font = painter.font();
                font.setBold(true);
                font.setPixelSize(count > 9 ? 15 : 21);
                painter.setFont(font);
                painter.setPen(Qt::white);
                painter.drawText(QRect(0, 0, 32, 32), Qt::AlignCenter,
                                 count > 99 ? QStringLiteral("99+") : QString::number(count));
                painter.end();
                icon = image.toHICON();
            }
            const auto windows = QGuiApplication::topLevelWindows();
            for (QWindow *window : windows) {
                if (window->type() != Qt::Window || !window->handle())
                    continue;
                taskbar->SetOverlayIcon(reinterpret_cast<HWND>(window->winId()), icon,
                                        reinterpret_cast<const wchar_t *>(
                                            QStringLiteral("Unread messages").utf16()));
            }
            if (icon)
                DestroyIcon(icon);
        }

        struct ButtonWatcher : QAbstractNativeEventFilter {
            bool nativeEventFilter(const QByteArray &type, void *message, qintptr *) override {
                if (type == "windows_generic_MSG" && button_created != 0
                    && static_cast<MSG *>(message)->message == button_created)
                    apply();
                return false;
            }
        };
    }
    #endif
}}

/// The unread count on the window's task bar button.
#[cfg(windows)]
pub struct Badge;

#[cfg(windows)]
impl Badge {
    /// Starts listening for the task bar making a button. Must be called on
    /// the GUI thread, once the application object exists.
    pub fn new(_desktop_id: String) -> Self {
        cpp!(unsafe [] {
            #ifdef _WIN32
            instacache_badge::button_created = RegisterWindowMessageA("TaskbarButtonCreated");
            QCoreApplication::instance()->installNativeEventFilter(new instacache_badge::ButtonWatcher());
            #endif
        });
        Badge
    }

    /// `0` hides the badge. On the GUI thread, as everything the scene calls.
    pub fn show(&self, count: u32) {
        cpp!(unsafe [count as "unsigned int"] {
            #ifdef _WIN32
            instacache_badge::count = count;
            instacache_badge::apply();
            #else
            (void)count;
            #endif
        });
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn the_object_path_is_valid_for_any_profile() {
        assert_eq!(
            object_path("instacache-x.site"),
            "/io/github/lightzirconite/instaCache/instacache_x_site"
        );
        assert!(zbus::zvariant::ObjectPath::try_from(object_path("instacache-a.b-c")).is_ok());
    }
}
