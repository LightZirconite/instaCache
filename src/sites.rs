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

use cpp::cpp;
use qttypes::QString;

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

/// Where icon themes live for this user. A site's icon is installed here
/// under its own name, rather than referenced from the profile by absolute
/// path: `Icon=` accepts a path, but a task bar resolving a window to an
/// application goes through the icon theme, and an absolute path is not
/// reliably honoured there. A wrong icon in the task bar with the right one in
/// the menu is what that looks like.
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

pub fn desktop_file(profile: &str) -> PathBuf {
    applications_dir().join(format!("{PROGRAM_NAME}-{profile}.desktop"))
}

cpp! {{
    #include <QtGui/QGuiApplication>
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
            ..Config::default()
        };
        crate::config::write(&config_file, &config).map_err(|err| err.to_string())?;
    }

    // The icon lives in the profile's config directory rather than in the icon
    // theme, and the entry points at it by absolute path, which the desktop
    // entry specification allows. That keeps a site's icon out of the shared
    // theme -- where it would need a size-matched directory and would outlive
    // an uninstall -- and it means clearing the session or the cache never
    // takes the icon with it.
    let icon_path = match icon {
        Some(path) => Some(install_icon_file(&paths, Path::new(path))?),
        None => fetch_icon(url).and_then(|(bytes, ext)| {
            let target = paths.config.join(format!("icon.{ext}"));
            std::fs::write(&target, bytes).ok().map(|()| target)
        }),
    };
    let icon_entry = match icon_path.as_ref() {
        Some(path) => {
            let bytes = std::fs::read(path).unwrap_or_default();
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default();
            install_theme_icon(&profile, &bytes, &ext, path)
        }
        None => ICON_NAME.to_string(),
    };

    let entry = desktop_file(&profile);
    if let Some(parent) = entry.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(
        &entry,
        desktop_entry(name.trim(), &profile, &exec_command(), &icon_entry),
    )
    .map_err(|err| err.to_string())?;
    refresh_desktop_database(&applications_dir());

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
