//! OAuth 1.0a, 3-legged, PLAINTEXT signature flow against Evernote.
//!
//! Evernote accepts the OAuth 1.0a PLAINTEXT signature method, which means the
//! signature is simply `{consumer_secret}&{token_secret}` (here the token
//! secret is empty, so it is `{consumer_secret}&`). No HMAC is required, which
//! keeps this module small and fully unit-testable: the URL builders and the
//! response parsers are pure functions, and only [`login`] does any I/O.
//!
//! The flow:
//! 1. GET `{base}/oauth` with the consumer key + PLAINTEXT signature and an
//!    `oauth_callback` pointing at a throwaway local HTTP listener. The
//!    response is a urlencoded body containing a temporary `oauth_token`.
//! 2. Open a browser to `{base}/OAuth.action?oauth_token=...` so the user can
//!    approve access. Evernote redirects back to our callback with an
//!    `oauth_verifier` query parameter, which we read off the single inbound
//!    HTTP request.
//! 3. GET `{base}/oauth` again with the temp token + verifier to exchange them
//!    for the long-lived `oauth_token` and the `edam_*` fields, which become
//!    [`Credentials`].

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;

use crate::config::{self, Credentials};
use crate::error::{EvergeekError, Result};

/// Production Evernote base URL.
pub const PROD_BASE: &str = "https://www.evernote.com";
/// Sandbox Evernote base URL.
pub const SANDBOX_BASE: &str = "https://sandbox.evernote.com";

/// Pick the API base URL for the given environment.
fn base_url(sandbox: bool) -> &'static str {
    if sandbox {
        SANDBOX_BASE
    } else {
        PROD_BASE
    }
}

/// Percent-encode a string per RFC 3986 / OAuth: every byte that is not an
/// unreserved character (`A-Z a-z 0-9 - . _ ~`) is encoded as `%XX` with
/// uppercase hex. This matches the encoding Evernote (and the OAuth spec)
/// expects for query parameter values.
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{byte:02X}"));
            }
        }
    }
    out
}

/// Percent-decode a urlencoded value: `%XX` escapes become their byte, `+`
/// becomes a space, everything else passes through. Invalid escapes are left
/// verbatim (we never produce them ourselves; this is for robustness).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                match (hi, lo) {
                    (Some(hi), Some(lo)) => {
                        out.push((hi * 16 + lo) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Build the temporary-token request URL (step 1 of the OAuth flow).
///
/// PLAINTEXT signature: `oauth_signature` is `{consumer_secret}&` with the `&`
/// percent-encoded to `%26` (the trailing empty token secret). `oauth_callback`
/// is fully percent-encoded.
fn temp_token_url(
    base: &str,
    consumer_key: &str,
    consumer_secret: &str,
    callback: &str,
    ts: i64,
    nonce: &str,
) -> String {
    format!(
        "{base}/oauth?oauth_consumer_key={key}&oauth_signature={secret}%26\
         &oauth_signature_method=PLAINTEXT&oauth_timestamp={ts}&oauth_nonce={nonce}\
         &oauth_callback={callback}",
        key = percent_encode(consumer_key),
        secret = percent_encode(consumer_secret),
        nonce = percent_encode(nonce),
        callback = percent_encode(callback),
    )
}

/// Build the access-token request URL (step 3 of the OAuth flow): same PLAINTEXT
/// signature, now carrying the temp `oauth_token` and the `oauth_verifier`.
fn access_token_url(
    base: &str,
    consumer_key: &str,
    consumer_secret: &str,
    temp_token: &str,
    verifier: &str,
    ts: i64,
    nonce: &str,
) -> String {
    format!(
        "{base}/oauth?oauth_consumer_key={key}&oauth_token={token}&oauth_verifier={verifier}\
         &oauth_signature={secret}%26&oauth_signature_method=PLAINTEXT\
         &oauth_timestamp={ts}&oauth_nonce={nonce}",
        key = percent_encode(consumer_key),
        token = percent_encode(temp_token),
        verifier = percent_encode(verifier),
        secret = percent_encode(consumer_secret),
        nonce = percent_encode(nonce),
    )
}

/// Parse a urlencoded OAuth response body (`k=v&k2=v2`) into a map, URL-decoding
/// both keys and values. Empty segments are ignored.
fn parse_oauth_response(body: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in body.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        map.insert(percent_decode(key), percent_decode(value));
    }
    map
}

/// Read a required key from an OAuth response map, mapping absence to an
/// `Api` error so the user sees a clear "Evernote API error" message.
fn require<'a>(map: &'a HashMap<String, String>, key: &str) -> Result<&'a str> {
    map.get(key)
        .map(String::as_str)
        .ok_or_else(|| EvergeekError::Api(format!("OAuth response missing `{key}`")))
}

/// Build [`Credentials`] from a parsed access-token response.
///
/// Required keys: `oauth_token`, `edam_shard`, `edam_userId`, `edam_expires`,
/// `edam_noteStoreUrl`, `edam_webApiUrlPrefix`. A missing or unparseable
/// required field yields an `Api` error.
fn credentials_from_access_response(
    map: &HashMap<String, String>,
    sandbox: bool,
) -> Result<Credentials> {
    let auth_token = require(map, "oauth_token")?.to_string();
    let shard = require(map, "edam_shard")?.to_string();
    let note_store_url = require(map, "edam_noteStoreUrl")?.to_string();
    let web_api_url_prefix = require(map, "edam_webApiUrlPrefix")?.to_string();

    let user_id = require(map, "edam_userId")?
        .parse::<i32>()
        .map_err(|e| EvergeekError::Api(format!("OAuth response has invalid edam_userId: {e}")))?;
    let expires = require(map, "edam_expires")?
        .parse::<i64>()
        .map_err(|e| EvergeekError::Api(format!("OAuth response has invalid edam_expires: {e}")))?;

    Ok(Credentials {
        auth_token,
        note_store_url,
        web_api_url_prefix,
        user_id,
        shard,
        expires,
        sandbox,
    })
}

/// Generate a (timestamp, nonce) pair for an OAuth request. The nonce only
/// needs to be unique per timestamp; a monotonic-ish value derived from the
/// clock is sufficient for PLAINTEXT.
fn timestamp_and_nonce() -> (i64, String) {
    use std::sync::atomic::{AtomicU64, Ordering};
    // A process-wide counter guarantees distinct nonces even for two calls that
    // land in the same nanosecond (e.g. temp + access requests on a fast CPU).
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let ts = now.as_secs() as i64;
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nonce = format!("{}{}{}", now.as_secs(), now.subsec_nanos(), seq);
    (ts, nonce)
}

/// GET `url` and parse its urlencoded body into an OAuth response map.
///
/// Split out from [`login`] so the request/response round-trip can be exercised
/// against an `httpmock` server in tests without driving the whole flow.
fn fetch_oauth(url: &str) -> Result<HashMap<String, String>> {
    let resp = reqwest::blocking::get(url)
        .map_err(|e| EvergeekError::Network(format!("OAuth request failed: {e}")))?;
    let status = resp.status();
    let body = resp
        .text()
        .map_err(|e| EvergeekError::Network(format!("failed to read OAuth response: {e}")))?;
    if !status.is_success() {
        return Err(EvergeekError::Api(format!(
            "OAuth request returned HTTP {status}: {body}"
        )));
    }
    Ok(parse_oauth_response(&body))
}

/// Wait for the single OAuth callback request on `listener`, send back a tiny
/// "you can close this tab" page, and return the `oauth_verifier` query value.
fn wait_for_verifier(listener: &TcpListener) -> Result<String> {
    let (mut stream, _addr) = listener
        .accept()
        .map_err(|e| EvergeekError::Network(format!("failed to accept OAuth callback: {e}")))?;

    // Bound the read: a connection that never sends a complete request (stalled
    // browser, local port scanner) must not hang the CLI forever.
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(120)))
        .map_err(|e| EvergeekError::Network(format!("failed to set callback read timeout: {e}")))?;

    // Accumulate across (possibly fragmented) reads until the end of the HTTP
    // headers, with a hard cap on untrusted local input. The request line —
    // which carries oauth_verifier — is the first line, so this always captures
    // it once headers complete (or the peer closes).
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream
            .read(&mut chunk)
            .map_err(|e| EvergeekError::Network(format!("failed to read OAuth callback: {e}")))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if buf.len() > 16_384 {
            return Err(EvergeekError::Api(
                "OAuth callback request was too large".to_string(),
            ));
        }
    }
    let request = String::from_utf8_lossy(&buf);

    let verifier = extract_verifier(&request).ok_or_else(|| {
        EvergeekError::Api("OAuth callback did not include an oauth_verifier".to_string())
    })?;

    let body = "<html><body>Authentication complete — you can close this tab.</body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    // Best-effort: a failed write here doesn't invalidate the verifier we got.
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();

    Ok(verifier)
}

/// Pull `oauth_verifier` out of an HTTP request's first line
/// (`GET /?oauth_token=...&oauth_verifier=XYZ HTTP/1.1`).
fn extract_verifier(request: &str) -> Option<String> {
    let request_line = request.lines().next()?;
    let path = request_line.split_whitespace().nth(1)?;
    let query = path.split_once('?').map(|(_, q)| q)?;
    parse_oauth_response(query).get("oauth_verifier").cloned()
}

/// Run the full 3-legged OAuth login flow and return fresh credentials.
///
/// Reads the consumer key/secret from the environment, spins up a throwaway
/// localhost listener for the callback, drives the browser-based approval, and
/// exchanges the verifier for long-lived credentials. This orchestration does
/// real network + browser I/O; the pieces it composes are unit-tested above.
pub fn login(sandbox: bool) -> Result<Credentials> {
    let base = base_url(sandbox);
    let consumer_key = config::consumer_key()?;
    let consumer_secret = config::consumer_secret()?;

    // Throwaway listener on an ephemeral port for the OAuth callback.
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| {
        EvergeekError::Network(format!("failed to bind OAuth callback listener: {e}"))
    })?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| EvergeekError::Network(format!("failed to read callback address: {e}")))?;
    // Use the `localhost` hostname (not the 127.0.0.1 literal from SocketAddr):
    // the spec specifies it and Evernote validates the callback against the
    // registered callback domain. The listener still binds to 127.0.0.1.
    let callback = format!("http://localhost:{}/", local_addr.port());

    // Step 1: temporary token.
    let (ts, nonce) = timestamp_and_nonce();
    let temp_url = temp_token_url(base, &consumer_key, &consumer_secret, &callback, ts, &nonce);
    let temp_resp = fetch_oauth(&temp_url)?;
    let temp_token = require(&temp_resp, "oauth_token")?.to_string();

    // Step 2: user approval in the browser.
    let auth_url = format!(
        "{base}/OAuth.action?oauth_token={}",
        percent_encode(&temp_token)
    );
    if webbrowser::open(&auth_url).is_err() {
        eprintln!(
            "Could not open a browser automatically. Visit this URL to authorize:\n{auth_url}"
        );
    }
    let verifier = wait_for_verifier(&listener)?;

    // Step 3: exchange for the access token.
    let (ts, nonce) = timestamp_and_nonce();
    let access_url = access_token_url(
        base,
        &consumer_key,
        &consumer_secret,
        &temp_token,
        &verifier,
        ts,
        &nonce,
    );
    let access_resp = fetch_oauth(&access_url)?;
    credentials_from_access_response(&access_resp, sandbox)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_leaves_unreserved_and_escapes_the_rest() {
        assert_eq!(percent_encode("abcXYZ-._~123"), "abcXYZ-._~123");
        assert_eq!(percent_encode("a&b"), "a%26b");
        assert_eq!(
            percent_encode("http://127.0.0.1:8080/"),
            "http%3A%2F%2F127.0.0.1%3A8080%2F"
        );
    }

    #[test]
    fn temp_token_url_uses_plaintext_signature_and_encodes_callback() {
        let url = temp_token_url(
            "https://sandbox.evernote.com",
            "my-key",
            "topsecret",
            "http://127.0.0.1:8080/",
            1700000000,
            "nonce123",
        );
        assert_eq!(
            url,
            "https://sandbox.evernote.com/oauth?oauth_consumer_key=my-key\
             &oauth_signature=topsecret%26&oauth_signature_method=PLAINTEXT\
             &oauth_timestamp=1700000000&oauth_nonce=nonce123\
             &oauth_callback=http%3A%2F%2F127.0.0.1%3A8080%2F"
        );
        // The PLAINTEXT signature is the consumer secret followed by an
        // encoded ampersand and an (empty) token secret.
        assert!(url.contains("oauth_signature=topsecret%26&"));
    }

    #[test]
    fn access_token_url_includes_temp_token_and_verifier() {
        let url = access_token_url(
            "https://www.evernote.com",
            "my-key",
            "topsecret",
            "tmp-token",
            "verifier-xyz",
            1700000001,
            "nonce456",
        );
        assert_eq!(
            url,
            "https://www.evernote.com/oauth?oauth_consumer_key=my-key\
             &oauth_token=tmp-token&oauth_verifier=verifier-xyz\
             &oauth_signature=topsecret%26&oauth_signature_method=PLAINTEXT\
             &oauth_timestamp=1700000001&oauth_nonce=nonce456"
        );
    }

    #[test]
    fn parse_oauth_response_decodes_keys_and_values() {
        let map = parse_oauth_response(
            "oauth_token=ABC%26123&oauth_token_secret=&edam_noteStoreUrl=https%3A%2F%2Fhost%2Fns",
        );
        assert_eq!(map.get("oauth_token").unwrap(), "ABC&123");
        assert_eq!(map.get("oauth_token_secret").unwrap(), "");
        assert_eq!(map.get("edam_noteStoreUrl").unwrap(), "https://host/ns");
    }

    #[test]
    fn parse_oauth_response_handles_plus_as_space() {
        let map = parse_oauth_response("note=hello+world");
        assert_eq!(map.get("note").unwrap(), "hello world");
    }

    fn full_access_map() -> HashMap<String, String> {
        parse_oauth_response(
            "oauth_token=S%3Ds1%3AU%3Dabc&oauth_token_secret=\
             &edam_shard=s1&edam_userId=12345&edam_expires=1700000000000\
             &edam_noteStoreUrl=https%3A%2F%2Fsandbox.evernote.com%2Fshard%2Fs1%2Fnotestore\
             &edam_webApiUrlPrefix=https%3A%2F%2Fsandbox.evernote.com%2Fshard%2Fs1%2F",
        )
    }

    #[test]
    fn credentials_from_access_response_success() {
        let creds = credentials_from_access_response(&full_access_map(), true).unwrap();
        assert_eq!(creds.auth_token, "S=s1:U=abc");
        assert_eq!(creds.shard, "s1");
        assert_eq!(creds.user_id, 12345);
        assert_eq!(creds.expires, 1_700_000_000_000);
        assert_eq!(
            creds.note_store_url,
            "https://sandbox.evernote.com/shard/s1/notestore"
        );
        assert_eq!(
            creds.web_api_url_prefix,
            "https://sandbox.evernote.com/shard/s1/"
        );
        assert!(creds.sandbox);
    }

    #[test]
    fn credentials_from_access_response_missing_key_errors() {
        let mut map = full_access_map();
        map.remove("edam_shard");
        let err = credentials_from_access_response(&map, true).unwrap_err();
        assert!(matches!(err, EvergeekError::Api(_)));
        assert!(err.to_string().contains("edam_shard"));
    }

    #[test]
    fn credentials_from_access_response_bad_user_id_errors() {
        let mut map = full_access_map();
        map.insert("edam_userId".to_string(), "not-a-number".to_string());
        let err = credentials_from_access_response(&map, false).unwrap_err();
        assert!(matches!(err, EvergeekError::Api(_)));
    }

    #[test]
    fn extract_verifier_from_request_line() {
        let request =
            "GET /?oauth_token=tmp&oauth_verifier=VER123 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(extract_verifier(request).as_deref(), Some("VER123"));
    }

    #[test]
    fn extract_verifier_returns_none_without_query() {
        let request = "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(extract_verifier(request), None);
    }

    #[test]
    fn base_url_selects_environment() {
        assert_eq!(base_url(true), SANDBOX_BASE);
        assert_eq!(base_url(false), PROD_BASE);
    }

    #[test]
    fn fetch_oauth_round_trip_against_mock() {
        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/oauth");
            then.status(200)
                .header("content-type", "text/plain")
                .body("oauth_token=tmp-token&oauth_token_secret=&oauth_callback_confirmed=true");
        });

        let url = format!("{}/oauth?whatever=1", server.base_url());
        let map = fetch_oauth(&url).unwrap();
        mock.assert();
        assert_eq!(map.get("oauth_token").unwrap(), "tmp-token");
        assert_eq!(map.get("oauth_callback_confirmed").unwrap(), "true");
    }

    #[test]
    fn fetch_oauth_maps_http_error_to_api_error() {
        let server = httpmock::MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/oauth");
            then.status(401).body("oauth_problem=signature_invalid");
        });

        let url = format!("{}/oauth", server.base_url());
        let err = fetch_oauth(&url).unwrap_err();
        assert!(matches!(err, EvergeekError::Api(_)));
    }
}
