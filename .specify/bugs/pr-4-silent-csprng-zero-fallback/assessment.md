# Bug Assessment: pr-4-silent-csprng-zero-fallback

- **Slug**: pr-4-silent-csprng-zero-fallback
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: `4660960349`)
- **Verdict**: valid
- **Status**: Assessed (Awaiting Fix)
- **Severity**: medium
- **PR Thread**: PR #4 (Comment ID: `4660960349`)
- **GitHub Issue**: #13
- **Related Spec**: None

## Symptom & Reproduction

`crypto_random_bytes` in `src/wallet/auth.rs` swallows OS CSPRNG errors with `.unwrap_or_default()` and returns `[0u8; 16]`, silently minting a predictable all-zero session token (`cdp_sess_00000000000000000000000000000000`) instead of failing closed.

**Observed**: On any `getrandom::getrandom` failure, `crypto_random_bytes()` ignores the `Err` and returns 16 zero bytes, which callers hex-encode into `session_ref`.
**Expected**: `crypto_random_bytes()` returns `Result<[u8; 16]>` and propagates OS CSPRNG errors with explicit context so authentication fails closed (matching `src/wallet/crypto.rs:19-20`).

**Reproduction**:
1. Inspect `src/wallet/auth.rs:290-294` (`crypto_random_bytes`) and its callers at `src/wallet/auth.rs:190` (`handle_loopback_stream`) and `src/wallet/auth.rs:280` (`run_terminal_otp`).
2. When `getrandom::getrandom(&mut b)` returns `Err(_)`, `.unwrap_or_default()` discards the error and returns `[0u8; 16]`.
3. Both callers format `cdp_sess_00000000000000000000000000000000` and persist it as `session_ref` without reporting an error.

## Root Cause Analysis

- **Suspected Files**:
  - `src/wallet/auth.rs:290-294` (`crypto_random_bytes`)
  - `src/wallet/auth.rs:186-190` (`handle_loopback_stream` caller)
  - `src/wallet/auth.rs:280` (`run_terminal_otp` caller)
  - Reference implementation: `src/wallet/crypto.rs:19-20` (`generate_local_key`)
- **Cause**: `crypto_random_bytes()` was implemented with an infallible signature (`fn crypto_random_bytes() -> [u8; 16]`) using `getrandom::getrandom(&mut b).unwrap_or_default()`. Both callers (`handle_loopback_stream` and `run_terminal_otp`) already return `anyhow::Result<_>`, so swallowing the RNG error is completely unnecessary and violates Constitution Principle V (explicit `anyhow` error propagation) and fail-closed security practice.
- **Adjacent Risks**:
  - `Cargo.toml:27` declares `getrandom = "0.2"` without the `std` feature, so `getrandom::Error` does not guaranteed-implement `std::error::Error` without transitive feature unification. Mapping the error via `.map_err(|e| anyhow::anyhow!("Secure random number generator unavailable: {}", e))?` (matching `src/wallet/crypto.rs:19-20`) avoids brittle reliance on transitive `getrandom/std`.
  - In `run_loopback_auth` (`src/wallet/auth.rs:95-101`), `Err` from `handle_loopback_stream` logs a warning and refuses to emit an `AuthResult`, so propagating `Err` from `handle_loopback_stream` guarantees no all-zero token is ever returned or persisted.
  - Confidence: **high**.

## Proposed Remediation Plan

**Files likely to change**:
- `src/wallet/auth.rs`

**Steps**:
1. Change `crypto_random_bytes()` in `src/wallet/auth.rs:290-294` to return `Result<[u8; 16]>` and propagate `getrandom::getrandom(&mut b)` errors via `.map_err(|e| anyhow::anyhow!("Secure random number generator unavailable: {}", e))?` (optionally factored through a tiny testable helper `crypto_random_bytes_with<F>` so failure propagation can be unit-tested deterministically).
2. Update both callers in `src/wallet/auth.rs`:
   - `handle_loopback_stream` (lines 186-190): replace `.unwrap_or_else(...)` with `match` / `if let Some(...)` and propagate `crypto_random_bytes()?`.
   - `run_terminal_otp` (line 280): propagate `crypto_random_bytes()?`.
3. Add hermetic unit tests in `src/wallet/auth.rs` verifying that `crypto_random_bytes()` succeeds with non-zero distinct output across calls and that an injected CSPRNG failure returns an `Err` rather than `[0u8; 16]`, then run `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --all -- --check`.
