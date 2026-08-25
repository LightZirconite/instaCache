//! instaCache — command-line entry point.
//!
//! Everything of substance lives in the library crate; this file only parses
//! arguments and starts the Qt application.

use std::process::ExitCode;
use std::rc::Rc;
use std::sync::atomic::Ordering;

use qmetaobject::qtcore::core_application::QCoreApplication;
use qmetaobject::{QObjectPinned, QmlEngine};

use instacache::bridge::{Shell, SHUTDOWN};
use instacache::{chromium, config, instance, paths, sites, updates, urls};
use instacache::{APP_NAME, VERSION};

/// The scene is compiled into the binary rather than installed beside it, so
/// there is still exactly one file to ship and no way for the two to drift
/// apart across an update.
const SCENE: &str = include_str!("qml/main.qml");

fn main() -> ExitCode {
    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("instacache: {message}");
            eprintln!("Try `instacache --help` for the list of options.");
            return ExitCode::from(2);
        }
    };

    match options.mode {
        Mode::Help => {
            print!("{}", usage());
            return ExitCode::SUCCESS;
        }
        Mode::Version => {
            println!("{APP_NAME} {VERSION}");
            return ExitCode::SUCCESS;
        }
        Mode::Update => {
            return match updates::update_now() {
                Ok(_) => ExitCode::SUCCESS,
                Err(message) => {
                    eprintln!("instacache: {message}");
                    ExitCode::FAILURE
                }
            };
        }
        Mode::AddSite => {
            let name = options.site.clone().unwrap_or_default();
            let url = options.url.clone().unwrap_or_default();
            return match sites::add(&name, &url, options.domains.as_deref()) {
                Ok(added) => {
                    println!("Added `{}` to your application menu.", added.profile);
                    println!("  settings  {}", added.config.display());
                    println!("  entry     {}", added.entry.display());
                    println!();
                    println!(
                        "It has its own session, cache and window. Open it from the menu, or run:  \
instacache --profile {}",
                        added.profile
                    );
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("instacache: {message}");
                    ExitCode::FAILURE
                }
            };
        }
        Mode::RemoveSite => {
            let name = options.site.clone().unwrap_or_default();
            return match sites::remove(&name) {
                Ok(entry) => {
                    println!("Removed {}", entry.display());
                    println!("Its session and cache are untouched; --clear-session deletes those.");
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("instacache: {message}");
                    ExitCode::FAILURE
                }
            };
        }
        Mode::ListSites => {
            let sites = sites::list();
            if sites.is_empty() {
                println!("No sites have been added. Try:  instacache --add-site x https://x.com/");
            } else {
                for site in sites {
                    println!("{site}");
                }
            }
            return ExitCode::SUCCESS;
        }
        Mode::Run | Mode::Clear(_) => {}
    }

    let paths = paths::Paths::for_profile(&options.profile);
    if let Err(err) = paths.ensure() {
        eprintln!("instacache: cannot create {}: {err}", paths.data.display());
        return ExitCode::FAILURE;
    }

    if let Mode::Clear(what) = options.mode {
        return clear(&paths, what);
    }

    // A second launch of the same profile focuses the window that exists
    // instead of starting a duplicate browser process tree over one cookie
    // jar. A failure here is not worth refusing to start over: the worst case
    // is two windows.
    let listener = match instance::claim(&paths.profile, options.url.as_deref()) {
        Ok(instance::Claim::Handed) => return ExitCode::SUCCESS,
        Ok(instance::Claim::Owner(listener)) => Some(listener),
        Err(error) => {
            eprintln!("instacache: could not claim the profile: {error}");
            None
        }
    };

    let config = Rc::new(config::Config::load_or_create(&paths));
    let profile = paths.profile.clone();
    let paths = Rc::new(paths);

    install_termination_handlers();

    // Both of these must run before any QGuiApplication exists, which is why
    // neither can wait until the engine is built: Qt WebEngine reads the flags
    // as it initialises, and initialising afterwards is not allowed at all.
    chromium::apply(&config);
    qmetaobject::webengine::initialize();

    let mut engine = QmlEngine::new();
    // What Wayland turns into the `app_id` and X11 into `WM_CLASS`; it has to
    // match `StartupWMClass` in the entry that launched us or the dock shows a
    // generic icon. It differs per profile so that each site added to the menu
    // gets its own icon instead of every window piling under one. See the
    // three-constants rule in AGENTS.md.
    QCoreApplication::set_application_name(sites::window_class(&profile).into());

    let shell = std::cell::RefCell::new(Shell::new(config, paths, listener, options.url.clone()));
    let pinned = unsafe { QObjectPinned::new(&shell) };
    engine.set_object_property("shell".into(), pinned);
    engine.load_data(SCENE.into());
    engine.exec();

    instance::release(&profile);
    ExitCode::SUCCESS
}

/// libc constants and the one libc call this needs, declared here rather than
/// pulled in as a dependency. Both signal numbers are the same on every Linux
/// architecture instaCache targets.
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;

extern "C" {
    fn signal(signum: i32, handler: extern "C" fn(i32)) -> usize;
}

/// Only sets a flag: a signal handler may call almost nothing safely, and
/// saving the window geometry from inside one would be writing a file from
/// an interrupted allocation. The scene's own timer notices the flag on its
/// next tick and shuts down through the ordinary path.
extern "C" fn on_terminate(_signum: i32) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

/// A desktop session ending, `systemctl --user stop` or a plain `kill` never
/// delivers a window close, so the geometry would be lost without this.
fn install_termination_handlers() {
    unsafe {
        signal(SIGINT, on_terminate);
        signal(SIGTERM, on_terminate);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clearable {
    Cache,
    Session,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Run,
    Help,
    Version,
    Clear(Clearable),
    Update,
    AddSite,
    RemoveSite,
    ListSites,
}

#[derive(Debug, Clone)]
struct Options {
    mode: Mode,
    profile: String,
    url: Option<String>,
    /// Display name for `--add-site`, and the site to look up for
    /// `--remove-site`. Kept separate from `profile`, which is the sanitised
    /// directory name derived from it.
    site: Option<String>,
    domains: Option<String>,
}

impl Options {
    fn parse<I: Iterator<Item = String>>(args: I) -> Result<Self, String> {
        let mut options = Options {
            mode: Mode::Run,
            profile: paths::DEFAULT_PROFILE.to_string(),
            url: None,
            site: None,
            domains: None,
        };
        let mut args = args.peekable();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    return Ok(Options {
                        mode: Mode::Help,
                        ..options
                    })
                }
                "-V" | "--version" => {
                    return Ok(Options {
                        mode: Mode::Version,
                        ..options
                    })
                }
                "-p" | "--profile" => {
                    options.profile = args
                        .next()
                        .ok_or_else(|| "--profile requires a name".to_string())?;
                }
                "--update" => options.mode = Mode::Update,
                "--add-site" => {
                    options.mode = Mode::AddSite;
                    options.site = Some(
                        args.next()
                            .ok_or_else(|| "--add-site requires a name and a URL".to_string())?,
                    );
                    let url = args
                        .next()
                        .ok_or_else(|| "--add-site requires a URL after the name".to_string())?;
                    if !urls::is_http(&url) {
                        return Err(format!("`{url}` is not an http or https URL"));
                    }
                    options.url = Some(url);
                }
                "--remove-site" => {
                    options.mode = Mode::RemoveSite;
                    options.site = Some(
                        args.next()
                            .ok_or_else(|| "--remove-site requires a name".to_string())?,
                    );
                }
                "--list-sites" => options.mode = Mode::ListSites,
                "--domains" => {
                    options.domains = Some(
                        args.next()
                            .ok_or_else(|| "--domains requires a list".to_string())?,
                    );
                }
                other if other.starts_with("--domains=") => {
                    options.domains = Some(other["--domains=".len()..].to_string());
                }
                "--clear-cache" => options.mode = Mode::Clear(Clearable::Cache),
                "--clear-session" => options.mode = Mode::Clear(Clearable::Session),
                other if other.starts_with("--profile=") => {
                    options.profile = other["--profile=".len()..].to_string();
                }
                other if urls::is_http(other) => options.url = Some(other.to_string()),
                other if other.starts_with('-') => {
                    return Err(format!("unknown option `{other}`"));
                }
                other => return Err(format!("unexpected argument `{other}`")),
            }
        }

        Ok(options)
    }
}

fn clear(paths: &paths::Paths, what: Clearable) -> ExitCode {
    let (label, target) = match what {
        Clearable::Cache => ("cache", &paths.cache),
        Clearable::Session => ("session data", &paths.data),
    };
    match paths::purge(target) {
        Ok(()) => {
            println!(
                "Cleared {label} for profile `{}`: {}",
                paths.profile,
                target.display()
            );
            if what == Clearable::Session {
                println!("You will be asked to sign in again on the next launch.");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("instacache: could not clear {label}: {err}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> String {
    format!(
        "\
{APP_NAME} {VERSION} — a native, ultra-light Instagram client for Linux.

USAGE:
    instacache [OPTIONS] [URL]

ARGS:
    <URL>    Open this Instagram URL instead of the configured home page.

OPTIONS:
    -p, --profile <NAME>   Use an isolated session, cache and config directory.
                           Lets several accounts run at the same time.
        --add-site <NAME> <URL>
                           Add a site to your application menu, with its own
                           icon, window, session and cache.
        --domains <LIST>   Hosts that site may open, comma-separated. Defaults
                           to the URL's own host. Use it when a site loads from
                           a separate CDN, e.g. --domains x.com,twimg.com
        --remove-site <NAME>
                           Take a site out of the menu. Its data is kept.
        --list-sites       Show the sites currently in the menu.
        --update           Check for a newer release and install it.
        --clear-cache      Delete the cached resources, keep the session.
        --clear-session    Delete cookies and site storage (signs you out).
    -h, --help             Show this message.
    -V, --version          Show the version.

FILES:
    ~/.config/instacache/config.json   Settings (created on first run).
    ~/.config/instacache/user.css      Optional user stylesheet.
    ~/.local/share/instacache/         Session: cookies, local storage.
    ~/.cache/instacache/               Resource cache (safe to delete).

SHORTCUTS:
    Ctrl+R / F5            Reload            Alt+Left / Alt+Right   Back / Forward
    Ctrl+Shift+R           Reload, no cache  Ctrl+H                 Home
    Ctrl+= / Ctrl+- / Ctrl+0   Zoom          F11                    Fullscreen
    Ctrl+W / Ctrl+Q        Quit
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_to_running_the_default_profile() {
        let options = parse(&[]).unwrap();
        assert_eq!(options.mode, Mode::Run);
        assert_eq!(options.profile, paths::DEFAULT_PROFILE);
        assert!(options.url.is_none());
    }

    #[test]
    fn accepts_both_profile_spellings() {
        assert_eq!(parse(&["--profile", "work"]).unwrap().profile, "work");
        assert_eq!(parse(&["--profile=work"]).unwrap().profile, "work");
        assert!(parse(&["--profile"]).is_err());
    }

    #[test]
    fn accepts_a_url_positional() {
        let options = parse(&["https://www.instagram.com/direct/inbox/"]).unwrap();
        assert_eq!(
            options.url.as_deref(),
            Some("https://www.instagram.com/direct/inbox/")
        );
    }

    #[test]
    fn rejects_unknown_flags_and_stray_words() {
        assert!(parse(&["--nope"]).is_err());
        assert!(parse(&["banana"]).is_err());
    }

    #[test]
    fn adding_a_site_needs_a_name_and_a_url() {
        let options = parse(&["--add-site", "X", "https://x.com/"]).unwrap();
        assert_eq!(options.mode, Mode::AddSite);
        assert_eq!(options.site.as_deref(), Some("X"));
        assert_eq!(options.url.as_deref(), Some("https://x.com/"));

        assert!(parse(&["--add-site"]).is_err());
        assert!(parse(&["--add-site", "X"]).is_err());
        // A menu entry that runs something other than a web page is not a site.
        assert!(parse(&["--add-site", "X", "file:///etc/passwd"]).is_err());
    }

    #[test]
    fn a_sites_hosts_can_be_stated() {
        for spelling in [
            vec![
                "--add-site",
                "X",
                "https://x.com/",
                "--domains",
                "x.com,twimg.com",
            ],
            vec![
                "--add-site",
                "X",
                "https://x.com/",
                "--domains=x.com,twimg.com",
            ],
        ] {
            let options = parse(&spelling).unwrap();
            assert_eq!(options.domains.as_deref(), Some("x.com,twimg.com"));
        }
    }

    #[test]
    fn sites_can_be_listed_and_removed() {
        assert_eq!(parse(&["--list-sites"]).unwrap().mode, Mode::ListSites);
        let options = parse(&["--remove-site", "x"]).unwrap();
        assert_eq!(options.mode, Mode::RemoveSite);
        assert_eq!(options.site.as_deref(), Some("x"));
        assert!(parse(&["--remove-site"]).is_err());
    }

    #[test]
    fn update_mode_is_recognised() {
        assert_eq!(parse(&["--update"]).unwrap().mode, Mode::Update);
    }

    #[test]
    fn clear_modes_are_recognised() {
        assert_eq!(
            parse(&["--clear-cache"]).unwrap().mode,
            Mode::Clear(Clearable::Cache)
        );
        assert_eq!(
            parse(&["--clear-session", "-p", "alt"]).unwrap().profile,
            "alt"
        );
    }
}
