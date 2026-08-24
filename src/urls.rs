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
/// would break signing in.
pub const INTERNAL_DOMAINS: &[&str] = &[
    "instagram.com",
    "cdninstagram.com",
    "facebook.com",
    "fbcdn.net",
    "fb.com",
    "messenger.com",
    "meta.com",
    "threads.com",
    "threads.net",
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

/// Whether a navigation belongs inside the app window.
pub fn is_internal(uri: &str) -> bool {
    if !is_http(uri) {
        return false;
    }
    let Some(host) = host_of(uri) else {
        return false;
    };
    INTERNAL_DOMAINS
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
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
