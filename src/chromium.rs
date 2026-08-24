//! Translating instaCache's settings into Chromium command-line flags.
//!
//! Qt WebEngine reads `QTWEBENGINE_CHROMIUM_FLAGS` from the environment when
//! it initialises, which has to happen before any Qt application object
//! exists. That is the only window in which these settings can be applied, so
//! they are computed here — as a pure function over the config, which makes
//! them testable without starting a browser.
//!
//! The two video settings kept their names across the engine change on
//! purpose: a `config.json` written by an older version still means what it
//! said. What changed is how the preference is carried out. Under WebKit it
//! reordered GStreamer's plugin ranks; Chromium does not use GStreamer at all,
//! so the same intent becomes a feature flag.

use crate::config::Config;

/// Chromium's Linux VA-API decoding, which is off by default in the version
/// Qt WebEngine embeds. `VaapiVideoDecodeLinuxGL` is the flag that covers the
/// GL-backed path Qt uses; `VaapiIgnoreDriverChecks` keeps drivers that
/// Chromium's allow-list has not heard of from being refused outright.
const GPU_DECODING: &str = "--enable-features=VaapiVideoDecodeLinuxGL,VaapiIgnoreDriverChecks \
--ignore-gpu-blocklist";

/// Leaves decoding to the CPU without switching off compositing as well.
const SOFTWARE_DECODING: &str = "--disable-accelerated-video-decode";

/// Everything on the CPU, including page compositing.
const NO_ACCELERATION: &str = "--disable-gpu --disable-gpu-compositing";

/// Builds the flag string for a configuration.
///
/// Anything the user already put in `QTWEBENGINE_CHROMIUM_FLAGS` is kept and
/// placed last, so it wins: a wrapper script or a distribution package must
/// still be able to override what the config asked for.
pub fn flags(config: &Config, inherited: Option<&str>) -> String {
    let mut parts: Vec<&str> = Vec::new();

    match config
        .hardware_acceleration
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "never" | "off" | "false" => parts.push(NO_ACCELERATION),
        // `always` and `auto` both leave Chromium's own decision alone: unlike
        // WebKit it does not switch compositing modes mid-page, which was the
        // reason that setting existed.
        _ => {}
    }

    // Asking for the CPU while acceleration is off entirely would be
    // redundant, and asking for the GPU would contradict it.
    if !parts.contains(&NO_ACCELERATION) {
        match config.video_decoding.trim().to_ascii_lowercase().as_str() {
            "software" | "cpu" | "libav" => parts.push(SOFTWARE_DECODING),
            // `auto`, and anything unrecognised, which is the safe reading of
            // a typo in a config file.
            "auto" | "default" => {}
            _ => parts.push(GPU_DECODING),
        }
    }

    let inherited = inherited.unwrap_or("").trim();
    if !inherited.is_empty() {
        parts.push(inherited);
    }
    parts.join(" ")
}

/// Computes the flags and puts them where Qt WebEngine will read them.
pub fn apply(config: &Config) {
    const VAR: &str = "QTWEBENGINE_CHROMIUM_FLAGS";
    let inherited = std::env::var(VAR).ok();
    let value = flags(config, inherited.as_deref());
    if value.is_empty() {
        return;
    }
    std::env::set_var(VAR, value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(hardware: &str, video: &str) -> Config {
        Config {
            hardware_acceleration: hardware.to_string(),
            video_decoding: video.to_string(),
            ..Config::default()
        }
    }

    #[test]
    fn the_default_asks_for_gpu_decoding() {
        let flags = flags(&Config::default(), None);
        assert!(flags.contains("VaapiVideoDecodeLinuxGL"), "{flags}");
    }

    #[test]
    fn software_decoding_does_not_switch_off_the_gpu_entirely() {
        let flags = flags(&config("always", "software"), None);
        assert!(
            flags.contains("disable-accelerated-video-decode"),
            "{flags}"
        );
        assert!(!flags.contains("--disable-gpu"), "{flags}");
    }

    #[test]
    fn auto_leaves_chromium_alone() {
        assert_eq!(flags(&config("always", "auto"), None), "");
    }

    #[test]
    fn refusing_acceleration_wins_over_the_decoder_preference() {
        let flags = flags(&config("never", "gpu"), None);
        assert!(flags.contains("--disable-gpu"), "{flags}");
        assert!(!flags.contains("Vaapi"), "{flags}");
    }

    #[test]
    fn the_environment_is_kept_and_placed_last() {
        let flags = flags(&Config::default(), Some("--single-process"));
        assert!(flags.ends_with("--single-process"), "{flags}");
        assert!(flags.contains("Vaapi"), "{flags}");
    }

    #[test]
    fn a_typo_falls_back_to_the_default_rather_than_to_nothing() {
        let flags = flags(&config("always", "gpuu"), None);
        assert!(flags.contains("Vaapi"), "{flags}");
    }
}
