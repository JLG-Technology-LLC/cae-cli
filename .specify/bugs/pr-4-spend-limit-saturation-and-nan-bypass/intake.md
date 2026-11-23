---
slug: pr-4-spend-limit-saturation-and-nan-bypass
pr_number: 4
comment_id: 4660960437
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960437"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCctQ"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: src/wallet/limit.rs
line: 88
start_line: 45
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: to_micro saturating cast and unvalidated spend inputs allow bypassing daily spend limits

- **Slug**: pr-4-spend-limit-saturation-and-nan-bypass
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960437
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960437
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `src/wallet/limit.rs:45-88`

## Finding

`to_micro` uses `(amount * 1_000_000.0).round() as i64`, which saturates `NaN` to `0` and values `>= 9.22e12` to `i64::MAX`. Neither `evaluate_spend` nor `authorize_spend` rejects `NaN`/negative `proposed` spends (so `NaN` poisons `spent_today_usdc` permanently and negative amounts reduce spent total), and `parse_limit` accepts `f64` values large enough to saturate `i64::MAX`. Also `rollover` uses `Local::now()` with `!=` so westward timezone or backward clock changes reset daily spend.

- **Category**: `correctness`
- **Rule**: Security-Sensitive Code — numeric conversions and overflow

## Diff Context

```diff
+pub fn evaluate_spend(profile: &CandidateWalletProfile, proposed: f64) -> SpendDecision {
+    let total = profile.spent_today_usdc + proposed;
+    if to_micro(total) > to_micro(profile.daily_limit_usdc) {
...
+fn to_micro(amount: f64) -> i64 {
+    (amount * 1_000_000.0).round() as i64
+}
```

## Proposed Direction (Not Applied)

Reject non-finite or negative `proposed` amounts (fail closed), bound `parse_limit` to a safe max micro-USDC ceiling and use checked `u128`/`i128` micro-USDC arithmetic, and only roll over when `today > profile.last_spend_date` using UTC consistently.

## Next Step

Run `/speckit-bug-assess slug=pr-4-spend-limit-saturation-and-nan-bypass` using this intake file.
