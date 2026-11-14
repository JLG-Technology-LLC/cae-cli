use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct TestHarness {
    home: PathBuf,
    projects_dir: PathBuf,
    sample_repo: PathBuf,
}

impl TestHarness {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!("cae_e2e_{}_{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&base);

        let home = base.join("home");
        let projects_dir = base.join("projects");
        let sample_repo = base.join("sample_repo");

        fs::create_dir_all(&home).expect("create test home");
        fs::create_dir_all(&projects_dir).expect("create test projects dir");
        fs::create_dir_all(&sample_repo).expect("create sample repo dir");

        let home = fs::canonicalize(&home).expect("canonicalize home");
        let projects_dir = fs::canonicalize(&projects_dir).expect("canonicalize projects dir");
        let sample_repo = fs::canonicalize(&sample_repo).expect("canonicalize sample repo");

        TestHarness {
            home,
            projects_dir,
            sample_repo,
        }
    }

    fn bin() -> &'static str {
        env!("CARGO_BIN_EXE_cae")
    }

    fn bin_dir() -> PathBuf {
        Path::new(Self::bin())
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf()
    }

    fn print_output(&self, out: &Output) {
        let text = stdout(out);
        if !text.trim().is_empty() {
            println!("{}", text.trim_end());
        }
        let err = stderr(out);
        if !err.trim().is_empty() {
            eprintln!("{}", err.trim_end());
        }
    }

    fn run_cae(&self, args: &[&str]) -> Output {
        println!("\n$ cae {}", args.join(" "));
        let out = self.cmd(args).output().expect("run cae command");
        self.print_output(&out);
        out
    }

    fn run_cae_in_repo(&self, args: &[&str]) -> Output {
        println!("\n$ (repo) cae {}", args.join(" "));
        let out = self
            .cmd(args)
            .current_dir(&self.sample_repo)
            .output()
            .expect("run cae in repo");
        self.print_output(&out);
        out
    }

    fn run_git_in_repo(&self, args: &[&str]) -> Output {
        println!("\n$ (repo) git {}", args.join(" "));
        let bin_dir = Self::bin_dir();
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", bin_dir.display(), current_path);

        let out = Command::new("git")
            .args(args)
            .env("CAE_HOME", &self.home)
            .env("CAE_PROJECTS_DIR", &self.projects_dir)
            .env("PATH", new_path)
            .current_dir(&self.sample_repo)
            .output()
            .expect("run git in repo");
        self.print_output(&out);
        assert!(
            out.status.success(),
            "git {} failed: {}",
            args.join(" "),
            stderr(&out)
        );
        out
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let bin_dir = Self::bin_dir();
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", bin_dir.display(), current_path);

        let mut cmd = Command::new(Self::bin());
        cmd.args(args)
            .env("CAE_HOME", &self.home)
            .env("CAE_PROJECTS_DIR", &self.projects_dir)
            .env("CAE_SKIP_BROWSER_OPEN", "1")
            .env("PATH", new_path)
            .env("NO_PROXY", "*")
            .env("no_proxy", "*")
            .stdin(Stdio::null());
        cmd
    }

    fn wallet_file(&self) -> PathBuf {
        self.home.join("wallet.json")
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        if let Some(p) = self.home.parent() {
            let _ = fs::remove_dir_all(p);
        }
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn assert_success(out: &Output, context: &str) {
    assert!(
        out.status.success(),
        "{} failed (code: {:?}):\nstdout: {}\nstderr: {}",
        context,
        out.status.code(),
        stdout(out),
        stderr(out)
    );
}

fn init_sample_git_repo(
    repo_path: &Path,
    projects_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let bin_dir = TestHarness::bin_dir();
    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), current_path);

    let run = |cmd: &str, args: &[&str]| {
        let status = Command::new(cmd)
            .args(args)
            .env("CAE_PROJECTS_DIR", projects_dir)
            .env("PATH", &new_path)
            .current_dir(repo_path)
            .status()
            .expect("exec git");
        assert!(status.success());
    };

    run("git", &["init"]);
    run("git", &["config", "user.name", "CAE CI Runner"]);
    run("git", &["config", "user.email", "ci@careerengine.local"]);

    // 1. Rust Cargo.toml
    fs::write(
        repo_path.join("Cargo.toml"),
        r#"[package]
name = "demo-rust"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
"#,
    )?;

    // 2. Node.js package.json
    fs::write(
        repo_path.join("package.json"),
        r#"{
  "name": "demo-node",
  "version": "1.0.0",
  "dependencies": {
    "lodash": "^4.17.21"
  }
}
"#,
    )?;

    // 3. Python pyproject.toml
    fs::write(
        repo_path.join("pyproject.toml"),
        r#"[project]
name = "demo-python"
version = "0.2.0"
dependencies = [
  "fastapi>=0.100.0"
]
"#,
    )?;

    // 4. Agent Context Documents
    fs::write(
        repo_path.join("README.md"),
        "# Demo Repository\nContinuous integration sample repository.\n",
    )?;
    fs::write(
        repo_path.join("ARCHITECTURE.md"),
        "# Architecture\nDescribes decoupling between components.\n",
    )?;
    fs::write(
        repo_path.join("DESIGN.md"),
        "# Design Specifications\nDesign token rules and guidelines.\n",
    )?;
    fs::write(
        repo_path.join("MEMORY.md"),
        "# Agent Memory\nPersistent project conventions and decisions.\n",
    )?;

    run("git", &["add", "."]);
    run("git", &["commit", "-m", "chore: initial repository assets"]);
    Ok(())
}

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
                "eth_call" => usdc_hex,
                _ => "0x0",
            };
            let _ = tx.send(method);
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

fn spawn_mock_faucet(tx_hash: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock faucet");
    let url = format!("http://{}", listener.local_addr().expect("local addr"));
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap_or(0);
            let body = String::from_utf8_lossy(&buf[..n]).to_string();
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
fn test_ci_cd_all_commands_and_subcommands_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n========================================================");
    println!("=== CAE-CLI COMPREHENSIVE END-TO-END PIPELINE TEST ===");
    println!("========================================================\n");

    let h = TestHarness::new("full_e2e");
    init_sample_git_repo(&h.sample_repo, &h.projects_dir)?;

    // -------------------------------------------------------------------------
    // 1. Meta Commands: Help & Version
    // -------------------------------------------------------------------------
    println!("[1/14] Testing Meta Commands (cae --help, cae --version)...");
    let help_out = h.run_cae(&["--help"]);
    assert_success(&help_out, "cae --help");
    let help_text = stdout(&help_out);
    assert!(help_text.contains("Career Asset Engine CLI"));
    assert!(help_text.contains("track"));
    assert!(help_text.contains("untrack"));
    assert!(help_text.contains("status"));
    assert!(help_text.contains("dir"));
    assert!(help_text.contains("record-commit"));
    assert!(help_text.contains("wallet"));

    let ver_out = h.run_cae(&["--version"]);
    assert_success(&ver_out, "cae --version");
    assert!(stdout(&ver_out).contains("cae 0.1.0"));

    // -------------------------------------------------------------------------
    // 2. Directory Inspection: dir & status --dir
    // -------------------------------------------------------------------------
    println!("[2/14] Testing Directory Inspection (cae dir, cae status --dir)...");
    let dir_out = h.run_cae(&["dir"]);
    assert_success(&dir_out, "cae dir");
    let dir_path = stdout(&dir_out).trim().to_string();
    assert_eq!(dir_path, h.projects_dir.to_string_lossy());

    let status_dir_out = h.run_cae(&["status", "--dir"]);
    assert_success(&status_dir_out, "cae status --dir");
    assert_eq!(stdout(&status_dir_out).trim(), dir_path);

    // -------------------------------------------------------------------------
    // 3. Project Tracking: track (with --id, --name, --tag, --notes)
    // -------------------------------------------------------------------------
    println!("[3/14] Testing Project Tracking (cae track)...");
    let track_out = h.run_cae(&[
        "track",
        h.sample_repo.to_str().ok_or("invalid utf8 path")?,
        "--id",
        "demo-pipeline-proj",
        "--name",
        "Pipeline Demo Project",
        "--tag",
        "ci-test",
        "--tag",
        "rust",
        "--notes",
        "Hermetic pipeline integration verification",
    ]);
    assert_success(&track_out, "cae track");
    let track_msg = stdout(&track_out);
    assert!(track_msg.contains("Successfully tracked project"));
    assert!(track_msg.contains("Pipeline Demo Project"));
    assert!(track_msg.contains("demo-pipeline-proj"));
    assert!(track_msg.contains("Cargo.toml"));
    assert!(track_msg.contains("package.json"));
    assert!(track_msg.contains("pyproject.toml"));
    assert!(track_msg.contains("ARCHITECTURE.md"));
    assert!(track_msg.contains("README.md"));
    assert!(track_msg.contains("DESIGN.md"));
    assert!(track_msg.contains("MEMORY.md"));

    // Verify ledger persistence on disk
    let ledger_file = h.projects_dir.join("demo-pipeline-proj.json");
    assert!(ledger_file.exists(), "Ledger JSON file must exist");

    // -------------------------------------------------------------------------
    // 4. Project Status: status, status <id>, status --all, status --json
    // -------------------------------------------------------------------------
    println!("[4/14] Testing Project Status (cae status variants)...");
    // In repo (auto-detection from current dir)
    let auto_status = h.run_cae_in_repo(&["status"]);
    assert_success(&auto_status, "cae status (auto)");
    assert!(stdout(&auto_status).contains("Pipeline Demo Project"));

    // Explicit ID
    let id_status = h.run_cae(&["status", "demo-pipeline-proj"]);
    assert_success(&id_status, "cae status <id>");
    assert!(stdout(&id_status).contains("Tags:        ci-test, rust"));
    assert!(stdout(&id_status).contains("Hermetic pipeline integration verification"));

    // All projects
    let all_status = h.run_cae(&["status", "--all"]);
    assert_success(&all_status, "cae status --all");
    assert!(stdout(&all_status).contains("Tracked Projects (1)"));
    assert!(stdout(&all_status).contains("demo-pipeline-proj"));

    // Machine-readable JSON
    let json_status = h.run_cae(&["status", "demo-pipeline-proj", "--json"]);
    assert_success(&json_status, "cae status --json");
    let status_val: serde_json::Value = serde_json::from_str(&stdout(&json_status))?;
    assert_eq!(status_val["id"], "demo-pipeline-proj");
    assert_eq!(status_val["name"], "Pipeline Demo Project");
    assert_eq!(
        status_val["manifests"]
            .as_array()
            .ok_or("expected array")?
            .len(),
        3
    );
    assert_eq!(
        status_val["context_files"]
            .as_array()
            .ok_or("expected array")?
            .len(),
        4
    );

    // -------------------------------------------------------------------------
    // 5. Commit Recording: record-commit
    // -------------------------------------------------------------------------
    println!("[5/14] Testing Commit Recording (cae record-commit)...");
    // Make a commit in the git repo
    fs::write(
        h.sample_repo.join("ARCHITECTURE.md"),
        "# Architecture Updated\nNew pipeline architecture specs.\n",
    )?;
    h.run_git_in_repo(&["commit", "-am", "feat: updated architecture specifications"]);

    // Auto-detect project in repo
    let rec_auto = h.run_cae_in_repo(&["record-commit", "--note", "Recorded in CI"]);
    assert_success(&rec_auto, "cae record-commit (auto)");
    assert!(stdout(&rec_auto).contains("Successfully recorded commit in ledger"));
    assert!(stdout(&rec_auto).contains("Recorded in CI"));

    // Explicit project ID
    let rec_proj = h.run_cae(&[
        "record-commit",
        "-p",
        "demo-pipeline-proj",
        "-m",
        "Override commit message for ledger",
        "-n",
        "Explicit project record",
    ]);
    assert_success(&rec_proj, "cae record-commit -p");
    assert!(stdout(&rec_proj).contains("Explicit project record"));

    // Verify commit count in status
    let status_after_commit = h.run_cae(&["status", "demo-pipeline-proj", "--json"]);
    let sc_val: serde_json::Value = serde_json::from_str(&stdout(&status_after_commit))?;
    assert!(!sc_val["commits"]
        .as_array()
        .ok_or("expected array")?
        .is_empty());

    // -------------------------------------------------------------------------
    // 6. Project Untracking: untrack
    // -------------------------------------------------------------------------
    println!("[6/14] Testing Project Untracking (cae untrack)...");
    let untrack_out = h.run_cae(&["untrack", "demo-pipeline-proj"]);
    assert_success(&untrack_out, "cae untrack");
    assert!(stdout(&untrack_out).contains("Untracked project 'Pipeline Demo Project'"));
    assert!(!h.projects_dir.join("demo-pipeline-proj.json").exists());

    // -------------------------------------------------------------------------
    // 7. Wallet Help: cae wallet --help
    // -------------------------------------------------------------------------
    println!("[7/14] Testing Wallet Help (cae wallet --help)...");
    let wallet_help = h.run_cae(&["wallet", "--help"]);
    assert_success(&wallet_help, "cae wallet --help");
    let w_text = stdout(&wallet_help);
    assert!(w_text.contains("init"));
    assert!(w_text.contains("login"));
    assert!(w_text.contains("logout"));
    assert!(w_text.contains("show"));
    assert!(w_text.contains("address"));
    assert!(w_text.contains("balance"));
    assert!(w_text.contains("fund"));
    assert!(w_text.contains("set-network"));
    assert!(w_text.contains("limit"));
    assert!(w_text.contains("export-key"));

    // -------------------------------------------------------------------------
    // 8. Wallet Initialization (Offline local-eoa), Address & Show
    // -------------------------------------------------------------------------
    println!("[8/14] Testing Wallet Local Initialization, Address & Show...");
    let init_local = h.run_cae(&["wallet", "init", "--local", "--force"]);
    assert_success(&init_local, "cae wallet init --local");
    assert!(stdout(&init_local).contains("Local wallet created"));

    // Check permissions are 0600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(h.wallet_file())?.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "Wallet profile must have strict 0600 permissions"
        );
    }

    // address command
    let addr_out = h.run_cae(&["wallet", "address"]);
    assert_success(&addr_out, "cae wallet address");
    let raw_addr = stdout(&addr_out).trim().to_string();
    assert_eq!(raw_addr.len(), 42);
    assert!(raw_addr.starts_with("0x"));

    // show command (human & json)
    let show_out = h.run_cae(&["wallet", "show"]);
    assert_success(&show_out, "cae wallet show");
    assert!(stdout(&show_out).contains("local-eoa"));
    assert!(stdout(&show_out).contains("base-sepolia"));

    let show_json = h.run_cae(&["wallet", "show", "--json"]);
    assert_success(&show_json, "cae wallet show --json");
    let sj_val: serde_json::Value = serde_json::from_str(&stdout(&show_json))?;
    assert_eq!(sj_val["wallet_type"], "local-eoa");
    assert_eq!(sj_val["evm_address"], raw_addr);
    assert_eq!(sj_val["network"], "base-sepolia");
    assert_eq!(sj_val["daily_limit_usdc"], 0.15);

    // -------------------------------------------------------------------------
    // 9. Wallet Daily Spend Limits: limit get, limit set, limit --json
    // -------------------------------------------------------------------------
    println!("[9/14] Testing Wallet Spend Limits (cae wallet limit)...");
    let limit_get = h.run_cae(&["wallet", "limit"]);
    assert_success(&limit_get, "cae wallet limit");
    assert!(stdout(&limit_get).contains("Daily spend allowance: $0.15 USDC"));

    let limit_set = h.run_cae(&["wallet", "limit", "set", "0.45"]);
    assert_success(&limit_set, "cae wallet limit set");
    assert!(stdout(&limit_set).contains("updated to $0.45 USDC"));

    let limit_json = h.run_cae(&["wallet", "limit", "get", "--json"]);
    assert_success(&limit_json, "cae wallet limit get --json");
    let lim_val: serde_json::Value = serde_json::from_str(&stdout(&limit_json))?;
    assert_eq!(lim_val["daily_limit_usdc"], 0.45);
    assert_eq!(lim_val["daily_remaining_usdc"], 0.45);

    // -------------------------------------------------------------------------
    // 10. Wallet Network Switching: set-network base & base-sepolia
    // -------------------------------------------------------------------------
    println!("[10/14] Testing Wallet Network Switching (cae wallet set-network)...");
    let set_net_base = h.run_cae(&["wallet", "set-network", "base"]);
    assert_success(&set_net_base, "cae wallet set-network base");
    assert!(stdout(&set_net_base).contains("base (Base Mainnet)"));

    let show_base = h.run_cae(&["wallet", "show", "--json"]);
    let sb_val: serde_json::Value = serde_json::from_str(&stdout(&show_base))?;
    assert_eq!(sb_val["network"], "base");

    let set_net_sepolia = h.run_cae(&["wallet", "set-network", "base-sepolia", "--json"]);
    assert_success(&set_net_sepolia, "cae wallet set-network base-sepolia");
    let ss_val: serde_json::Value = serde_json::from_str(&stdout(&set_net_sepolia))?;
    assert_eq!(ss_val["network"], "base-sepolia");
    assert_eq!(ss_val["chain_id"], 84532);

    // -------------------------------------------------------------------------
    // 11. Wallet Key Export: export-key (--force)
    // -------------------------------------------------------------------------
    println!("[11/14] Testing Wallet Key Export (cae wallet export-key)...");
    let export_out = h.run_cae(&["wallet", "export-key", "--force"]);
    assert_success(&export_out, "cae wallet export-key --force");
    let raw_priv = stdout(&export_out).trim().to_string();
    assert_eq!(raw_priv.len(), 66);
    assert!(raw_priv.starts_with("0x"));

    // Verify JSON export mode
    let export_json = h.run_cae(&["wallet", "export-key", "--force", "--json"]);
    assert_success(&export_json, "cae wallet export-key --json");
    let exp_val: serde_json::Value = serde_json::from_str(&stdout(&export_json))?;
    assert_eq!(exp_val["evm_address"], raw_addr);
    assert_eq!(exp_val["private_key"], raw_priv);

    // -------------------------------------------------------------------------
    // 12. Wallet Balance & Tailoring Quota Inspection: balance
    // -------------------------------------------------------------------------
    println!("[12/14] Testing Wallet Balance & Tailoring Runs (cae wallet balance)...");
    // Mock RPC returning 0.005 ETH and 1.50 USDC (floor(1.50/0.03) = 50 runs)
    let (rpc_url, _calls) = spawn_mock_rpc("0x11c37937e08000", "0x16e360");

    println!("\n$ cae wallet balance --json");
    let mut bal_cmd = h.cmd(&["wallet", "balance", "--json"]);
    bal_cmd.env("CAE_RPC_URL_BASE_SEPOLIA", &rpc_url);
    let bal_out = bal_cmd.output()?;
    h.print_output(&bal_out);
    assert_success(&bal_out, "cae wallet balance --json");

    let bal_val: serde_json::Value = serde_json::from_str(&stdout(&bal_out))?;
    assert_eq!(bal_val["wallet_address"], raw_addr);
    assert_eq!(bal_val["network"], "base-sepolia");
    assert_eq!(bal_val["eth_balance"], 0.005);
    assert_eq!(bal_val["usdc_balance"], 1.5);
    assert_eq!(bal_val["tailoring_runs_available"], 50);
    assert_eq!(bal_val["cost_per_run_usdc"], 0.03);

    // -------------------------------------------------------------------------
    // 13. Testnet Faucet Funding & Mainnet Refusal Guard: fund
    // -------------------------------------------------------------------------
    println!("[13/14] Testing Faucet Funding & Mainnet Guard (cae wallet fund)...");
    // 1. Refusal on Base Mainnet
    let _ = h.run_cae(&["wallet", "set-network", "base"]);
    let fund_refused = h.run_cae(&["wallet", "fund"]);
    assert_eq!(
        fund_refused.status.code(),
        Some(5),
        "Faucet must exit with code 5 on mainnet"
    );
    assert!(stderr(&fund_refused).contains("Faucets are only available on testnets"));

    // 2. Success on Base Sepolia with mock relayer
    let _ = h.run_cae(&["wallet", "set-network", "base-sepolia"]);
    let (faucet_url, _fcalls) = spawn_mock_faucet("0x98f7b01234567890abcdef1234567890abcdef61a");
    println!("\n$ cae wallet fund --json");
    let mut fund_cmd = h.cmd(&["wallet", "fund", "--json"]);
    fund_cmd.env("CAE_CDP_FAUCET_URL", &faucet_url);
    let fund_out = fund_cmd.output()?;
    h.print_output(&fund_out);
    assert_success(&fund_out, "cae wallet fund --json");

    let fund_val: serde_json::Value = serde_json::from_str(&stdout(&fund_out))?;
    assert_eq!(fund_val["recipient"], raw_addr);
    assert_eq!(fund_val["network"], "base-sepolia");
    assert_eq!(fund_val["token"], "USDC");
    assert_eq!(
        fund_val["transaction_hash"],
        "0x98f7b01234567890abcdef1234567890abcdef61a"
    );
    assert_eq!(
        fund_val["explorer_url"],
        "https://sepolia.basescan.org/tx/0x98f7b01234567890abcdef1234567890abcdef61a"
    );
    assert_eq!(fund_val["status"], "confirmed");

    // 3. Verify updated wallet balance after funding
    println!("\n$ cae wallet balance (after funding)");
    let (rpc_url_funded, _funded_calls) = spawn_mock_rpc("0x2386f26fc10000", "0x989680"); // 0.01 ETH, 10.0 USDC (333 tailoring runs)
    let mut bal_after_fund = h.cmd(&["wallet", "balance"]);
    bal_after_fund.env("CAE_RPC_URL_BASE_SEPOLIA", &rpc_url_funded);
    let bal_af_out = bal_after_fund.output()?;
    h.print_output(&bal_af_out);
    assert_success(&bal_af_out, "cae wallet balance after funding");
    assert!(stdout(&bal_af_out).contains("10.00 USDC"));

    println!("\n$ cae wallet balance --json (after funding)");
    let mut bal_after_fund_json = h.cmd(&["wallet", "balance", "--json"]);
    bal_after_fund_json.env("CAE_RPC_URL_BASE_SEPOLIA", &rpc_url_funded);
    let bal_af_json_out = bal_after_fund_json.output()?;
    h.print_output(&bal_af_json_out);
    assert_success(&bal_af_json_out, "cae wallet balance --json after funding");
    let bal_af_val: serde_json::Value = serde_json::from_str(&stdout(&bal_af_json_out))?;
    assert_eq!(bal_af_val["usdc_balance"], 10.0);
    assert_eq!(bal_af_val["eth_balance"], 0.01);
    assert_eq!(bal_af_val["tailoring_runs_available"], 333);

    // -------------------------------------------------------------------------
    // 14. CDP User Auth, Backup, Enclave Guard & Logout: init, login, logout
    // -------------------------------------------------------------------------
    println!("[14/14] Testing CDP User Auth, Local Backup & Session Logout...");
    // A. Terminal OTP flow with pipe
    println!("\n$ cae wallet init --force --no-browser --json (terminal OTP)");
    let mut child = Command::new(TestHarness::bin())
        .args(["wallet", "init", "--force", "--no-browser", "--json"])
        .env("CAE_HOME", &h.home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    {
        let mut stdin = child.stdin.take().ok_or("missing stdin")?;
        stdin.write_all(b"candidate@careerengine.local\n123456\n")?;
    }
    let otp_out = child.wait_with_output()?;
    h.print_output(&otp_out);
    assert_success(&otp_out, "cae wallet init (terminal OTP)");
    let cdp_val: serde_json::Value = serde_json::from_str(&stdout(&otp_out))?;
    assert_eq!(cdp_val["wallet_type"], "cdp-embedded");
    assert_eq!(cdp_val["email"], "candidate@careerengine.local");
    assert_eq!(cdp_val["status"], "active");

    // Verify enclave export-key refusal
    let enclave_guard = h.run_cae(&["wallet", "export-key", "--force"]);
    assert!(
        !enclave_guard.status.success(),
        "Exporting CDP enclave key must fail"
    );
    assert!(stderr(&enclave_guard).contains("enclave"));

    // B. Loopback redirect flow with login
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
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
        let req = "GET /callback?address=0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c&email=synced@example.com&session_ref=cdp_sess_verified HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let _ = stream.write_all(req.as_bytes());
    });

    println!("\n$ cae wallet login --json (loopback auth)");
    let mut login_cmd = h.cmd(&["wallet", "login", "--json"]);
    login_cmd
        .env("CAE_AUTH_PORT", port.to_string())
        .env("CAE_FORCE_BROWSER", "1");
    let login_out = login_cmd.output()?;
    h.print_output(&login_out);
    assert_success(&login_out, "cae wallet login (loopback)");
    let login_val: serde_json::Value = serde_json::from_str(&stdout(&login_out))?;
    assert_eq!(login_val["wallet_type"], "cdp-embedded");
    assert_eq!(
        login_val["evm_address"],
        "0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c"
    );
    assert_eq!(login_val["email"], "synced@example.com");

    // C. Logout
    let logout_out = h.run_cae(&["wallet", "logout", "--json"]);
    assert_success(&logout_out, "cae wallet logout --json");
    let lo_val: serde_json::Value = serde_json::from_str(&stdout(&logout_out))?;
    assert_eq!(lo_val["status"], "logged-out");
    assert_eq!(lo_val["session_cleared"], true);

    let show_logged_out = h.run_cae(&["wallet", "show", "--json"]);
    let slo_val: serde_json::Value = serde_json::from_str(&stdout(&show_logged_out))?;
    assert_eq!(slo_val["status"], "logged-out");

    println!("\n========================================================");
    println!("=== ALL 14 COMMAND GROUPS & SUBCOMMANDS VERIFIED! ===");
    println!("========================================================\n");
    Ok(())
}
