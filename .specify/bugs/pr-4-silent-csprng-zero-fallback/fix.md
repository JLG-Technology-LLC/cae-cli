# Bug Fix: Propagate CSPRNG errors instead of returning all-zero session tokens

- **Slug**: pr-4-silent-csprng-zero-fallback
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **Status**: Verified
- **Commit SHA**: `de85f5b`
- **GitHub Issue**: #13

## Summary

Changed `crypto_random_bytes()` in `src/wallet/auth.rs` to return `Result<[u8; 16]>` and propagate `getrandom::getrandom` errors with explicit context (`"Secure random number generator unavailable: {}"`) instead of swallowing errors with `.unwrap_or_default()`. Updated both callers (`handle_loopback_stream` and `run_terminal_otp`) to propagate failures with `?` so authentication fails closed rather than minting a predictable all-zero `session_ref`.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `src/wallet/auth.rs` | modified | Imported `anyhow::anyhow`; changed `crypto_random_bytes()` to return `Result<[u8; 16]>` via testable helper `crypto_random_bytes_with<F, E>` using `.map_err(...)`; propagated `?` in `handle_loopback_stream` and `run_terminal_otp`. |
| `src/wallet/auth.rs` | added tests | Added two unit tests covering CSPRNG non-zero distinct entropy and fail-closed error propagation. |

## Diff Highlights

```rust
fn crypto_random_bytes() -> Result<[u8; 16]> {
    crypto_random_bytes_with(getrandom::getrandom)
}

fn crypto_random_bytes_with<F, E>(fill: F) -> Result<[u8; 16]>
where
    F: FnOnce(&mut [u8]) -> std::result::Result<(), E>,
    E: std::fmt::Display,
{
    let mut b = [0u8; 16];
    fill(&mut b).map_err(|e| anyhow!("Secure random number generator unavailable: {}", e))?;
    Ok(b)
}
```

## Tests Added or Updated

- `src/wallet/auth.rs::tests::crypto_random_bytes_returns_non_zero_distinct_entropy` — verifies `crypto_random_bytes()` returns `Ok` with non-zero bytes and produces distinct entropy across consecutive calls.
- `src/wallet/auth.rs::tests::crypto_random_bytes_propagates_csprng_error` — verifies an injected CSPRNG failure returns `Err` containing `"Secure random number generator unavailable"` and the underlying error text rather than falling back to `[0u8; 16]`.

## Local Verification

- Commands run: Deferred to coordinator post-wave verification (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`) due to concurrent Mode B edits in `src/wallet/`.
- Manual checks: Verified all callers of `crypto_random_bytes` in `src/wallet/auth.rs` (`handle_loopback_stream` and `run_terminal_otp`) propagate `Result` via `?`, matched `.map_err(|e| anyhow!("Secure random number generator unavailable: {}", e))?` to `src/wallet/crypto.rs:20` (compatible with `getrandom = "0.2"` without `std` feature), and confirmed no unrelated code in `src/wallet/auth.rs` was touched.

## Deviations from Assessment

None. Implemented the assessment's proposed remediation including the optional `crypto_random_bytes_with<F, E>` helper for deterministic failure-path testing.

## Follow-ups

- Run coordinator wave verification (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`) and proceed to `/speckit-bug-test slug=pr-4-silent-csprng-zero-fallback`.
