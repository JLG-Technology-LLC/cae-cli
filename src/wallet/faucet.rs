use anyhow::{anyhow, bail, Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

use super::profile::Network;
use super::WalletError;

const FAUCET_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_ERROR_SNIPPET_CHARS: usize = 256;

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

    let body = response
        .text()
        .map_err(reqwest::Error::without_url)
        .with_context(|| format!("Failed to read response body from CDP Faucet at {}", url))?;

    if !status.is_success() {
        let detail = extract_http_error_detail(&body, status);
        bail!("CDP Faucet returned HTTP {}: {}", status, detail);
    }

    let res_json: serde_json::Value = serde_json::from_str(&body)
        .with_context(|| format!("CDP Faucet endpoint {} returned non-JSON response", url))?;

    if let Some(err_str) = extract_faucet_error(&res_json) {
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

fn truncate_snippet(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    match trimmed.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => format!("{}...", &trimmed[..byte_idx]),
        None => trimmed.to_string(),
    }
}

fn format_json_error_value(val: &serde_json::Value) -> Option<String> {
    match val {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(truncate_snippet(trimmed, MAX_ERROR_SNIPPET_CHARS))
            }
        }
        serde_json::Value::Object(map) => {
            for key in ["message", "errorMessage", "detail"] {
                if let Some(msg) = map
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    return Some(truncate_snippet(msg, MAX_ERROR_SNIPPET_CHARS));
                }
            }
            Some(truncate_snippet(&val.to_string(), MAX_ERROR_SNIPPET_CHARS))
        }
        other => Some(truncate_snippet(
            &other.to_string(),
            MAX_ERROR_SNIPPET_CHARS,
        )),
    }
}

fn extract_faucet_error(val: &serde_json::Value) -> Option<String> {
    let err = val.get("error").filter(|v| !v.is_null())?;
    Some(format_json_error_value(err).unwrap_or_else(|| "Unknown faucet error".to_string()))
}

fn extract_http_error_detail(body: &str, status: reqwest::StatusCode) -> String {
    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(err) = json_val.get("error").and_then(format_json_error_value) {
            return err;
        }
        if let Some(map) = json_val.as_object() {
            for key in ["errorMessage", "message", "detail"] {
                if let Some(msg) = map.get(key).and_then(format_json_error_value) {
                    return msg;
                }
            }
        }
        return truncate_snippet(&json_val.to_string(), MAX_ERROR_SNIPPET_CHARS);
    }

    let trimmed = body.trim();
    if trimmed.is_empty() {
        status
            .canonical_reason()
            .unwrap_or("empty response body")
            .to_string()
    } else {
        truncate_snippet(trimmed, MAX_ERROR_SNIPPET_CHARS)
    }
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

    #[test]
    fn extract_faucet_error_ignores_null_error_and_informational_message() {
        let with_null_and_msg = json!({
            "transactionHash": "0xabc123",
            "error": null,
            "message": "queued"
        });
        assert_eq!(extract_faucet_error(&with_null_and_msg), None);

        let with_msg_only = json!({
            "transaction_hash": "0xabc123",
            "message": "Dispatched"
        });
        assert_eq!(extract_faucet_error(&with_msg_only), None);
    }

    #[test]
    fn extract_faucet_error_extracts_structured_and_string_errors() {
        let structured = json!({
            "error": {
                "code": "invalid_address",
                "message": "Address not allowed"
            }
        });
        assert_eq!(
            extract_faucet_error(&structured).as_deref(),
            Some("Address not allowed")
        );

        let string_err = json!({"error": "Daily cap reached"});
        assert_eq!(
            extract_faucet_error(&string_err).as_deref(),
            Some("Daily cap reached")
        );

        let object_without_message = json!({"error": {"code": 5001}});
        assert_eq!(
            extract_faucet_error(&object_without_message).as_deref(),
            Some(r#"{"code":5001}"#)
        );

        let numeric_err = json!({"error": 42});
        assert_eq!(extract_faucet_error(&numeric_err).as_deref(), Some("42"));

        let blank_err = json!({"error": "   "});
        assert_eq!(
            extract_faucet_error(&blank_err).as_deref(),
            Some("Unknown faucet error")
        );
    }

    #[test]
    fn extract_http_error_detail_handles_structured_json_and_non_json() {
        let structured = r#"{"error":{"code":"limit","message":"Daily cap reached"}}"#;
        assert_eq!(
            extract_http_error_detail(structured, reqwest::StatusCode::BAD_REQUEST),
            "Daily cap reached"
        );

        let null_error_with_message = r#"{"error":null,"message":"Service unavailable"}"#;
        assert_eq!(
            extract_http_error_detail(
                null_error_with_message,
                reqwest::StatusCode::INTERNAL_SERVER_ERROR
            ),
            "Service unavailable"
        );

        let detail_only = r#"{"code":"bad_request","detail":"invalid address"}"#;
        assert_eq!(
            extract_http_error_detail(detail_only, reqwest::StatusCode::BAD_REQUEST),
            "invalid address"
        );

        let compact_fallback = r#"{"code":"bad_request"}"#;
        assert_eq!(
            extract_http_error_detail(compact_fallback, reqwest::StatusCode::BAD_REQUEST),
            r#"{"code":"bad_request"}"#
        );

        let html_502 = "<html><body><h1>502 Bad Gateway</h1></body></html>";
        assert_eq!(
            extract_http_error_detail(html_502, reqwest::StatusCode::BAD_GATEWAY),
            html_502
        );

        assert_eq!(
            extract_http_error_detail("   ", reqwest::StatusCode::BAD_GATEWAY),
            "Bad Gateway"
        );

        let overlong_utf8 = "é".repeat(MAX_ERROR_SNIPPET_CHARS + 20);
        let truncated = extract_http_error_detail(&overlong_utf8, reqwest::StatusCode::BAD_GATEWAY);
        assert_eq!(
            truncated,
            format!("{}...", "é".repeat(MAX_ERROR_SNIPPET_CHARS))
        );
    }
}
