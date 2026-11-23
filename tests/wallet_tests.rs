use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Sandbox {
    home: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let home =
            std::env::temp_dir().join(format!("cae_wallet_it_{}_{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).expect("create sandbox home");
        Sandbox { home }
    }

    fn cae(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cae"))
            .arg("wallet")
            .args(args)
            .env("CAE_HOME", &self.home)
            .env("CAE_SKIP_BROWSER_OPEN", "1")
            .stdin(std::process::Stdio::null())
            .output()
            .expect("run cae")
    }

    fn wallet_file(&self) -> PathBuf {
        self.home.join("wallet.json")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn is_address(s: &str) -> bool {
    s.len() == 42 && s.starts_with("0x") && s[2..].chars().all(|c| c.is_ascii_hexdigit())
}

#[test]
fn init_local_then_address_and_show() -> TestResult {
    let sb = Sandbox::new("init");
    let init = sb.cae(&["init", "--local"]);
    assert!(init.status.success(), "{}", stderr(&init));

    let address = sb.cae(&["address"]);
    assert!(address.status.success());
    let printed = stdout(&address);
    assert_eq!(printed.matches('\n').count(), 1);
    assert!(is_address(printed.trim()), "bad address: {printed}");

    let show = sb.cae(&["show", "--json"]);
    assert!(show.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout(&show)).expect("show json");
    assert_eq!(json["wallet_type"], "local-eoa");
    assert_eq!(json["network"], "base-sepolia");
    assert_eq!(json["status"], "active");
    assert_eq!(json["daily_limit_usdc"], 0.15);
    assert_eq!(
        json["evm_address"].as_str().ok_or("missing evm_address")?,
        printed.trim()
    );
    assert!(json.get("encrypted_privkey").is_none());
    assert!(json.get("session_ref").is_none());

    let human = stdout(&sb.cae(&["show"]));
    assert!(human.contains("local-eoa"));
    let stored: serde_json::Value = serde_json::from_str(&fs::read_to_string(sb.wallet_file())?)?;
    let privkey = stored["encrypted_privkey"]
        .as_str()
        .ok_or("missing privkey")?;
    assert!(
        privkey.starts_with("enc:v1:aes256gcm:hkdf:"),
        "expected encrypted envelope, got {privkey}"
    );
    assert!(!human.contains(privkey));
    assert!(!stdout(&init).contains(privkey));
    Ok(())
}

#[cfg(unix)]
#[test]
fn wallet_file_is_owner_only() -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    let sb = Sandbox::new("perms");
    assert!(sb.cae(&["init", "--local"]).status.success());
    let mode = fs::metadata(sb.wallet_file())?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    Ok(())
}

#[test]
fn init_refuses_overwrite_without_force_and_backs_up_with_it() -> TestResult {
    let sb = Sandbox::new("force");
    assert!(sb.cae(&["init", "--local"]).status.success());
    let first = stdout(&sb.cae(&["address"]));

    let again = sb.cae(&["init", "--local"]);
    assert!(!again.status.success());
    assert!(stderr(&again).contains("--force"));
    assert_eq!(stdout(&sb.cae(&["address"])), first);

    let forced = sb.cae(&["init", "--local", "--force"]);
    assert!(forced.status.success(), "{}", stderr(&forced));
    assert_ne!(stdout(&sb.cae(&["address"])), first);

    let bak = fs::read_to_string(sb.home.join("wallet.json.bak")).expect("backup written");
    assert!(bak.contains(first.trim()));
    Ok(())
}

#[test]
fn missing_wallet_exits_with_code_3() -> TestResult {
    let sb = Sandbox::new("missing");
    for args in [
        vec!["address"],
        vec!["show"],
        vec!["limit"],
        vec!["set-network", "base"],
        vec!["export-key", "--force"],
    ] {
        let out = sb.cae(&args);
        assert_eq!(out.status.code(), Some(3), "{args:?}: {}", stderr(&out));
        assert!(stderr(&out).contains("cae wallet init"));
    }
    Ok(())
}

#[test]
fn init_with_network_flag_sets_default_network() -> TestResult {
    let sb = Sandbox::new("network_flag");
    assert!(sb
        .cae(&["init", "--local", "--network", "base"])
        .status
        .success());
    let json: serde_json::Value = serde_json::from_str(&stdout(&sb.cae(&["show", "--json"])))?;
    assert_eq!(json["network"], "base");
    Ok(())
}

#[test]
fn set_network_switches_and_rejects_unknown() -> TestResult {
    let sb = Sandbox::new("setnet");
    assert!(sb.cae(&["init", "--local"]).status.success());

    let out = sb.cae(&["set-network", "base"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("base (Base Mainnet)"));

    let json: serde_json::Value = serde_json::from_str(&stdout(&sb.cae(&["show", "--json"])))?;
    assert_eq!(json["network"], "base");

    assert!(!sb.cae(&["set-network", "ethereum"]).status.success());
    Ok(())
}

#[test]
fn limit_get_set_and_validation() -> TestResult {
    let sb = Sandbox::new("limit");
    assert!(sb.cae(&["init", "--local"]).status.success());

    let get = sb.cae(&["limit"]);
    assert!(stdout(&get).contains("$0.15 USDC"));

    let set = sb.cae(&["limit", "set", "0.30"]);
    assert!(set.status.success(), "{}", stderr(&set));
    assert!(stdout(&set).contains("updated to $0.30"));

    let json: serde_json::Value =
        serde_json::from_str(&stdout(&sb.cae(&["limit", "get", "--json"])))?;
    assert_eq!(json["daily_limit_usdc"], 0.3);
    assert_eq!(json["daily_remaining_usdc"], 0.3);

    assert!(!sb.cae(&["limit", "set", "-1"]).status.success());
    assert!(!sb.cae(&["limit", "set", "abc"]).status.success());
    let after: serde_json::Value = serde_json::from_str(&stdout(&sb.cae(&["limit", "--json"])))?;
    assert_eq!(after["daily_limit_usdc"], 0.3);
    Ok(())
}

#[test]
fn limit_set_rejects_non_finite_and_excessive_values() -> TestResult {
    let sb = Sandbox::new("limit_bounds");
    assert!(sb.cae(&["init", "--local"]).status.success());
    assert!(sb.cae(&["limit", "set", "0.30"]).status.success());

    for bad in [
        "NaN",
        "inf",
        "-inf",
        "$-inf",
        "1e15",
        "1e300",
        "1000000000.01",
    ] {
        let out = sb.cae(&["limit", "set", bad]);
        assert!(
            !out.status.success(),
            "expected limit set '{bad}' to fail, got stdout: {}",
            stdout(&out)
        );
    }

    let after: serde_json::Value = serde_json::from_str(&stdout(&sb.cae(&["limit", "--json"])))?;
    assert_eq!(after["daily_limit_usdc"], 0.3);

    // Exact ceiling (1,000,000,000 USDC) is accepted.
    let at_max = sb.cae(&["limit", "set", "1000000000"]);
    assert!(at_max.status.success(), "{}", stderr(&at_max));
    let max_json: serde_json::Value = serde_json::from_str(&stdout(&sb.cae(&["limit", "--json"])))?;
    assert_eq!(max_json["daily_limit_usdc"], 1_000_000_000.0);
    Ok(())
}

#[test]
fn rollover_does_not_reset_spend_for_future_last_spend_date() -> TestResult {
    let sb = Sandbox::new("rollover_monotonic");
    fs::write(
        sb.wallet_file(),
        r#"{
  "wallet_type": "cdp-embedded",
  "evm_address": "0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c",
  "email": "candidate@example.com",
  "network": "base-sepolia",
  "created_at": "2026-11-11T17:30:00Z",
  "session_ref": "cdp_sess_example",
  "encrypted_privkey": null,
  "daily_limit_usdc": 0.15,
  "spent_today_usdc": 0.09,
  "last_spend_date": "2999-12-31"
}"#,
    )?;

    let show: serde_json::Value = serde_json::from_str(&stdout(&sb.cae(&["show", "--json"])))?;
    assert_eq!(show["spent_today_usdc"], 0.09);

    let limit_json: serde_json::Value =
        serde_json::from_str(&stdout(&sb.cae(&["limit", "--json"])))?;
    assert_eq!(limit_json["spent_today_usdc"], 0.09);
    let rem = limit_json["daily_remaining_usdc"]
        .as_f64()
        .ok_or("missing daily_remaining_usdc")?;
    assert!((rem - 0.06).abs() < 1e-9, "unexpected remaining: {rem}");

    let stored: serde_json::Value = serde_json::from_str(&fs::read_to_string(sb.wallet_file())?)?;
    assert_eq!(stored["spent_today_usdc"], 0.09);
    assert_eq!(stored["last_spend_date"], "2999-12-31");
    Ok(())
}

#[test]
fn export_key_requires_confirmation_or_force() -> TestResult {
    let sb = Sandbox::new("export");
    assert!(sb.cae(&["init", "--local"]).status.success());

    // stdin is not a terminal, so the [y/N] prompt cannot be answered: fail closed.
    let refused = sb.cae(&["export-key"]);
    assert!(!refused.status.success());
    assert!(!stdout(&refused).contains("0x"));

    let forced = sb.cae(&["export-key", "--force"]);
    assert!(forced.status.success());
    let key = stdout(&forced);
    assert_eq!(key.trim().len(), 66);
    assert!(key.starts_with("0x"));

    // On disk the private key must be stored as an AES-256-GCM envelope, never plaintext hex.
    let stored: serde_json::Value = serde_json::from_str(&fs::read_to_string(sb.wallet_file())?)?;
    let stored_ciphertext = stored["encrypted_privkey"]
        .as_str()
        .ok_or("missing privkey")?;
    let raw_hex = key.trim().trim_start_matches("0x");
    assert!(
        stored_ciphertext.starts_with("enc:v1:aes256gcm:"),
        "expected encrypted envelope, got {stored_ciphertext}"
    );
    assert!(
        !stored_ciphertext.contains(raw_hex),
        "wallet.json must not contain plaintext private key"
    );

    // Backup files must likewise never contain the plaintext private key.
    assert!(sb.cae(&["init", "--local", "--force"]).status.success());
    let bak_content = fs::read_to_string(sb.home.join("wallet.json.bak"))?;
    assert!(
        !bak_content.contains(raw_hex),
        "wallet.json.bak must not contain plaintext private key"
    );
    Ok(())
}

#[test]
fn passphrase_encrypted_local_wallet_roundtrip_and_wrong_passphrase_fails() -> TestResult {
    let sb = Sandbox::new("passphrase_wallet");
    let init = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "init", "--local", "--json"])
        .env("CAE_HOME", &sb.home)
        .env("CAE_WALLET_PASSPHRASE", "super-secret-passphrase")
        .stdin(std::process::Stdio::null())
        .output()?;
    assert!(init.status.success(), "{}", stderr(&init));

    let stored: serde_json::Value = serde_json::from_str(&fs::read_to_string(sb.wallet_file())?)?;
    let stored_ciphertext = stored["encrypted_privkey"]
        .as_str()
        .ok_or("missing encrypted_privkey")?;
    assert!(
        stored_ciphertext.starts_with("enc:v1:aes256gcm:pbkdf2:"),
        "{stored_ciphertext}"
    );
    assert!(!sb.home.join(".wallet.key").exists());

    let missing_pass = sb.cae(&["export-key", "--force"]);
    assert!(!missing_pass.status.success());
    assert!(stderr(&missing_pass).contains("CAE_WALLET_PASSPHRASE"));

    let wrong_pass = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "export-key", "--force"])
        .env("CAE_HOME", &sb.home)
        .env("CAE_WALLET_PASSPHRASE", "wrong-passphrase")
        .stdin(std::process::Stdio::null())
        .output()?;
    assert!(!wrong_pass.status.success());

    let right_pass = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "export-key", "--force"])
        .env("CAE_HOME", &sb.home)
        .env("CAE_WALLET_PASSPHRASE", "super-secret-passphrase")
        .stdin(std::process::Stdio::null())
        .output()?;
    assert!(right_pass.status.success(), "{}", stderr(&right_pass));
    let exported = stdout(&right_pass);
    assert_eq!(exported.trim().len(), 66);
    assert!(!stored_ciphertext.contains(exported.trim().trim_start_matches("0x")));
    Ok(())
}

#[test]
fn export_key_is_refused_for_cdp_wallets() -> TestResult {
    let sb = Sandbox::new("export_cdp");
    fs::write(
        sb.wallet_file(),
        r#"{
  "wallet_type": "cdp-embedded",
  "evm_address": "0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c",
  "email": "candidate@example.com",
  "network": "base-sepolia",
  "created_at": "2026-11-11T17:30:00Z",
  "session_ref": "cdp_sess_example",
  "encrypted_privkey": null,
  "daily_limit_usdc": 0.15,
  "spent_today_usdc": 0.03,
  "last_spend_date": "2026-01-01"
}"#,
    )?;

    let out = sb.cae(&["export-key", "--force"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("enclave"));
    assert!(!stdout(&out).contains("0x"));

    // Stale spend date rolls over in the reported view.
    let show: serde_json::Value = serde_json::from_str(&stdout(&sb.cae(&["show", "--json"])))?;
    assert_eq!(show["spent_today_usdc"], 0.0);
    assert_eq!(show["status"], "active");
    assert_eq!(show["email"], "candidate@example.com");

    // Rollover is persisted to disk
    let stored: serde_json::Value = serde_json::from_str(&fs::read_to_string(sb.wallet_file())?)?;
    assert_eq!(stored["spent_today_usdc"], 0.0);
    assert_eq!(
        stored["last_spend_date"],
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    );
    Ok(())
}

#[test]
fn logout_clears_session_but_keeps_address() -> TestResult {
    let sb = Sandbox::new("logout");
    fs::write(
        sb.wallet_file(),
        r#"{
  "wallet_type": "cdp-embedded",
  "evm_address": "0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c",
  "email": "candidate@example.com",
  "network": "base-sepolia",
  "created_at": "2026-11-11T17:30:00Z",
  "session_ref": "cdp_sess_example",
  "encrypted_privkey": null,
  "daily_limit_usdc": 0.15,
  "spent_today_usdc": 0.0,
  "last_spend_date": "2026-11-11"
}"#,
    )?;

    let out = sb.cae(&["logout", "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out))?;
    assert_eq!(json["session_cleared"], true);

    let raw = fs::read_to_string(sb.wallet_file())?;
    assert!(!raw.contains("cdp_sess_example"));
    let show: serde_json::Value = serde_json::from_str(&stdout(&sb.cae(&["show", "--json"])))?;
    assert_eq!(show["status"], "logged-out");
    assert_eq!(
        show["evm_address"],
        "0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c"
    );
    Ok(())
}

#[test]
fn logout_on_local_wallet_is_an_error() -> TestResult {
    let sb = Sandbox::new("logout_local");
    assert!(sb.cae(&["init", "--local"]).status.success());
    assert!(!sb.cae(&["logout"]).status.success());
    Ok(())
}

#[test]
fn relative_cae_home_is_rejected() -> TestResult {
    let out = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "show"])
        .env("CAE_HOME", "relative/path")
        .output()?;
    assert!(!out.status.success());
    assert!(stderr(&out).contains("absolute"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Balance (US2): exercised against an in-process mock JSON-RPC server.
// ---------------------------------------------------------------------------

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

/// Serves canned JSON-RPC answers on 127.0.0.1 and records the methods it saw.
fn spawn_mock_rpc(
    eth_hex: &'static str,
    usdc_hex: &'static str,
) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock rpc");
    let url = format!("http://{}", listener.local_addr().expect("local addr"));
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 2048];
            let body = loop {
                let n = stream.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break String::new();
                }
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(idx) = text.find("\r\n\r\n") {
                    let len = text[..idx]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if text.len() >= idx + 4 + len {
                        break text[idx + 4..idx + 4 + len].to_string();
                    }
                }
            };
            let request: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            let method = request["method"].as_str().unwrap_or("").to_string();
            let result = match method.as_str() {
                "eth_getBalance" => eth_hex,
                "eth_call" => {
                    let data = request["params"][0]["data"].as_str().unwrap_or("");
                    assert!(data.starts_with("0x70a08231"), "unexpected calldata {data}");
                    usdc_hex
                }
                _ => "0x0",
            };
            let _ = tx.send(format!("{method}|{}", request["params"]));
            let payload = format!(r#"{{"jsonrpc":"2.0","id":1,"result":"{result}"}}"#);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (url, rx)
}

impl Sandbox {
    fn cae_with_rpc(&self, args: &[&str], sepolia_url: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cae"))
            .arg("wallet")
            .args(args)
            .env("CAE_HOME", &self.home)
            .env("CAE_RPC_URL_BASE_SEPOLIA", sepolia_url)
            .env("CAE_RPC_URL_BASE", "http://127.0.0.1:1")
            .env("NO_PROXY", "*")
            .env("no_proxy", "*")
            .stdin(std::process::Stdio::null())
            .output()
            .expect("run cae")
    }
}

#[test]
fn balance_reports_eth_usdc_and_tailoring_quota() -> TestResult {
    let sb = Sandbox::new("balance");
    assert!(sb.cae(&["init", "--local"]).status.success());
    let address = stdout(&sb.cae(&["address"])).trim().to_string();

    // 0.005 ETH and 1.50 USDC => floor(1.50 / 0.03) = 50 runs.
    let (url, calls) = spawn_mock_rpc("0x11c37937e08000", "0x16e360");

    let out = sb.cae_with_rpc(&["balance", "--json"], &url);
    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out))?;
    assert_eq!(
        json["wallet_address"]
            .as_str()
            .ok_or("missing wallet_address")?,
        address
    );
    assert_eq!(json["network"], "base-sepolia");
    assert_eq!(json["eth_balance"], 0.005);
    assert_eq!(json["usdc_balance"], 1.5);
    assert_eq!(json["tailoring_runs_available"], 50);
    assert_eq!(json["cost_per_run_usdc"], 0.03);
    assert_eq!(json["balances"]["usdc"]["raw"], "1500000");
    assert_eq!(json["tailoring"]["daily_remaining_usdc"], 0.15);

    let seen: Vec<String> = calls.try_iter().collect();
    assert_eq!(
        seen.len(),
        2,
        "expected eth_getBalance + eth_call, got {seen:?}"
    );
    assert!(seen[0].starts_with("eth_getBalance"));
    assert!(seen[0].contains(&address));
    assert!(seen[1].starts_with("eth_call"));
    assert!(seen[1].contains("0x036CbD53842c5426634e7929541eC2318f3dCF7e"));

    let human = stdout(&sb.cae_with_rpc(&["balance"], &url));
    assert!(human.contains("Base Sepolia (Chain ID: 84532)"));
    assert!(human.contains("USDC:    1.50 USDC"));
    assert!(human.contains("Tailoring Capacity: 50 runs ($0.03 USDC per run)"));
    Ok(())
}

#[test]
fn balance_network_flag_targets_mainnet_rpc_and_contract() -> TestResult {
    let sb = Sandbox::new("balance_mainnet");
    assert!(sb.cae(&["init", "--local"]).status.success());
    let (url, calls) = spawn_mock_rpc("0x0", "0x0");

    let out = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "balance", "--network", "base", "--json"])
        .env("CAE_HOME", &sb.home)
        .env("CAE_RPC_URL_BASE", &url)
        .env("CAE_RPC_URL_BASE_SEPOLIA", "http://127.0.0.1:1")
        .env("NO_PROXY", "*")
        .env("no_proxy", "*")
        .stdin(std::process::Stdio::null())
        .output()?;
    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out))?;
    assert_eq!(json["network"], "base");
    assert_eq!(json["tailoring_runs_available"], 0);

    let seen: Vec<String> = calls.try_iter().collect();
    assert!(seen
        .iter()
        .any(|c| c.contains("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913")));
    Ok(())
}

#[test]
fn balance_uses_wallet_default_network() -> TestResult {
    let sb = Sandbox::new("balance_default_net");
    assert!(sb
        .cae(&["init", "--local", "--network", "base"])
        .status
        .success());
    // Sepolia endpoint is live but mainnet is dead; success would mean the wrong network was used.
    let (url, _calls) = spawn_mock_rpc("0x0", "0x0");
    let out = sb.cae_with_rpc(&["balance", "--json"], &url);
    assert!(!out.status.success());
    Ok(())
}

#[test]
fn balance_offline_reports_cached_address() -> TestResult {
    let sb = Sandbox::new("balance_offline");
    assert!(sb.cae(&["init", "--local"]).status.success());
    let address = stdout(&sb.cae(&["address"])).trim().to_string();

    let out = sb.cae_with_rpc(&["balance"], "http://127.0.0.1:1");
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.contains(&address), "{err}");
    assert!(err.contains("network connection"), "{err}");
    Ok(())
}

#[test]
fn balance_without_wallet_exits_with_code_3() -> TestResult {
    let sb = Sandbox::new("balance_nowallet");
    let out = sb.cae(&["balance"]);
    assert_eq!(out.status.code(), Some(3));
    Ok(())
}

#[test]
fn balance_succeeds_when_jsonrpc_error_is_null() -> TestResult {
    let sb = Sandbox::new("balance_null_error");
    assert!(sb.cae(&["init", "--local"]).status.success());

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock rpc");
    let url = format!("http://{}", listener.local_addr().expect("local addr"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 2048];
            let body = loop {
                let n = stream.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break String::new();
                }
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(idx) = text.find("\r\n\r\n") {
                    let len = text[..idx]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if text.len() >= idx + 4 + len {
                        break text[idx + 4..idx + 4 + len].to_string();
                    }
                }
            };
            let request: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            let result = match request["method"].as_str().unwrap_or("") {
                "eth_getBalance" => "0x11c37937e08000",
                "eth_call" => "0x16e360",
                _ => "0x0",
            };
            let payload = format!(r#"{{"jsonrpc":"2.0","id":1,"result":"{result}","error":null}}"#);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let out = sb.cae_with_rpc(&["balance", "--json"], &url);
    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out))?;
    assert_eq!(json["eth_balance"], 0.005);
    assert_eq!(json["usdc_balance"], 1.5);
    Ok(())
}

#[test]
fn rpc_and_faucet_errors_redact_url_secrets() -> TestResult {
    let sb = Sandbox::new("redact_url_secrets");
    assert!(sb.cae(&["init", "--local"]).status.success());

    let rpc_url =
        "http://rpcuser:SECRET_RPC_PASS@127.0.0.1:1/v2/SECRET_RPC_PATH?api_key=SECRET_RPC_QUERY";
    let rpc_out = sb.cae_with_rpc(&["balance"], rpc_url);
    assert!(!rpc_out.status.success());
    let rpc_err = stderr(&rpc_out);
    assert!(rpc_err.contains("http://127.0.0.1:1"), "{rpc_err}");
    for secret in [
        "SECRET_RPC_PASS",
        "SECRET_RPC_PATH",
        "SECRET_RPC_QUERY",
        "rpcuser",
    ] {
        assert!(!rpc_err.contains(secret), "leaked {secret} in: {rpc_err}");
    }

    let faucet_url =
        "http://faucetuser:SECRET_FAUCET_PASS@127.0.0.1:1/v2/SECRET_FAUCET_PATH?token=SECRET_FAUCET_QUERY";
    let faucet_out = run_fund_with_faucet_url(&sb, faucet_url);
    assert!(!faucet_out.status.success());
    let faucet_err = stderr(&faucet_out);
    assert!(faucet_err.contains("http://127.0.0.1:1"), "{faucet_err}");
    for secret in [
        "SECRET_FAUCET_PASS",
        "SECRET_FAUCET_PATH",
        "SECRET_FAUCET_QUERY",
        "faucetuser",
    ] {
        assert!(
            !faucet_err.contains(secret),
            "leaked {secret} in: {faucet_err}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Faucet (US3): testnet funding & mainnet refusal
// ---------------------------------------------------------------------------

fn read_http_request(stream: &mut TcpStream) -> String {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(idx) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let header_text = String::from_utf8_lossy(&buf[..idx]);
            let content_len = header_text
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                })
                .unwrap_or(0);
            if buf.len() >= idx + 4 + content_len {
                break;
            }
        }
    }
    String::from_utf8_lossy(&buf).to_string()
}

fn spawn_mock_faucet(tx_hash: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock faucet");
    let url = format!("http://{}", listener.local_addr().expect("local addr"));
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let request = read_http_request(&mut stream);
            let _ = tx.send(request);

            let payload = format!(r#"{{"transaction_hash":"{tx_hash}","status":"confirmed"}}"#);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    (url, rx)
}

#[test]
fn faucet_refused_on_mainnet_with_exit_code_5() -> TestResult {
    let sb = Sandbox::new("faucet_mainnet");
    assert!(sb
        .cae(&["init", "--local", "--network", "base"])
        .status
        .success());

    let out = sb.cae(&["fund"]);
    assert_eq!(out.status.code(), Some(5));
    let err = stderr(&out);
    assert!(
        err.contains("Faucets are only available on testnets"),
        "{err}"
    );
    assert!(err.contains("Base Mainnet"), "{err}");
    Ok(())
}

#[test]
fn faucet_success_on_base_sepolia() -> TestResult {
    let sb = Sandbox::new("faucet_sepolia");
    assert!(sb.cae(&["init", "--local"]).status.success());
    let address = stdout(&sb.cae(&["address"])).trim().to_string();

    let (url, calls) = spawn_mock_faucet("0x98f7b01234567890abcdef1234567890abcdef61a");

    let out = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "fund", "--json"])
        .env("CAE_HOME", &sb.home)
        .env("CAE_CDP_FAUCET_URL", &url)
        .env("NO_PROXY", "*")
        .env("no_proxy", "*")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run cae fund");

    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("faucet json");
    assert_eq!(json["recipient"], address);
    assert_eq!(json["network"], "base-sepolia");
    assert_eq!(json["token"], "USDC");
    assert_eq!(
        json["transaction_hash"],
        "0x98f7b01234567890abcdef1234567890abcdef61a"
    );
    assert_eq!(
        json["explorer_url"],
        "https://sepolia.basescan.org/tx/0x98f7b01234567890abcdef1234567890abcdef61a"
    );
    assert_eq!(json["status"], "confirmed");

    let seen: Vec<String> = calls.try_iter().collect();
    assert_eq!(seen.len(), 1);
    assert!(seen[0].contains(&address));
    assert!(seen[0].contains("usdc"));
    Ok(())
}

fn spawn_mock_faucet_response(
    status_line: &'static str,
    content_type: &'static str,
    payload: &'static str,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock faucet response");
    let url = format!("http://{}", listener.local_addr().expect("local addr"));

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = read_http_request(&mut stream);

            let response = format!(
                "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len(),
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    url
}

#[test]
fn mock_faucet_captures_split_tcp_post_body() -> TestResult {
    let (url, calls) = spawn_mock_faucet("0xsplitbodyhash");
    let addr = url.trim_start_matches("http://");
    let body = r#"{"address":"0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf","network":"base-sepolia","token":"usdc"}"#;
    let headers = format!(
        "POST / HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );

    let mut stream = TcpStream::connect(addr)?;
    stream.write_all(headers.as_bytes())?;
    stream.flush()?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    stream.write_all(body.as_bytes())?;
    stream.flush()?;

    let mut resp = String::new();
    stream.read_to_string(&mut resp)?;
    assert!(resp.starts_with("HTTP/1.1 200 OK"), "{resp}");
    assert!(resp.contains("0xsplitbodyhash"), "{resp}");

    let captured = calls.recv_timeout(std::time::Duration::from_secs(2))?;
    assert!(captured.contains("0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf"));
    assert!(captured.contains("\"token\":\"usdc\""));
    Ok(())
}

fn run_fund_with_faucet_url(sb: &Sandbox, url: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "fund", "--json"])
        .env("CAE_HOME", &sb.home)
        .env("CAE_CDP_FAUCET_URL", url)
        .env("NO_PROXY", "*")
        .env("no_proxy", "*")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run cae fund")
}

#[test]
fn faucet_200_with_null_error_and_informational_message_succeeds() -> TestResult {
    let sb = Sandbox::new("faucet_200_null_err");
    assert!(sb.cae(&["init", "--local"]).status.success());

    let url = spawn_mock_faucet_response(
        "200 OK",
        "application/json",
        r#"{"transactionHash":"0xdeadbeef1234567890abcdef1234567890abcdef","error":null,"message":"queued"}"#,
    );

    let out = run_fund_with_faucet_url(&sb, &url);
    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("faucet json");
    assert_eq!(
        json["transaction_hash"],
        "0xdeadbeef1234567890abcdef1234567890abcdef"
    );
    Ok(())
}

#[test]
fn faucet_400_and_500_structured_error_reports_http_status_and_message() -> TestResult {
    let sb = Sandbox::new("faucet_http_structured_err");
    assert!(sb.cae(&["init", "--local"]).status.success());

    let url_400 = spawn_mock_faucet_response(
        "400 Bad Request",
        "application/json",
        r#"{"error":{"code":"invalid_address","message":"Address not allowed"}}"#,
    );
    let out_400 = run_fund_with_faucet_url(&sb, &url_400);
    assert!(!out_400.status.success());
    let err_400 = stderr(&out_400);
    assert!(err_400.contains("400"), "{err_400}");
    assert!(err_400.contains("Address not allowed"), "{err_400}");
    assert!(!err_400.contains("Unknown faucet error"), "{err_400}");

    let url_500 = spawn_mock_faucet_response(
        "500 Internal Server Error",
        "application/json",
        r#"{"error":{"code":"internal_error","message":"Upstream signer unavailable"}}"#,
    );
    let out_500 = run_fund_with_faucet_url(&sb, &url_500);
    assert!(!out_500.status.success());
    let err_500 = stderr(&out_500);
    assert!(err_500.contains("500"), "{err_500}");
    assert!(err_500.contains("Upstream signer unavailable"), "{err_500}");
    assert!(!err_500.contains("Unknown faucet error"), "{err_500}");
    Ok(())
}

#[test]
fn faucet_502_non_json_body_reports_http_status_and_snippet() -> TestResult {
    let sb = Sandbox::new("faucet_502_html");
    assert!(sb.cae(&["init", "--local"]).status.success());

    let url = spawn_mock_faucet_response(
        "502 Bad Gateway",
        "text/html",
        "<html><body><h1>502 Bad Gateway</h1></body></html>",
    );
    let out = run_fund_with_faucet_url(&sb, &url);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("502"), "{err}");
    assert!(err.contains("502 Bad Gateway"), "{err}");
    assert!(!err.contains("non-JSON response"), "{err}");
    Ok(())
}

#[test]
fn faucet_429_preserves_rate_limit_message() -> TestResult {
    let sb = Sandbox::new("faucet_429");
    assert!(sb.cae(&["init", "--local"]).status.success());

    let url = spawn_mock_faucet_response(
        "429 Too Many Requests",
        "application/json",
        r#"{"error":{"code":"rate_limit","message":"Too many requests"}}"#,
    );
    let out = run_fund_with_faucet_url(&sb, &url);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("rate limit exceeded"), "{err}");
    assert!(err.contains("faucet.circle.com"), "{err}");
    Ok(())
}

// ---------------------------------------------------------------------------
// CDP User Auth & Login (US4)
// ---------------------------------------------------------------------------

fn extract_loopback_url_from_stderr(
    stderr: std::process::ChildStderr,
) -> (
    std::sync::mpsc::Receiver<String>,
    std::thread::JoinHandle<String>,
) {
    use std::io::BufRead;
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stderr);
        let mut all_stderr = String::new();
        let mut sent = false;
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            if !sent {
                if let Some(pos) = line.find("http://localhost:") {
                    let candidate = line[pos..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .trim_end_matches(|c: char| {
                            !c.is_ascii_alphanumeric()
                                && c != '/'
                                && c != ':'
                                && c != '?'
                                && c != '='
                                && c != '&'
                                && c != '-'
                                && c != '_'
                        })
                        .to_string();
                    if !candidate.is_empty() {
                        let _ = tx.send(candidate);
                        sent = true;
                    }
                }
            }
            all_stderr.push_str(&line);
            line.clear();
        }
        all_stderr
    });
    (rx, handle)
}

fn parse_loopback_port(url: &str) -> Result<u16, Box<dyn std::error::Error>> {
    let parsed = url
        .strip_prefix("http://localhost:")
        .ok_or("missing http://localhost: prefix")?;
    let port_str = parsed.split(['/', '?']).next().unwrap_or("");
    Ok(port_str.parse::<u16>()?)
}

fn send_loopback_callback(port: u16, path_and_query: String) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .unwrap_or_else(|e| panic!("connect to loopback port {port} failed: {e}"));
        let req = format!("GET {path_and_query} HTTP/1.1\r\nHost: localhost\r\n\r\n");
        stream
            .write_all(req.as_bytes())
            .expect("write loopback callback");
        stream.flush().expect("flush loopback callback");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("read loopback HTTP response");
        response
    })
}

#[test]
fn cdp_init_loopback_captures_auth_and_persists_embedded_profile() -> TestResult {
    let sb = Sandbox::new("cdp_init");

    let mut child = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "init", "--json"])
        .env("CAE_HOME", &sb.home)
        .env("CAE_AUTH_PORT", "0")
        .env("CAE_FORCE_BROWSER", "1")
        .env("CAE_SKIP_BROWSER_OPEN", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let (url_rx, stderr_handle) =
        extract_loopback_url_from_stderr(child.stderr.take().ok_or("missing stderr")?);
    let url = url_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .map_err(|_| "timeout waiting for loopback URL on stderr")?;
    let port = parse_loopback_port(&url)?;

    let callback_handle = send_loopback_callback(
        port,
        "/callback?address=0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c&email=candidate@example.com&session_ref=cdp_sess_token123".to_string(),
    );
    let http_response = callback_handle.join().expect("callback thread panicked");
    assert!(
        http_response.starts_with("HTTP/1.1 200 OK"),
        "{http_response}"
    );

    let out = child.wait_with_output()?;
    let child_stderr = stderr_handle.join().expect("stderr reader thread panicked");

    assert!(out.status.success(), "{child_stderr}");
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("init json");
    assert_eq!(json["wallet_type"], "cdp-embedded");
    assert_eq!(
        json["evm_address"],
        "0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c"
    );
    assert_eq!(json["email"], "candidate@example.com");
    assert_eq!(json["network"], "base-sepolia");
    assert_eq!(json["status"], "active");

    let stored: serde_json::Value = serde_json::from_str(&fs::read_to_string(sb.wallet_file())?)?;
    assert_eq!(stored["session_ref"], "cdp_sess_token123");
    assert!(stored["encrypted_privkey"].is_null());
    Ok(())
}

#[test]
fn cdp_login_backs_up_active_local_wallet() -> TestResult {
    let sb = Sandbox::new("cdp_login_bak");
    assert!(sb.cae(&["init", "--local"]).status.success());
    let local_addr = stdout(&sb.cae(&["address"])).trim().to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "login", "--json"])
        .env("CAE_HOME", &sb.home)
        .env("CAE_AUTH_PORT", "0")
        .env("CAE_FORCE_BROWSER", "1")
        .env("CAE_SKIP_BROWSER_OPEN", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let (url_rx, stderr_handle) =
        extract_loopback_url_from_stderr(child.stderr.take().ok_or("missing stderr")?);
    let url = url_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .map_err(|_| "timeout waiting for loopback URL on stderr")?;
    let port = parse_loopback_port(&url)?;

    let callback_handle = send_loopback_callback(
        port,
        "/callback?address=0x1111111111111111111111111111111111111111&email=new@example.com&session_ref=cdp_sess_new".to_string(),
    );
    let http_response = callback_handle.join().expect("callback thread panicked");
    assert!(
        http_response.starts_with("HTTP/1.1 200 OK"),
        "{http_response}"
    );

    let out = child.wait_with_output()?;
    let child_stderr = stderr_handle.join().expect("stderr reader thread panicked");

    assert!(out.status.success(), "{child_stderr}");

    // Verify local backup file exists and has previous local address
    let bak_path = sb.home.join("wallet.local.bak");
    assert!(bak_path.exists(), "wallet.local.bak missing");
    let bak_content = fs::read_to_string(&bak_path)?;
    assert!(bak_content.contains(&local_addr));
    assert!(bak_content.contains("local-eoa"));

    // Verify current profile is now cdp-embedded
    let show: serde_json::Value = serde_json::from_str(&stdout(&sb.cae(&["show", "--json"])))?;
    assert_eq!(show["wallet_type"], "cdp-embedded");
    assert_eq!(
        show["evm_address"],
        "0x1111111111111111111111111111111111111111"
    );
    assert_eq!(show["email"], "new@example.com");
    Ok(())
}

#[test]
fn cdp_init_terminal_otp() -> TestResult {
    let sb = Sandbox::new("cdp_otp");
    let mut child = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "init", "--no-browser", "--json"])
        .env("CAE_HOME", &sb.home)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    {
        let mut stdin = child.stdin.take().ok_or("missing stdin")?;
        stdin.write_all(b"candidate@example.com\n123456\n")?;
    }

    let out = child.wait_with_output()?;
    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("init json");
    assert_eq!(json["wallet_type"], "cdp-embedded");
    assert_eq!(json["email"], "candidate@example.com");
    assert_eq!(json["status"], "active");
    Ok(())
}
