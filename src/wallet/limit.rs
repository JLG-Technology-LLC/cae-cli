use anyhow::{bail, Result};
use std::io::IsTerminal;

use super::profile::CandidateWalletProfile;
use super::WalletError;

pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Reset the daily spend tracker when `today` differs from the recorded spend date.
/// Returns `true` when a rollover happened.
pub fn rollover(profile: &mut CandidateWalletProfile, today: &str) -> bool {
    if profile.last_spend_date == today {
        return false;
    }
    profile.spent_today_usdc = 0.0;
    profile.last_spend_date = today.to_string();
    true
}

pub fn remaining(profile: &CandidateWalletProfile) -> f64 {
    (profile.daily_limit_usdc - profile.spent_today_usdc).max(0.0)
}

pub fn parse_limit(raw: &str) -> Result<f64> {
    let amount: f64 = raw
        .trim_start_matches('$')
        .parse()
        .map_err(|_| anyhow::anyhow!("'{}' is not a valid USDC amount", raw))?;
    if !amount.is_finite() || amount < 0.0 {
        bail!("Daily limit must be a non-negative number, got '{}'", raw);
    }
    Ok(amount)
}

#[allow(dead_code)] // consumed by the tailoring payment flow
#[derive(Debug, PartialEq)]
pub enum SpendDecision {
    Allowed,
    NeedsConfirmation { overage: f64 },
}

#[allow(dead_code)] // consumed by the tailoring payment flow
pub fn evaluate_spend(profile: &CandidateWalletProfile, proposed: f64) -> SpendDecision {
    let total = profile.spent_today_usdc + proposed;
    // Compare in micro-USDC so 0.12 + 0.03 vs 0.15 is not tripped by float error.
    if to_micro(total) > to_micro(profile.daily_limit_usdc) {
        SpendDecision::NeedsConfirmation {
            overage: total - profile.daily_limit_usdc,
        }
    } else {
        SpendDecision::Allowed
    }
}

fn to_micro(amount: f64) -> i64 {
    (amount * 1_000_000.0).round() as i64
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
    rollover(profile, &today());
    if let SpendDecision::NeedsConfirmation { overage } = evaluate_spend(profile, proposed) {
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
    profile.spent_today_usdc += proposed;
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
    fn remaining_never_negative() {
        let mut p = sample();
        p.spent_today_usdc = 0.03;
        assert!((remaining(&p) - 0.12).abs() < 1e-9);
        p.spent_today_usdc = 0.5;
        assert_eq!(remaining(&p), 0.0);
    }

    #[test]
    fn spend_exactly_at_limit_is_allowed() {
        let mut p = sample();
        p.spent_today_usdc = 0.12;
        assert_eq!(evaluate_spend(&p, 0.03), SpendDecision::Allowed);
        assert!(matches!(
            evaluate_spend(&p, 0.04),
            SpendDecision::NeedsConfirmation { .. }
        ));
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
    fn within_budget_records_spend() {
        let mut p = sample();
        authorize_spend(&mut p, 0.03, true).unwrap();
        assert!((p.spent_today_usdc - 0.03).abs() < 1e-9);
        assert_eq!(p.last_spend_date, today());
    }

    #[test]
    fn parses_limits() {
        assert_eq!(parse_limit("0.30").unwrap(), 0.30);
        assert_eq!(parse_limit("$1").unwrap(), 1.0);
        assert!(parse_limit("-1").is_err());
        assert!(parse_limit("abc").is_err());
        assert!(parse_limit("NaN").is_err());
    }
}
