//! Starting with the desktop session, the way a messenger does.
//!
//! One file in `~/.config/autostart`, which is what every desktop reads —
//! Plasma, GNOME, XFCE, Cinnamon and the rest all implement the same XDG
//! specification, so there is nothing per-desktop here and nothing to detect.
//!
//! **The file is the truth, not the setting.** `start_with_session` in
//! `config.json` only decides what the *first* run puts there; after that the
//! entry is read to know whether it is on. That is deliberate: the user can
//! turn instaCache off in their desktop's own "Startup Applications" panel,
//! and a setting that re-created the file at every launch would quietly undo
//! them. It is their session.
//!
//! The entry starts instaCache with `--background`, so logging in does not
//! throw a window at you — it leaves a tray icon and a page already warm.

use std::path::{Path, PathBuf};

use crate::paths::DEFAULT_PROFILE;
use crate::{sites, APP_NAME, PROGRAM_NAME};

/// `~/.config/autostart`, from the real XDG config home.
///
/// Not from `INSTACACHE_CONFIG_HOME`: that override exists so a portable or a
/// test install keeps its own settings out of the way, and redirecting the
/// session's autostart directory with it would write nothing the session ever
/// reads.
pub fn autostart_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(dirs::config_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("autostart")
}

/// The entry for a profile. A site added with `--add-site` gets its own, named
/// after it, so turning one on never turns another on.
pub fn entry_name(profile: &str) -> String {
    if profile == DEFAULT_PROFILE {
        format!("{PROGRAM_NAME}.desktop")
    } else {
        format!("{PROGRAM_NAME}-{profile}.desktop")
    }
}

pub fn entry_path(profile: &str) -> PathBuf {
    autostart_dir().join(entry_name(profile))
}

/// Whether instaCache starts with the session.
///
/// A desktop that disables an entry through its own panel writes
/// `Hidden=true` or `X-GNOME-Autostart-enabled=false` into the file rather
/// than deleting it, so the contents decide, not the file's existence.
pub fn is_enabled(profile: &str) -> bool {
    enabled_in(std::fs::read_to_string(entry_path(profile)).ok().as_deref())
}

fn enabled_in(entry: Option<&str>) -> bool {
    let Some(entry) = entry else {
        return false;
    };
    !entry.lines().any(|line| {
        let line = line.trim();
        line.eq_ignore_ascii_case("Hidden=true")
            || line.eq_ignore_ascii_case("X-GNOME-Autostart-enabled=false")
    })
}

/// Turns it on or off. Returns what the user should be told when it failed.
pub fn set(profile: &str, enabled: bool) -> Result<(), String> {
    let path = entry_path(profile);
    if !enabled {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            // Already off is the state that was asked for, not a failure.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("could not remove {}: {error}", path.display())),
        };
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    std::fs::write(&path, entry(profile, &sites::exec_command()))
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

/// What the first run of a profile does, once and only once.
///
/// The marker is what keeps it to once: without it, a user who turned
/// autostart off would find it back at the next launch, and a setting that
/// cannot be turned off is a bug however it is spelled.
pub fn apply_default_once(config_dir: &Path, profile: &str, wanted: bool) {
    let marker = config_dir.join("autostart-applied");
    if marker.exists() {
        return;
    }
    // Written first. A failure to create the entry must not leave the marker
    // missing, or the next launch would try again forever.
    let _ = std::fs::write(&marker, "");
    if !wanted {
        return;
    }
    if let Err(message) = set(profile, true) {
        eprintln!("instacache: {message}");
    }
}

/// The text of the entry. Separate from writing it so it can be tested without
/// touching the session.
pub fn entry(profile: &str, exec: &str) -> String {
    // `--background` rather than a plain launch: logging in should leave a
    // warm page behind a tray icon, not a window over whatever you opened the
    // machine to do.
    let arguments = if profile == DEFAULT_PROFILE {
        "--background".to_string()
    } else {
        format!("--profile {profile} --background")
    };
    let name = if profile == DEFAULT_PROFILE {
        APP_NAME.to_string()
    } else {
        profile.to_string()
    };
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name={name}\n\
         Comment=Keep {name} running so messages and calls arrive\n\
         Exec={exec} {arguments}\n\
         TryExec={exec}\n\
         Icon={icon}\n\
         Terminal=false\n\
         Categories=Network;\n\
         StartupNotify=false\n\
         X-GNOME-Autostart-enabled=true\n\
         Hidden=false\n",
        icon = sites::window_class(profile),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_profile_owns_the_plain_name() {
        assert_eq!(entry_name("default"), "instacache.desktop");
        assert_eq!(entry_name("xcache"), "instacache-xcache.desktop");
    }

    #[test]
    fn the_entry_starts_hidden_and_names_its_profile() {
        let default = entry("default", "/usr/bin/instacache");
        assert!(
            default.contains("Exec=/usr/bin/instacache --background"),
            "{default}"
        );
        assert!(
            !default.contains("--profile"),
            "the default profile is implied"
        );
        assert!(default.contains("Icon=instacache"));

        let site = entry("xcache", "/usr/bin/instacache");
        assert!(
            site.contains("Exec=/usr/bin/instacache --profile xcache --background"),
            "{site}"
        );
        assert!(site.contains("Icon=instacache-xcache"), "{site}");
    }

    #[test]
    fn a_desktop_that_switched_it_off_is_believed() {
        assert!(!enabled_in(None), "no file at all is off");
        assert!(enabled_in(Some(&entry("default", "/usr/bin/instacache"))));
        assert!(!enabled_in(Some("[Desktop Entry]\nHidden=true\n")));
        assert!(!enabled_in(Some(
            "[Desktop Entry]\nX-GNOME-Autostart-enabled=false\n"
        )));
        // Plasma writes it capitalised differently from GNOME.
        assert!(!enabled_in(Some("[Desktop Entry]\nhidden=TRUE\n")));
    }
}
