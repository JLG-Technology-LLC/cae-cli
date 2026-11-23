---
slug: pr-4-readme-schema-and-faq-mismatches
pr_number: 4
comment_id: 4660961325
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660961325"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCgLQ"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: README.md
line: 352
start_line: 343
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: README balance JSON, project ledger JSON, and CAE_SKIP_BROWSER_OPEN FAQ mismatch actual CLI behavior

- **Slug**: pr-4-readme-schema-and-faq-mismatches
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660961325
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660961325
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `README.md:343-352`

## Finding

Three documented schemas/behaviors in `README.md` diverge from the Rust implementation: (1) `wallet balance --json` example (lines 343-352) uses `"address"` and string floats instead of `BalanceSummary::to_json()` (`"wallet_address"`, numeric floats, nested `balances`/`tailoring` objects); (2) project ledger schema (lines 450-470) uses nonexistent field names (`path`, `category`, `byte_size`) instead of `relative_path`, `context_type`, `file_size_bytes`, plus omits `tags`/`notes`; (3) FAQ (lines 599-600) claims `CAE_SKIP_BROWSER_OPEN=1` selects headless terminal fallback when it actually only suppresses browser launch inside loopback auth (`--no-browser` or non-TTY triggers terminal fallback).

- **Category**: `correctness`
- **Rule**: project / *.md — Documentation (BLOCKING)

## Diff Context

```diff
+{
+  "address": "0x4B20993Bc481177ec7E8f571ceCaE8A9e22C02db",
+  "network": "base-sepolia",
+  "eth_balance": "0.015000000000000000",
+  "usdc_balance": "1.500000",
+  "tailoring_runs_available": 50,
+  "cost_per_run_usdc": "0.030000"
+}
```

## Proposed Direction (Not Applied)

Align the `wallet balance --json` example with `BalanceSummary::to_json()`, align the project ledger JSON with `ProjectLedger`/`ManifestInfo`/`ContextFile`, and clarify the FAQ distinction between `CAE_SKIP_BROWSER_OPEN=1` and `--no-browser`.

## Next Step

Run `/speckit-bug-assess slug=pr-4-readme-schema-and-faq-mismatches` using this intake file.
