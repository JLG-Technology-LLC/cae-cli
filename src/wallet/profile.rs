use anyhow::{anyhow, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::ledger::normalize_home_dir;

pub const DEFAULT_DAILY_LIMIT_USDC: f64 = 0.15;

#[derive(Serialize, Deserialize, ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletType {
    #[serde(rename = "cdp-embedded")]
    #[value(name = "cdp-embedded")]
    CdpEmbedded,
    #[serde(rename = "local-eoa")]
    #[value(name = "local-eoa")]
    LocalEoa,
}

impl WalletType {
    pub fn as_str(self) -> &'static str {
        match self {
            WalletType::CdpEmbedded => "cdp-embedded",
            WalletType::LocalEoa => "local-eoa",
        }
    }
}

#[derive(Serialize, Deserialize, ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    #[serde(rename = "base-sepolia")]
    #[value(name = "base-sepolia")]
    BaseSepolia,
    #[serde(rename = "base")]
    #[value(name = "base")]
    Base,
}

impl Network {
    pub fn as_str(self) -> &'static str {
        match self {
            Network::BaseSepolia => "base-sepolia",
            Network::Base => "base",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Network::BaseSepolia => "Base Sepolia",
            Network::Base => "Base Mainnet",
        }
    }

    pub fn chain_id(self) -> u64 {
        match self {
            Network::BaseSepolia => 84532,
            Network::Base => 8453,
        }
    }
}

/// On-disk candidate wallet record (`~/.cae/wallet.json`), conforming to
/// `contracts/wallet-profile.schema.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CandidateWalletProfile {
    pub wallet_type: WalletType,
    pub evm_address: String,
    #[serde(default)]
    pub email: Option<String>,
    pub network: Network,
    pub created_at: String,
    #[serde(default)]
    pub session_ref: Option<String>,
    /// Hex-encoded 32-byte private key for `local-eoa` wallets (never printed by `show`).
    #[serde(default)]
    pub encrypted_privkey: Option<String>,
    pub daily_limit_usdc: f64,
    pub spent_today_usdc: f64,
    pub last_spend_date: String,
}

/// Resolve the wallet file location: `$CAE_HOME/wallet.json` when `CAE_HOME` is set,
/// otherwise `~/.cae/wallet.json`.
pub fn wallet_path() -> Result<PathBuf> {
    if let Some(custom) = std::env::var_os("CAE_HOME").filter(|v| !v.is_empty()) {
        let custom = PathBuf::from(custom);
        if !custom.is_absolute() {
            return Err(anyhow!("CAE_HOME must be an absolute path"));
        }
        return Ok(custom.join("wallet.json"));
    }
    let raw_home = dirs::home_dir().context("Could not determine user home directory")?;
    Ok(normalize_home_dir(raw_home)
        .join(".cae")
        .join("wallet.json"))
}

pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".bak");
    PathBuf::from(name)
}

pub fn backup_local_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or(Path::new(""));
    parent.join("wallet.local.bak")
}

pub fn load_profile(path: &Path) -> Result<Option<CandidateWalletProfile>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(path).with_context(|| format!("Reading wallet {}", path.display()))?;
    let profile: CandidateWalletProfile = serde_json::from_str(&raw)
        .with_context(|| format!("Parsing wallet profile {}", path.display()))?;
    if !crate::wallet::crypto::is_valid_address(&profile.evm_address) {
        return Err(anyhow!(
            "Wallet profile {} has an invalid evm_address '{}'",
            path.display(),
            profile.evm_address
        ));
    }
    Ok(Some(profile))
}

/// Atomically persist the profile (temp file + rename) with `0600` permissions.
pub fn save_profile(path: &Path, profile: &CandidateWalletProfile) -> Result<()> {
    let json = serde_json::to_string_pretty(profile)?;
    write_private_file(path, json.as_bytes())
}

/// Copy the current wallet file to `<path>.bak` with `0600` permissions.
pub fn backup_profile(path: &Path) -> Result<PathBuf> {
    let bytes = fs::read(path).with_context(|| format!("Reading wallet {}", path.display()))?;
    let dest = backup_path(path);
    write_private_file(&dest, &bytes)?;
    Ok(dest)
}

/// Copy the current wallet file to `wallet.local.bak` with `0600` permissions.
pub fn backup_local_profile(path: &Path) -> Result<PathBuf> {
    let bytes = fs::read(path).with_context(|| format!("Reading wallet {}", path.display()))?;
    let dest = backup_local_path(path);
    write_private_file(&dest, &bytes)?;
    Ok(dest)
}

fn write_private_file(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Wallet path {} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Creating wallet directory {}", parent.display()))?;

    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(format!(".tmp{}", std::process::id()));
    let tmp_path = PathBuf::from(tmp_name);

    let result = (|| -> Result<()> {
        let mut file = open_private(&tmp_path)?;
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&tmp_path, path)
            .with_context(|| format!("Replacing wallet file {}", path.display()))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

#[cfg(unix)]
fn open_private(path: &Path) -> Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("Creating {}", path.display()))
}

#[cfg(not(unix))]
fn open_private(path: &Path) -> Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .with_context(|| format!("Creating {}", path.display()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample() -> CandidateWalletProfile {
        CandidateWalletProfile {
            wallet_type: WalletType::LocalEoa,
            evm_address: "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf".to_string(),
            email: None,
            network: Network::BaseSepolia,
            created_at: "2026-11-11T17:30:00+00:00".to_string(),
            session_ref: None,
            encrypted_privkey: Some("01".repeat(32)),
            daily_limit_usdc: DEFAULT_DAILY_LIMIT_USDC,
            spent_today_usdc: 0.0,
            last_spend_date: "2026-11-11".to_string(),
        }
    }

    fn temp_dir(tag: &str) -> Result<PathBuf> {
        let dir = std::env::temp_dir().join(format!("cae_wallet_{}_{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    #[test]
    fn roundtrip_and_missing_file() -> Result<()> {
        let dir = temp_dir("roundtrip")?;
        let path = dir.join("nested").join("wallet.json");
        assert!(load_profile(&path)?.is_none());

        let profile = sample();
        save_profile(&path, &profile)?;
        assert_eq!(load_profile(&path)?, Some(profile));
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn saved_file_is_owner_only() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("perms")?;
        let path = dir.join("wallet.json");
        save_profile(&path, &sample())?;
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);

        // Overwriting an existing file keeps the restrictive mode.
        save_profile(&path, &sample())?;
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);

        let bak = backup_profile(&path)?;
        assert_eq!(fs::metadata(&bak)?.permissions().mode() & 0o777, 0o600);
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn serializes_contract_enum_values() -> Result<()> {
        let json = serde_json::to_value(sample())?;
        assert_eq!(json["wallet_type"], "local-eoa");
        assert_eq!(json["network"], "base-sepolia");
        Ok(())
    }

    #[test]
    fn rejects_unknown_fields() {
        if let Ok(mut json) = serde_json::to_value(sample()) {
            json["surprise"] = serde_json::json!(true);
            assert!(serde_json::from_value::<CandidateWalletProfile>(json).is_err());
        }
    }
}
