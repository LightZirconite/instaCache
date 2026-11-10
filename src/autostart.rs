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
//!
//! Windows has no such directory. Its equivalent is a value under the user's
//! `Run` key, and the rule is the same one: Task Manager's Startup tab turns
//! an entry off by writing to `StartupApproved\Run` rather than deleting it,
//! so that is read too, and believed.

use std::path::Path;
#[cfg(not(windows))]
use std::path::PathBuf;

use crate::paths::DEFAULT_PROFILE;
#[cfg(not(windows))]
use crate::PROGRAM_NAME;
use crate::{sites, APP_NAME};

/// `~/.config/autostart`, from the real XDG config home.
///
/// Not from `INSTACACHE_CONFIG_HOME`: that override exists so a portable or a
/// test install keeps its own settings out of the way, and redirecting the
/// session's autostart directory with it would write nothing the session ever
/// reads.
#[cfg(not(windows))]
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
#[cfg(not(windows))]
pub fn entry_name(profile: &str) -> String {
    if profile == DEFAULT_PROFILE {
        format!("{PROGRAM_NAME}.desktop")
    } else {
        format!("{PROGRAM_NAME}-{profile}.desktop")
    }
}

#[cfg(not(windows))]
pub fn entry_path(profile: &str) -> PathBuf {
    autostart_dir().join(entry_name(profile))
}

/// Whether instaCache starts with the session.
///
/// A desktop that disables an entry through its own panel writes
/// `Hidden=true` or `X-GNOME-Autostart-enabled=false` into the file rather
/// than deleting it, so the contents decide, not the file's existence.
#[cfg(not(windows))]
pub fn is_enabled(profile: &str) -> bool {
    enabled_in(std::fs::read_to_string(entry_path(profile)).ok().as_deref())
}

#[cfg(not(windows))]
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
#[cfg(not(windows))]
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
#[cfg(not(windows))]
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

/// Where Windows reads what to start at login, and where Task Manager writes
/// what it was told not to start.
#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const APPROVED_KEY: &str =
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

/// The name of a profile's value under the `Run` key, which is also what
/// Task Manager shows when it cannot read the executable's own description.
#[cfg(any(windows, test))]
fn value_name(profile: &str) -> String {
    if profile == DEFAULT_PROFILE {
        APP_NAME.to_string()
    } else {
        format!("{APP_NAME} ({profile})")
    }
}

/// The command line Windows runs at login.
#[cfg(any(windows, test))]
fn run_command(profile: &str, exec: &str) -> String {
    if profile == DEFAULT_PROFILE {
        format!("\"{exec}\" --background")
    } else {
        format!("\"{exec}\" --profile {profile} --background")
    }
}

/// Whether what `reg query` printed for a `StartupApproved` value says the
/// entry is switched off: its first byte is odd — 3 or 7 — when it is.
#[cfg(any(windows, test))]
fn switched_off(query: &str, name: &str) -> bool {
    query
        .lines()
        .find(|line| line.trim_start().starts_with(name) && line.contains("REG_BINARY"))
        .and_then(|line| line.split_whitespace().last())
        .and_then(|data| u8::from_str_radix(data.get(..2)?, 16).ok())
        .is_some_and(|first| first & 1 == 1)
}

/// `reg.exe` with these arguments, its output when it succeeded.
#[cfg(windows)]
fn reg(arguments: &[&str]) -> Option<String> {
    let mut command = std::process::Command::new("reg.exe");
    command.args(arguments);
    crate::windows::no_console(&mut command);
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Whether instaCache starts with the session.
#[cfg(windows)]
pub fn is_enabled(profile: &str) -> bool {
    let name = value_name(profile);
    if reg(&["query", RUN_KEY, "/v", &name]).is_none() {
        return false;
    }
    !reg(&["query", APPROVED_KEY, "/v", &name]).is_some_and(|query| switched_off(&query, &name))
}

/// Turns it on or off. Returns what the user should be told when it failed.
///
/// Turning it on also clears what Task Manager recorded, because the user
/// has just asked for it, here, by name.
#[cfg(windows)]
pub fn set(profile: &str, enabled: bool) -> Result<(), String> {
    let name = value_name(profile);
    if !enabled {
        // A value that is not there is the state that was asked for.
        let _ = reg(&["delete", RUN_KEY, "/v", &name, "/f"]);
        return if reg(&["query", RUN_KEY, "/v", &name]).is_none() {
            Ok(())
        } else {
            Err(format!("could not remove {name} from {RUN_KEY}"))
        };
    }
    let command = run_command(profile, &sites::exec_command());
    reg(&[
        "add", RUN_KEY, "/v", &name, "/t", "REG_SZ", "/d", &command, "/f",
    ])
    .ok_or_else(|| format!("could not write {name} into {RUN_KEY}"))?;
    let _ = reg(&["delete", APPROVED_KEY, "/v", &name, "/f"]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_starts_the_right_profile_hidden() {
        assert_eq!(value_name("default"), "instaCache");
        assert_eq!(value_name("xcache"), "instaCache (xcache)");
        assert_eq!(
            run_command("default", r"C:\Apps\instacache.exe"),
            r#""C:\Apps\instacache.exe" --background"#
        );
        assert_eq!(
            run_command("xcache", r"C:\Apps\instacache.exe"),
            r#""C:\Apps\instacache.exe" --profile xcache --background"#
        );
    }

    #[test]
    fn task_managers_switch_is_believed() {
        let query = |data: &str| {
            format!(
                "\r\nHKEY_CURRENT_USER\\Software\\...\\StartupApproved\\Run\r\n    instaCache    REG_BINARY    {data}\r\n"
            )
        };
        assert!(switched_off(
            &query("030000000000000000000000"),
            "instaCache"
        ));
        assert!(switched_off(
            &query("070000001234567890ABCDEF"),
            "instaCache"
        ));
        assert!(!switched_off(
            &query("020000000000000000000000"),
            "instaCache"
        ));
        assert!(!switched_off(
            &query("060000000000000000000000"),
            "instaCache"
        ));
        assert!(!switched_off("", "instaCache"), "no record is on");
    }

    #[cfg(not(windows))]
    #[test]
    fn the_default_profile_owns_the_plain_name() {
        assert_eq!(entry_name("default"), "instacache.desktop");
        assert_eq!(entry_name("xcache"), "instacache-xcache.desktop");
    }

    #[cfg(not(windows))]
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

    #[cfg(not(windows))]
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
