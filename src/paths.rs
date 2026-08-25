//! XDG-compliant location resolution, with optional named profiles.
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

        let data = root("INSTACACHE_DATA_HOME", dirs::data_dir(), ".local/share");
        let cache = root("INSTACACHE_CACHE_HOME", dirs::cache_dir(), ".cache");
        let config = root("INSTACACHE_CONFIG_HOME", dirs::config_dir(), ".config");

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

    pub fn is_default_profile(&self) -> bool {
        self.profile == DEFAULT_PROFILE
    }
}

fn root(env_var: &str, xdg: Option<PathBuf>, home_fallback: &str) -> PathBuf {
    if let Some(custom) = std::env::var_os(env_var).filter(|v| !v.is_empty()) {
        return PathBuf::from(custom);
    }
    let base = xdg.unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(home_fallback)
    });
    base.join("instacache")
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
fn sanitize_profile(raw: &str) -> String {
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
}
