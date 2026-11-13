use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

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
fn init_local_then_address_and_show() {
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
    assert_eq!(json["evm_address"].as_str().unwrap(), printed.trim());
    assert!(json.get("encrypted_privkey").is_none());
    assert!(json.get("session_ref").is_none());

    let human = stdout(&sb.cae(&["show"]));
    assert!(human.contains("local-eoa"));
    let stored: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(sb.wallet_file()).unwrap()).unwrap();
    let privkey = stored["encrypted_privkey"].as_str().unwrap();
    assert!(!human.contains(privkey));
    assert!(!stdout(&init).contains(privkey));
}

#[cfg(unix)]
#[test]
fn wallet_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let sb = Sandbox::new("perms");
    assert!(sb.cae(&["init", "--local"]).status.success());
    let mode = fs::metadata(sb.wallet_file()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn init_refuses_overwrite_without_force_and_backs_up_with_it() {
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
}

#[test]
fn missing_wallet_exits_with_code_3() {
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
}

#[test]
fn init_with_network_flag_sets_default_network() {
    let sb = Sandbox::new("network_flag");
    assert!(sb
        .cae(&["init", "--local", "--network", "base"])
        .status
        .success());
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&sb.cae(&["show", "--json"]))).unwrap();
    assert_eq!(json["network"], "base");
}

#[test]
fn set_network_switches_and_rejects_unknown() {
    let sb = Sandbox::new("setnet");
    assert!(sb.cae(&["init", "--local"]).status.success());

    let out = sb.cae(&["set-network", "base"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("base (Base Mainnet)"));

    let json: serde_json::Value =
        serde_json::from_str(&stdout(&sb.cae(&["show", "--json"]))).unwrap();
    assert_eq!(json["network"], "base");

    assert!(!sb.cae(&["set-network", "ethereum"]).status.success());
}

#[test]
fn limit_get_set_and_validation() {
    let sb = Sandbox::new("limit");
    assert!(sb.cae(&["init", "--local"]).status.success());

    let get = sb.cae(&["limit"]);
    assert!(stdout(&get).contains("$0.15 USDC"));

    let set = sb.cae(&["limit", "set", "0.30"]);
    assert!(set.status.success(), "{}", stderr(&set));
    assert!(stdout(&set).contains("updated to $0.30"));

    let json: serde_json::Value =
        serde_json::from_str(&stdout(&sb.cae(&["limit", "get", "--json"]))).unwrap();
    assert_eq!(json["daily_limit_usdc"], 0.3);
    assert_eq!(json["daily_remaining_usdc"], 0.3);

    assert!(!sb.cae(&["limit", "set", "-1"]).status.success());
    assert!(!sb.cae(&["limit", "set", "abc"]).status.success());
    let after: serde_json::Value =
        serde_json::from_str(&stdout(&sb.cae(&["limit", "--json"]))).unwrap();
    assert_eq!(after["daily_limit_usdc"], 0.3);
}

#[test]
fn export_key_requires_confirmation_or_force() {
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

    // The exported key must correspond to the stored address.
    let stored: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(sb.wallet_file()).unwrap()).unwrap();
    assert_eq!(
        format!("0x{}", stored["encrypted_privkey"].as_str().unwrap()),
        key.trim()
    );
}

#[test]
fn export_key_is_refused_for_cdp_wallets() {
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
    )
    .unwrap();

    let out = sb.cae(&["export-key", "--force"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("enclave"));
    assert!(!stdout(&out).contains("0x"));

    // Stale spend date rolls over in the reported view.
    let show: serde_json::Value =
        serde_json::from_str(&stdout(&sb.cae(&["show", "--json"]))).unwrap();
    assert_eq!(show["spent_today_usdc"], 0.0);
    assert_eq!(show["status"], "active");
    assert_eq!(show["email"], "candidate@example.com");

    // Rollover is persisted to disk
    let stored: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(sb.wallet_file()).unwrap()).unwrap();
    assert_eq!(stored["spent_today_usdc"], 0.0);
    assert_eq!(
        stored["last_spend_date"],
        chrono::Local::now().format("%Y-%m-%d").to_string()
    );
}

#[test]
fn logout_clears_session_but_keeps_address() {
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
    )
    .unwrap();

    let out = sb.cae(&["logout", "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(json["session_cleared"], true);

    let raw = fs::read_to_string(sb.wallet_file()).unwrap();
    assert!(!raw.contains("cdp_sess_example"));
    let show: serde_json::Value =
        serde_json::from_str(&stdout(&sb.cae(&["show", "--json"]))).unwrap();
    assert_eq!(show["status"], "logged-out");
    assert_eq!(
        show["evm_address"],
        "0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c"
    );
}

#[test]
fn logout_on_local_wallet_is_an_error() {
    let sb = Sandbox::new("logout_local");
    assert!(sb.cae(&["init", "--local"]).status.success());
    assert!(!sb.cae(&["logout"]).status.success());
}

#[test]
fn relative_cae_home_is_rejected() {
    let out = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "show"])
        .env("CAE_HOME", "relative/path")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("absolute"));
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
    let url = format!("http://{}", listener.local_addr().unwrap());
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
fn balance_reports_eth_usdc_and_tailoring_quota() {
    let sb = Sandbox::new("balance");
    assert!(sb.cae(&["init", "--local"]).status.success());
    let address = stdout(&sb.cae(&["address"])).trim().to_string();

    // 0.005 ETH and 1.50 USDC => floor(1.50 / 0.03) = 50 runs.
    let (url, calls) = spawn_mock_rpc("0x11c37937e08000", "0x16e360");

    let out = sb.cae_with_rpc(&["balance", "--json"], &url);
    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(json["wallet_address"].as_str().unwrap(), address);
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
}

#[test]
fn balance_network_flag_targets_mainnet_rpc_and_contract() {
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
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(json["network"], "base");
    assert_eq!(json["tailoring_runs_available"], 0);

    let seen: Vec<String> = calls.try_iter().collect();
    assert!(seen
        .iter()
        .any(|c| c.contains("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913")));
}

#[test]
fn balance_uses_wallet_default_network() {
    let sb = Sandbox::new("balance_default_net");
    assert!(sb
        .cae(&["init", "--local", "--network", "base"])
        .status
        .success());
    // Sepolia endpoint is live but mainnet is dead; success would mean the wrong network was used.
    let (url, _calls) = spawn_mock_rpc("0x0", "0x0");
    let out = sb.cae_with_rpc(&["balance", "--json"], &url);
    assert!(!out.status.success());
}

#[test]
fn balance_offline_reports_cached_address() {
    let sb = Sandbox::new("balance_offline");
    assert!(sb.cae(&["init", "--local"]).status.success());
    let address = stdout(&sb.cae(&["address"])).trim().to_string();

    let out = sb.cae_with_rpc(&["balance"], "http://127.0.0.1:1");
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.contains(&address), "{err}");
    assert!(err.contains("network connection"), "{err}");
}

#[test]
fn balance_without_wallet_exits_with_code_3() {
    let sb = Sandbox::new("balance_nowallet");
    let out = sb.cae(&["balance"]);
    assert_eq!(out.status.code(), Some(3));
}

// ---------------------------------------------------------------------------
// Faucet (US3): testnet funding & mainnet refusal
// ---------------------------------------------------------------------------

fn spawn_mock_faucet(tx_hash: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock faucet");
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 1024];
            let n = stream.read(&mut chunk).unwrap_or(0);
            buf.extend_from_slice(&chunk[..n]);
            let body = String::from_utf8_lossy(&buf).to_string();
            let _ = tx.send(body);

            let payload = format!(r#"{{"transaction_hash":"{tx_hash}","status":"confirmed"}}"#);
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

#[test]
fn faucet_refused_on_mainnet_with_exit_code_5() {
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
}

#[test]
fn faucet_success_on_base_sepolia() {
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
}

// ---------------------------------------------------------------------------
// CDP User Auth & Login (US4)
// ---------------------------------------------------------------------------

#[test]
fn cdp_init_loopback_captures_auth_and_persists_embedded_profile() {
    let sb = Sandbox::new("cdp_init");

    // Pick an ephemeral port for test loopback
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Spawn client callback thread to hit loopback
    std::thread::spawn(move || {
        // Wait briefly for cae to bind
        std::thread::sleep(std::time::Duration::from_millis(300));
        let mut attempts = 0;
        let mut stream = loop {
            if let Ok(s) = TcpStream::connect(format!("127.0.0.1:{}", port)) {
                break s;
            }
            attempts += 1;
            if attempts >= 100 {
                panic!("Timeout waiting to connect to 127.0.0.1:{}", port);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        let req =
            "GET /callback?address=0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c&email=candidate@example.com&session_ref=cdp_sess_token123 HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let _ = stream.write_all(req.as_bytes());
    });

    let out = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "init", "--json"])
        .env("CAE_HOME", &sb.home)
        .env("CAE_AUTH_PORT", port.to_string())
        .env("CAE_FORCE_BROWSER", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run cae init");

    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("init json");
    assert_eq!(json["wallet_type"], "cdp-embedded");
    assert_eq!(
        json["evm_address"],
        "0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c"
    );
    assert_eq!(json["email"], "candidate@example.com");
    assert_eq!(json["network"], "base-sepolia");
    assert_eq!(json["status"], "active");

    let stored: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(sb.wallet_file()).unwrap()).unwrap();
    assert_eq!(stored["session_ref"], "cdp_sess_token123");
    assert!(stored["encrypted_privkey"].is_null());
}

#[test]
fn cdp_login_backs_up_active_local_wallet() {
    let sb = Sandbox::new("cdp_login_bak");
    assert!(sb.cae(&["init", "--local"]).status.success());
    let local_addr = stdout(&sb.cae(&["address"])).trim().to_string();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let mut attempts = 0;
        let mut stream = loop {
            if let Ok(s) = TcpStream::connect(format!("127.0.0.1:{}", port)) {
                break s;
            }
            attempts += 1;
            if attempts >= 100 {
                panic!("Timeout waiting to connect to 127.0.0.1:{}", port);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        let req =
            "GET /callback?address=0x1111111111111111111111111111111111111111&email=new@example.com&session_ref=cdp_sess_new HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let _ = stream.write_all(req.as_bytes());
    });

    let out = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "login", "--json"])
        .env("CAE_HOME", &sb.home)
        .env("CAE_AUTH_PORT", port.to_string())
        .env("CAE_FORCE_BROWSER", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run cae login");

    assert!(out.status.success(), "{}", stderr(&out));

    // Verify local backup file exists and has previous local address
    let bak_path = sb.home.join("wallet.local.bak");
    assert!(bak_path.exists(), "wallet.local.bak missing");
    let bak_content = fs::read_to_string(&bak_path).unwrap();
    assert!(bak_content.contains(&local_addr));
    assert!(bak_content.contains("local-eoa"));

    // Verify current profile is now cdp-embedded
    let show: serde_json::Value =
        serde_json::from_str(&stdout(&sb.cae(&["show", "--json"]))).unwrap();
    assert_eq!(show["wallet_type"], "cdp-embedded");
    assert_eq!(
        show["evm_address"],
        "0x1111111111111111111111111111111111111111"
    );
    assert_eq!(show["email"], "new@example.com");
}

#[test]
fn cdp_init_terminal_otp() {
    let sb = Sandbox::new("cdp_otp");
    let mut child = Command::new(env!("CARGO_BIN_EXE_cae"))
        .args(["wallet", "init", "--no-browser", "--json"])
        .env("CAE_HOME", &sb.home)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"candidate@example.com\n123456\n").unwrap();
    }

    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("init json");
    assert_eq!(json["wallet_type"], "cdp-embedded");
    assert_eq!(json["email"], "candidate@example.com");
    assert_eq!(json["status"], "active");
}
