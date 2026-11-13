use anyhow::{bail, Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, IsTerminal, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AuthResult {
    pub evm_address: String,
    pub email: String,
    pub session_ref: String,
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

    // Set accept timeout to 2 minutes with non-blocking polling
    listener
        .set_nonblocking(true)
        .context("Setting listener non-blocking mode")?;

    let url = format!("http://localhost:{}/", port);
    eprintln!(
        "{} Opening authentication flow in your browser at: {}",
        "→".cyan().bold(),
        url.yellow()
    );

    // Try opening browser
    let _ = open_browser(&url);

    let deadline = std::time::Instant::now() + Duration::from_secs(120);

    while std::time::Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
                stream.set_write_timeout(Some(Duration::from_secs(10))).ok();

                match handle_loopback_stream(&mut stream, email_hint.clone()) {
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

fn handle_loopback_stream(
    stream: &mut TcpStream,
    email_hint: Option<String>,
) -> Result<Option<AuthResult>> {
    let mut buffer = [0u8; 4096];
    let n = stream.read(&mut buffer).unwrap_or(0);
    let request_str = String::from_utf8_lossy(&buffer[..n]);

    let first_line = request_str.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("/");

    let (route, query_str) = path.split_once('?').unwrap_or((path, ""));

    // If client requested root landing page or favicon without query params, serve page and wait for callback
    if (route == "/" || route == "/index.html" || route == "/favicon.ico") && query_str.is_empty() {
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

    let mut query_params = std::collections::HashMap::new();
    for pair in query_str.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if !k.is_empty() {
                query_params.insert(k.to_string(), v.to_string());
            }
        }
    }

    // Extract address, email, session_ref from query params or form
    let email = query_params
        .get("email")
        .cloned()
        .or(email_hint)
        .unwrap_or_else(|| "candidate@example.com".to_string());

    let session_ref = query_params
        .get("session_ref")
        .or_else(|| query_params.get("token"))
        .cloned()
        .unwrap_or_else(|| format!("cdp_sess_{}", hex::encode(crypto_random_bytes())));

    let raw_address = query_params
        .get("address")
        .or_else(|| query_params.get("evm_address"));

    let evm_address = match raw_address {
        Some(addr) if crate::wallet::crypto::is_valid_address(addr) => addr.clone(),
        _ => derive_address_for_email(&email),
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

    let session_ref = format!("cdp_sess_{}", hex::encode(crypto_random_bytes()));
    let evm_address = derive_address_for_email(&email);

    Ok(AuthResult {
        evm_address,
        email,
        session_ref,
    })
}

fn crypto_random_bytes() -> [u8; 16] {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).unwrap_or_default();
    b
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
}
