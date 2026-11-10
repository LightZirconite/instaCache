//! XDG-compliant location resolution, with optional named profiles.
//!
//! On Windows the same three roots live in the user's known folders: the
//! settings in `%APPDATA%\instacache`, which may roam with the account, and
//! the session and the cache in `%LOCALAPPDATA%\instacache\{data,cache}`,
//! which must not — a roaming profile that carries a gigabyte of cache to
//! every machine is worse than no cache.
//!
//! Everything instaCache persists lives under three roots:
//!   * data  — cookies, localStorage, IndexedDB, service workers (the session)
//!   * cache — the aggressive on-disk HTTP/resource cache (safe to delete)
//!   * config — user-editable settings and window geometry
//!
//! Each root can be overridden with `INSTACACHE_{DATA,CACHE,CONFIG}_HOME`, which
//! makes fully portable installs possible without touching the code.

use std::path::{Path, PathBuf};

pub const DEFAULT_PROFILE: &str = "default";

#[derive(Debug, Clone)]
pub struct Paths {
    pub profile: String,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub config: PathBuf,
}

impl Paths {
    pub fn for_profile(profile: &str) -> Self {
        let profile = sanitize_profile(profile);

        #[cfg(not(windows))]
        let (data, cache, config) = (
            root(
                "INSTACACHE_DATA_HOME",
                dirs::data_dir(),
                ".local/share",
                None,
            ),
            root("INSTACACHE_CACHE_HOME", dirs::cache_dir(), ".cache", None),
            root(
                "INSTACACHE_CONFIG_HOME",
                dirs::config_dir(),
                ".config",
                None,
            ),
        );
        #[cfg(windows)]
        let (data, cache, config) = (
            root(
                "INSTACACHE_DATA_HOME",
                dirs::data_local_dir(),
                "AppData/Local",
                Some("data"),
            ),
            root(
                "INSTACACHE_CACHE_HOME",
                dirs::cache_dir(),
                "AppData/Local",
                Some("cache"),
            ),
            root(
                "INSTACACHE_CONFIG_HOME",
                dirs::config_dir(),
                "AppData/Roaming",
                None,
            ),
        );

        Self {
            data: scoped(data, &profile),
            cache: scoped(cache, &profile),
            config: scoped(config, &profile),
            profile,
        }
    }

    /// Creates every root. Called once before the WebKit context is built,
    /// because WebKit will not create missing parents for some sub-stores.
    pub fn ensure(&self) -> std::io::Result<()> {
        for dir in [&self.data, &self.cache, &self.config] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn config_file(&self) -> PathBuf {
        self.config.join("config.json")
    }

    pub fn state_file(&self) -> PathBuf {
        self.config.join("window-state.json")
    }

    /// User JavaScript, run on every page once it has loaded. The nearest
    /// thing to an extension this app has; Qt WebEngine implements no
    /// extension API at all.
    pub fn user_script(&self) -> PathBuf {
        self.config.join("user.js")
    }

    pub fn user_stylesheet(&self) -> PathBuf {
        self.config.join("user.css")
    }

    /// Cookie jar. Kept in `data` (not `cache`) so clearing the cache never
    /// logs the user out.
    pub fn cookie_jar(&self) -> PathBuf {
        self.data.join("cookies.sqlite")
    }

    /// Present once the user has been told that closing the window leaves
    /// instaCache running. The notice is worth giving once, not every time —
    /// but once more when a tray icon changes what it has to say.
    pub fn background_notice_marker(&self, tray: bool) -> PathBuf {
        if tray {
            self.config.join("tray-notice-shown")
        } else {
            self.config.join("background-notice-shown")
        }
    }

    /// The version that last ran this profile, so the next one can say it
    /// was updated.
    pub fn version_file(&self) -> PathBuf {
        self.config.join("last-version")
    }

    pub fn is_default_profile(&self) -> bool {
        self.profile == DEFAULT_PROFILE
    }
}

/// `<base>/instacache`, or `<base>/instacache/<below>` where two roots share
/// one known folder, as the session and the cache do on Windows. An override
/// from the environment is taken exactly as given.
fn root(env_var: &str, base: Option<PathBuf>, home_fallback: &str, below: Option<&str>) -> PathBuf {
    if let Some(custom) = std::env::var_os(env_var).filter(|v| !v.is_empty()) {
        return PathBuf::from(custom);
    }
    let base = base.unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(home_fallback)
    });
    let root = base.join("instacache");
    match below {
        Some(below) => root.join(below),
        None => root,
    }
}

fn scoped(base: PathBuf, profile: &str) -> PathBuf {
    if profile == DEFAULT_PROFILE {
        base
    } else {
        base.join("profiles").join(profile)
    }
}

/// Profile names end up as directory names and as a D-Bus application-id
/// suffix, so restrict them to a conservative, always-valid character set.
pub fn sanitize_profile(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('-').to_string();
    if cleaned.is_empty() {
        DEFAULT_PROFILE.to_string()
    } else {
        cleaned
    }
}

/// Turns a profile name into a valid D-Bus name segment (must not start with a
/// digit, and `-` is not allowed in D-Bus names).
pub fn dbus_segment(profile: &str) -> String {
    let mut out = String::with_capacity(profile.len() + 1);
    for c in profile.chars() {
        out.push(if c.is_ascii_alphanumeric() { c } else { '_' });
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, 'p');
    }
    out
}

/// Recursively removes `dir` but keeps the directory itself, so WebKit can
/// reuse the handle on the next launch.
pub fn purge(dir: &Path) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Profile names reach a D-Bus name through `dbus_segment`, which must
    /// never produce an invalid one. This lived in main.rs while a
    /// GtkApplication owned the name; the check outlived it.
    #[test]
    fn profile_names_become_valid_dbus_segments() {
        assert_eq!(super::dbus_segment("default"), "default");
        assert_eq!(super::dbus_segment("work-2"), "work_2");
        assert_eq!(super::dbus_segment("2fa"), "p2fa");
    }

    use super::*;

    #[test]
    fn profile_names_are_sanitized() {
        assert_eq!(sanitize_profile("Work"), "work");
        assert_eq!(sanitize_profile("../../etc"), "etc");
        assert_eq!(sanitize_profile("  "), DEFAULT_PROFILE);
        assert_eq!(sanitize_profile("a b/c"), "a-b-c");
    }

    #[test]
    fn dbus_segments_are_valid() {
        assert_eq!(dbus_segment("work-2"), "work_2");
        assert_eq!(dbus_segment("2nd"), "p2nd");
    }

    #[test]
    fn default_profile_is_not_nested() {
        let p = Paths::for_profile("default");
        assert!(p.is_default_profile());
        assert!(!p.data.to_string_lossy().contains("profiles"));
    }

    #[test]
    fn the_three_roots_never_share_a_directory() {
        let p = Paths::for_profile("work");
        assert_ne!(p.data, p.cache);
        assert_ne!(p.data, p.config);
        assert_ne!(p.cache, p.config);
        assert!(!p.cache.starts_with(&p.data) && !p.data.starts_with(&p.cache));
    }
}
