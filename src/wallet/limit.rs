use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use std::io::IsTerminal;

use super::profile::CandidateWalletProfile;
use super::WalletError;

/// Maximum supported daily USDC spend limit ($1,000,000,000.00 USDC = 10^15 micro-USDC),
/// safely within exact `f64` integer precision (2^53) and `u128` arithmetic.
pub const MAX_DAILY_LIMIT_USDC: f64 = 1_000_000_000.0;

const MICRO_PER_USDC: f64 = 1_000_000.0;

pub fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Reset the daily spend tracker only when `today` is strictly after the recorded UTC spend date.
/// Returns `true` when a rollover happened.
pub fn rollover(profile: &mut CandidateWalletProfile, today: &str) -> bool {
    let Ok(today_date) = NaiveDate::parse_from_str(today, "%Y-%m-%d") else {
        return false;
    };
    let Ok(last_date) = NaiveDate::parse_from_str(&profile.last_spend_date, "%Y-%m-%d") else {
        return false;
    };
    if today_date <= last_date {
        return false;
    }
    profile.spent_today_usdc = 0.0;
    profile.last_spend_date = today.to_string();
    true
}

pub fn remaining(profile: &CandidateWalletProfile) -> f64 {
    if !profile.daily_limit_usdc.is_finite()
        || !profile.spent_today_usdc.is_finite()
        || profile.daily_limit_usdc < 0.0
        || profile.spent_today_usdc < 0.0
    {
        return 0.0;
    }
    let rem = profile.daily_limit_usdc - profile.spent_today_usdc;
    if !rem.is_finite() {
        0.0
    } else {
        rem.max(0.0)
    }
}

pub fn parse_limit(raw: &str) -> Result<f64> {
    let amount: f64 = raw
        .trim_start_matches('$')
        .parse()
        .map_err(|_| anyhow::anyhow!("'{}' is not a valid USDC amount", raw))?;
    if !amount.is_finite() || amount < 0.0 {
        bail!("Daily limit must be a non-negative number, got '{}'", raw);
    }
    if amount > MAX_DAILY_LIMIT_USDC {
        bail!(
            "Daily limit must not exceed ${:.2} USDC, got '{}'",
            MAX_DAILY_LIMIT_USDC,
            raw
        );
    }
    Ok(if amount == 0.0 { 0.0 } else { amount })
}

#[allow(dead_code)] // consumed by the tailoring payment flow
#[derive(Debug, PartialEq)]
pub enum SpendDecision {
    Allowed,
    NeedsConfirmation { overage: f64 },
}

#[allow(dead_code)] // consumed by the tailoring payment flow
pub fn evaluate_spend(profile: &CandidateWalletProfile, proposed: f64) -> Result<SpendDecision> {
    let spent_micro = to_micro(profile.spent_today_usdc)
        .context("Invalid recorded spent_today_usdc in wallet profile")?;
    let proposed_micro = to_micro(proposed).context("Invalid proposed spend amount")?;
    let limit_micro =
        to_micro(profile.daily_limit_usdc).context("Invalid daily_limit_usdc in wallet profile")?;

    // Compare in micro-USDC so 0.12 + 0.03 vs 0.15 is not tripped by float error.
    let total_micro = spent_micro
        .checked_add(proposed_micro)
        .ok_or_else(|| anyhow::anyhow!("Daily spend total overflowed micro-USDC arithmetic"))?;

    if total_micro > limit_micro {
        let overage_micro = total_micro - limit_micro;
        Ok(SpendDecision::NeedsConfirmation {
            overage: overage_micro as f64 / MICRO_PER_USDC,
        })
    } else {
        Ok(SpendDecision::Allowed)
    }
}

fn to_micro(amount: f64) -> Result<u128> {
    if !amount.is_finite() || amount < 0.0 {
        bail!(
            "USDC amount must be a finite non-negative number, got {}",
            amount
        );
    }
    if amount > MAX_DAILY_LIMIT_USDC {
        bail!(
            "USDC amount {} exceeds maximum supported limit (${:.2} USDC)",
            amount,
            MAX_DAILY_LIMIT_USDC
        );
    }
    let scaled = (amount * MICRO_PER_USDC).round();
    if !scaled.is_finite() || scaled < 0.0 {
        bail!("USDC amount {} cannot be represented in micro-USDC", amount);
    }
    Ok(scaled as u128)
}

/// Authorize a spend against the daily allowance.
///
/// Interactive terminals are prompted `[y/N]`; non-interactive (or `--json`) sessions
/// fail closed with a [`WalletError::SpendLimitExceeded`] (exit code 2).
#[allow(dead_code)] // consumed by the tailoring payment flow
pub fn authorize_spend(
    profile: &mut CandidateWalletProfile,
    proposed: f64,
    json: bool,
) -> Result<()> {
    let proposed_micro = to_micro(proposed).context("Invalid proposed spend amount")?;
    rollover(profile, &today());
    if let SpendDecision::NeedsConfirmation { overage } = evaluate_spend(profile, proposed)? {
        let interactive = !json && std::io::stdin().is_terminal();
        let approved = interactive
            && super::confirm(&format!(
                "This ${:.2} USDC payment exceeds your daily allowance (${:.2}) by ${:.2}. Proceed?",
                proposed, profile.daily_limit_usdc, overage
            ))?;
        if !approved {
            return Err(WalletError::SpendLimitExceeded(
                "Raise the limit with `cae wallet limit set <amount>` and retry.".to_string(),
            )
            .into());
        }
    }
    let spent_micro = to_micro(profile.spent_today_usdc)
        .context("Invalid recorded spent_today_usdc in wallet profile")?;
    let updated_micro = spent_micro
        .checked_add(proposed_micro)
        .ok_or_else(|| anyhow::anyhow!("Daily spend total overflowed micro-USDC arithmetic"))?;
    profile.spent_today_usdc = updated_micro as f64 / MICRO_PER_USDC;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::profile::tests::sample;

    #[test]
    fn rolls_over_on_new_day_only() {
        let mut p = sample();
        p.spent_today_usdc = 0.09;
        assert!(!rollover(&mut p, "2026-11-11"));
        assert_eq!(p.spent_today_usdc, 0.09);

        assert!(rollover(&mut p, "2026-11-12"));
        assert_eq!(p.spent_today_usdc, 0.0);
        assert_eq!(p.last_spend_date, "2026-11-12");
    }

    #[test]
    fn rollover_does_not_reset_on_backward_clock_or_invalid_dates() {
        let mut p = sample();
        p.spent_today_usdc = 0.09;
        assert!(!rollover(&mut p, "2026-11-10"));
        assert_eq!(p.spent_today_usdc, 0.09);
        assert_eq!(p.last_spend_date, "2026-11-11");

        assert!(!rollover(&mut p, "not-a-date"));
        assert_eq!(p.spent_today_usdc, 0.09);
        assert_eq!(p.last_spend_date, "2026-11-11");

        p.last_spend_date = "corrupted-date".to_string();
        assert!(!rollover(&mut p, "2026-11-12"));
        assert_eq!(p.spent_today_usdc, 0.09);
    }

    #[test]
    fn today_uses_utc_date() {
        assert_eq!(today(), chrono::Utc::now().format("%Y-%m-%d").to_string());
    }

    #[test]
    fn remaining_never_negative() {
        let mut p = sample();
        p.spent_today_usdc = 0.03;
        assert!((remaining(&p) - 0.12).abs() < 1e-9);
        p.spent_today_usdc = 0.5;
        assert_eq!(remaining(&p), 0.0);
    }

    #[test]
    fn remaining_guards_non_finite_and_negative_values() {
        let mut p = sample();
        p.spent_today_usdc = f64::NAN;
        assert_eq!(remaining(&p), 0.0);

        p.spent_today_usdc = f64::INFINITY;
        assert_eq!(remaining(&p), 0.0);

        p.spent_today_usdc = -1.0;
        assert_eq!(remaining(&p), 0.0);

        p.spent_today_usdc = 0.0;
        p.daily_limit_usdc = f64::NAN;
        assert_eq!(remaining(&p), 0.0);

        p.daily_limit_usdc = f64::INFINITY;
        assert_eq!(remaining(&p), 0.0);

        p.daily_limit_usdc = -0.5;
        assert_eq!(remaining(&p), 0.0);
    }

    #[test]
    fn spend_exactly_at_limit_is_allowed() -> Result<()> {
        let mut p = sample();
        p.spent_today_usdc = 0.12;
        assert_eq!(evaluate_spend(&p, 0.03)?, SpendDecision::Allowed);
        assert!(matches!(
            evaluate_spend(&p, 0.04)?,
            SpendDecision::NeedsConfirmation { .. }
        ));
        Ok(())
    }

    #[test]
    fn non_interactive_over_budget_fails_closed() {
        let mut p = sample();
        p.last_spend_date = today();
        p.spent_today_usdc = 0.15;
        let err = authorize_spend(&mut p, 0.03, true).unwrap_err();
        assert!(matches!(
            err.downcast_ref::<WalletError>(),
            Some(WalletError::SpendLimitExceeded(_))
        ));
        assert_eq!(p.spent_today_usdc, 0.15);
    }

    #[test]
    fn within_budget_records_spend() -> Result<()> {
        let mut p = sample();
        p.last_spend_date = "2020-01-01".to_string();
        authorize_spend(&mut p, 0.03, true)?;
        assert!((p.spent_today_usdc - 0.03).abs() < 1e-9);
        assert_eq!(p.last_spend_date, today());
        Ok(())
    }

    #[test]
    fn parses_limits() -> Result<()> {
        assert_eq!(parse_limit("0.30")?, 0.30);
        assert_eq!(parse_limit("$1")?, 1.0);
        assert_eq!(parse_limit("0")?, 0.0);
        assert_eq!(parse_limit("1000000000")?, MAX_DAILY_LIMIT_USDC);
        assert!(parse_limit("-1").is_err());
        assert!(parse_limit("abc").is_err());
        assert!(parse_limit("NaN").is_err());
        assert!(parse_limit("inf").is_err());
        assert!(parse_limit("-inf").is_err());
        assert!(parse_limit("1e15").is_err());
        assert!(parse_limit("1e300").is_err());
        assert!(parse_limit("1000000000.01").is_err());
        Ok(())
    }

    #[test]
    fn rejects_non_finite_and_negative_proposed_spends() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, -5.0] {
            let mut p = sample();
            p.last_spend_date = today();
            p.spent_today_usdc = 0.03;

            assert!(evaluate_spend(&p, bad).is_err(), "expected err for {bad}");
            assert!(
                authorize_spend(&mut p, bad, true).is_err(),
                "expected err for {bad}"
            );
            assert_eq!(p.spent_today_usdc, 0.03);
            assert!(p.spent_today_usdc.is_finite());
        }
    }

    #[test]
    fn rejects_saturating_and_corrupted_profile_amounts() {
        let mut p = sample();
        p.last_spend_date = today();
        assert!(evaluate_spend(&p, 1e20).is_err());
        assert!(authorize_spend(&mut p, 1e20, true).is_err());
        assert_eq!(p.spent_today_usdc, 0.0);

        p.daily_limit_usdc = 1e20;
        assert!(evaluate_spend(&p, 1e19).is_err());
        assert!(authorize_spend(&mut p, 1.0, true).is_err());

        p.daily_limit_usdc = 0.15;
        p.spent_today_usdc = f64::NAN;
        assert!(evaluate_spend(&p, 0.03).is_err());
        assert!(authorize_spend(&mut p, 0.03, true).is_err());

        p.spent_today_usdc = -1.0;
        assert!(evaluate_spend(&p, 0.03).is_err());
        assert!(authorize_spend(&mut p, 0.03, true).is_err());
    }

    #[test]
    fn max_valid_limit_overage_does_not_saturate() -> Result<()> {
        let mut p = sample();
        p.daily_limit_usdc = MAX_DAILY_LIMIT_USDC;
        p.spent_today_usdc = MAX_DAILY_LIMIT_USDC;
        match evaluate_spend(&p, 1.0)? {
            SpendDecision::NeedsConfirmation { overage } => {
                assert!((overage - 1.0).abs() < 1e-6, "unexpected overage {overage}");
            }
            SpendDecision::Allowed => panic!("expected overage confirmation at max limit"),
        }
        Ok(())
    }
}
