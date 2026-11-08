//! Built-in mods: small changes to the page itself, the way Vencord changes
//! Discord, rather than features of the window around it.
//!
//! Each mod is a script compiled into the binary, switched by a setting of
//! its own in `config.json`, and run in the page's own world at document
//! creation — before Instagram's code — through the profile's user scripts.
//! A mod that throws is caught and logged, and the page carries on.
//!
//! `~/.config/instacache/user.js` and `user.css` stay what they were: the
//! user's own changes, applied after each load.

use serde_json::json;

use crate::config::Config;

/// One mod: the setting that switches it, and its source.
struct Mod {
    setting: &'static str,
    source: &'static str,
}

const BUILT_IN: &[Mod] = &[Mod {
    setting: "videos_with_sound",
    source: include_str!("mods/videos_with_sound.js"),
}];

/// Whether the mod behind `setting` is switched on, or `None` for a setting
/// that is not a mod's.
fn enabled(config: &Config, setting: &str) -> Option<bool> {
    match setting {
        "videos_with_sound" => Some(config.videos_with_sound),
        _ => None,
    }
}

/// What the mods are told about the configuration.
fn options(config: &Config) -> serde_json::Value {
    json!({
        "volume": f64::from(config.video_volume.min(100)) / 100.0,
    })
}

/// The one script that carries every enabled mod, or an empty string when
/// none is. Each mod runs in its own function, so a mistake in one neither
/// reaches the page nor stops the others.
pub fn script(config: &Config) -> String {
    let mods: Vec<&Mod> = BUILT_IN
        .iter()
        .filter(|m| enabled(config, m.setting) == Some(true))
        .collect();
    if mods.is_empty() {
        return String::new();
    }
    let mut out = String::from("(function (options) {\n\"use strict\";\n");
    for m in mods {
        out.push_str(&format!(
            "try {{ (function () {{\n{}\n}})(); }} catch (e) {{ \
             console.error(\"instacache: mod {} failed:\", e); }}\n",
            m.source, m.setting
        ));
    }
    out.push_str(&format!("}})({});\n", options(config)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_enabled_mods_are_carried() {
        let mut config = Config {
            videos_with_sound: true,
            ..Config::default()
        };
        assert!(script(&config).contains("Videos with sound"));
        config.videos_with_sound = false;
        assert_eq!(script(&config), "");
    }

    #[test]
    fn the_volume_reaches_the_mods_and_never_exceeds_full() {
        let mut config = Config {
            videos_with_sound: true,
            video_volume: 40,
            ..Config::default()
        };
        assert!(script(&config).contains(r#"{"volume":0.4}"#));
        config.video_volume = 250;
        assert!(script(&config).contains(r#"{"volume":1.0}"#));
    }

    #[test]
    fn every_built_in_mod_has_a_setting() {
        for m in BUILT_IN {
            assert!(
                enabled(&Config::default(), m.setting).is_some(),
                "{} is not wired to a setting",
                m.setting
            );
        }
    }
}
