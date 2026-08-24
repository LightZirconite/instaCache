//! Self-updating.
//!
//! instaCache is installed by unpacking an archive, not by a package manager,
//! so nothing else will ever update it. This module asks GitHub whether a
//! newer release exists and, when the install is one this user can write to,
//! runs the same installer that put the app there in the first place.
//!
//! The check runs at most once per [`Config::update_check_interval_hours`] and
//! never blocks the interface: the network call happens on a plain thread and
//! the answer is handed back to the GTK main loop.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::paths::Paths;
use crate::VERSION;

/// The repository releases are published from.
pub const REPO: &str = "LightZirconite/instaCache";

/// Give up rather than leave a request hanging for a whole session.
const NETWORK_TIMEOUT_SECS: u32 = 10;

/// What a finished check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A newer release exists and was installed. The app must restart to run it.
    Installed { version: String },
    /// A newer release exists but installing it here is not possible, usually
    /// a system-wide install that would need root.
    Available { version: String },
    /// Nothing to do, or the check could not complete.
    UpToDate,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct CheckRecord {
    /// Seconds since the Unix epoch.
    last_check: u64,
    /// The newest version seen, for reference when reading the file by hand.
    last_seen_version: String,
}

/// Starts a check if one is due. `finished` is called on the main thread,
/// exactly once, unless no check was started.
pub fn check_in_background<F>(config: &Config, paths: &Paths, finished: F)
where
    F: Fn(Outcome) + 'static,
{
    if !config.auto_update {
        return;
    }
    if !check_is_due(paths, config.update_check_interval_hours) {
        return;
    }
    // Record the attempt before making it, so a failing network or a crash
    // cannot turn into a request on every single launch.
    record_check(paths, "");

    let paths = paths.clone();
    let auto_install = config.auto_update;

    let (sender, receiver) = std::sync::mpsc::channel::<Outcome>();
    std::thread::spawn(move || {
        let outcome = run_check(&paths, auto_install);
        // The receiver lives until the idle callback runs; a send failure only
        // means the app is shutting down.
        let _ = sender.send(outcome);
    });

    // Poll the channel from the main loop so `finished` runs where GTK calls
    // are legal.
    gtk::glib::timeout_add_local(Duration::from_millis(250), move || {
        match receiver.try_recv() {
            Ok(outcome) => {
                finished(outcome);
                gtk::glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => gtk::glib::ControlFlow::Break,
        }
    });
}

fn run_check(paths: &Paths, auto_install: bool) -> Outcome {
    let Some(latest) = fetch_latest_version() else {
        return Outcome::UpToDate;
    };

    if !is_newer(&latest, VERSION) {
        record_check(paths, &latest);
        return Outcome::UpToDate;
    }
    record_check(paths, &latest);

    if !auto_install {
        return Outcome::Available { version: latest };
    }

    match install_update() {
        Ok(()) => Outcome::Installed { version: latest },
        Err(reason) => {
            eprintln!("instacache: could not install the update: {reason}");
            Outcome::Available { version: latest }
        }
    }
}

/// Reads the newest release tag from the GitHub API. Uses whichever downloader
/// is installed rather than linking an HTTP stack into a 500 KB binary.
fn fetch_latest_version() -> Option<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let timeout = NETWORK_TIMEOUT_SECS.to_string();

    let body = if which("curl").is_some() {
        Command::new("curl")
            .args(["-fsSL", "--max-time", &timeout, &url])
            .output()
            .ok()?
    } else if which("wget").is_some() {
        Command::new("wget")
            .args(["-qO-", "--timeout", &timeout, &url])
            .output()
            .ok()?
    } else {
        return None;
    };

    if !body.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&body.stdout).ok()?;
    let tag = json.get("tag_name")?.as_str()?;
    Some(tag.trim_start_matches('v').to_string())
}

/// Re-runs the installer that this copy was installed with. It is kept beside
/// the binary precisely so an update needs nothing from the internet but the
/// archive itself.
fn install_update() -> Result<(), String> {
    let updater = updater_path().ok_or("the updater script is not installed")?;
    let prefix = install_prefix().ok_or("could not work out where this is installed")?;

    if !is_writable(&prefix) {
        return Err(format!("{} is not writable by this user", prefix.display()));
    }

    // An update always runs the installer from the *newer* archive, whose
    // options this version cannot know. Ask for the quiet behaviour first and
    // fall back to the bare invocation if it is rejected, so a future flag
    // rename cannot strand everyone on an old version.
    //
    // `--no-deps` because system packages were settled at install time and
    // there is no terminal here to authenticate a package manager with.
    let attempts: [&[&str]; 2] = [&["--yes", "--no-deps"], &[]];
    let mut last_error = String::new();

    for extra in attempts {
        let mut command = Command::new("sh");
        command
            .arg(&updater)
            .args(extra)
            .arg("--prefix")
            .arg(&prefix);

        match command.status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => last_error = format!("the installer exited with {status}"),
            Err(err) => return Err(format!("could not run {}: {err}", updater.display())),
        }
    }

    Err(last_error)
}

/// `<prefix>/bin/instacache` -> `<prefix>`
pub fn install_prefix() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bin = exe.parent()?;
    if bin.file_name()? != "bin" {
        return None;
    }
    Some(bin.parent()?.to_path_buf())
}

/// `<prefix>/share/instacache/update.sh`
pub fn updater_path() -> Option<PathBuf> {
    let candidate = install_prefix()?
        .join("share")
        .join("instacache")
        .join("update.sh");
    candidate.is_file().then_some(candidate)
}

fn is_writable(path: &Path) -> bool {
    let probe = path.join(".instacache-write-test");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

// ------------------------------------------------------------ throttling ---

fn record_path(paths: &Paths) -> PathBuf {
    paths.config.join("update-check.json")
}

fn check_is_due(paths: &Paths, interval_hours: u64) -> bool {
    // Zero means "every launch", which is useful when testing.
    if interval_hours == 0 {
        return true;
    }
    let Ok(raw) = std::fs::read_to_string(record_path(paths)) else {
        return true;
    };
    let Ok(record) = serde_json::from_str::<CheckRecord>(&raw) else {
        return true;
    };
    let Some(elapsed) = now_secs().checked_sub(record.last_check) else {
        // A clock that went backwards; check rather than wait forever.
        return true;
    };
    elapsed >= interval_hours.saturating_mul(3600)
}

fn record_check(paths: &Paths, version: &str) {
    let record = CheckRecord {
        last_check: now_secs(),
        last_seen_version: version.to_string(),
    };
    if let Ok(body) = serde_json::to_string_pretty(&record) {
        let _ = std::fs::create_dir_all(&paths.config);
        let _ = std::fs::write(record_path(paths), body);
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ------------------------------------------------------------- versioning ---

/// Compares two dot-separated versions numerically, so 1.10.0 sorts above
/// 1.9.0 where a string comparison would get it backwards. A pre-release
/// suffix (`1.2.0-rc1`) is treated as older than the release it precedes.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    order(candidate) > order(current)
}

fn order(version: &str) -> (Vec<u64>, u8) {
    let (core, pre) = match version.split_once('-') {
        Some((core, _)) => (core, 0u8),
        None => (version, 1u8),
    };
    let numbers = core
        .split('.')
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .collect();
    (numbers, pre)
}

/// Runs the update from the command line, printing what happens.
pub fn update_now() -> Result<Outcome, String> {
    println!("Current version: {VERSION}");
    let latest = fetch_latest_version()
        .ok_or("could not reach the GitHub release API; check your connection")?;

    if !is_newer(&latest, VERSION) {
        println!("Already up to date.");
        return Ok(Outcome::UpToDate);
    }

    println!("Updating to {latest}…");
    install_update()?;
    println!("Updated to {latest}. Restart instaCache to run it.");
    Ok(Outcome::Installed { version: latest })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("1.10.0", "1.9.0"));
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
        assert!(!is_newer("0.9.0", "1.0.0"));
    }

    #[test]
    fn a_prerelease_is_older_than_its_release() {
        assert!(is_newer("1.2.0", "1.2.0-rc1"));
        assert!(!is_newer("1.2.0-rc1", "1.2.0"));
        assert!(is_newer("1.2.0-rc1", "1.1.0"));
    }

    #[test]
    fn tolerates_malformed_versions() {
        assert!(!is_newer("", "1.0.0"));
        assert!(!is_newer("not.a.version", "1.0.0"));
        assert!(is_newer("1.0", "0.9"));
    }

    #[test]
    fn a_zero_interval_always_checks() {
        let paths = Paths::for_profile("update-test");
        assert!(check_is_due(&paths, 0));
    }
}
