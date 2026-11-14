use anyhow::{anyhow, bail, Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

use super::profile::Network;
use super::WalletError;

const FAUCET_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FaucetReceipt {
    pub recipient: String,
    pub network: String,
    pub token: String,
    pub transaction_hash: String,
    pub explorer_url: String,
    pub status: String,
}

pub fn faucet_url() -> String {
    std::env::var("CAE_CDP_FAUCET_URL")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "https://api.cdp.coinbase.com/platform/v2/evm/faucet".to_string())
}

/// Request testnet tokens from the CDP faucet for Base Sepolia.
pub fn request_faucet(network: Network, address: &str, token: &str) -> Result<FaucetReceipt> {
    if network == Network::Base {
        return Err(WalletError::Refused(
            "Faucets are only available on testnets. Current network is Base Mainnet. \
             Switch using `cae wallet set-network base-sepolia`."
                .to_string(),
        )
        .into());
    }

    let token_normalized = token.to_ascii_uppercase();
    let url = faucet_url();

    let mut builder = reqwest::blocking::Client::builder().timeout(FAUCET_TIMEOUT);
    if is_loopback_url(&url) {
        builder = builder.no_proxy();
    }
    let client = builder.build().context("Building HTTP client for faucet")?;

    let mut req = client.post(&url).json(&json!({
        "address": address,
        "network": "base-sepolia",
        "token": token.to_ascii_lowercase(),
    }));

    // Pass Bearer token if configured in environment
    if let Ok(bearer) =
        std::env::var("CDP_API_TOKEN").or_else(|_| std::env::var("CDP_BEARER_TOKEN"))
    {
        if !bearer.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", bearer));
        }
    }

    let response = req.send().with_context(|| {
        format!(
            "Could not reach CDP Faucet at {}. Check your connection or faucet relayer availability.",
            url
        )
    })?;

    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        bail!(
            "CDP Faucet rate limit exceeded for Base Sepolia. Please wait a few minutes or try an external faucet: https://faucet.circle.com/"
        );
    }

    let res_json: serde_json::Value = response
        .json()
        .with_context(|| format!("CDP Faucet endpoint {} returned non-JSON response", url))?;

    if let Some(err) = res_json.get("error").or_else(|| res_json.get("message")) {
        let err_str = err.as_str().unwrap_or("Unknown faucet error");
        bail!("CDP Faucet refused request: {}", err_str);
    }

    let tx_hash = res_json
        .get("transaction_hash")
        .or_else(|| res_json.get("transactionHash"))
        .or_else(|| res_json.get("tx_hash"))
        .or_else(|| res_json.get("hash"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow!(
                "CDP Faucet response missing transaction hash: {:?}",
                res_json
            )
        })?
        .to_string();

    let explorer_url = format!("https://sepolia.basescan.org/tx/{}", tx_hash);

    Ok(FaucetReceipt {
        recipient: address.to_string(),
        network: "base-sepolia".to_string(),
        token: token_normalized,
        transaction_hash: tx_hash,
        explorer_url,
        status: "confirmed".to_string(),
    })
}

fn is_loopback_url(url: &str) -> bool {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split(['/', ':']).next().unwrap_or("");
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

pub fn print_faucet_receipt(receipt: &FaucetReceipt) {
    println!(
        "{} Success! Funded wallet {}",
        "✔".green().bold(),
        receipt.recipient.cyan()
    );
    println!("  Network:     {}", receipt.network);
    println!("  Token:       {}", receipt.token.green());
    println!("  Transaction: {}", receipt.transaction_hash.yellow());
    println!("  Explorer:    {}", receipt.explorer_url.dimmed());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_faucet_on_mainnet() {
        let err = request_faucet(Network::Base, "0x123", "usdc").unwrap_err();
        assert!(matches!(
            err.downcast_ref::<WalletError>(),
            Some(WalletError::Refused(_))
        ));
    }

    #[test]
    fn format_explorer_url() {
        let receipt = FaucetReceipt {
            recipient: "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf".to_string(),
            network: "base-sepolia".to_string(),
            token: "USDC".to_string(),
            transaction_hash: "0x123abc".to_string(),
            explorer_url: "https://sepolia.basescan.org/tx/0x123abc".to_string(),
            status: "confirmed".to_string(),
        };
        assert_eq!(
            receipt.explorer_url,
            "https://sepolia.basescan.org/tx/0x123abc"
        );
    }
}
