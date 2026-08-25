//! Turning a site into its own application.
//!
//! A named profile already gives a site its own session, cache and window;
//! what it did not give it was a way in. This writes the two files that make
//! one appear in the desktop's application menu: the profile's `config.json`,
//! and a `.desktop` entry that launches instaCache against it.
//!
//! Nothing here is Instagram-specific. The result is the same binary, the same
//! engine and the same window, pointed somewhere else — which is why it is
//! worth doing at all rather than shipping a second application.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::paths::{sanitize_profile, Paths};
use crate::{ICON_NAME, PROGRAM_NAME};

/// Where a site's menu entry goes. User-level on purpose: a site somebody
/// added is their data, not part of the installation, and it must survive an
/// upgrade that rewrites the shared files.
pub fn applications_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("applications")
}

pub fn desktop_file(profile: &str) -> PathBuf {
    applications_dir().join(format!("{PROGRAM_NAME}-{profile}.desktop"))
}

/// What the window reports as its Wayland `app_id` and X11 `WM_CLASS`.
///
/// It has to differ per profile or the desktop groups every site under one
/// icon, and it has to match the entry's `StartupWMClass` or the window shows
/// up as a second, unnamed item next to the launcher. Those two constraints
/// are the whole reason this is a function rather than a constant.
pub fn window_class(profile: &str) -> String {
    if profile == crate::paths::DEFAULT_PROFILE {
        PROGRAM_NAME.to_string()
    } else {
        format!("{PROGRAM_NAME}-{profile}")
    }
}

/// The host a URL is for, as an allow-list entry: `https://www.x.com/i/` gives
/// `x.com`.
///
/// `www.` is dropped because the entry already matches sub-domains, so keeping
/// it would allow *less* than the user asked for — `www.x.com` would be in and
/// the bare `x.com` they typed would not.
pub fn default_domain(url: &str) -> Option<String> {
    let host = crate::urls::host_of(url)?;
    Some(host.strip_prefix("www.").unwrap_or(&host).to_string())
}

/// The text of the menu entry. Separate from writing it so it can be tested
/// without touching the filesystem.
pub fn desktop_entry(name: &str, profile: &str, exec: &str) -> String {
    // Anything from the user is quoted out of the Exec line by construction:
    // the profile name is already restricted to `[a-z0-9_-]`, and the display
    // name never reaches Exec at all.
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name={name}\n\
         GenericName=Web Application\n\
         Comment=Opened in its own instaCache window, with its own session\n\
         Exec={exec} --profile {profile} %u\n\
         TryExec={exec}\n\
         Icon={ICON_NAME}\n\
         Terminal=false\n\
         Categories=Network;\n\
         StartupNotify=true\n\
         StartupWMClass={class}\n",
        class = window_class(profile),
    )
}

/// The command a menu entry should run. The running binary's own path, so an
/// entry keeps working when instaCache is installed somewhere that is not on
/// `PATH`; the bare name is the fallback for the odd platform that cannot
/// report it.
fn exec_command() -> String {
    std::env::current_exe()
        .ok()
        .filter(|path| path.is_absolute())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| PROGRAM_NAME.to_string())
}

pub struct Added {
    pub profile: String,
    pub config: PathBuf,
    pub entry: PathBuf,
}

/// Creates the profile and its menu entry.
///
/// An existing profile's `config.json` is never overwritten: somebody who has
/// already tuned a site should not lose it by re-running this, so only the
/// menu entry is refreshed.
pub fn add(name: &str, url: &str, domains: Option<&str>) -> Result<Added, String> {
    if !crate::urls::is_http(url) {
        return Err(format!("`{url}` is not an http or https URL"));
    }

    let profile = sanitize_profile(name);
    if profile == crate::paths::DEFAULT_PROFILE {
        return Err(format!(
            "`{name}` is reserved; the default profile is instaCache itself"
        ));
    }

    let paths = Paths::for_profile(&profile);
    paths.ensure().map_err(|err| err.to_string())?;

    let config_file = paths.config_file();
    if !config_file.exists() {
        let allowed: Vec<String> = match domains {
            Some(list) => list
                .split(',')
                .map(|d| d.trim().trim_start_matches('.').to_ascii_lowercase())
                .filter(|d| !d.is_empty())
                .collect(),
            None => default_domain(url).into_iter().collect(),
        };
        if allowed.is_empty() {
            return Err(format!("could not work out a host from `{url}`"));
        }

        let config = Config {
            home_url: url.to_string(),
            internal_domains: allowed,
            ..Config::default()
        };
        crate::config::write(&config_file, &config).map_err(|err| err.to_string())?;
    }

    let entry = desktop_file(&profile);
    if let Some(parent) = entry.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(
        &entry,
        desktop_entry(name.trim(), &profile, &exec_command()),
    )
    .map_err(|err| err.to_string())?;
    refresh_desktop_database(&applications_dir());

    Ok(Added {
        profile,
        config: config_file,
        entry,
    })
}

/// Removes the menu entry. The profile's session and cache are deliberately
/// left alone — deleting an account's data is not what "remove from the menu"
/// means, and `--clear-session` already exists for that.
pub fn remove(name: &str) -> Result<PathBuf, String> {
    let profile = sanitize_profile(name);
    let entry = desktop_file(&profile);
    match std::fs::remove_file(&entry) {
        Ok(()) => {
            refresh_desktop_database(&applications_dir());
            Ok(entry)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("no site named `{profile}` is in the menu"))
        }
        Err(err) => Err(err.to_string()),
    }
}

/// Every site currently in the menu.
pub fn list() -> Vec<String> {
    let prefix = format!("{PROGRAM_NAME}-");
    let Ok(entries) = std::fs::read_dir(applications_dir()) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|file| {
            file.strip_suffix(".desktop")
                .and_then(|stem| stem.strip_prefix(&prefix))
                .map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

/// Best-effort: a stale cache only means the entry appears at the next login.
fn refresh_desktop_database(dir: &Path) {
    let _ = std::process::Command::new("update-desktop-database")
        .arg(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_profile_keeps_the_shared_window_class() {
        // It has to stay in step with StartupWMClass in instacache.desktop.
        assert_eq!(window_class("default"), PROGRAM_NAME);
    }

    #[test]
    fn every_site_gets_its_own_window_class() {
        assert_eq!(window_class("x"), "instacache-x");
        assert_ne!(window_class("x"), window_class("y"));
    }

    #[test]
    fn a_host_becomes_an_allow_list_entry() {
        assert_eq!(default_domain("https://x.com/"), Some("x.com".into()));
        // Keeping `www.` would allow less than was asked for, since the entry
        // already matches sub-domains.
        assert_eq!(
            default_domain("https://www.x.com/i/flow"),
            Some("x.com".into())
        );
        assert_eq!(default_domain("not a url"), None);
    }

    #[test]
    fn the_entry_launches_the_right_profile_and_names_its_class() {
        let entry = desktop_entry("X", "x", "/usr/bin/instacache");
        assert!(entry.contains("Exec=/usr/bin/instacache --profile x %u"));
        assert!(entry.contains("StartupWMClass=instacache-x"));
        assert!(entry.contains("Name=X"));
        assert!(entry.starts_with("[Desktop Entry]\n"));
        // The version the packaging validators on old runners accept.
        assert!(entry.contains("Version=1.0"));
    }

    #[test]
    fn a_site_cannot_hijack_the_main_window() {
        assert!(add("default", "https://x.com/", None).is_err());
        assert!(add("x", "javascript:alert(1)", None).is_err());
        assert!(add("x", "file:///etc/passwd", None).is_err());
    }
}
