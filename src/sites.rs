//! Turning a site into its own application.
//!
//! A named profile already gives a site its own session, cache and window;
//! what it did not give it was a way in. This writes the two files that make
//! one appear in the desktop's application menu: the profile's `config.json`,
//! and a `.desktop` entry that launches instaCache against it — on Windows, a
//! shortcut in the Start menu, whose path the profile keeps in
//! `start-menu-entry` because a shortcut is named after the site, not after
//! the profile.
//!
//! Nothing here is Instagram-specific. The result is the same binary, the same
//! engine and the same window, pointed somewhere else — which is why it is
//! worth doing at all rather than shipping a second application.

use std::path::{Path, PathBuf};

use cpp::cpp;
use qttypes::QString;

use crate::config::Config;
use crate::paths::{sanitize_profile, Paths};
#[cfg(not(windows))]
use crate::ICON_NAME;
use crate::PROGRAM_NAME;

/// Where a site's menu entry goes. User-level on purpose: a site somebody
/// added is their data, not part of the installation, and it must survive an
/// upgrade that rewrites the shared files.
#[cfg(not(windows))]
pub fn applications_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("applications")
}

/// Where icon themes live for this user. A site's icon is installed here
/// under its own name, rather than referenced from the profile by absolute
/// path: `Icon=` accepts a path, but a task bar resolving a window to an
/// application goes through the icon theme, and an absolute path is not
/// reliably honoured there. A wrong icon in the task bar with the right one in
/// the menu is what that looks like.
#[cfg(not(windows))]
pub fn icon_theme_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("icons/hicolor")
}

/// The theme name a site's icon is installed under, which is also what its
/// entry names in `Icon=`.
pub fn icon_name(profile: &str) -> String {
    format!("{PROGRAM_NAME}-{profile}")
}

/// Width and height of a PNG, read from its header.
///
/// The icon theme wants a size-matched directory, and there is no way to know
/// one without looking: a favicon may be 16, 32 or 512 pixels square. Thirteen
/// bytes of IHDR beat pulling in an image crate for it.
pub fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let ihdr = bytes.get(12..16)?;
    if ihdr != b"IHDR" {
        return None;
    }
    let read = |at: usize| -> Option<u32> {
        let raw: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
        Some(u32::from_be_bytes(raw))
    };
    let (width, height) = (read(16)?, read(20)?);
    (width > 0 && height > 0).then_some((width, height))
}

/// Installs a site's icon into the icon theme and returns the name to put in
/// `Icon=`. Falls back to the absolute path when the image is one the theme
/// cannot file away — an `.ico` has no single size to sort it under.
#[cfg(not(windows))]
fn install_theme_icon(profile: &str, bytes: &[u8], ext: &str, fallback: &Path) -> String {
    let name = icon_name(profile);
    let dir = match ext {
        "svg" => Some(icon_theme_dir().join("scalable/apps")),
        "png" => png_dimensions(bytes).map(|(w, h)| icon_theme_dir().join(format!("{w}x{h}/apps"))),
        _ => None,
    };

    let Some(dir) = dir else {
        return fallback.to_string_lossy().into_owned();
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return fallback.to_string_lossy().into_owned();
    }
    if std::fs::write(dir.join(format!("{name}.{ext}")), bytes).is_err() {
        return fallback.to_string_lossy().into_owned();
    }
    name
}

/// Removes every copy of a site's icon from the theme, whatever size it went
/// in under.
#[cfg(not(windows))]
fn remove_theme_icon(profile: &str) {
    let name = icon_name(profile);
    let Ok(sizes) = std::fs::read_dir(icon_theme_dir()) else {
        return;
    };
    for size in sizes.filter_map(|entry| entry.ok()) {
        for ext in ["png", "svg"] {
            let _ = std::fs::remove_file(size.path().join("apps").join(format!("{name}.{ext}")));
        }
    }
}

#[cfg(not(windows))]
pub fn desktop_file(profile: &str) -> PathBuf {
    applications_dir().join(format!("{PROGRAM_NAME}-{profile}.desktop"))
}

cpp! {{
    #include <QtGui/QGuiApplication>
    #include <QtGui/QIcon>
    #include <QtCore/QString>
}}

/// Tells Qt which desktop entry this process belongs to.
///
/// This is what actually puts a site's own icon in the task bar, and it is not
/// the same thing as the application name. Qt's Wayland plugin calls
/// `QGuiApplication::desktopFileName()` and passes the result straight to
/// `xdg_toplevel::set_app_id`; it never looks at `applicationName()`. Setting
/// only the latter — which is what this code did at first — leaves every
/// window announcing `instacache`, so the task bar matches all of them to
/// `instacache.desktop` and draws instaCache's icon over the site's.
///
/// Must be called before the first window is created. The name is passed
/// without the `.desktop` suffix, which Qt would strip with a warning.
pub fn announce_desktop_file(name: &str) {
    let name = QString::from(name);
    cpp!(unsafe [name as "QString"] {
        QGuiApplication::setDesktopFileName(name);
    });
}

/// Gives every window of this process `icon`. Needs the application object,
/// so it runs after the engine is made and before the scene is loaded.
#[cfg(windows)]
pub fn set_window_icon(icon: &Path) {
    let path = QString::from(icon.to_string_lossy().as_ref());
    cpp!(unsafe [path as "QString"] {
        QGuiApplication::setWindowIcon(QIcon(path));
    });
}

/// The icon a site's window should wear on Windows: the one the site keeps,
/// in a format Qt reads without an extra plugin. `None` for instaCache
/// itself, whose icon is inside the executable.
#[cfg(windows)]
pub fn window_icon(paths: &Paths) -> Option<PathBuf> {
    if paths.is_default_profile() {
        return None;
    }
    saved_icon(paths).filter(|icon| {
        icon.extension()
            .is_some_and(|ext| ["png", "ico", "jpg"].iter().any(|known| ext == *known))
    })
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
///
/// `icon` is either an absolute path to the site's own icon or a plain theme
/// name; the desktop entry specification allows both, which is what lets a
/// site keep its icon without installing anything into the icon theme.
#[cfg(not(windows))]
pub fn desktop_entry(name: &str, profile: &str, exec: &str, icon: &str) -> String {
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
         Icon={icon}\n\
         Terminal=false\n\
         Categories=Network;\n\
         StartupNotify=true\n\
         StartupWMClass={class}\n",
        class = window_class(profile),
    )
}

/// The command a menu entry should run.
///
/// The installed copy is preferred over the running one. They are the same
/// thing for anybody using instaCache normally, and different in exactly one
/// case that matters: a build tree. An entry written from `target/debug`
/// points at a binary that gets rebuilt, moved and broken all day, and the
/// site stops opening -- which is not a theoretical failure, it happened here.
///
/// Falling back to the running binary keeps entries working when instaCache is
/// installed somewhere that is not on `PATH`, and the bare name is the last
/// resort for a platform that cannot report either.
pub(crate) fn exec_command() -> String {
    choose_exec(
        crate::http::which(PROGRAM_NAME),
        std::env::current_exe().ok(),
    )
}

fn choose_exec(on_path: Option<PathBuf>, running: Option<PathBuf>) -> String {
    on_path
        .into_iter()
        .chain(running)
        .find(|path| path.is_absolute())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| PROGRAM_NAME.to_string())
}

/// The image formats a desktop can be expected to display, recognised by what
/// the bytes actually are rather than by the URL they came from.
///
/// That distinction is not pedantry: `x.com/favicon.ico` serves a PNG. Trusting
/// the extension would write a file called `.ico` that is not one, and some
/// icon loaders refuse it.
pub fn icon_extension(bytes: &[u8]) -> Option<&'static str> {
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    const GIF: &[u8] = b"GIF8";
    const ICO: &[u8] = &[0x00, 0x00, 0x01, 0x00];
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF];
    const RIFF: &[u8] = b"RIFF";

    if bytes.starts_with(PNG) {
        return Some("png");
    }
    if bytes.starts_with(GIF) {
        return Some("gif");
    }
    if bytes.starts_with(ICO) {
        return Some("ico");
    }
    if bytes.starts_with(JPEG) {
        return Some("jpg");
    }
    if bytes.starts_with(RIFF) && bytes.get(8..12) == Some(b"WEBP") {
        return Some("webp");
    }
    // SVG is text, and may open with a declaration, a comment or whitespace.
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    if head.contains("<svg") {
        return Some("svg");
    }
    None
}

/// Turns an `href` from a page into an absolute URL.
pub fn resolve_url(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }
    let scheme = crate::urls::scheme_of(base)?;
    let host = crate::urls::host_of(base)?;
    // A URL that only omits the scheme, e.g. `//cdn.example/icon.png`.
    if let Some(rest) = href.strip_prefix("//") {
        return Some(format!("{scheme}://{rest}"));
    }
    if let Some(rest) = href.strip_prefix('/') {
        return Some(format!("{scheme}://{host}/{rest}"));
    }
    Some(format!("{scheme}://{host}/{href}"))
}

/// The best icon a page declares, as an absolute URL.
///
/// "Best" is the largest declared `sizes`, because a dock draws these at 48
/// pixels or more and a 16-pixel favicon looks like a mistake. An
/// `apple-touch-icon` wins ties: it is by convention the large, square,
/// opaque one, which is exactly what a launcher wants.
pub fn pick_icon_link(html: &str, base: &str) -> Option<String> {
    let mut best: Option<(u32, String)> = None;

    for tag in html.split('<').filter(|t| t.len() > 4) {
        let lower = tag.to_ascii_lowercase();
        if !lower.starts_with("link") {
            continue;
        }
        let rel = attribute(&lower, "rel")?.to_string();
        if !rel.contains("icon") {
            continue;
        }
        // The href is taken from the original so its case is preserved.
        let Some(href) = attribute(tag, "href") else {
            continue;
        };
        let Some(url) = resolve_url(base, href) else {
            continue;
        };

        let declared = attribute(&lower, "sizes")
            .and_then(|s| s.split(['x', 'X']).next()?.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let score = declared
            + if rel.contains("apple-touch-icon") {
                1
            } else {
                0
            };

        if best.as_ref().is_none_or(|(seen, _)| score > *seen) {
            best = Some((score, url));
        }
    }

    best.map(|(_, url)| url)
}

/// The value of `name="..."` in a tag, single or double quoted.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let at = tag.to_ascii_lowercase().find(&format!("{name}="))?;
    let rest = &tag[at + name.len() + 1..];
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        rest[1..].find(quote).map(|end| &rest[1..1 + end])
    } else {
        let end = rest.find([' ', '>', '\t', '\n']).unwrap_or(rest.len());
        Some(&rest[..end])
    }
}

/// Downloads the site's own icon: what the page declares, or the favicon every
/// site is expected to serve. `None` simply means the site keeps instaCache's
/// icon, which is not worth failing over.
fn fetch_icon(site_url: &str) -> Option<(Vec<u8>, &'static str)> {
    let declared = crate::http::get_text(site_url).and_then(|html| pick_icon_link(&html, site_url));
    let fallback = resolve_url(site_url, "/favicon.ico");

    for candidate in [declared, fallback].into_iter().flatten() {
        if let Some(bytes) = crate::http::get(&candidate) {
            if let Some(ext) = icon_extension(&bytes) {
                return Some((bytes, ext));
            }
        }
    }
    None
}

/// Sites instaCache sets up for you on a first install.
///
/// Instagram is the application itself and is not in this list; these are the
/// extras. Kept deliberately short — a dedicated browser that fills somebody's
/// menu with applications they did not ask for is spam, not a feature.
pub const DEFAULT_SITES: &[(&str, &str, &str)] = &[(
    "XCache",
    "https://x.com/",
    // Posts load from x.com but every image and video comes from twimg.com,
    // so a window that allows only the first shows a feed with no pictures.
    "x.com,twimg.com",
)];

/// Adds the default sites, but only ones that have never existed.
///
/// `install.sh` calls this, and it runs again on every update, so "already
/// there" is not the only case to skip: somebody who removed a site must not
/// find it back in their menu after the next update. The profile's directory
/// is what records that it once existed — `--remove-site` deliberately leaves
/// it — so its presence is the signal to keep out of the way.
pub fn ensure_defaults() -> Vec<String> {
    let mut added = Vec::new();
    for (name, url, domains) in DEFAULT_SITES {
        if Paths::for_profile(&sanitize_profile(name)).config.exists() {
            continue;
        }
        match add(name, url, Some(domains), None) {
            Ok(site) => added.push(site.profile),
            Err(error) => eprintln!("instacache: could not add {name}: {error}"),
        }
    }
    added
}

pub struct Added {
    pub profile: String,
    pub config: PathBuf,
    pub entry: PathBuf,
    /// The site's own icon, when one was found or supplied.
    pub icon: Option<PathBuf>,
}

/// Creates the profile and its menu entry.
///
/// An existing profile's `config.json` is never overwritten: somebody who has
/// already tuned a site should not lose it by re-running this, so only the
/// menu entry is refreshed.
pub fn add(
    name: &str,
    url: &str,
    domains: Option<&str>,
    icon: Option<&str>,
) -> Result<Added, String> {
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
            ..Config::for_home(url)
        };
        crate::config::write(&config_file, &config).map_err(|err| err.to_string())?;
    }

    let icon_path = match icon {
        Some(path) => Some(install_icon_file(&paths, Path::new(path))?),
        None => fetch_site_icon(&paths, url),
    };
    let entry = write_entry(name.trim(), &paths, icon_path.as_deref())?;

    Ok(Added {
        profile,
        config: config_file,
        entry,
        icon: icon_path,
    })
}

/// Copies a hand-picked icon into the profile, refusing anything that is not
/// an image a desktop can draw -- an entry pointing at a text file shows no
/// icon at all, with nothing to say why.
/// Downloads the site's icon into the profile's config directory, which is
/// the copy that is kept: clearing the session or the cache never takes it,
/// and the theme copy is made from it.
fn fetch_site_icon(paths: &Paths, url: &str) -> Option<PathBuf> {
    let (bytes, ext) = fetch_icon(url)?;
    let target = paths.config.join(format!("icon.{ext}"));
    std::fs::write(&target, bytes).ok().map(|()| target)
}

/// The icon a profile has kept, if any.
fn saved_icon(paths: &Paths) -> Option<PathBuf> {
    ["png", "svg", "ico", "jpg", "webp"]
        .iter()
        .map(|ext| paths.config.join(format!("icon.{ext}")))
        .find(|path| path.is_file())
}

/// Writes the desktop entry for a site and tells the desktop about it.
#[cfg(not(windows))]
fn write_entry(name: &str, paths: &Paths, icon: Option<&Path>) -> Result<PathBuf, String> {
    let profile = &paths.profile;
    let icon_entry = theme_icon_entry(profile, icon);
    let entry = desktop_file(profile);
    if let Some(parent) = entry.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(
        &entry,
        desktop_entry(name, profile, &exec_command(), &icon_entry),
    )
    .map_err(|err| err.to_string())?;
    refresh_desktop_database(&applications_dir());
    Ok(entry)
}

/// What `Icon=` says for a site: its own icon installed into the theme under
/// the site's name, or instaCache's when it has none.
#[cfg(not(windows))]
fn theme_icon_entry(profile: &str, icon: Option<&Path>) -> String {
    match icon {
        Some(path) => {
            let bytes = std::fs::read(path).unwrap_or_default();
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default();
            install_theme_icon(profile, &bytes, &ext, path)
        }
        None => ICON_NAME.to_string(),
    }
}

/// The `Name=` of a menu entry, so rewriting it keeps what the user called
/// the site.
#[cfg(not(windows))]
fn entry_name(entry: &str) -> Option<&str> {
    entry
        .lines()
        .find_map(|line| line.strip_prefix("Name="))
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

/// Rewrites every site's menu entry in the current format, and puts its icon
/// back into the theme — fetching it again if it never arrived. Returns the
/// sites whose entry changed.
///
/// An entry is written once, by `--add-site` or the first install, and
/// nothing rewrote it afterwards: a site added by an older version kept that
/// version's entry for good, through every fix to how an entry and its icon
/// are made. A site whose window sat in the task bar under instaCache's icon
/// stayed that way. `install.sh` runs this at every install and update.
#[cfg(not(windows))]
pub fn refresh_existing() -> Vec<String> {
    let mut changed = Vec::new();
    for profile in list() {
        let entry = desktop_file(&profile);
        let Ok(old) = std::fs::read_to_string(&entry) else {
            continue;
        };
        let paths = Paths::for_profile(&profile);
        let icon = saved_icon(&paths).or_else(|| {
            let home = Config::load_or_create(&paths).home_url;
            fetch_site_icon(&paths, &home)
        });
        let name = entry_name(&old).unwrap_or(&profile).to_string();
        let new = desktop_entry(
            &name,
            &profile,
            &exec_command(),
            &theme_icon_entry(&profile, icon.as_deref()),
        );
        if new != old && std::fs::write(&entry, new).is_ok() {
            changed.push(profile);
        }
    }
    // The icons went back into the theme whether or not an entry changed.
    refresh_desktop_database(&applications_dir());
    changed
}

fn install_icon_file(paths: &Paths, source: &Path) -> Result<PathBuf, String> {
    let bytes = std::fs::read(source)
        .map_err(|err| format!("could not read {}: {err}", source.display()))?;
    let ext = icon_extension(&bytes)
        .ok_or_else(|| format!("{} is not an image a desktop can show", source.display()))?;
    let target = paths.config.join(format!("icon.{ext}"));
    std::fs::write(&target, bytes).map_err(|err| err.to_string())?;
    Ok(target)
}

/// Removes the menu entry. The profile's session and cache are deliberately
/// left alone — deleting an account's data is not what "remove from the menu"
/// means, and `--clear-session` already exists for that.
#[cfg(not(windows))]
pub fn remove(name: &str) -> Result<PathBuf, String> {
    let profile = sanitize_profile(name);
    let entry = desktop_file(&profile);
    match std::fs::remove_file(&entry) {
        Ok(()) => {
            // The theme copy is an installed artefact, not the user's data, so
            // unlike the session it goes when the entry does.
            remove_theme_icon(&profile);
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
#[cfg(not(windows))]
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

/// Tells the desktop a new entry and a new icon exist.
///
/// Three caches, because they are read by different things and refreshing only
/// some leaves a site half-visible: `update-desktop-database` rebuilds the
/// freedesktop association cache, `gtk-update-icon-cache` the icon theme
/// index, and `kbuildsycoca6` KDE's own, which is what its menu and task bar
/// consult. Both are best-effort — a stale cache
/// only means the entry turns up at the next login rather than at once, which
/// is not worth failing an install over.
#[cfg(not(windows))]
fn refresh_desktop_database(dir: &Path) {
    let quiet = |mut command: std::process::Command| {
        let _ = command
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    };

    let mut update = std::process::Command::new("update-desktop-database");
    update.arg(dir);
    quiet(update);

    if crate::http::which("gtk-update-icon-cache").is_some() {
        let mut icons = std::process::Command::new("gtk-update-icon-cache");
        icons.args(["-f", "-t"]).arg(icon_theme_dir());
        quiet(icons);
    }

    for tool in ["kbuildsycoca6", "kbuildsycoca5"] {
        if crate::http::which(tool).is_some() {
            quiet(std::process::Command::new(tool));
            break;
        }
    }
}

// --------------------------------------------------------------- Windows ---

/// The user's own Start menu, where a site's shortcut goes.
#[cfg(windows)]
fn start_menu_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
}

/// The file in a profile that names its shortcut.
#[cfg(windows)]
fn entry_record(paths: &Paths) -> PathBuf {
    paths.config.join("start-menu-entry")
}

#[cfg(windows)]
fn recorded_entry(paths: &Paths) -> Option<PathBuf> {
    let text = std::fs::read_to_string(entry_record(paths)).ok()?;
    let path = text.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// A site's name as a file name. The Start menu shows a shortcut under its
/// file name, so the name is kept as typed, minus what Windows forbids.
#[cfg(any(windows, test))]
fn shortcut_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').trim().to_string();
    if cleaned.is_empty() {
        "Site".to_string()
    } else {
        cleaned
    }
}

/// A one-image `.ico` around a PNG, which every Windows since Vista reads.
/// A shortcut's icon has to be an `.ico`, and a site's icon is most often a
/// PNG; wrapping it is twenty-two bytes, where converting it would be an
/// image library.
#[cfg(any(windows, test))]
fn ico_from_png(png: &[u8]) -> Option<Vec<u8>> {
    let (width, height) = png_dimensions(png)?;
    let size = u32::try_from(png.len()).ok()?;
    // 0 means 256, and is what a larger image says too.
    let side = |pixels: u32| u8::try_from(pixels).unwrap_or(0);
    let mut ico = Vec::with_capacity(22 + png.len());
    ico.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    ico.extend_from_slice(&[side(width), side(height), 0, 0]);
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&size.to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(png);
    Some(ico)
}

/// The `.ico` a site's shortcut shows, made from the icon it keeps. `None`
/// leaves the shortcut with instaCache's own.
#[cfg(windows)]
fn shortcut_icon(paths: &Paths, icon: &Path) -> Option<PathBuf> {
    let bytes = std::fs::read(icon).ok()?;
    match icon_extension(&bytes)? {
        "ico" => Some(icon.to_path_buf()),
        "png" => {
            let target = paths.config.join("shortcut.ico");
            std::fs::write(&target, ico_from_png(&bytes)?).ok()?;
            Some(target)
        }
        _ => None,
    }
}

/// Writes a shortcut through the Windows Script Host, which every Windows
/// has; the alternative is the shell's COM interfaces from Rust. The values
/// travel in the environment, never in the script's text, so a site's name
/// cannot become PowerShell.
#[cfg(windows)]
fn create_shortcut(
    link: &Path,
    arguments: &str,
    icon: &str,
    description: &str,
) -> Result<(), String> {
    const SCRIPT: &str =
        "$s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:INSTACACHE_LINK); \
$s.TargetPath = $env:INSTACACHE_TARGET; \
$s.Arguments = $env:INSTACACHE_ARGUMENTS; \
$s.IconLocation = $env:INSTACACHE_ICON; \
$s.Description = $env:INSTACACHE_DESCRIPTION; \
$s.WorkingDirectory = Split-Path -Parent $env:INSTACACHE_TARGET; \
$s.Save()";
    let mut command = std::process::Command::new("powershell.exe");
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            SCRIPT,
        ])
        .env("INSTACACHE_LINK", link)
        .env("INSTACACHE_TARGET", exec_command())
        .env("INSTACACHE_ARGUMENTS", arguments)
        .env("INSTACACHE_ICON", icon)
        .env("INSTACACHE_DESCRIPTION", description)
        .stdout(std::process::Stdio::null());
    crate::windows::no_console(&mut command);
    match command.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!(
            "could not write {}: PowerShell exited with {status}",
            link.display()
        )),
        Err(error) => Err(format!("could not run PowerShell: {error}")),
    }
}

/// Writes the Start menu shortcut for a site, replacing the one it had under
/// another name.
#[cfg(windows)]
fn write_entry(name: &str, paths: &Paths, icon: Option<&Path>) -> Result<PathBuf, String> {
    let dir = start_menu_dir();
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let link = dir.join(format!("{}.lnk", shortcut_name(name)));
    if let Some(old) = recorded_entry(paths).filter(|old| *old != link) {
        let _ = std::fs::remove_file(old);
    }
    let icon = icon
        .and_then(|icon| shortcut_icon(paths, icon))
        .map(|icon| icon.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("{},0", exec_command()));
    create_shortcut(
        &link,
        &format!("--profile {}", paths.profile),
        &icon,
        "Opened in its own instaCache window, with its own session",
    )?;
    std::fs::write(entry_record(paths), link.to_string_lossy().as_bytes())
        .map_err(|err| err.to_string())?;
    Ok(link)
}

/// Rewrites every site's shortcut, so one written by an older version, or
/// pointing at an executable that moved, is brought up to date. `install.ps1`
/// runs this at every install and update.
#[cfg(windows)]
pub fn refresh_existing() -> Vec<String> {
    let mut changed = Vec::new();
    for profile in list() {
        let paths = Paths::for_profile(&profile);
        let Some(link) = recorded_entry(&paths) else {
            continue;
        };
        let name = link
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| profile.clone());
        let icon = saved_icon(&paths).or_else(|| {
            let home = Config::load_or_create(&paths).home_url;
            fetch_site_icon(&paths, &home)
        });
        match write_entry(&name, &paths, icon.as_deref()) {
            Ok(_) => changed.push(profile),
            Err(error) => eprintln!("instacache: {error}"),
        }
    }
    changed
}

/// Removes the shortcut. The profile's session and cache are left alone,
/// for the same reason as on Linux.
#[cfg(windows)]
pub fn remove(name: &str) -> Result<PathBuf, String> {
    let profile = sanitize_profile(name);
    let paths = Paths::for_profile(&profile);
    let Some(link) = recorded_entry(&paths) else {
        return Err(format!("no site named `{profile}` is in the menu"));
    };
    match std::fs::remove_file(&link) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.to_string()),
    }
    let _ = std::fs::remove_file(entry_record(&paths));
    let _ = std::fs::remove_file(paths.config.join("shortcut.ico"));
    Ok(link)
}

/// Every site with a shortcut in the Start menu.
#[cfg(windows)]
pub fn list() -> Vec<String> {
    let root = Paths::for_profile(crate::paths::DEFAULT_PROFILE)
        .config
        .join("profiles");
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|profile| {
            recorded_entry(&Paths::for_profile(profile)).is_some_and(|link| link.is_file())
        })
        .collect();
    names.sort();
    names
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
    fn a_shortcut_is_named_as_the_site_was() {
        assert_eq!(shortcut_name("XCache"), "XCache");
        assert_eq!(shortcut_name("My: X?"), "My X");
        assert_eq!(shortcut_name("..\\..\\evil"), "....evil");
        assert_eq!(shortcut_name("  ...  "), "Site");
    }

    #[test]
    fn a_png_becomes_an_icon_windows_reads() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&[0, 0, 0, 13]);
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&48u32.to_be_bytes());
        png.extend_from_slice(&512u32.to_be_bytes());
        let ico = ico_from_png(&png).unwrap();
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 1, 0], "one icon");
        assert_eq!(ico[6], 48);
        assert_eq!(ico[7], 0, "larger than 255 is written as 0");
        assert_eq!(&ico[14..18], &(png.len() as u32).to_le_bytes());
        assert_eq!(&ico[18..22], &22u32.to_le_bytes());
        assert_eq!(&ico[22..], &png[..]);
        assert_eq!(icon_extension(&ico), Some("ico"));
        assert!(ico_from_png(b"not a png").is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn the_entry_launches_the_right_profile_and_names_its_class() {
        let entry = desktop_entry("X", "x", "/usr/bin/instacache", "instacache");
        assert!(entry.contains("Exec=/usr/bin/instacache --profile x %u"));
        assert!(entry.contains("StartupWMClass=instacache-x"));
        assert!(entry.contains("Name=X"));
        assert!(entry.starts_with("[Desktop Entry]\n"));
        // The version the packaging validators on old runners accept.
        assert!(entry.contains("Version=1.0"));
    }

    #[test]
    fn an_image_is_recognised_by_its_bytes_not_its_name() {
        // The case this exists for: x.com/favicon.ico is a PNG.
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
        assert_eq!(icon_extension(&png), Some("png"));
        assert_eq!(icon_extension(&[0x00, 0x00, 0x01, 0x00, 1, 0]), Some("ico"));
        assert_eq!(icon_extension(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(icon_extension(b"GIF89a...."), Some("gif"));
        assert_eq!(
            icon_extension(b"<?xml version=\"1.0\"?><svg viewBox=\"0 0 1 1\"/>"),
            Some("svg")
        );
        // A login page returned instead of an image must not become icon.png.
        assert_eq!(icon_extension(b"<!doctype html><title>hi</title>"), None);
        assert_eq!(icon_extension(b""), None);
    }

    #[test]
    fn hrefs_become_absolute_urls() {
        let base = "https://x.com/home";
        assert_eq!(
            resolve_url(base, "/favicon.ico").as_deref(),
            Some("https://x.com/favicon.ico")
        );
        assert_eq!(
            resolve_url(base, "//cdn.example/i.png").as_deref(),
            Some("https://cdn.example/i.png")
        );
        assert_eq!(
            resolve_url(base, "https://other.example/i.png").as_deref(),
            Some("https://other.example/i.png")
        );
        assert_eq!(
            resolve_url(base, "icon.png").as_deref(),
            Some("https://x.com/icon.png")
        );
        assert_eq!(resolve_url(base, "  "), None);
    }

    #[test]
    fn the_biggest_declared_icon_wins() {
        let html = r#"
            <link rel="shortcut icon" href="/small.ico" sizes="16x16">
            <link rel="icon" href="/medium.png" sizes="64x64">
            <link rel="apple-touch-icon" href="/big.png" sizes="180x180">
            <link rel="stylesheet" href="/not-an-icon.css">
        "#;
        assert_eq!(
            pick_icon_link(html, "https://example.com/").as_deref(),
            Some("https://example.com/big.png")
        );
    }

    #[test]
    fn a_page_declaring_no_icon_yields_nothing() {
        // x.com is exactly this: a JavaScript shell with no icon link at all,
        // which is why /favicon.ico has to stay as the fallback.
        assert_eq!(
            pick_icon_link("<html><body>hi</body></html>", "https://x.com/"),
            None
        );
    }

    #[test]
    fn single_quoted_attributes_are_read_too() {
        let html = "<link rel='icon' href='/i.png' sizes='32x32'>";
        assert_eq!(
            pick_icon_link(html, "https://example.com/").as_deref(),
            Some("https://example.com/i.png")
        );
    }

    #[test]
    fn an_entry_prefers_the_installed_binary_over_a_build_tree() {
        // Absolute on the system the test runs on, which is what is checked.
        let (installed, built) = if cfg!(windows) {
            (
                PathBuf::from(r"C:\Users\someone\AppData\Local\Programs\instaCache\instacache.exe"),
                PathBuf::from(r"C:\src\instaCache\target\debug\instacache.exe"),
            )
        } else {
            (
                PathBuf::from("/home/someone/.local/bin/instacache"),
                PathBuf::from("/src/instaCache/target/debug/instacache"),
            )
        };

        // The case this exists for: writing the entry from a build tree.
        assert_eq!(
            choose_exec(Some(installed.clone()), Some(built.clone())),
            installed.to_string_lossy()
        );
        // Installed somewhere off PATH: the running binary still works.
        assert_eq!(
            choose_exec(None, Some(built.clone())),
            built.to_string_lossy()
        );
        // A relative path would break the moment the menu runs it elsewhere.
        assert_eq!(
            choose_exec(None, Some(PathBuf::from("./instacache"))),
            PROGRAM_NAME
        );
        assert_eq!(choose_exec(None, None), PROGRAM_NAME);
    }

    #[test]
    fn a_pngs_size_is_read_from_its_header() {
        // An 8x8 PNG: signature, length, IHDR, width, height.
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&[0, 0, 0, 13]);
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&8u32.to_be_bytes());
        png.extend_from_slice(&8u32.to_be_bytes());
        assert_eq!(png_dimensions(&png), Some((8, 8)));

        // Anything that is not a PNG header must not produce a size, or the
        // icon lands in a directory that claims a size it does not have.
        assert_eq!(png_dimensions(b"<svg/>"), None);
        assert_eq!(png_dimensions(&[0u8; 4]), None);
    }

    #[cfg(not(windows))]
    #[test]
    fn rewriting_an_entry_keeps_the_name_it_was_given() {
        let entry = desktop_entry("My X", "xcache", "/usr/bin/instacache", "instacache-xcache");
        assert_eq!(entry_name(&entry), Some("My X"));
        assert_eq!(entry_name("[Desktop Entry]\nName=\n"), None);
        // `GenericName=` is not the name.
        assert_eq!(entry_name("GenericName=Web\nName=X\n"), Some("X"));
    }

    #[test]
    fn a_sites_icon_is_named_after_its_profile() {
        assert_eq!(icon_name("xcache"), "instacache-xcache");
        // Same shape as the window class, so an entry, a window and an icon
        // all agree without anyone having to remember to keep them in step.
        assert_eq!(icon_name("xcache"), window_class("xcache"));
    }

    #[test]
    fn the_default_sites_are_well_formed() {
        for (name, url, domains) in DEFAULT_SITES {
            assert!(crate::urls::is_http(url), "{name} needs a web address");
            assert_ne!(
                sanitize_profile(name),
                crate::paths::DEFAULT_PROFILE,
                "{name} would collide with instaCache itself"
            );
            // A site whose media host is missing shows a feed with no pictures.
            let listed: Vec<&str> = domains.split(',').collect();
            assert!(
                listed.contains(&default_domain(url).unwrap().as_str()),
                "{name} does not allow its own host"
            );
        }
    }

    #[test]
    fn a_site_cannot_hijack_the_main_window() {
        assert!(add("default", "https://x.com/", None, None).is_err());
        assert!(add("x", "javascript:alert(1)", None, None).is_err());
        assert!(add("x", "file:///etc/passwd", None, None).is_err());
    }
}
