//! Minimal URI inspection.
//!
//! Deliberately dependency-free: instaCache only needs the scheme and the host
//! to decide whether a navigation stays in the app or is handed to the system
//! browser, which does not justify pulling in a full URL parser.

/// Domains that make up the Instagram experience. A navigation is kept inside
/// the app when its host is one of these or a sub-domain of one.
///
/// `facebook.com` / `meta.com` are included because Instagram's login,
/// two-factor and Accounts Center flows redirect through them; dropping them
/// would break signing in. That is the only reason a non-Instagram host is
/// here, and it is the test for adding another.
///
/// Threads used to be in this list. It was never needed for anything —
/// Instagram works without it — so a click on Instagram's Threads button now
/// leaves for the system browser like any other link to another application.
/// Anyone who wants it back can name it in `internal_domains`.
pub const INTERNAL_DOMAINS: &[&str] = &[
    "instagram.com",
    "cdninstagram.com",
    "facebook.com",
    "fbcdn.net",
    "fb.com",
    "messenger.com",
    "meta.com",
];

/// Schemes WebKit must keep handling itself; they carry no host and are used
/// internally by the page (iframes, blob downloads, inline data URIs).
const ENGINE_SCHEMES: &[&str] = &["about", "blob", "data", "javascript", "file", "webkit"];

pub fn scheme_of(uri: &str) -> Option<&str> {
    let (scheme, _) = uri.split_once(':')?;
    if scheme.is_empty()
        || !scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return None;
    }
    Some(scheme)
}

pub fn host_of(uri: &str) -> Option<String> {
    let (_, rest) = uri.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    // Strip any `user:password@` prefix before reading the host.
    let authority = match authority.rsplit_once('@') {
        Some((_, host)) => host,
        None => authority,
    };
    let host = if let Some(v6) = authority.strip_prefix('[') {
        v6.split_once(']')?.0
    } else {
        authority.split(':').next()?
    };
    if host.is_empty() {
        None
    } else {
        Some(host.trim_end_matches('.').to_ascii_lowercase())
    }
}

pub fn is_http(uri: &str) -> bool {
    matches!(
        scheme_of(uri).map(str::to_ascii_lowercase).as_deref(),
        Some("http") | Some("https")
    )
}

/// True when WebKit should be left alone to handle the URI.
pub fn is_engine_scheme(uri: &str) -> bool {
    match scheme_of(uri) {
        Some(s) => ENGINE_SCHEMES.contains(&s.to_ascii_lowercase().as_str()),
        // No scheme at all (relative or malformed) — not ours to redirect.
        None => true,
    }
}

/// Whether a navigation belongs inside the app window, against the built-in
/// list. Kept for callers that have no configuration to hand, such as tests.
pub fn is_internal(uri: &str) -> bool {
    is_internal_in(INTERNAL_DOMAINS, uri)
}

/// Whether a navigation belongs inside a window whose allowed domains are
/// `domains`.
///
/// This is an allow-list and nothing else: a host is inside only when it
/// matches an entry exactly or is a sub-domain of one. That is the whole
/// security property of the window — it holds a logged-in session, so
/// everything not named here is handed to the system browser instead.
///
/// The list is per profile so that a second profile can be a dedicated window
/// for a different site. Widening it is the user's decision, and a widened
/// list is still an allow-list: adding `x.com` lets in `x.com` and its
/// sub-domains, not the web.
pub fn is_internal_in<S: AsRef<str>>(domains: &[S], uri: &str) -> bool {
    if !is_http(uri) {
        return false;
    }
    let Some(host) = host_of(uri) else {
        return false;
    };
    domains.iter().any(|domain| {
        let domain = domain.as_ref();
        !domain.is_empty() && (host == domain || host.ends_with(&format!(".{domain}")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_hosts() {
        assert_eq!(
            host_of("https://www.instagram.com/p/abc/"),
            Some("www.instagram.com".into())
        );
        assert_eq!(
            host_of("https://USER:pw@Example.COM:8443/x"),
            Some("example.com".into())
        );
        assert_eq!(host_of("http://[::1]:8080/"), Some("::1".into()));
        assert_eq!(host_of("mailto:a@b.com"), None);
        assert_eq!(host_of("not a url"), None);
    }

    #[test]
    fn recognises_instagram_and_login_partners() {
        assert!(is_internal("https://instagram.com/"));
        assert!(is_internal("https://www.instagram.com/direct/inbox/"));
        assert!(is_internal(
            "https://scontent-cdg4-1.cdninstagram.com/v/x.jpg"
        ));
        assert!(is_internal("https://accountscenter.instagram.com/"));
        assert!(is_internal("https://m.facebook.com/login"));
    }

    #[test]
    fn rejects_lookalike_and_external_hosts() {
        // The suffix check must not match a domain that merely ends with the
        // same characters.
        assert!(!is_internal("https://notinstagram.com/"));
        assert!(!is_internal("https://instagram.com.evil.example/"));
        assert!(!is_internal("https://youtube.com/watch?v=1"));
        assert!(!is_internal("ftp://instagram.com/"));
    }

    #[test]
    fn threads_is_another_application_and_leaves() {
        assert!(!is_internal("https://www.threads.com/@someone"));
        assert!(!is_internal("https://threads.net/"));
        // Still reachable for anyone who names it themselves.
        assert!(is_internal_in(
            &["threads.com".to_string()],
            "https://www.threads.com/"
        ));
    }

    #[test]
    fn a_configured_list_is_still_an_allow_list() {
        // A profile pointed at another site lets that site in, and nothing
        // else -- including the sites the built-in list would have allowed.
        let mine = ["x.com".to_string()];
        assert!(is_internal_in(&mine, "https://x.com/home"));
        assert!(is_internal_in(&mine, "https://mobile.x.com/home"));
        assert!(!is_internal_in(&mine, "https://instagram.com/"));

        // The lookalike checks must survive a hand-written list.
        assert!(!is_internal_in(&mine, "https://notx.com/"));
        assert!(!is_internal_in(&mine, "https://x.com.evil.example/"));
        assert!(!is_internal_in(&mine, "ftp://x.com/"));
    }

    #[test]
    fn an_empty_entry_never_matches() {
        // An empty string would otherwise turn `host.ends_with(".")` into a
        // wildcard and let the whole web in.
        let sloppy = ["".to_string()];
        assert!(!is_internal_in(&sloppy, "https://anything.example/"));
        assert!(!is_internal_in(&sloppy, "https://evil.example./"));
    }

    #[test]
    fn classifies_schemes() {
        assert!(is_engine_scheme("about:blank"));
        assert!(is_engine_scheme("blob:https://instagram.com/uuid"));
        assert!(is_engine_scheme("data:text/html,hi"));
        assert!(!is_engine_scheme("https://example.com"));
        assert!(!is_engine_scheme("mailto:hi@example.com"));
        assert_eq!(scheme_of("https://x"), Some("https"));
        assert_eq!(scheme_of("1abc:x"), None);
    }
}
