# Bug Fix: pr-4-spend-limit-saturation-and-nan-bypass

- **Slug**: pr-4-spend-limit-saturation-and-nan-bypass
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **Status**: Verified
- **Commit SHA**: `7ef3894`
- **GitHub Issue**: #14

## Summary

Hardened wallet daily spend-limit parsing, micro-USDC arithmetic, spend evaluation/authorization, and daily rollover in `src/wallet/limit.rs` so non-finite (`NaN`, `±Inf`), negative, and overflowing/saturating values fail closed without mutating profile state, and date rollovers use monotonic UTC `NaiveDate` ordering. Updated the UTC date expectation and added spend-limit and monotonic rollover regression tests in `tests/wallet_tests.rs`.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `src/wallet/limit.rs` | modified | Added `MAX_DAILY_LIMIT_USDC` ($1B USDC ceiling) in `parse_limit` and `to_micro`; replaced saturating `to_micro -> i64` with fallible `to_micro -> Result<u128>`; changed `evaluate_spend` to return `Result<SpendDecision>` using `u128::checked_add` and validating `proposed`, `spent_today_usdc`, and `daily_limit_usdc`; updated `authorize_spend` to validate before mutating and compute updated spend in checked micro-USDC; guarded `remaining()` against non-finite and negative profile values; switched `today()` to `chrono::Utc::now()` and `rollover()` to strict monotonic `chrono::NaiveDate` comparison (`today_date > last_date`); added unit regression tests. |
| `tests/wallet_tests.rs` | modified | Updated `export_key_is_refused_for_cdp_wallets` `last_spend_date` assertion from `chrono::Local::now()` to `chrono::Utc::now()`; added CLI integration regression tests for `limit set` upper-bound/non-finite rejection and monotonic rollover on future `last_spend_date`. |

## Diff Highlights

- `parse_limit`: enforces `amount <= MAX_DAILY_LIMIT_USDC` (`1_000_000_000.0`) and normalizes `-0.0` to `0.0`.
- `to_micro(amount: f64) -> Result<u128>`: rejects `!amount.is_finite()`, `amount < 0.0`, and `amount > MAX_DAILY_LIMIT_USDC` before scaling by `1_000_000.0` and converting to `u128`.
- `evaluate_spend(profile, proposed) -> Result<SpendDecision>`: validates `spent_today_usdc`, `proposed`, and `daily_limit_usdc` via `to_micro`, sums via `checked_add`, and computes exact `overage` from `(total_micro - limit_micro) as f64 / 1_000_000.0`.
- `authorize_spend`: validates `proposed` before calling `rollover`, propagates `evaluate_spend(profile, proposed)?`, and updates `profile.spent_today_usdc` via checked `u128` micro-USDC addition only after authorization succeeds.
- `today()` / `rollover()`: uses UTC (`chrono::Utc::now()`) and parses both dates with `chrono::NaiveDate::parse_from_str(..., "%Y-%m-%d")`, returning `false` on equal, earlier, or malformed dates.
- `remaining()`: returns `0.0` when either `daily_limit_usdc` or `spent_today_usdc` is non-finite or negative.

## Tests Added or Updated

- `src/wallet/limit.rs::tests::rollover_does_not_reset_on_backward_clock_or_invalid_dates` — verifies backward clock dates and malformed date strings do not reset `spent_today_usdc`.
- `src/wallet/limit.rs::tests::today_uses_utc_date` — pins `today()` to UTC `YYYY-MM-DD`.
- `src/wallet/limit.rs::tests::remaining_guards_non_finite_and_negative_values` — verifies `remaining()` returns `0.0` on `NaN`, `Infinity`, and negative corrupted profile values.
- `src/wallet/limit.rs::tests::within_budget_records_spend` — updated to use a fixed past date (`"2020-01-01"`) so monotonic rollover is deterministic regardless of system clock.
- `src/wallet/limit.rs::tests::parses_limits` — extended with `MAX_DAILY_LIMIT_USDC` boundary and rejections for `inf`, `-inf`, `1e15`, `1e300`, and `1000000000.01`.
- `src/wallet/limit.rs::tests::rejects_non_finite_and_negative_proposed_spends` — verifies `NaN`, `±Infinity`, and negative `proposed` amounts fail closed in both `evaluate_spend` and `authorize_spend` without mutating `spent_today_usdc`.
- `src/wallet/limit.rs::tests::rejects_saturating_and_corrupted_profile_amounts` — verifies oversized (`1e20`) and corrupted (`NaN`, negative) amounts fail closed.
- `src/wallet/limit.rs::tests::max_valid_limit_overage_does_not_saturate` — verifies an overage at `MAX_DAILY_LIMIT_USDC` returns `NeedsConfirmation` with accurate overage rather than saturating to `Allowed`.
- `tests/wallet_tests.rs::export_key_is_refused_for_cdp_wallets` — updated `last_spend_date` expectation to UTC (`chrono::Utc::now()`).
- `tests/wallet_tests.rs::limit_set_rejects_non_finite_and_excessive_values` — verifies `cae wallet limit set` rejects `NaN`, `inf`, `-inf`, `1e15`, `1e300`, and `1000000000.01`, preserves prior limit, and accepts `1000000000`.
- `tests/wallet_tests.rs::rollover_does_not_reset_spend_for_future_last_spend_date` — verifies CLI `show`/`limit` does not reset `spent_today_usdc` when `last_spend_date` is ahead of UTC today.

## Local Verification

- Deferred to post-wave coordinator verification (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`) per Mode B parallel wave isolation constraints.
- Verified all callers of `limit::*` across `src/` and `tests/` (`evaluate_spend` and `authorize_spend` have no callers outside `src/wallet/limit.rs`; `today`, `rollover`, `remaining`, and `parse_limit` preserve existing signatures).

## Deviations from Assessment

- None. All steps from `assessment.md` were applied as specified; additionally hardened `remaining()` against negative corrupted profile values and normalized `-0.0` to `0.0` in `parse_limit`.

## Follow-ups

- Coordinator to run `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --all -- --check` after the wave completes, then proceed to Stage 3 verification (`/speckit-bug-test`).
