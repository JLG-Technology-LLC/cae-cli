use anyhow::{anyhow, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zeroize::{Zeroize, Zeroizing};

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

impl CandidateWalletProfile {
    fn zeroize_secrets(&mut self) {
        self.encrypted_privkey.zeroize();
        self.session_ref.zeroize();
    }
}

impl Drop for CandidateWalletProfile {
    fn drop(&mut self) {
        self.zeroize_secrets();
    }
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
    let raw = Zeroizing::new(
        fs::read_to_string(path).with_context(|| format!("Reading wallet {}", path.display()))?,
    );
    let profile: CandidateWalletProfile = serde_json::from_str(raw.as_str())
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
    let json = Zeroizing::new(serde_json::to_string_pretty(profile)?);
    write_private_file(path, json.as_bytes())
}

/// Copy the current wallet file to `<path>.bak` with `0600` permissions.
pub fn backup_profile(path: &Path) -> Result<PathBuf> {
    let bytes = Zeroizing::new(
        fs::read(path).with_context(|| format!("Reading wallet {}", path.display()))?,
    );
    let dest = backup_path(path);
    write_private_file(&dest, bytes.as_slice())?;
    Ok(dest)
}

/// Copy the current wallet file to `wallet.local.bak` with `0600` permissions.
pub fn backup_local_profile(path: &Path) -> Result<PathBuf> {
    let bytes = Zeroizing::new(
        fs::read(path).with_context(|| format!("Reading wallet {}", path.display()))?,
    );
    let dest = backup_local_path(path);
    write_private_file(&dest, bytes.as_slice())?;
    Ok(dest)
}

const MAX_TEMP_ATTEMPTS: usize = 8;

pub(crate) fn write_private_file(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Wallet path {} has no parent directory", path.display()))?;
    create_private_dir(parent)?;

    let (tmp_path, file) = create_private_temp(path)?;
    let result = write_and_replace(&tmp_path, path, file, contents);
    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

fn write_and_replace(
    tmp_path: &Path,
    dest_path: &Path,
    mut file: fs::File,
    contents: &[u8],
) -> Result<()> {
    file.write_all(contents)
        .with_context(|| format!("Writing temporary wallet file {}", tmp_path.display()))?;
    file.sync_all()
        .with_context(|| format!("Syncing temporary wallet file {}", tmp_path.display()))?;
    drop(file);
    fs::rename(tmp_path, dest_path)
        .with_context(|| format!("Replacing wallet file {}", dest_path.display()))?;
    Ok(())
}

fn create_private_temp(path: &Path) -> Result<(PathBuf, fs::File)> {
    for _ in 0..MAX_TEMP_ATTEMPTS {
        let mut random_bytes = [0u8; 16];
        getrandom::getrandom(&mut random_bytes)
            .map_err(|e| anyhow!("Secure random number generator unavailable: {}", e))?;
        let mut tmp_name = path.as_os_str().to_owned();
        tmp_name.push(".tmp.");
        tmp_name.push(hex::encode(random_bytes));
        let tmp_path = PathBuf::from(tmp_name);

        match open_private(&tmp_path) {
            Ok(file) => return Ok((tmp_path, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(
                    anyhow::Error::new(err).context(format!("Creating {}", tmp_path.display()))
                );
            }
        }
    }
    Err(anyhow!(
        "Failed to allocate a unique temporary file for {} after {} attempts",
        path.display(),
        MAX_TEMP_ATTEMPTS
    ))
}

#[cfg(unix)]
fn create_private_dir(parent: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .with_context(|| format!("Creating wallet directory {}", parent.display()))
}

#[cfg(not(unix))]
fn create_private_dir(parent: &Path) -> Result<()> {
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    fs::create_dir_all(parent)
        .with_context(|| format!("Creating wallet directory {}", parent.display()))
}

#[cfg(unix)]
fn open_private(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    if let Err(err) = file.set_permissions(fs::Permissions::from_mode(0o600)) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(err);
    }
    Ok(file)
}

#[cfg(not(unix))]
fn open_private(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
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

    #[cfg(unix)]
    #[test]
    fn creates_parent_directory_owner_only() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("parent_dir_perms")?;
        let nested = dir.join("nested");
        let deep = nested.join("deep");
        let path = deep.join("wallet.json");

        save_profile(&path, &sample())?;
        assert_eq!(fs::metadata(&nested)?.permissions().mode() & 0o777, 0o700);
        assert_eq!(fs::metadata(&deep)?.permissions().mode() & 0o777, 0o700);
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn ignores_preexisting_predictable_tmp_and_keeps_0600() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("preexisting_tmp")?;
        let path = dir.join("wallet.json");

        let mut legacy_name = path.as_os_str().to_owned();
        legacy_name.push(format!(".tmp{}", std::process::id()));
        let legacy_tmp = PathBuf::from(legacy_name);
        fs::write(&legacy_tmp, b"preexisting")?;
        fs::set_permissions(&legacy_tmp, fs::Permissions::from_mode(0o644))?;

        let profile = sample();
        save_profile(&path, &profile)?;

        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        assert_eq!(load_profile(&path)?, Some(profile));
        assert_eq!(fs::read(&legacy_tmp)?, b"preexisting");
        assert_eq!(
            fs::metadata(&legacy_tmp)?.permissions().mode() & 0o777,
            0o644
        );

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn refuses_preexisting_symlink_at_temp_path() -> Result<()> {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = temp_dir("symlink_refusal")?;
        let victim = dir.join("victim.txt");
        fs::write(&victim, b"untouched")?;

        // Direct open_private must refuse a symlink to an existing file without touching target.
        let symlink_tmp = dir.join("wallet.json.tmp.existing");
        symlink(&victim, &symlink_tmp)?;
        let err = open_private(&symlink_tmp)
            .err()
            .ok_or_else(|| anyhow!("open_private must reject a pre-existing symlink"))?;
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&victim)?, b"untouched");

        // Direct open_private must also refuse a dangling symlink without creating its target.
        let dangling_target = dir.join("dangling_victim.txt");
        let dangling_link = dir.join("wallet.json.tmp.dangling");
        symlink(&dangling_target, &dangling_link)?;
        let err = open_private(&dangling_link)
            .err()
            .ok_or_else(|| anyhow!("open_private must reject a dangling symlink"))?;
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(!dangling_target.exists());

        // Direct open_private must also refuse a pre-existing permissive regular file.
        let existing_file = dir.join("wallet.json.tmp.regular");
        fs::write(&existing_file, b"keep")?;
        fs::set_permissions(&existing_file, fs::Permissions::from_mode(0o644))?;
        let err = open_private(&existing_file)
            .err()
            .ok_or_else(|| anyhow!("open_private must reject an existing file"))?;
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&existing_file)?, b"keep");

        // End-to-end save_profile with a symlink planted at the legacy <path>.tmp<pid> location.
        let path = dir.join("wallet.json");
        let mut legacy_name = path.as_os_str().to_owned();
        legacy_name.push(format!(".tmp{}", std::process::id()));
        let legacy_symlink = PathBuf::from(legacy_name);
        symlink(&victim, &legacy_symlink)?;

        save_profile(&path, &sample())?;
        assert_eq!(fs::read(&victim)?, b"untouched");
        assert!(!fs::symlink_metadata(&path)?.file_type().is_symlink());
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn concurrent_saves_do_not_collide() -> Result<()> {
        let dir = temp_dir("concurrent")?;
        let path = dir.join("wallet.json");

        let mut handles = Vec::new();
        for worker in 0..8u32 {
            let target = path.clone();
            handles.push(std::thread::spawn(move || -> Result<()> {
                let mut profile = sample();
                profile.spent_today_usdc = f64::from(worker) * 0.01;
                for _ in 0..10 {
                    save_profile(&target, &profile)?;
                }
                Ok(())
            }));
        }

        for handle in handles {
            handle
                .join()
                .map_err(|_| anyhow!("concurrent save worker thread panicked"))??;
        }

        let loaded = load_profile(&path)?;
        assert!(loaded.is_some());

        let mut entries = Vec::new();
        for entry in fs::read_dir(&dir)? {
            entries.push(entry?.file_name().to_string_lossy().into_owned());
        }
        assert_eq!(entries, ["wallet.json"]);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        }

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn zeroizes_secret_fields_before_drop() {
        assert!(std::mem::needs_drop::<CandidateWalletProfile>());

        let mut profile = sample();
        profile.session_ref = Some("cdp_sess_secret_token".to_string());
        assert!(profile
            .encrypted_privkey
            .as_deref()
            .is_some_and(|s| !s.is_empty()));
        assert!(profile
            .session_ref
            .as_deref()
            .is_some_and(|s| !s.is_empty()));

        profile.zeroize_secrets();
        assert_eq!(profile.encrypted_privkey, None);
        assert_eq!(profile.session_ref, None);
        assert_eq!(
            profile.evm_address,
            "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf"
        );

        let mut without_secrets = sample();
        without_secrets.encrypted_privkey = None;
        without_secrets.session_ref = None;
        drop(without_secrets);
    }
}
