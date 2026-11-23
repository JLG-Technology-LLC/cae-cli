# Bug Fix: Align README JSON Schemas and Headless CI FAQ with Implementation

- **Slug**: pr-4-readme-schema-and-faq-mismatches
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **GitHub Issue**: #23
- **Status**: Verified
- **Commit SHA**: `24946f2`

## Summary

Updated `README.md` so all documented JSON payloads and headless CI authentication guidance match the actual Rust structs and runtime control flow verbatim: aligned the `cae wallet balance --json` sample with `BalanceSummary::to_json()`, aligned the `~/.cae/projects/<id>.json` ledger sample with `ProjectLedger`, `GitStatusInfo`, `ManifestInfo`, and `ContextFile`, and clarified the FAQ distinction between `--no-browser` / non-TTY / `SSH_CONNECTION` headless terminal fallback and `CAE_SKIP_BROWSER_OPEN=1`.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `README.md` | modified | Updated `cae wallet balance --json` sample output (lines 344–371) to emit `"wallet_address"`, numeric floats (`eth_balance`, `usdc_balance`, `cost_per_run_usdc`), nested `"balances"` (`eth` and `usdc` with `raw` and `formatted`), and nested `"tailoring"` (`cost_per_run_usdc`, `available_runs`, `daily_limit_usdc`, `spent_today_usdc`, `daily_remaining_usdc`) matching `BalanceSummary::to_json()` (`src/wallet/balance.rs:55-81`). |
| `README.md` | modified | Updated `1. Project Ledger (~/.cae/projects/<id>.json)` (lines 448–521) to match `ProjectLedger` (`tags`, `notes`), `GitStatusInfo` (`head_commit_message`, `head_author`, `head_date`, `remote_origin_url`, `status_summary`), `ManifestInfo` (`file_name`, `relative_path`, `dependencies_count`, `dependencies`, `metadata`), and `ContextFile` (`context_type`, `relative_path`, `file_size_bytes`, `word_count`, `last_modified`, `summary`, `preview`, and `#`-stripped `headings`). |
| `README.md` | modified | Updated the headless CI FAQ (lines 660–663) to clarify that `--no-browser`, non-TTY `stdin`, or `SSH_CONNECTION` (unless `CAE_FORCE_BROWSER=1`) select the headless terminal OTP flow (and `cae wallet init --local` runs completely offline), whereas `CAE_SKIP_BROWSER_OPEN=1` only suppresses OS browser launch while keeping the loopback HTTP callback listener active for tests. |

## Diff Highlights

```diff
--- a/README.md
+++ b/README.md
@@ -344,12 +344,29 @@ cae wallet balance --json
 **Sample JSON Output:**
 ```json
 {
-  "address": "0x4B20993Bc481177ec7E8f571ceCaE8A9e22C02db",
+  "wallet_address": "0x4B20993Bc481177ec7E8f571ceCaE8A9e22C02db",
   "network": "base-sepolia",
-  "eth_balance": "0.015000000000000000",
-  "usdc_balance": "1.500000",
+  "eth_balance": 0.015,
+  "usdc_balance": 1.5,
   "tailoring_runs_available": 50,
-  "cost_per_run_usdc": "0.030000"
+  "cost_per_run_usdc": 0.03,
+  "balances": {
+    "eth": {
+      "raw": "15000000000000000",
+      "formatted": "0.0150 ETH"
+    },
+    "usdc": {
+      "raw": "1500000",
+      "formatted": "1.50 USDC"
+    }
+  },
+  "tailoring": {
+    "cost_per_run_usdc": 0.03,
+    "available_runs": 50,
+    "daily_limit_usdc": 0.15,
+    "spent_today_usdc": 0.03,
+    "daily_remaining_usdc": 0.12
+  }
 }
 ```
```

```diff
--- a/README.md
+++ b/README.md
@@ -610,7 +658,9 @@ docker run --rm \
 ### Q: How do I run tests headlessly in CI without browser prompts?
-**A:** Set the environment variable `CAE_SKIP_BROWSER_OPEN=1`. This instructs `cae` to fall back to headless terminal flows.
+**A:** Different mechanisms control the headless terminal flow vs. suppressing browser launches during loopback auth tests:
+* **Headless terminal flow (`--no-browser` / non-TTY / SSH):** Pass `--no-browser` to `cae wallet init` or `cae wallet login` to select the in-terminal email OTP flow. `cae` also automatically selects this headless terminal flow whenever `stdin` is not a TTY or `SSH_CONNECTION` is set (unless `CAE_FORCE_BROWSER=1` is set). For completely offline CI or local development workflows with no browser or OTP prompt, use `cae wallet init --local`.
+* **Suppressing OS browser launch in loopback tests (`CAE_SKIP_BROWSER_OPEN=1`):** Setting `CAE_SKIP_BROWSER_OPEN=1` does **not** switch to the terminal OTP flow; instead, it suppresses launching the OS browser window (`open` / `xdg-open` / `start`) while keeping the local loopback HTTP callback listener active. Automated loopback auth tests pair `CAE_SKIP_BROWSER_OPEN=1` with `CAE_FORCE_BROWSER=1` so the loopback server runs headlessly without opening a browser tab.
```

## Tests Added or Updated

- Documentation-only bug; no Rust source or test files were modified.

## Local Verification

- **Cross-reference against source code**: Verified every field name, order, type, and runtime branch against `src/wallet/balance.rs` (`BalanceSummary::to_json()`), `src/ledger.rs` (`ProjectLedger`), `src/git.rs` (`GitStatusInfo`), `src/manifest.rs` (`ManifestInfo`, `DependencyInfo`, `parse_pubspec_str`), `src/context.rs` (`ContextFile`, `ContextType`, `extract_headings`), and `src/wallet/auth.rs` (`authenticate_user`, `open_browser`).
- **JSON & schema validation**:
  - Command: `python3 -c '...'` extracting all 3 `json` code blocks from `README.md`, parsing each with `json.loads`, asserting exact key lists and value types for `BalanceSummary::to_json()`, `ProjectLedger` (including `GitStatusInfo`, `ManifestInfo`, `DependencyInfo`, `ContextFile`, and `#`-free `headings`), and `CandidateWalletProfile`, plus verifying `#9` (Semgrep `--severity=WARNING --severity=ERROR`) and `#22` (mainnet CDP auth & `wallet.local.bak` security docs) remained intact.
  - Result: Exit `0` — `ALL 3 JSON BLOCKS VALIDATED AND #9/#22 PRESERVATION VERIFIED!`.
- **Diff scope check**:
  - Command: `git -C /Users/jlgtechnology/Developer/cae/cae-cli diff -- README.md`
  - Result: Exit `0` — confirmed only the 3 target `#23` hunks in `README.md` were modified; `#9` and `#22` fixes were preserved untouched.

## Deviations from Assessment

None. All steps in `assessment.md` were applied verbatim.

## Follow-ups

- Stage 3 verification (`/speckit-bug-test slug=pr-4-readme-schema-and-faq-mismatches`) should deserialize the `README.md` Project Ledger JSON block through `serde_json::from_str::<ProjectLedger>` and verify `BalanceSummary::to_json()` key/type parity and FAQ accuracy.
