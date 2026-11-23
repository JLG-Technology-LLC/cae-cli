# Bug Assessment: pr-4-spend-limit-saturation-and-nan-bypass

- **Slug**: pr-4-spend-limit-saturation-and-nan-bypass
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: `4660960437`)
- **Verdict**: valid
- **Status**: Assessed (Awaiting Fix)
- **Severity**: medium
- **PR Thread**: PR #4 (Comment ID: `4660960437`)
- **GitHub Issue**: #14
- **Related Spec**: None

## Symptom & Reproduction

Numeric conversion saturation, unvalidated `proposed` spend amounts, unbounded `parse_limit` ceilings, and non-monotonic local-time date rollovers in `src/wallet/limit.rs` allow bypassing or permanently poisoning the daily USDC spend limit.

**Observed**:
1. `to_micro(f64::NAN)` evaluates `(NaN * 1_000_000.0).round() as i64 == 0`, so `evaluate_spend(&profile, f64::NAN)` returns `SpendDecision::Allowed` and `authorize_spend` sets `profile.spent_today_usdc = NaN`, permanently disabling subsequent spend checks (and producing invalid `null` JSON if serialized).
2. `evaluate_spend` / `authorize_spend` accept negative `proposed` values (e.g., `-10.0`), decreasing `spent_today_usdc` and bypassing the daily allowance.
3. `parse_limit` accepts arbitrarily large finite floats (e.g., `1e15`, `1e300`) that saturate `to_micro` to `i64::MAX`, causing `to_micro(total) > to_micro(daily_limit)` (`i64::MAX > i64::MAX`) to return `false` (`Allowed`) even when `total` exceeds `daily_limit_usdc`.
4. `today()` uses `chrono::Local::now()` and `rollover()` checks `profile.last_spend_date == today`, so moving west across timezones or setting the system clock backwards resets `spent_today_usdc` to `0.0`.

**Expected**:
- Non-finite (`NaN`, `±Inf`) and negative spend inputs fail closed with an error.
- `parse_limit` enforces a safe finite upper bound well below integer micro-USDC overflow / `f64` precision loss, and micro-USDC arithmetic uses checked `u128`/`i128` operations without lossy saturating `as i64` casts.
- Daily rollover uses UTC dates (`chrono::Utc::now()`) consistently and only resets when `today` is strictly after `profile.last_spend_date`.

**Reproduction**:
1. Inspect `src/wallet/limit.rs:7-88` (`today`, `rollover`, `parse_limit`, `evaluate_spend`, `to_micro`, `authorize_spend`).
2. Call `authorize_spend(&mut p, f64::NAN, true)` → returns `Ok(())` and leaves `p.spent_today_usdc.is_nan() == true`.
3. Call `authorize_spend(&mut p, -5.0, true)` → returns `Ok(())` and makes `p.spent_today_usdc` negative.
4. Call `parse_limit("1e20")` → returns `Ok(1e20)`; `to_micro(1e20 + 1e19) > to_micro(1e20)` evaluates `i64::MAX > i64::MAX` → `false` (`Allowed`).
5. Call `rollover(&mut p, "2026-11-10")` on a profile with `last_spend_date = "2026-11-11"` and `spent_today_usdc = 0.15` → returns `true` and zeroes `spent_today_usdc`.

## Root Cause Analysis

- **Suspected Files**:
  - `src/wallet/limit.rs:7-88` (`today`, `rollover`, `remaining`, `parse_limit`, `evaluate_spend`, `to_micro`, `authorize_spend`)
  - `tests/wallet_tests.rs:254-256` (`export_key_is_refused_for_cdp_wallets` asserts `stored["last_spend_date"] == chrono::Local::now()...`)
- **Cause**:
  - Rust's `as i64` float-to-integer cast is saturating (`NaN -> 0`, `>= 2^63 -> i64::MAX`, `<= -2^63 -> i64::MIN`) rather than fallible. Using `to_micro` without validating finiteness, sign, or bounds silently turns invalid or overflowing values into `0` or `i64::MAX`.
  - `evaluate_spend` and `authorize_spend` do not validate `proposed` (nor defensive checks on `profile.spent_today_usdc` / `profile.daily_limit_usdc` loaded from disk).
  - `parse_limit` checks `!amount.is_finite() || amount < 0.0` but imposes no upper bound.
  - `today()` uses `chrono::Local::now()` (inconsistent with `Utc::now()` used for `created_at` in `src/wallet/mod.rs:249,277,347`), and `rollover` uses `!=` instead of monotonic date ordering (`today > last_spend_date`).
- **Adjacent Risks**:
  - Switching `today()` from `chrono::Local::now()` to `chrono::Utc::now()` will cause `tests/wallet_tests.rs:254-256` to fail whenever local date differs from UTC date unless that test assertion is updated to `chrono::Utc::now()`.
  - `evaluate_spend` is currently `#[allow(dead_code)]` with callers only inside `src/wallet/limit.rs` (line 72 and unit tests at lines 120-122), so changing its signature to `Result<SpendDecision>` breaks no external callers and ensures invalid inputs fail closed everywhere.
  - `remaining(profile)` (`src/wallet/limit.rs:22-24`) should also guard against non-finite values (`if !rem.is_finite() { 0.0 } else { rem.max(0.0) }`) so corrupted profiles cannot emit `NaN` in JSON output.
  - Malformed `last_spend_date` values in `rollover`: parsing both dates with `chrono::NaiveDate::parse_from_str(..., "%Y-%m-%d")` ensures rollover only happens when both dates parse validly and `today_date > last_date` (avoiding lexicographical surprises on malformed strings).
  - Confidence: **high**.

## Proposed Remediation Plan

**Files likely to change**:
- `src/wallet/limit.rs`
- `tests/wallet_tests.rs`

**Steps**:
1. Define a safe ceiling constant in `src/wallet/limit.rs` (e.g., `pub const MAX_DAILY_LIMIT_USDC: f64 = 1_000_000_000.0;`, well within exact `f64` micro-USDC precision `2^53 / 10^6 ≈ 9.007e9` and `u128` range) and enforce `amount <= MAX_DAILY_LIMIT_USDC` in `parse_limit`.
2. Replace `fn to_micro(amount: f64) -> i64` with a fallible `fn to_micro(amount: f64) -> Result<u128>` that rejects non-finite, negative, or out-of-range (`> MAX_DAILY_LIMIT_USDC`) amounts before converting to `u128` micro-USDC.
3. Update `evaluate_spend` to return `Result<SpendDecision>`, validating `proposed`, `profile.spent_today_usdc`, and `profile.daily_limit_usdc` via `to_micro` and using `u128::checked_add` for `spent_micro + proposed_micro`; fail closed on invalid amounts or overflow. Update `authorize_spend` to propagate `evaluate_spend(profile, proposed)?` and avoid mutating `profile.spent_today_usdc` on any error. Guard `remaining()` against non-finite results.
4. Change `today()` to use `chrono::Utc::now().format("%Y-%m-%d").to_string()` and update `rollover()` to parse `YYYY-MM-DD` via `chrono::NaiveDate::parse_from_str` and reset only when `today_date > last_date` (returning `false` on equal, earlier, or unparseable dates).
5. Update `tests/wallet_tests.rs:255` from `chrono::Local::now()` to `chrono::Utc::now()`, and add unit tests in `src/wallet/limit.rs` covering `NaN`, `±Infinity`, negative spends, huge/saturating limits and spends, and backward-clock / same-day / forward-day rollovers. Run `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --all -- --check`.
