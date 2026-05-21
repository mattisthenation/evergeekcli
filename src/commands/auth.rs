//! `auth` command group: login / status / logout.
//!
//! Signatures are frozen per the FROZEN COMMAND CONTRACT. These commands take
//! no client; they use [`crate::auth::oauth`] and [`crate::config`].
//!
//! Note on testing: these commands ultimately touch process-wide state
//! (`EVERGEEK_*` env vars, the on-disk credentials file). Mutating env vars in
//! tests would race with `config`'s own tests inside the same test binary, so
//! the human-readable status text is factored into the pure [`render_status`]
//! and that is what we unit-test.

use chrono::TimeZone;

use crate::auth::oauth;
use crate::config::{self, Credentials};
use crate::error::Result;

/// Run the OAuth login flow, persist the resulting credentials, and return a
/// short confirmation line.
pub fn login(sandbox: bool) -> Result<String> {
    let creds = oauth::login(sandbox)?;
    config::save_credentials(&creds)?;
    Ok(format!(
        "Authenticated as user {} ({}).",
        creds.user_id,
        if creds.sandbox {
            "sandbox"
        } else {
            "production"
        }
    ))
}

/// Report the current authentication state. Never prints the auth token.
pub fn status() -> Result<String> {
    Ok(render_status(config::load_credentials()?.as_ref()))
}

/// Forget the stored credentials.
pub fn logout() -> Result<String> {
    config::delete_credentials()?;
    Ok("Logged out — stored credentials removed.".into())
}

/// Render the human-readable status text for the given credentials.
///
/// Pure and side-effect free so it can be tested without touching env vars or
/// the filesystem. The auth token is *never* included in the output.
pub fn render_status(creds: Option<&Credentials>) -> String {
    match creds {
        None => "Not authenticated — run `evergeek auth login`.".to_string(),
        Some(c) => {
            let environment = if c.sandbox { "sandbox" } else { "production" };
            format!(
                "Authenticated.\n  User ID:    {user}\n  Environment: {env}\n  Note store: {host}\n  Expires:    {expires}",
                user = c.user_id,
                env = environment,
                host = note_store_host(&c.note_store_url),
                expires = format_expiry(c.expires),
            )
        }
    }
}

/// Extract the host portion of the note-store URL for compact display, falling
/// back to the full URL if it can't be parsed.
fn note_store_host(note_store_url: &str) -> String {
    url::Url::parse(note_store_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| note_store_url.to_string())
}

/// Format an Evernote expiry timestamp (ms since epoch) as a human-readable
/// UTC datetime. Out-of-range values fall back to the raw number.
fn format_expiry(expires_ms: i64) -> String {
    match chrono::Utc.timestamp_millis_opt(expires_ms) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        _ => format!("{expires_ms} (ms since epoch)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_credentials() -> Credentials {
        Credentials {
            auth_token: "S=s1:U=abc:E=18a:SUPER-SECRET-TOKEN".to_string(),
            note_store_url: "https://sandbox.evernote.com/shard/s1/notestore".to_string(),
            web_api_url_prefix: "https://sandbox.evernote.com/shard/s1/".to_string(),
            user_id: 12345,
            shard: "s1".to_string(),
            // 2023-11-14T22:13:20Z
            expires: 1_700_000_000_000,
            sandbox: true,
        }
    }

    #[test]
    fn render_status_none_says_not_authenticated() {
        let text = render_status(None);
        assert!(text.to_lowercase().contains("not authenticated"));
        assert!(text.contains("auth login"));
    }

    #[test]
    fn render_status_some_shows_fields_and_never_the_token() {
        let creds = sample_credentials();
        let text = render_status(Some(&creds));

        // Required fields present.
        assert!(text.contains("12345"), "missing user_id: {text}");
        assert!(text.contains("sandbox"), "missing environment: {text}");
        assert!(
            text.contains("sandbox.evernote.com"),
            "missing note-store host: {text}"
        );
        // Expiry rendered as a human date, not the raw millis.
        assert!(text.contains("2023-11-14"), "missing human expiry: {text}");

        // The secret token must never appear, in whole or in part.
        assert!(
            !text.contains(&creds.auth_token),
            "status leaked the auth token: {text}"
        );
        assert!(
            !text.contains("SUPER-SECRET-TOKEN"),
            "status leaked the auth token: {text}"
        );
    }

    #[test]
    fn render_status_production_says_production() {
        let mut creds = sample_credentials();
        creds.sandbox = false;
        creds.note_store_url = "https://www.evernote.com/shard/s100/notestore".to_string();
        let text = render_status(Some(&creds));
        assert!(text.contains("production"), "{text}");
        assert!(!text.contains("sandbox"), "{text}");
        assert!(text.contains("www.evernote.com"), "{text}");
    }

    #[test]
    fn note_store_host_extracts_host_or_falls_back() {
        assert_eq!(
            note_store_host("https://sandbox.evernote.com/shard/s1/notestore"),
            "sandbox.evernote.com"
        );
        // Unparseable input falls back to the raw string.
        assert_eq!(note_store_host("not a url"), "not a url");
    }

    #[test]
    fn format_expiry_renders_known_timestamp() {
        assert_eq!(format_expiry(1_700_000_000_000), "2023-11-14 22:13:20 UTC");
    }
}
