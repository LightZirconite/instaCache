//! WebKit context, persistent storage and the web view itself.
//!
//! The two properties that make instaCache feel fast are configured here:
//!   * `CacheModel::WebBrowser` — WebKit's most generous memory/disk cache
//!     budget, plus the back/forward page cache.
//!   * a `WebsiteDataManager` rooted in the profile directories, so the HTTP
//!     cache, service workers, IndexedDB and the cookie jar all survive across
//!     runs instead of being rebuilt on every launch.

use gtk::gio;
use gtk::prelude::*;
use webkit2gtk::{
    CacheModel, CookieAcceptPolicy, CookieManagerExt, CookiePersistentStorage,
    HardwareAccelerationPolicy, NavigationPolicyDecision, NavigationPolicyDecisionExt,
    PolicyDecision, PolicyDecisionExt, PolicyDecisionType, Settings, SettingsExt, URIRequestExt,
    UserContentInjectedFrames, UserContentManager, UserContentManagerExt, UserStyleLevel,
    UserStyleSheet, WebContext, WebContextExt, WebView, WebViewExt, WebsiteDataManager,
    WebsiteDataManagerExt,
};

use crate::config::Config;
use crate::paths::Paths;
use crate::urls;

pub struct Browser {
    pub context: WebContext,
    pub view: WebView,
}

pub fn build(config: &Config, paths: &Paths) -> Browser {
    if config.hardware_video_decoding {
        prefer_gpu_video_decoders();
    }

    let data_manager = WebsiteDataManager::builder()
        .base_data_directory(paths.data.to_string_lossy().as_ref())
        .base_cache_directory(paths.cache.to_string_lossy().as_ref())
        .build();

    // Persist cookies to SQLite. Without this the session lives in memory only
    // and every launch would land on the login page.
    if let Some(cookies) = data_manager.cookie_manager() {
        cookies.set_persistent_storage(
            paths.cookie_jar().to_string_lossy().as_ref(),
            CookiePersistentStorage::Sqlite,
        );
        // Instagram's login and Accounts Center flows bounce through
        // facebook.com, so third-party cookies must be accepted.
        cookies.set_accept_policy(CookieAcceptPolicy::Always);
    }

    let context = WebContext::with_website_data_manager(&data_manager);
    context.set_cache_model(CacheModel::WebBrowser);

    let languages = preferred_languages();
    context.set_preferred_languages(&languages.iter().map(String::as_str).collect::<Vec<_>>());

    if config.spell_checking_languages.is_empty() {
        context.set_spell_checking_enabled(false);
    } else {
        context.set_spell_checking_enabled(true);
        let langs: Vec<&str> = config
            .spell_checking_languages
            .iter()
            .map(String::as_str)
            .collect();
        context.set_spell_checking_languages(&langs);
    }

    let settings = build_settings(config);
    let user_content = UserContentManager::new();
    install_user_stylesheet(&user_content, paths);

    let view = WebView::builder()
        .web_context(&context)
        .settings(&settings)
        .user_content_manager(&user_content)
        .build();

    connect_link_routing(&view, config.open_external_links_in_browser);

    Browser { context, view }
}

/// GPU video decoders GStreamer may have available, in the order WebKit is
/// most likely to need them. Raising their rank is the documented way to tell
/// GStreamer which decoder to reach for first.
const GPU_DECODERS: &[&str] = &[
    // Mesa / Intel / AMD through VA-API, from gst-plugins-bad.
    "vah264dec",
    "vah265dec",
    "vavp8dec",
    "vavp9dec",
    "vaav1dec",
    // The older VA-API plugin, still shipped by some distributions.
    "vaapih264dec",
    "vaapih265dec",
    "vaapivp9dec",
    // Intel Media SDK and NVIDIA, for the machines that have them.
    "msdkh264dec",
    "nvh264dec",
    "nvh265dec",
];

/// WebKit decodes video through GStreamer, and GStreamer ranks the libav
/// software decoders at the same level as the hardware ones, so which decoder
/// gets used is effectively arbitrary. Software decoding a Reel is what
/// produces stutter and single-frame freezes on a laptop or a handheld.
///
/// Ranks are read from the environment when GStreamer initialises, and the
/// variable is inherited by the WebProcess, so setting it here — before the
/// WebKit context exists — reaches the process that actually decodes.
///
/// Names that no plugin provides are ignored, and if a preferred decoder fails
/// to negotiate, GStreamer still falls back to the next candidate. An explicit
/// `GST_PLUGIN_FEATURE_RANK` from the user is never overwritten.
fn prefer_gpu_video_decoders() {
    const VAR: &str = "GST_PLUGIN_FEATURE_RANK";
    if std::env::var_os(VAR).is_some() {
        return;
    }
    let ranks: Vec<String> = GPU_DECODERS
        .iter()
        .map(|decoder| format!("{decoder}:MAX"))
        .collect();
    std::env::set_var(VAR, ranks.join(","));
}

fn build_settings(config: &Config) -> Settings {
    let settings = Settings::new();

    if !config.user_agent.trim().is_empty() {
        settings.set_user_agent(Some(config.user_agent.trim()));
    }

    // Speed.
    settings.set_enable_page_cache(true);
    settings.set_enable_smooth_scrolling(true);
    settings.set_hardware_acceleration_policy(hardware_policy(&config.hardware_acceleration));
    settings.set_enable_webgl(true);

    // Storage the Instagram web app relies on.
    settings.set_enable_html5_local_storage(true);
    settings.set_enable_html5_database(true);

    // WebKit ships per-site workarounds, several of which target instagram.com.
    settings.set_enable_site_specific_quirks(true);

    // Feed, Stories and Reels playback.
    settings.set_enable_media(true);
    settings.set_enable_mediasource(true);
    // Lets the page ask which codecs and resolutions decode smoothly here, so
    // Instagram can pick a stream this machine can actually sustain.
    settings.set_enable_media_capabilities(true);
    settings.set_enable_encrypted_media(true);
    settings.set_enable_webaudio(true);
    settings.set_media_playback_requires_user_gesture(false);
    settings.set_enable_fullscreen(true);

    // "Copy link" buttons.
    settings.set_javascript_can_access_clipboard(true);

    // Touchpad swipe to go back/forward.
    settings.set_enable_back_forward_navigation_gestures(true);

    // Keep the window under our own control: the page cannot spawn modal
    // dialogs that would block the GTK main loop.
    settings.set_allow_modal_dialogs(false);

    settings.set_enable_developer_extras(config.developer_tools);
    settings.set_enable_write_console_messages_to_stdout(config.developer_tools);

    settings
}

fn hardware_policy(value: &str) -> HardwareAccelerationPolicy {
    match value.trim().to_ascii_lowercase().as_str() {
        "always" | "on" | "force" => HardwareAccelerationPolicy::Always,
        "never" | "off" | "disabled" => HardwareAccelerationPolicy::Never,
        // `auto` and anything unrecognised: let WebKit decide per page, which
        // is the only setting that degrades gracefully without a GPU.
        _ => HardwareAccelerationPolicy::OnDemand,
    }
}

/// Loads `~/.config/instacache/user.css` if present, so users can restyle the
/// site without rebuilding.
fn install_user_stylesheet(manager: &UserContentManager, paths: &Paths) {
    let path = paths.user_stylesheet();
    let Ok(source) = std::fs::read_to_string(&path) else {
        return;
    };
    if source.trim().is_empty() {
        return;
    }
    manager.add_style_sheet(&UserStyleSheet::new(
        &source,
        UserContentInjectedFrames::TopFrame,
        UserStyleLevel::User,
        &[],
        &[],
    ));
}

/// Builds an `Accept-Language` list from the active locale, always ending with
/// English as a fallback.
fn preferred_languages() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let raw = ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default();

    for entry in raw.split(':') {
        // `fr_FR.UTF-8@euro` -> `fr-FR`
        let entry = entry
            .split(['.', '@'])
            .next()
            .unwrap_or("")
            .replace('_', "-");
        if entry.is_empty() || entry == "C" || entry == "POSIX" {
            continue;
        }
        push_unique(&mut out, entry.clone());
        if let Some((base, _)) = entry.split_once('-') {
            push_unique(&mut out, base.to_string());
        }
    }
    push_unique(&mut out, "en".to_string());
    out
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !list.iter().any(|v| v.eq_ignore_ascii_case(&value)) {
        list.push(value);
    }
}

/// Keeps Instagram (and the Meta hosts its login flow needs) inside the window
/// and hands everything else to the system browser.
fn connect_link_routing(view: &WebView, external_in_browser: bool) {
    view.connect_decide_policy(move |view, decision, decision_type| {
        let Some(uri) = navigation_uri(decision, decision_type) else {
            return false;
        };

        // Never intercept the engine's own schemes: blob: URLs back video
        // playback and downloads, about:blank backs popups.
        if urls::is_engine_scheme(&uri) {
            return false;
        }

        match decision_type {
            // `target="_blank"` and `window.open`. WebKit would otherwise ask
            // us to create a second view, which a single-window app has
            // nowhere to put.
            PolicyDecisionType::NewWindowAction => {
                if urls::is_internal(&uri) {
                    view.load_uri(&uri);
                } else {
                    open_externally(&uri);
                }
                decision.ignore();
                true
            }
            PolicyDecisionType::NavigationAction => {
                if !external_in_browser || urls::is_internal(&uri) {
                    return false;
                }
                open_externally(&uri);
                decision.ignore();
                true
            }
            _ => false,
        }
    });
}

fn navigation_uri(decision: &PolicyDecision, decision_type: PolicyDecisionType) -> Option<String> {
    if !matches!(
        decision_type,
        PolicyDecisionType::NavigationAction | PolicyDecisionType::NewWindowAction
    ) {
        return None;
    }
    let decision = decision.downcast_ref::<NavigationPolicyDecision>()?;
    decision
        .navigation_action()?
        .request()?
        .uri()
        .map(Into::into)
}

fn open_externally(uri: &str) {
    if let Err(err) = gio::AppInfo::launch_default_for_uri(uri, gio::AppLaunchContext::NONE) {
        eprintln!("instacache: could not open {uri} externally: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_policy_parsing() {
        assert!(matches!(
            hardware_policy("always"),
            HardwareAccelerationPolicy::Always
        ));
        assert!(matches!(
            hardware_policy("NEVER"),
            HardwareAccelerationPolicy::Never
        ));
        assert!(matches!(
            hardware_policy("auto"),
            HardwareAccelerationPolicy::OnDemand
        ));
        assert!(matches!(
            hardware_policy("nonsense"),
            HardwareAccelerationPolicy::OnDemand
        ));
    }

    #[test]
    fn language_list_is_derived_and_deduplicated() {
        let mut list = Vec::new();
        push_unique(&mut list, "fr-FR".into());
        push_unique(&mut list, "fr-fr".into());
        push_unique(&mut list, "en".into());
        assert_eq!(list, vec!["fr-FR".to_string(), "en".to_string()]);
    }
}
