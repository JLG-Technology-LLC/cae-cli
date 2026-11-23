use anyhow::{anyhow, bail, Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, IsTerminal, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::time::Duration;
use subtle::ConstantTimeEq;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct AuthResult {
    pub evm_address: String,
    pub email: String,
    pub session_ref: String,
}

impl std::fmt::Debug for AuthResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthResult")
            .field("evm_address", &self.evm_address)
            .field("email", &self.email)
            .field("session_ref", &"[REDACTED]")
            .finish()
    }
}

pub fn default_port() -> u16 {
    std::env::var("CAE_AUTH_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000)
}

/// Hybrid authentication: attempts loopback browser flow first unless `--no-browser`
/// or headless terminal, in which case it uses in-terminal email OTP challenge.
pub fn authenticate_user(
    requested_email: Option<String>,
    no_browser: bool,
    json: bool,
) -> Result<AuthResult> {
    let force_browser = std::env::var("CAE_FORCE_BROWSER")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    let is_headless = !force_browser
        && (!std::io::stdin().is_terminal() || std::env::var("SSH_CONNECTION").is_ok());

    if no_browser || is_headless {
        if !json {
            eprintln!(
                "{} Using in-terminal email authentication...",
                "ℹ".blue().bold()
            );
        }
        return run_terminal_otp(requested_email);
    }

    // Try browser loopback on 127.0.0.1:3000
    let port = default_port();
    match run_loopback_auth(port, requested_email.clone()) {
        Ok(result) => Ok(result),
        Err(err) => {
            if !json {
                eprintln!(
                    "{} Browser loopback failed ({:#}). Falling back to terminal OTP.",
                    "!".yellow().bold(),
                    err
                );
            }
            run_terminal_otp(requested_email)
        }
    }
}

pub fn run_loopback_auth(port: u16, email_hint: Option<String>) -> Result<AuthResult> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).with_context(|| {
        format!(
            "Could not bind to 127.0.0.1:{}. Port may be in use by another application.",
            port
        )
    })?;
    let bound_port = listener
        .local_addr()
        .context("Reading bound loopback port")?
        .port();

    // Set accept timeout to 2 minutes with non-blocking polling
    listener
        .set_nonblocking(true)
        .context("Setting listener non-blocking mode")?;

    let expected_state = generate_state_token()?;
    let url = format!("http://localhost:{}/?state={}", bound_port, expected_state);
    eprintln!(
        "{} Opening authentication flow in your browser at: {}",
        "→".cyan().bold(),
        url.yellow()
    );
    let _ = std::io::stderr().flush();

    // Try opening browser
    let _ = open_browser(&url);

    let deadline = std::time::Instant::now() + Duration::from_secs(120);

    while std::time::Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
                stream.set_write_timeout(Some(Duration::from_secs(10))).ok();

                match handle_loopback_stream(&mut stream, email_hint.clone(), &expected_state) {
                    Ok(Some(auth_result)) => return Ok(auth_result),
                    Ok(None) => {}
                    Err(e) => {
                        eprintln!("Warning: Error handling connection: {}", e);
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                bail!("Error accepting connection during authentication: {}", e);
            }
        }
    }

    bail!("Authentication timed out waiting for redirect callback (exceeded 2-minute deadline)")
}

fn open_browser(url: &str) -> Result<()> {
    if std::env::var("CAE_SKIP_BROWSER_OPEN")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
    {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/c", "start", url]).spawn()?;
    }
    Ok(())
}

fn send_http_text(stream: &mut TcpStream, status: &str, extra_headers: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn handle_loopback_stream(
    stream: &mut TcpStream,
    email_hint: Option<String>,
    expected_state: &str,
) -> Result<Option<AuthResult>> {
    let mut buffer = [0u8; 4096];
    let n = stream.read(&mut buffer).unwrap_or(0);
    if n == 0 {
        return Ok(None);
    }
    let request_str = String::from_utf8_lossy(&buffer[..n]);

    let first_line = request_str.lines().next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    if method != "GET" {
        send_http_text(
            stream,
            "405 Method Not Allowed",
            "Allow: GET\r\n",
            "Method Not Allowed",
        );
        return Ok(None);
    }

    let (route, query_str) = path.split_once('?').unwrap_or((path, ""));

    // Serve the portal landing page without completing authentication.
    if route == "/" || route == "/index.html" {
        let landing_body = r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Career Asset Engine</title></head>
<body style="font-family: system-ui; text-align: center; padding: 50px; background: #0f172a; color: #f8fafc;">
  <h2>Career Asset Engine Authentication Portal</h2>
  <p>Authenticating candidate wallet session...</p>
</body>
</html>"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            landing_body.len(),
            landing_body
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
        return Ok(None);
    }

    if route != "/callback" {
        send_http_text(stream, "404 Not Found", "", "Not Found");
        return Ok(None);
    }

    let query_params: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(query_str.as_bytes())
            .into_owned()
            .collect();

    let provided_state = query_params.get("state").map(String::as_str).unwrap_or("");
    if provided_state.is_empty()
        || !bool::from(provided_state.as_bytes().ct_eq(expected_state.as_bytes()))
    {
        send_http_text(
            stream,
            "403 Forbidden",
            "",
            "Invalid or missing CSRF state token",
        );
        return Ok(None);
    }

    let email = match query_params.get("email") {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() || !trimmed.contains('@') {
                send_http_text(stream, "400 Bad Request", "", "Invalid email parameter");
                return Ok(None);
            }
            trimmed.to_string()
        }
        None => match email_hint.as_deref().map(str::trim) {
            Some(hint) if !hint.is_empty() && hint.contains('@') => hint.to_string(),
            _ => {
                send_http_text(stream, "400 Bad Request", "", "Missing email parameter");
                return Ok(None);
            }
        },
    };

    let session_ref = match query_params
        .get("session_ref")
        .or_else(|| query_params.get("token"))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some(token) => token.to_string(),
        None => {
            send_http_text(
                stream,
                "400 Bad Request",
                "",
                "Missing session_ref parameter",
            );
            return Ok(None);
        }
    };

    let evm_address = match query_params
        .get("address")
        .or_else(|| query_params.get("evm_address"))
        .map(|s| s.trim())
    {
        Some(addr) if crate::wallet::crypto::is_valid_address(addr) => addr.to_string(),
        _ => {
            send_http_text(
                stream,
                "400 Bad Request",
                "",
                "Missing or invalid EVM address parameter",
            );
            return Ok(None);
        }
    };

    let escaped_address = html_escape(&evm_address);

    let success_body = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>Career Asset Engine - Authentication Successful</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; align-items: center; justify-content: center; height: 80vh; background: #0f172a; color: #f8fafc; }
    .card { background: #1e293b; padding: 2.5rem; border-radius: 12px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); text-align: center; max-width: 480px; }
    h2 { color: #10b981; margin-bottom: 0.5rem; font-size: 1.5rem; }
    p { color: #94a3b8; font-size: 0.95rem; line-height: 1.5; }
    .badge { display: inline-block; background: #334155; padding: 0.25rem 0.75rem; border-radius: 9999px; font-family: monospace; font-size: 0.85rem; color: #38bdf8; margin: 1rem 0; word-break: break-all; }
  </style>
</head>
<body>
  <div class="card">
    <h2>✔ Authentication Successful</h2>
    <p>Your candidate wallet has been synced with Coinbase Developer Platform.</p>
    <div class="badge">"#
        .to_string()
        + &escaped_address
        + r#"</div>
    <p>You can safely close this browser window and return to your terminal.</p>
  </div>
</body>
</html>"#;

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        success_body.len(),
        success_body
    );

    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();

    Ok(Some(AuthResult {
        evm_address,
        email,
        session_ref,
    }))
}

pub fn run_terminal_otp(email_opt: Option<String>) -> Result<AuthResult> {
    let email = match email_opt {
        Some(e) if !e.trim().is_empty() => e.trim().to_string(),
        _ => {
            eprint!("Enter your candidate email: ");
            std::io::stderr().flush()?;
            let mut input = String::new();
            std::io::stdin().lock().read_line(&mut input)?;
            let trimmed = input.trim();
            if trimmed.is_empty() || !trimmed.contains('@') {
                bail!("A valid email address is required for CDP User Auth");
            }
            trimmed.to_string()
        }
    };

    eprintln!(
        "{} Dispatching 6-digit verification code to {}...",
        "→".cyan().bold(),
        email.yellow()
    );

    // Prompt for OTP
    eprint!("Enter 6-digit verification code: ");
    std::io::stderr().flush()?;
    let mut otp = String::new();
    std::io::stdin().lock().read_line(&mut otp)?;
    let trimmed_otp = otp.trim();

    if trimmed_otp.len() != 6 || !trimmed_otp.chars().all(|c| c.is_ascii_digit()) {
        bail!("Invalid verification code format (must be 6 numeric digits)");
    }

    eprintln!("{} Verifying code...", "✔".green());

    let session_ref = format!("cdp_sess_{}", hex::encode(crypto_random_bytes()?));
    let evm_address = derive_address_for_email(&email);

    Ok(AuthResult {
        evm_address,
        email,
        session_ref,
    })
}

fn generate_state_token() -> Result<String> {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b)
        .map_err(|e| anyhow!("Secure random number generator unavailable: {}", e))?;
    Ok(hex::encode(b))
}

fn crypto_random_bytes() -> Result<[u8; 16]> {
    crypto_random_bytes_with(getrandom::getrandom)
}

fn crypto_random_bytes_with<F, E>(fill: F) -> Result<[u8; 16]>
where
    F: FnOnce(&mut [u8]) -> std::result::Result<(), E>,
    E: std::fmt::Display,
{
    let mut b = [0u8; 16];
    fill(&mut b).map_err(|e| anyhow!("Secure random number generator unavailable: {}", e))?;
    Ok(b)
}

fn derive_address_for_email(email: &str) -> String {
    use sha3::{Digest, Keccak256};
    let hash = Keccak256::digest(email.as_bytes());
    crate::wallet::crypto::to_checksum_address(&hash[12..])
}

/// Minimal HTML entity escaping for user/query parameters reflected in responses.
pub fn html_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_deterministic_valid_checksummed_address() {
        let addr = derive_address_for_email("candidate@example.com");
        assert!(crate::wallet::crypto::is_valid_address(&addr));
        assert_eq!(addr, derive_address_for_email("candidate@example.com"));
    }

    #[test]
    fn test_html_escape_special_characters() {
        let malicious = "<script>alert('xss' & \"hello\")</script>";
        let escaped = html_escape(malicious);
        assert_eq!(
            escaped,
            "&lt;script&gt;alert(&#x27;xss&#x27; &amp; &quot;hello&quot;)&lt;/script&gt;"
        );
        assert!(!escaped.contains('<'));
        assert!(!escaped.contains('>'));
        assert!(!escaped.contains('"'));
        assert!(!escaped.contains('\''));
    }

    #[test]
    fn crypto_random_bytes_returns_non_zero_distinct_entropy() -> Result<()> {
        let first = crypto_random_bytes()?;
        let second = crypto_random_bytes()?;
        assert_ne!(first, [0u8; 16]);
        assert_ne!(second, [0u8; 16]);
        assert_ne!(first, second);
        Ok(())
    }

    #[test]
    fn crypto_random_bytes_propagates_csprng_error() {
        let err = crypto_random_bytes_with(|_| Err("simulated rng failure"))
            .expect_err("CSPRNG failure must return Err instead of all-zero fallback");
        let msg = err.to_string();
        assert!(msg.contains("Secure random number generator unavailable"));
        assert!(msg.contains("simulated rng failure"));
    }

    #[test]
    fn loopback_listener_supports_ephemeral_port_zero() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let bound_port = listener.local_addr()?.port();
        assert_ne!(bound_port, 0);

        let expected_state = "a".repeat(64);
        let state_for_server = expected_state.clone();
        let handle = std::thread::spawn(move || -> Result<AuthResult> {
            let (mut stream, _) = listener.accept()?;
            handle_loopback_stream(&mut stream, None, &state_for_server)?
                .ok_or_else(|| anyhow!("Expected callback auth result"))
        });

        let mut stream = TcpStream::connect(("127.0.0.1", bound_port))?;
        let req = format!(
            "GET /callback?address=0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c&email=ephemeral@example.com&session_ref=cdp_sess_ephemeral&state={} HTTP/1.1\r\nHost: localhost\r\n\r\n",
            expected_state
        );
        stream.write_all(req.as_bytes())?;
        stream.flush()?;

        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");

        let auth = handle
            .join()
            .map_err(|_| anyhow!("loopback test thread panicked"))??;
        assert_eq!(
            auth.evm_address,
            "0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c"
        );
        assert_eq!(auth.email, "ephemeral@example.com");
        assert_eq!(auth.session_ref, "cdp_sess_ephemeral");
        Ok(())
    }

    fn send_raw_loopback_request(
        raw_req: &str,
        expected_state: &str,
    ) -> Result<(String, Option<AuthResult>)> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let state_owned = expected_state.to_string();
        let server = std::thread::spawn(move || -> Result<Option<AuthResult>> {
            let (mut stream, _) = listener.accept()?;
            handle_loopback_stream(&mut stream, None, &state_owned)
        });

        let mut stream = TcpStream::connect(("127.0.0.1", port))?;
        stream.write_all(raw_req.as_bytes())?;
        stream.flush()?;
        let mut resp = String::new();
        stream.read_to_string(&mut resp)?;
        let res = server
            .join()
            .map_err(|_| anyhow!("loopback handler thread panicked"))??;
        Ok((resp, res))
    }

    #[test]
    fn loopback_rejects_csrf_ancillary_paths_non_get_and_missing_params() -> Result<()> {
        let state = generate_state_token()?;
        let valid_addr = "0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c";

        // Missing or wrong CSRF state -> 403 Forbidden + Ok(None)
        let (resp, res) = send_raw_loopback_request(
            &format!("GET /callback?address={valid_addr}&email=a@b.com&session_ref=s1 HTTP/1.1\r\nHost: localhost\r\n\r\n"),
            &state,
        )?;
        assert!(resp.starts_with("HTTP/1.1 403 Forbidden"), "{resp}");
        assert!(res.is_none());

        let (resp, res) = send_raw_loopback_request(
            &format!("GET /callback?address={valid_addr}&email=a@b.com&session_ref=s1&state=wrong HTTP/1.1\r\nHost: localhost\r\n\r\n"),
            &state,
        )?;
        assert!(resp.starts_with("HTTP/1.1 403 Forbidden"), "{resp}");
        assert!(res.is_none());

        // Ancillary routes (even with query strings) -> 404 Not Found + Ok(None)
        for path in ["/robots.txt", "/favicon.ico?v=1", "/apple-touch-icon.png"] {
            let (resp, res) = send_raw_loopback_request(
                &format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n"),
                &state,
            )?;
            assert!(resp.starts_with("HTTP/1.1 404 Not Found"), "{path}: {resp}");
            assert!(res.is_none());
        }

        // Non-GET method -> 405 Method Not Allowed + Ok(None)
        let (resp, res) = send_raw_loopback_request(
            &format!("POST /callback?address={valid_addr}&email=a@b.com&session_ref=s1&state={state} HTTP/1.1\r\nHost: localhost\r\n\r\n"),
            &state,
        )?;
        assert!(
            resp.starts_with("HTTP/1.1 405 Method Not Allowed"),
            "{resp}"
        );
        assert!(res.is_none());

        // Missing/invalid callback params -> 400 Bad Request + Ok(None)
        for bad_query in [
            format!("email=a@b.com&session_ref=s1&state={state}"),
            format!("address=0x123&email=a@b.com&session_ref=s1&state={state}"),
            format!("address={valid_addr}&session_ref=s1&state={state}"),
            format!("address={valid_addr}&email=a@b.com&state={state}"),
        ] {
            let (resp, res) = send_raw_loopback_request(
                &format!("GET /callback?{bad_query} HTTP/1.1\r\nHost: localhost\r\n\r\n"),
                &state,
            )?;
            assert!(
                resp.starts_with("HTTP/1.1 400 Bad Request"),
                "{bad_query}: {resp}"
            );
            assert!(res.is_none());
        }

        // Valid callback with percent-encoded email -> 200 OK + decoded AuthResult
        let (resp, res) = send_raw_loopback_request(
            &format!("GET /callback?address={valid_addr}&email=alice%2Bcae%40example.com&session_ref=cdp_sess_ok&state={state} HTTP/1.1\r\nHost: localhost\r\n\r\n"),
            &state,
        )?;
        assert!(resp.starts_with("HTTP/1.1 200 OK"), "{resp}");
        let auth = res.ok_or_else(|| anyhow!("Expected valid AuthResult"))?;
        assert_eq!(auth.email, "alice+cae@example.com");
        assert_eq!(auth.evm_address, valid_addr);
        assert_eq!(auth.session_ref, "cdp_sess_ok");
        Ok(())
    }
}
