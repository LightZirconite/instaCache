//! The one connection to the session bus.
//!
//! Every D-Bus conversation instaCache has — notifications, Do Not Disturb,
//! the unread badge — goes over this connection. A zbus connection is a
//! socket, a match list on the bus and a thread of its own, so opening one per
//! notification, as the notification library used to, cost all three for
//! every message that arrived and kept them for as long as the notification
//! stayed in the desktop's history.
//!
//! The connection is opened the first time something needs it and kept for
//! the life of the process. Task bars drop a badge when the connection that set
//! it goes away, which is one more reason for it to stay.

use std::sync::Mutex;

use zbus::blocking::Connection;

static SESSION: Mutex<Option<Connection>> = Mutex::new(None);

/// The session bus connection, opened on first use. `None` when there is no
/// session bus, which costs the features that need one and nothing else.
pub fn session() -> Option<Connection> {
    let mut held = SESSION.lock().ok()?;
    if let Some(connection) = held.as_ref() {
        return Some(connection.clone());
    }
    match Connection::session() {
        Ok(connection) => {
            *held = Some(connection.clone());
            Some(connection)
        }
        Err(error) => {
            eprintln!("instacache: no session bus: {error}");
            None
        }
    }
}

/// Drops the connection, so the next [`session`] opens a new one. For when the
/// bus went away under it, which in practice means the session is ending.
pub fn forget() {
    if let Ok(mut held) = SESSION.lock() {
        *held = None;
    }
}
