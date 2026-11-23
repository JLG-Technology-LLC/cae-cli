# Bug Assessment: pr-4-ipv6-loopback-url-check-broken

- **Slug**: pr-4-ipv6-loopback-url-check-broken
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: `4660960540`)
- **Verdict**: valid
- **Status**: Verified
- **Severity**: medium
- **PR Thread**: PR #4 (Comment ID: `4660960540`)
- **GitHub Issue**: #15
- **Related Spec**: None

## Symptom & Reproduction

`is_loopback_url` in both `src/wallet/balance.rs` and `src/wallet/faucet.rs` splits the URL authority on `['/', ':']`, which splits IPv6 literals on their internal colons and produces `host = "["`, making the `"[::1]"` match arm completely unreachable. It also fails on uppercase `LOCALHOST`, URLs containing userinfo (`user:pass@localhost`), and non-`127.0.0.1` loopback addresses (`127.0.0.0/8`, expanded `[0:0:0:0:0:0:0:1]`).

**Observed**: `is_loopback_url("http://[::1]:8545")`, `is_loopback_url("http://LOCALHOST:8545")`, and `is_loopback_url("http://user:pass@127.0.0.1:8545")` all return `false`, so `reqwest::blocking::Client::builder()` does not call `.no_proxy()`, routing local RPC/faucet traffic through corporate/system HTTP proxies and breaking local dev/test nodes.
**Expected**: `is_loopback_url` parses the URL structurally with `reqwest::Url::parse` and returns `true` for all loopback hosts (`localhost` case-insensitively, any IPv4 in `127.0.0.0/8`, and IPv6 `::1`), sharing a single helper across `balance.rs` and `faucet.rs`.

**Reproduction**:
1. Inspect `src/wallet/balance.rs:133-137` and the duplicated copy at `src/wallet/faucet.rs:113-117`:
   ```rust
   let rest = url.split("://").nth(1).unwrap_or(url);
   let host = rest.split(['/', ':']).next().unwrap_or("");
   matches!(host, "localhost" | "127.0.0.1" | "[::1]")
   ```
2. Trace `"http://[::1]:8545"`: `rest = "[::1]:8545"`, `host = "["` → `false`.
3. Trace `"http://LOCALHOST:8545"`: `host = "LOCALHOST"` → `false`.
4. Trace `"http://user:pass@127.0.0.1:8545"`: `host = "user"` → `false`.

## Root Cause Analysis

- **Suspected Files**:
  - `src/wallet/balance.rs:133-137` (`is_loopback_url`), `src/wallet/balance.rs:143` (caller), `src/wallet/balance.rs:221-226` (tests)
  - `src/wallet/faucet.rs:113-117` (duplicated `is_loopback_url`), `src/wallet/faucet.rs:44` (caller)
  - `src/wallet/mod.rs` (ideal home for shared `pub(crate) fn is_loopback_url`)
- **Cause**: Hand-rolled string splitting on `['/', ':']` does not understand RFC 3986 URL grammar (bracketed IPv6 literals, userinfo `@`, case-insensitive DNS names, or IPv4/IPv6 loopback normalization). The defective helper was duplicated in both `balance.rs` and `faucet.rs`.
- **Adjacent Risks**:
  - **Dependency check (`Cargo.toml:24`)**: `reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls"] }` (resolved to `0.12.28` in `Cargo.lock`) unconditionally re-exports `reqwest::Url` (`pub use url::Url;`) with the existing features — no `Cargo.toml` edit is required.
  - **Important compile caveat**: `reqwest` does **not** re-export `url::Host`, and `url` is not a direct dependency in `Cargo.toml`. Matching `parsed.host()` against `url::Host::{Domain, Ipv4, Ipv6}` directly would fail to compile unless `url` is added to `Cargo.toml`. Instead, inspecting `parsed.host_str()` with zero new dependencies handles every case cleanly: `reqwest::Url::parse` strips userinfo/port/path/query, lowercases domains (`LOCALHOST` → `localhost`), normalizes IPv4 shorthand (`127.1` → `127.0.0.1`), and canonicalizes IPv6 in brackets (`[0::1]` → `[::1]`). Stripping `[` / `]` and parsing as `std::net::IpAddr` enables `ip.is_loopback()` (covering all of `127.0.0.0/8` and `::1`), plus `host.trim_end_matches('.').eq_ignore_ascii_case("localhost")`.
  - Overlap with `pr-4-rpc-null-error-and-url-secret-leak`: both bugs modify `src/wallet/mod.rs`, `src/wallet/balance.rs`, and `src/wallet/faucet.rs`, so Stage 2 must schedule them in separate waves.
  - Confidence: **high**.

## Proposed Remediation Plan

**Files likely to change**:
- `src/wallet/mod.rs`
- `src/wallet/balance.rs`
- `src/wallet/faucet.rs`

**Steps**:
1. Add a shared `pub(crate) fn is_loopback_url(url: &str) -> bool` helper in `src/wallet/mod.rs` implemented with `reqwest::Url::parse(url)` and `parsed.host_str()`:
   - Return `true` if `host.trim_end_matches('.').eq_ignore_ascii_case("localhost")`.
   - Strip enclosing `[` and `]` (for IPv6) and parse as `std::net::IpAddr`; return `ip.is_loopback()`.
   - Return `false` on parse failure or non-loopback hosts.
2. Remove the duplicated private `is_loopback_url` functions from `src/wallet/balance.rs:133-137` and `src/wallet/faucet.rs:113-117`, and call `super::is_loopback_url(&url)` in both modules.
3. Expand unit tests (in `src/wallet/mod.rs` or `src/wallet/balance.rs`) to cover `http://[::1]:8545`, `http://[0:0:0:0:0:0:0:1]:8545/rpc`, `http://LOCALHOST:8545`, `http://localhost.:8545`, `http://user:pass@127.0.0.1:8545`, `http://127.0.0.2:8545`, `https://sepolia.base.org`, `https://127.0.0.1.example.com`, and invalid URLs. Run `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --all -- --check`.
