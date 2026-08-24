//! instaCache — command-line entry point.
//!
//! Everything of substance lives in the library crate; this file only parses
//! arguments and starts the GTK application.

use std::process::ExitCode;
use std::rc::Rc;

use gtk::prelude::*;

use instacache::{config, paths, ui, urls, APP_ID, APP_NAME, PROGRAM_NAME, VERSION};

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

    let config = Rc::new(config::Config::load_or_create(&paths));
    let paths = Rc::new(paths);

    gtk::glib::set_prgname(Some(PROGRAM_NAME));
    gtk::glib::set_application_name(APP_NAME);

    let app = gtk::Application::builder()
        .application_id(application_id(&paths.profile))
        .build();

    {
        let config = config.clone();
        let paths = paths.clone();
        let start_url = options.url.clone();
        app.connect_activate(move |app| {
            // A second launch of the same profile focuses the existing window
            // instead of starting a duplicate WebKit process tree.
            if let Some(existing) = app.active_window() {
                existing.present();
                return;
            }
            ui::build_window(app, config.clone(), paths.clone(), start_url.clone());
        });
    }

    // GTK must not try to parse our own flags.
    let argv: [&str; 1] = [PROGRAM_NAME];
    let status = app.run_with_args(&argv);
    if status == gtk::glib::ExitCode::SUCCESS {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Non-default profiles get their own D-Bus name so several accounts can run
/// side by side, each with an independent session.
fn application_id(profile: &str) -> String {
    if profile == paths::DEFAULT_PROFILE {
        APP_ID.to_string()
    } else {
        format!("{APP_ID}.{}", paths::dbus_segment(profile))
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
}

#[derive(Debug, Clone)]
struct Options {
    mode: Mode,
    profile: String,
    url: Option<String>,
}

impl Options {
    fn parse<I: Iterator<Item = String>>(args: I) -> Result<Self, String> {
        let mut options = Options {
            mode: Mode::Run,
            profile: paths::DEFAULT_PROFILE.to_string(),
            url: None,
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

    #[test]
    fn application_ids_stay_valid_dbus_names() {
        assert_eq!(application_id("default"), APP_ID);
        assert_eq!(application_id("work-2"), format!("{APP_ID}.work_2"));
    }
}
