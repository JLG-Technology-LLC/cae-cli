pub mod auth;
pub mod balance;
pub mod crypto;
pub mod faucet;
pub mod limit;
pub mod profile;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::{Args, Subcommand};
use colored::*;
use serde_json::json;
use std::fmt;
use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;

use profile::{
    backup_local_path, backup_local_profile, backup_profile, load_profile, save_profile,
    wallet_path, CandidateWalletProfile, Network, WalletType, DEFAULT_DAILY_LIMIT_USDC,
};

/// Failures that map to the exit codes in `contracts/wallet-cli-contract.md`.
#[derive(Debug)]
pub enum WalletError {
    /// Exit code 2: spend allowance exceeded in a non-interactive session.
    SpendLimitExceeded(String),
    /// Exit code 3: no wallet has been initialized.
    NoWallet,
    /// Exit code 5: action refused on the current network / wallet type.
    #[allow(dead_code)] // raised by `cae wallet fund` on mainnet
    Refused(String),
}

impl fmt::Display for WalletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WalletError::SpendLimitExceeded(hint) => {
                write!(f, "Daily spend allowance exceeded. {}", hint)
            }
            WalletError::NoWallet => {
                write!(f, "No wallet found. Run `cae wallet init` to create one.")
            }
            WalletError::Refused(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for WalletError {}

/// Map an error from a wallet command to its process exit code.
pub fn exit_code(err: &anyhow::Error) -> i32 {
    match err.downcast_ref::<WalletError>() {
        Some(WalletError::SpendLimitExceeded(_)) => 2,
        Some(WalletError::NoWallet) => 3,
        Some(WalletError::Refused(_)) => 5,
        None => 1,
    }
}

#[derive(Subcommand, Debug)]
pub enum WalletCommand {
    /// Initialize a candidate wallet (CDP User Auth by default, `--local` for offline)
    Init(InitArgs),
    /// Re-authenticate or sync an existing CDP user wallet
    Login(LoginArgs),
    /// De-authorize the active CDP session
    Logout {
        #[arg(long)]
        json: bool,
    },
    /// Display wallet status, address, network, and limits
    Show {
        #[arg(long)]
        json: bool,
    },
    /// Print only the 42-character wallet address
    Address,
    /// Query ETH/USDC balances and available tailoring runs
    Balance(BalanceArgs),
    /// Request testnet tokens on Base Sepolia
    Fund(FundArgs),
    /// Switch the active default network
    SetNetwork {
        network: Network,
        #[arg(long)]
        json: bool,
    },
    /// View or adjust the daily spend allowance
    Limit {
        #[command(subcommand)]
        action: Option<LimitAction>,
        #[arg(long, global = true)]
        json: bool,
    },
    /// Export the private key of a local-eoa wallet
    ExportKey {
        /// Skip the interactive confirmation prompt
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Generate an offline, device-local wallet instead of using CDP User Auth
    #[arg(long)]
    local: bool,
    /// Default network for the new wallet
    #[arg(long, value_enum, default_value = "base-sepolia")]
    network: Network,
    /// Use terminal email OTP instead of opening a browser
    #[arg(long)]
    no_browser: bool,
    /// Overwrite an existing wallet (the previous file is kept as wallet.json.bak)
    #[arg(long)]
    force: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
pub struct LoginArgs {
    #[arg(long)]
    email: Option<String>,
    #[arg(long)]
    no_browser: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
pub struct BalanceArgs {
    #[arg(long, value_enum)]
    network: Option<Network>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
pub struct FundArgs {
    /// Token to request (defaults to usdc)
    #[arg(long, default_value = "usdc")]
    token: String,
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand, Debug)]
pub enum LimitAction {
    /// Show the current daily spend allowance
    Get,
    /// Set the daily spend allowance in USDC
    Set { amount: String },
}

pub fn run(command: WalletCommand) -> Result<()> {
    match command {
        WalletCommand::Init(args) => handle_init(args),
        WalletCommand::Login(args) => handle_login(args),
        WalletCommand::Logout { json } => handle_logout(json),
        WalletCommand::Show { json } => handle_show(json),
        WalletCommand::Address => handle_address(),
        WalletCommand::Balance(args) => handle_balance(args),
        WalletCommand::Fund(args) => handle_fund(args),
        WalletCommand::SetNetwork { network, json } => handle_set_network(network, json),
        WalletCommand::Limit { action, json } => handle_limit(action, json),
        WalletCommand::ExportKey { force, json } => handle_export_key(force, json),
    }
}

/// Ask a `[y/N]` question on the terminal. Returns `false` when stdin is not interactive.
pub fn confirm(question: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        return Ok(false);
    }
    eprint!("{} [y/N]: ", question);
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn require_profile(path: &Path) -> Result<CandidateWalletProfile> {
    load_profile(path)?.ok_or_else(|| WalletError::NoWallet.into())
}

fn load_with_rollover(path: &Path) -> Result<CandidateWalletProfile> {
    let mut profile = require_profile(path)?;
    limit::rollover(&mut profile, &limit::today());
    Ok(profile)
}

fn status_label(profile: &CandidateWalletProfile) -> &'static str {
    if profile.wallet_type == WalletType::CdpEmbedded && profile.session_ref.is_none() {
        "logged-out"
    } else {
        "active"
    }
}

fn show_json(profile: &CandidateWalletProfile, path: &Path) -> serde_json::Value {
    json!({
        "status": status_label(profile),
        "wallet_type": profile.wallet_type.as_str(),
        "evm_address": profile.evm_address,
        "email": profile.email,
        "network": profile.network.as_str(),
        "created_at": profile.created_at,
        "daily_limit_usdc": profile.daily_limit_usdc,
        "spent_today_usdc": profile.spent_today_usdc,
        "file_path": path.display().to_string(),
    })
}

// -----------------------------------------------------------------------------
// init
// -----------------------------------------------------------------------------
fn handle_init(args: InitArgs) -> Result<()> {
    let path = wallet_path()?;
    if path.exists() {
        if !args.force {
            bail!(
                "A wallet already exists at {}. Re-run with --force to replace it \
                 (the current file will be kept as wallet.json.bak).",
                path.display()
            );
        }
        let bak = backup_profile(&path)?;
        if !args.json {
            eprintln!(
                "{} Existing wallet backed up to {}",
                "!".yellow().bold(),
                bak.display()
            );
        }
    }

    if args.local {
        let key = crypto::generate_local_key()?;
        let profile = CandidateWalletProfile {
            wallet_type: WalletType::LocalEoa,
            evm_address: key.address,
            email: None,
            network: args.network,
            created_at: Utc::now().to_rfc3339(),
            session_ref: None,
            encrypted_privkey: Some(key.private_key_hex),
            daily_limit_usdc: DEFAULT_DAILY_LIMIT_USDC,
            spent_today_usdc: 0.0,
            last_spend_date: limit::today(),
        };
        save_profile(&path, &profile)?;

        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&show_json(&profile, &path))?
            );
        } else {
            println!("{} Local wallet created", "✔".green().bold());
            print_profile(&profile, &path);
        }
        return Ok(());
    }

    // Default: CDP User Auth
    let auth_res = auth::authenticate_user(None, args.no_browser, args.json)?;
    let profile = CandidateWalletProfile {
        wallet_type: WalletType::CdpEmbedded,
        evm_address: auth_res.evm_address,
        email: Some(auth_res.email),
        network: args.network,
        created_at: Utc::now().to_rfc3339(),
        session_ref: Some(auth_res.session_ref),
        encrypted_privkey: None,
        daily_limit_usdc: DEFAULT_DAILY_LIMIT_USDC,
        spent_today_usdc: 0.0,
        last_spend_date: limit::today(),
    };
    save_profile(&path, &profile)?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&show_json(&profile, &path))?
        );
    } else {
        println!("{} CDP Candidate wallet initialized", "✔".green().bold());
        print_profile(&profile, &path);
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// login
// -----------------------------------------------------------------------------
fn handle_login(args: LoginArgs) -> Result<()> {
    let path = wallet_path()?;
    let existing = load_profile(&path)?;

    if let Some(ref prev) = existing {
        if prev.wallet_type == WalletType::LocalEoa {
            let prompt = format!(
                "Local wallet ({}) is active. Link CDP cloud account? Existing local key will be backed up to {}",
                prev.evm_address,
                backup_local_path(&path).display()
            );
            if !args.json && !confirm(&prompt)? {
                bail!("Login cancelled. Existing local wallet was kept.");
            }
            let local_bak = backup_local_profile(&path)?;
            if !args.json {
                eprintln!(
                    "{} Local wallet backed up to {}",
                    "!".yellow().bold(),
                    local_bak.display()
                );
            }
        }
    }

    let auth_res = auth::authenticate_user(args.email, args.no_browser, args.json)?;
    let (network, daily_limit, spent_today, last_spend) = match existing {
        Some(p) => (
            p.network,
            p.daily_limit_usdc,
            p.spent_today_usdc,
            p.last_spend_date,
        ),
        None => (
            Network::BaseSepolia,
            DEFAULT_DAILY_LIMIT_USDC,
            0.0,
            limit::today(),
        ),
    };

    let profile = CandidateWalletProfile {
        wallet_type: WalletType::CdpEmbedded,
        evm_address: auth_res.evm_address,
        email: Some(auth_res.email),
        network,
        created_at: Utc::now().to_rfc3339(),
        session_ref: Some(auth_res.session_ref),
        encrypted_privkey: None,
        daily_limit_usdc: daily_limit,
        spent_today_usdc: spent_today,
        last_spend_date: last_spend,
    };
    save_profile(&path, &profile)?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&show_json(&profile, &path))?
        );
    } else {
        println!("{} CDP Candidate wallet logged in", "✔".green().bold());
        print_profile(&profile, &path);
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// show / address
// -----------------------------------------------------------------------------
fn print_profile(profile: &CandidateWalletProfile, path: &Path) {
    println!("Active Wallet: {}", profile.evm_address.cyan());
    println!("Type:          {}", profile.wallet_type.as_str());
    if let Some(email) = &profile.email {
        println!("Email:         {}", email);
    }
    println!("Network:       {}", profile.network.as_str());
    println!(
        "Daily Limit:   ${:.2} USDC (spent today: ${:.2})",
        profile.daily_limit_usdc, profile.spent_today_usdc
    );
    println!("Created:       {}", profile.created_at.dimmed());
    println!("Storage:       {}", path.display().to_string().dimmed());
    if status_label(profile) == "logged-out" {
        println!(
            "{} Logged out of CDP. Run `cae wallet login` to refresh your session.",
            "!".yellow().bold()
        );
    }
}

fn handle_show(json_output: bool) -> Result<()> {
    let path = wallet_path()?;
    let profile = load_with_rollover(&path)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&show_json(&profile, &path))?
        );
    } else {
        print_profile(&profile, &path);
    }
    Ok(())
}

fn handle_address() -> Result<()> {
    let profile = require_profile(&wallet_path()?)?;
    println!("{}", profile.evm_address);
    Ok(())
}

// -----------------------------------------------------------------------------
// balance
// -----------------------------------------------------------------------------
fn handle_balance(args: BalanceArgs) -> Result<()> {
    let path = wallet_path()?;
    let profile = load_with_rollover(&path)?;
    let network = args.network.unwrap_or(profile.network);

    let (eth_raw, usdc_raw) =
        balance::fetch_balances(network, &profile.evm_address).with_context(|| {
            format!(
                "Could not read balances for {} on {}. Cached address: {}. \
                 Check your network connection and try again.",
                profile.evm_address,
                network.as_str(),
                profile.evm_address
            )
        })?;

    let summary = balance::BalanceSummary {
        wallet_address: profile.evm_address.clone(),
        network,
        eth_raw,
        usdc_raw,
        tailoring_runs: balance::tailoring_runs(usdc_raw),
        daily_limit_usdc: profile.daily_limit_usdc,
        spent_today_usdc: profile.spent_today_usdc,
        daily_remaining_usdc: limit::remaining(&profile),
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary.to_json())?);
    } else {
        println!(
            "Network: {} (Chain ID: {})",
            network.display_name(),
            network.chain_id()
        );
        println!("Wallet:  {}", summary.wallet_address.cyan());
        println!("{}", "-".repeat(40));
        println!("ETH:     {:.4} ETH", summary.eth());
        println!("USDC:    {:.2} USDC", summary.usdc());
        println!(
            "Tailoring Capacity: {} runs (${:.2} USDC per run)",
            summary.tailoring_runs,
            balance::COST_PER_RUN_USDC
        );
        println!(
            "Daily allowance remaining: ${:.2} USDC",
            summary.daily_remaining_usdc
        );
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// fund
// -----------------------------------------------------------------------------
fn handle_fund(args: FundArgs) -> Result<()> {
    let path = wallet_path()?;
    let profile = require_profile(&path)?;

    if profile.network == Network::Base {
        return Err(WalletError::Refused(
            "Faucets are only available on testnets. Current network is Base Mainnet. \
             Switch using `cae wallet set-network base-sepolia`."
                .to_string(),
        )
        .into());
    }

    if !args.json {
        println!(
            "Requesting testnet {} from CDP Faucet on Base Sepolia...",
            args.token.to_ascii_uppercase()
        );
    }

    let receipt = faucet::request_faucet(profile.network, &profile.evm_address, &args.token)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    } else {
        faucet::print_faucet_receipt(&receipt);
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// logout / set-network / limit / export-key
// -----------------------------------------------------------------------------
fn handle_logout(json_output: bool) -> Result<()> {
    let path = wallet_path()?;
    let mut profile = require_profile(&path)?;
    if profile.wallet_type != WalletType::CdpEmbedded {
        bail!("The active wallet is a local-eoa wallet; there is no CDP session to log out of.");
    }
    let had_session = profile.session_ref.take().is_some();
    save_profile(&path, &profile)?;

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "status": "logged-out",
                "session_cleared": had_session,
                "evm_address": profile.evm_address,
            }))?
        );
    } else if had_session {
        println!(
            "{} CDP session cleared from local storage.",
            "✔".green().bold()
        );
    } else {
        println!("No active CDP session was stored.");
    }
    Ok(())
}

fn handle_set_network(network: Network, json_output: bool) -> Result<()> {
    let path = wallet_path()?;
    let mut profile = require_profile(&path)?;
    profile.network = network;
    save_profile(&path, &profile)?;

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "network": network.as_str(),
                "chain_id": network.chain_id(),
            }))?
        );
    } else {
        println!(
            "Active network updated to: {} ({})",
            network.as_str(),
            network.display_name()
        );
    }
    Ok(())
}

fn handle_limit(action: Option<LimitAction>, json_output: bool) -> Result<()> {
    let path = wallet_path()?;
    let mut profile = load_with_rollover(&path)?;

    let updated = match action {
        None | Some(LimitAction::Get) => false,
        Some(LimitAction::Set { amount }) => {
            profile.daily_limit_usdc = limit::parse_limit(&amount)?;
            save_profile(&path, &profile)?;
            true
        }
    };

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "daily_limit_usdc": profile.daily_limit_usdc,
                "spent_today_usdc": profile.spent_today_usdc,
                "daily_remaining_usdc": limit::remaining(&profile),
                "updated": updated,
            }))?
        );
    } else if updated {
        println!(
            "Daily spend allowance updated to ${:.2} USDC.",
            profile.daily_limit_usdc
        );
    } else {
        println!(
            "Daily spend allowance: ${:.2} USDC (Spent today: ${:.2} USDC)",
            profile.daily_limit_usdc, profile.spent_today_usdc
        );
    }
    Ok(())
}

fn handle_export_key(force: bool, json_output: bool) -> Result<()> {
    let path = wallet_path()?;
    let profile = require_profile(&path)?;

    if profile.wallet_type == WalletType::CdpEmbedded {
        bail!(
            "This is a cdp-embedded wallet: its keys are held in Coinbase's secure enclave \
             and cannot be exported."
        );
    }
    let key = profile
        .encrypted_privkey
        .as_deref()
        .context("Wallet file has no private key to export")?;
    if !crypto::address_from_private_key_hex(key)?.eq_ignore_ascii_case(&profile.evm_address) {
        bail!("Stored private key does not match the wallet address; refusing to export.");
    }

    if !force {
        eprintln!(
            "{} Never share your private key. Anyone with this key can access your funds.",
            "WARNING:".red().bold()
        );
        if !confirm("Reveal private key?")? {
            bail!("Export cancelled. Run interactively and answer 'y', or pass --force.");
        }
    }

    let hex_key = format!("0x{}", key.trim_start_matches("0x"));
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "evm_address": profile.evm_address,
                "private_key": hex_key,
            }))?
        );
    } else {
        println!("{}", hex_key);
    }
    Ok(())
}
