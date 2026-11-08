//! Built-in mods: small changes to the page itself, the way Vencord changes
//! Discord, rather than features of the window around it.
//!
//! Each mod is a script, a stylesheet or both, compiled into the binary and
//! switched by a setting of its own in `config.json`. They run in the page's
//! own world at document creation — before Instagram's code — through the
//! profile's user scripts. A mod that throws is caught and logged, and the
//! page carries on.
//!
//! `~/.config/instacache/user.js` and `user.css` stay what they were: the
//! user's own changes, applied after each load.

use serde_json::json;

use crate::config::Config;

/// One mod: the setting that switches it, and what it adds to the page.
struct Mod {
    setting: &'static str,
    script: Option<&'static str>,
    style: Option<&'static str>,
}

const BUILT_IN: &[Mod] = &[
    Mod {
        setting: "videos_with_sound",
        script: Some(include_str!("mods/videos_with_sound.js")),
        style: None,
    },
    Mod {
        setting: "instagram_colors",
        script: Some(include_str!("mods/instagram_colors.js")),
        style: Some(include_str!("mods/instagram_colors.css")),
    },
    Mod {
        setting: "page_animations",
        script: None,
        style: Some(include_str!("mods/animations.css")),
    },
    Mod {
        setting: "shortcuts",
        script: Some(include_str!("mods/shortcuts.js")),
        style: None,
    },
    Mod {
        setting: "hide_also_from_meta",
        script: Some(include_str!("mods/hide_meta.js")),
        style: None,
    },
];

/// What every mod may use: Instagram's labels for its controls, and ways of
/// finding a post, the media in view, and a change of page.
const PAGE: &str = include_str!("mods/page.js");

/// Whether the mod behind `setting` is switched on, or `None` for a setting
/// that is not a mod's.
fn enabled(config: &Config, setting: &str) -> Option<bool> {
    match setting {
        "videos_with_sound" => Some(config.videos_with_sound),
        "instagram_colors" => Some(config.instagram_colors),
        "page_animations" => Some(config.page_animations),
        "shortcuts" => Some(config.shortcuts),
        "hide_also_from_meta" => Some(config.hide_also_from_meta),
        _ => None,
    }
}

/// What the mods are told about the configuration.
fn options(config: &Config) -> serde_json::Value {
    json!({
        "volume": f64::from(config.video_volume.min(100)) / 100.0,
        "ambient": config.ambient_colors,
        // A right-click is the engine's menu when the user asked for it,
        // and a repost otherwise.
        "contextMenu": config.context_menu,
    })
}

/// Puts a stylesheet into the document as soon as it has somewhere to go.
/// At document creation there is no `<head>` yet.
const ADD_STYLE: &str = r#"function addStyle(id, css) {
    function add() {
        var parent = document.head || document.documentElement;
        if (!parent)
            return false;
        var style = document.createElement("style");
        style.id = id;
        style.textContent = css;
        parent.appendChild(style);
        return true;
    }
    if (!add()) {
        new MutationObserver(function (_, observer) {
            if (add())
                observer.disconnect();
        }).observe(document, { childList: true, subtree: true });
    }
}
"#;

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
    out.push_str(ADD_STYLE);
    out.push_str(PAGE);
    for m in mods {
        let mut body = String::new();
        if let Some(style) = m.style {
            body.push_str(&format!(
                "addStyle({}, {});\n",
                json!(format!("instacache-mod-{}", m.setting)),
                json!(style)
            ));
        }
        if let Some(script) = m.script {
            body.push_str(script);
        }
        out.push_str(&format!(
            "try {{ (function () {{\n{body}\n}})(); }} catch (e) {{ \
             console.error(\"instacache: mod {} failed:\", e); }}\n",
            m.setting
        ));
    }
    out.push_str(&format!("}})({});\n", options(config)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none_enabled() -> Config {
        Config {
            videos_with_sound: false,
            instagram_colors: false,
            page_animations: false,
            shortcuts: false,
            hide_also_from_meta: false,
            ..Config::default()
        }
    }

    #[test]
    fn only_enabled_mods_are_carried() {
        assert_eq!(script(&none_enabled()), "");
        let config = Config {
            videos_with_sound: true,
            ..none_enabled()
        };
        let carried = script(&config);
        assert!(carried.contains("Videos with sound"));
        assert!(!carried.contains("instacache-mod-instagram_colors"));
    }

    #[test]
    fn a_stylesheet_is_carried_as_a_string_the_page_cannot_break_out_of() {
        let config = Config {
            page_animations: true,
            ..none_enabled()
        };
        let carried = script(&config);
        assert!(carried.contains(r#"addStyle("instacache-mod-page_animations", "/* Small"#));
        // Quoted by serde: no raw newline or unescaped quote of the CSS
        // reaches the script's own syntax.
        assert!(!carried.contains("@keyframes instacache-fade-in {\n"));
    }

    #[test]
    fn the_volume_reaches_the_mods_and_never_exceeds_full() {
        let mut config = Config {
            videos_with_sound: true,
            video_volume: 40,
            ..none_enabled()
        };
        assert!(script(&config).contains(r#""volume":0.4"#));
        config.video_volume = 250;
        assert!(script(&config).contains(r#""volume":1.0"#));
    }

    #[test]
    fn the_shared_helpers_come_once_and_before_the_mods() {
        let config = Config {
            shortcuts: true,
            videos_with_sound: true,
            ..none_enabled()
        };
        let carried = script(&config);
        assert_eq!(carried.matches("function postOf(").count(), 1);
        let helpers = carried.find("function postOf(").unwrap_or(usize::MAX);
        let first_mod = carried.find("Videos with sound").unwrap_or(0);
        assert!(helpers < first_mod);
    }

    #[test]
    fn every_built_in_mod_has_a_setting_and_something_to_add() {
        for m in BUILT_IN {
            assert!(
                enabled(&Config::default(), m.setting).is_some(),
                "{} is not wired to a setting",
                m.setting
            );
            assert!(
                m.script.is_some() || m.style.is_some(),
                "{} is empty",
                m.setting
            );
        }
    }
}
