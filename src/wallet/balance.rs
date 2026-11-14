use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

use super::profile::Network;

/// Platform tailoring fee: $0.03 USDC = 30,000 atomic units (6 decimals).
pub const COST_PER_RUN_USDC: f64 = 0.03;
pub const COST_PER_RUN_UNITS: u128 = 30_000;

const BALANCE_OF_SELECTOR: &str = "0x70a08231";
const RPC_TIMEOUT: Duration = Duration::from_secs(10);

pub fn usdc_contract(network: Network) -> &'static str {
    match network {
        Network::BaseSepolia => "0x036CbD53842c5426634e7929541eC2318f3dCF7e",
        Network::Base => "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
    }
}

/// JSON-RPC endpoint for a network. `CAE_RPC_URL_BASE_SEPOLIA` / `CAE_RPC_URL_BASE`
/// override the public defaults (used for private RPC providers and hermetic tests).
pub fn rpc_url(network: Network) -> String {
    let (var, default) = match network {
        Network::BaseSepolia => ("CAE_RPC_URL_BASE_SEPOLIA", "https://sepolia.base.org"),
        Network::Base => ("CAE_RPC_URL_BASE", "https://mainnet.base.org"),
    };
    std::env::var(var)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[derive(Debug, Clone, PartialEq)]
pub struct BalanceSummary {
    pub wallet_address: String,
    pub network: Network,
    pub eth_raw: u128,
    pub usdc_raw: u128,
    pub tailoring_runs: u32,
    pub daily_limit_usdc: f64,
    pub spent_today_usdc: f64,
    pub daily_remaining_usdc: f64,
}

impl BalanceSummary {
    pub fn eth(&self) -> f64 {
        self.eth_raw as f64 / 1e18
    }

    pub fn usdc(&self) -> f64 {
        self.usdc_raw as f64 / 1e6
    }

    pub fn to_json(&self) -> Value {
        json!({
            "wallet_address": self.wallet_address,
            "network": self.network.as_str(),
            "eth_balance": self.eth(),
            "usdc_balance": self.usdc(),
            "tailoring_runs_available": self.tailoring_runs,
            "cost_per_run_usdc": COST_PER_RUN_USDC,
            "balances": {
                "eth": {
                    "raw": self.eth_raw.to_string(),
                    "formatted": format!("{:.4} ETH", self.eth()),
                },
                "usdc": {
                    "raw": self.usdc_raw.to_string(),
                    "formatted": format!("{:.2} USDC", self.usdc()),
                },
            },
            "tailoring": {
                "cost_per_run_usdc": COST_PER_RUN_USDC,
                "available_runs": self.tailoring_runs,
                "daily_limit_usdc": self.daily_limit_usdc,
                "spent_today_usdc": self.spent_today_usdc,
                "daily_remaining_usdc": self.daily_remaining_usdc,
            },
        })
    }
}

/// Number of whole tailoring runs a USDC balance (atomic units) can fund.
pub fn tailoring_runs(usdc_raw: u128) -> u32 {
    u32::try_from(usdc_raw / COST_PER_RUN_UNITS).unwrap_or(u32::MAX)
}

/// ABI-encode `balanceOf(address)` call data.
pub fn balance_of_calldata(address: &str) -> String {
    format!(
        "{}{:0>64}",
        BALANCE_OF_SELECTOR,
        address.trim_start_matches("0x").to_lowercase()
    )
}

pub fn parse_hex_quantity(value: &str) -> Result<u128> {
    let digits = value.trim_start_matches("0x");
    if digits.is_empty() {
        return Ok(0);
    }
    u128::from_str_radix(digits, 16)
        .map_err(|_| anyhow!("RPC returned an unparseable quantity '{}'", value))
}

fn rpc_call(
    client: &reqwest::blocking::Client,
    url: &str,
    method: &str,
    params: Value,
) -> Result<String> {
    let response: Value = client
        .post(url)
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
        .send()
        .with_context(|| format!("Could not reach RPC endpoint {}", url))?
        .error_for_status()
        .with_context(|| format!("RPC endpoint {} returned an error status", url))?
        .json()
        .with_context(|| format!("RPC endpoint {} returned invalid JSON", url))?;

    if let Some(err) = response.get("error") {
        bail!("RPC error from {}: {}", url, err);
    }
    response
        .get("result")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("RPC response from {} had no result", url))
}

fn is_loopback_url(url: &str) -> bool {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split(['/', ':']).next().unwrap_or("");
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// Read native ETH and USDC balances (atomic units) for an address.
pub fn fetch_balances(network: Network, address: &str) -> Result<(u128, u128)> {
    let url = rpc_url(network);
    let mut builder = reqwest::blocking::Client::builder().timeout(RPC_TIMEOUT);
    if is_loopback_url(&url) {
        // Local/dev RPC nodes must never be routed through an HTTP(S) system proxy.
        builder = builder.no_proxy();
    }
    let client = builder.build().context("Building HTTP client")?;

    let eth = rpc_call(&client, &url, "eth_getBalance", json!([address, "latest"]))?;
    let usdc = rpc_call(
        &client,
        &url,
        "eth_call",
        json!([
            {"to": usdc_contract(network), "data": balance_of_calldata(address)},
            "latest"
        ]),
    )?;
    Ok((parse_hex_quantity(&eth)?, parse_hex_quantity(&usdc)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calldata_pads_address_to_32_bytes() {
        let data = balance_of_calldata("0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf");
        assert_eq!(data.len(), 10 + 64);
        assert!(data.starts_with("0x70a08231000000000000000000000000"));
        assert!(data.ends_with("7e5f4552091a69125d5dfcb7b8c2659029395bdf"));
    }

    #[test]
    fn parses_hex_quantities() -> Result<()> {
        assert_eq!(parse_hex_quantity("0x0")?, 0);
        assert_eq!(parse_hex_quantity("0x")?, 0);
        assert_eq!(parse_hex_quantity("0x16e360")?, 1_500_000);
        assert_eq!(
            parse_hex_quantity("0x11c37937e08000")?,
            5_000_000_000_000_000
        );
        // balanceOf returns a full 32-byte word.
        assert_eq!(parse_hex_quantity(&format!("0x{:064x}", 42u8))?, 42);
        assert!(parse_hex_quantity("0xzz").is_err());
        Ok(())
    }

    #[test]
    fn quota_is_floor_of_balance_over_fee() {
        assert_eq!(tailoring_runs(0), 0);
        assert_eq!(tailoring_runs(29_999), 0);
        assert_eq!(tailoring_runs(30_000), 1);
        assert_eq!(tailoring_runs(1_500_000), 50);
        assert_eq!(tailoring_runs(1_599_999), 53);
    }

    #[test]
    fn summary_json_exposes_contract_fields() {
        let summary = BalanceSummary {
            wallet_address: "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf".to_string(),
            network: Network::BaseSepolia,
            eth_raw: 5_000_000_000_000_000,
            usdc_raw: 1_500_000,
            tailoring_runs: 50,
            daily_limit_usdc: 0.15,
            spent_today_usdc: 0.03,
            daily_remaining_usdc: 0.12,
        };
        let json = summary.to_json();
        assert_eq!(json["usdc_balance"], 1.5);
        assert_eq!(json["eth_balance"], 0.005);
        assert_eq!(json["tailoring_runs_available"], 50);
        assert_eq!(json["balances"]["usdc"]["raw"], "1500000");
        assert_eq!(json["balances"]["usdc"]["formatted"], "1.50 USDC");
        assert_eq!(json["balances"]["eth"]["formatted"], "0.0050 ETH");
        assert_eq!(json["tailoring"]["daily_remaining_usdc"], 0.12);
    }

    #[test]
    fn detects_loopback_endpoints() {
        assert!(is_loopback_url("http://127.0.0.1:8545"));
        assert!(is_loopback_url("http://localhost:8545/rpc"));
        assert!(!is_loopback_url("https://sepolia.base.org"));
        assert!(!is_loopback_url("https://127.0.0.1.example.com"));
    }

    #[test]
    fn known_usdc_contracts() {
        assert_eq!(
            usdc_contract(Network::BaseSepolia),
            "0x036CbD53842c5426634e7929541eC2318f3dCF7e"
        );
        assert_eq!(
            usdc_contract(Network::Base),
            "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
        );
    }
}
