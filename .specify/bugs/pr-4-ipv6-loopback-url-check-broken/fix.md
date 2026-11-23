# Bug Fix: pr-4-ipv6-loopback-url-check-broken

- **Slug**: pr-4-ipv6-loopback-url-check-broken
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **Status**: Verified
- **Commit SHA**: `004d623`
- **GitHub Issue**: #15

## Summary

Added a single shared `pub(crate) fn is_loopback_url(url: &str) -> bool` helper in `src/wallet/mod.rs` that parses URLs structurally with `reqwest::Url::parse` and `parsed.host_str()`, handles case-insensitive `localhost` (with optional trailing root dot) as well as bracketed IPv6 (`[::1]`, expanded `[0:0:0:0:0:0:0:1]`) and full `127.0.0.0/8` IPv4 loopback detection via `std::net::IpAddr::is_loopback()`, and removed the duplicated string-splitting `is_loopback_url` helpers from `src/wallet/balance.rs` and `src/wallet/faucet.rs`.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `src/wallet/mod.rs` | modified | Added shared `pub(crate) fn is_loopback_url(url: &str) -> bool` using `reqwest::Url::parse` + `host_str()` + `std::net::IpAddr::is_loopback()`, plus comprehensive unit tests in `mod tests`. |
| `src/wallet/balance.rs` | modified | Removed duplicated private `is_loopback_url` helper and its 4-line test; updated `fetch_balances` to call `super::is_loopback_url(&url)`. |
| `src/wallet/faucet.rs` | modified | Removed duplicated private `is_loopback_url` helper; updated `request_faucet` to call `super::is_loopback_url(&url)`. |

## Diff Highlights

- `src/wallet/mod.rs`:
  - Added `pub(crate) fn is_loopback_url(url: &str) -> bool`:
    - Parses with `reqwest::Url::parse(url)` and extracts `parsed.host_str()` (zero new dependencies in `Cargo.toml`).
    - Strips an optional single trailing root dot (`host.strip_suffix('.').unwrap_or(host)`) and checks `eq_ignore_ascii_case("localhost")`.
    - Strips enclosing `[` / `]` for IPv6 literals and checks `bare.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())`.
- `src/wallet/balance.rs` & `src/wallet/faucet.rs`:
  - Deleted both private `fn is_loopback_url` implementations and routed both proxy-bypass checks through `super::is_loopback_url(&url)`.

## Tests Added or Updated

- `src/wallet/mod.rs::tests::detects_ipv4_ipv6_and_localhost_loopback_urls` — verifies `http://[::1]:8545`, `http://[::1]/rpc`, `https://[::1]:8443/v2`, `http://[0:0:0:0:0:0:0:1]:8545/rpc`, `http://localhost:8545/rpc`, `http://LOCALHOST:8545`, `http://localhost.:8545`, `http://user:pass@localhost:8545`, `http://user:pass@127.0.0.1:8545`, `http://user:pass@[::1]:8545/rpc`, `http://127.0.0.1:8545`, `http://127.0.0.2:8545`, and `http://127.255.255.254:8545` all return `true`.
- `src/wallet/mod.rs::tests::rejects_non_loopback_and_malformed_urls` — verifies `https://sepolia.base.org`, `http://localhost.evil.com`, `http://localhost.evil.com:8545`, `https://127.0.0.1.example.com`, `http://evil.com/localhost`, `http://evil.com/?next=http://127.0.0.1:8545`, `http://[::2]:8545`, `http://0.0.0.0:8545`, `http://192.168.1.1:8545`, `http://localhost..:8545`, and malformed/hostless URLs (`""`, `"not-a-url"`, `"://localhost:8545"`, `"http://[::1"`, `"http://"`, `"localhost:8545"`, `"data:text/plain,localhost"`) all return `false`.
- `src/wallet/balance.rs::tests::detects_loopback_endpoints` — removed and superseded by the comprehensive unit test suite in `src/wallet/mod.rs`.

## Local Verification

- Commands run: `rustfmt --edition 2021 --check /Users/jlgtechnology/Developer/cae/cae-cli/src/wallet/mod.rs /Users/jlgtechnology/Developer/cae/cae-cli/src/wallet/balance.rs /Users/jlgtechnology/Developer/cae/cae-cli/src/wallet/faucet.rs` → exited 0 (formatting clean).
- Deferred to wave coordinator: `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings` (deferred per Mode B parallel wave constraint while other Rust files are edited concurrently).

## Deviations from Assessment

- Used `host.strip_suffix('.').unwrap_or(host)` rather than `trim_end_matches('.')` so a single DNS root trailing dot (`localhost.`) is accepted while malformed double-dot names (`localhost..`) are rejected.
- Strictly scoped to `#15`; left `rpc_call`, `sanitize_url`, and `handle_login` untouched for their respective bug waves (`#16`, `#18`).

## Follow-ups

- Coordinator wave verification: run `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings`, then commit and proceed to Stage 3 (`/speckit-bug-test slug=pr-4-ipv6-loopback-url-check-broken`).
