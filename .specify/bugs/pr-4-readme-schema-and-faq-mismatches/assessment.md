# Bug Assessment: pr-4-readme-schema-and-faq-mismatches

- **Slug**: pr-4-readme-schema-and-faq-mismatches
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: `4660961325`)
- **Verdict**: valid
- **Status**: Verified
- **Severity**: medium
- **PR Thread**: PR #4 (Comment ID: `4660961325`)
- **GitHub Issue**: #23
- **Related Spec**: None

## Symptom & Reproduction

Three documented JSON contracts and CLI behavior descriptions in `README.md` disagree with the actual Rust structs and runtime code in `cae-cli`, causing consumers (scripts, AI agents, and `AICareerAssetEngine` integrators) following the README to parse wrong field names/types or configure CI headlessness incorrectly.

**Observed vs. Expected**:
1. **`cae wallet balance --json` (`README.md:343-352` vs `src/wallet/balance.rs:55-81`)**:
   - README shows `"address"`, string floats (`"0.015000000000000000"`, `"1.500000"`, `"0.030000"`), and omits nested objects.
   - Actual `BalanceSummary::to_json()` emits `"wallet_address"`, numeric JSON floats (`eth_balance`, `usdc_balance`, `cost_per_run_usdc`), a nested `"balances"` object (`eth.{raw,formatted}`, `usdc.{raw,formatted}`), and a nested `"tailoring"` object (`cost_per_run_usdc`, `available_runs`, `daily_limit_usdc`, `spent_today_usdc`, `daily_remaining_usdc`).
2. **Project Ledger JSON (`README.md:431-471` vs `src/ledger.rs:11-24`, `src/git.rs:21-36`, `src/manifest.rs:15-25`, `src/context.rs:30-41`)**:
   - `ProjectLedger`: README omits `"tags"` and `"notes"`.
   - `GitStatusInfo`: README omits `"head_commit_message"`, `"head_author"`, `"head_date"`, `"remote_origin_url"`, and `"status_summary"`.
   - `ManifestInfo`: README uses nonexistent `"path"` instead of `"file_name"` and `"relative_path"`, and omits `"dependencies_count"`, `"dependencies"`, and `"metadata"`.
   - `ContextFile`: README uses nonexistent `"category"`, `"path"`, and `"byte_size"` instead of `"context_type"`, `"relative_path"`, and `"file_size_bytes"`; omits `"word_count"`, `"last_modified"`, `"summary"`, and `"preview"`; and shows `"headings"` with leading `#` markers even though `extract_headings` (`src/context.rs:198-211`) strips `#` prefixes.
3. **Headless CI FAQ (`README.md:599-600` vs `src/wallet/auth.rs:25-44`, `115-135`)**:
   - README claims `CAE_SKIP_BROWSER_OPEN=1` "instructs `cae` to fall back to headless terminal flows."
   - Actual code: `authenticate_user` (`src/wallet/auth.rs:30-44`) selects terminal OTP fallback when `--no-browser` is passed OR when `!CAE_FORCE_BROWSER && (!stdin.is_terminal() || SSH_CONNECTION is set)`. `CAE_SKIP_BROWSER_OPEN=1` is checked only in `open_browser` (`src/wallet/auth.rs:116-121`) and merely suppresses spawning the OS browser while staying in the loopback HTTP auth flow (primarily used in tests with `CAE_FORCE_BROWSER=1`).

**Reproduction**:
1. Compare `README.md:343-352` with `BalanceSummary::to_json()` in `src/wallet/balance.rs:55-81` (or run `cae wallet balance --json` against a mock RPC).
2. Compare `README.md:431-471` with `ProjectLedger` (`src/ledger.rs:11-24`), `GitStatusInfo` (`src/git.rs:21-36`), `ManifestInfo` (`src/manifest.rs:15-25`), and `ContextFile` (`src/context.rs:30-41`) (or run `cae status <id> --json`).
3. Compare `README.md:599-600` with `authenticate_user` (`src/wallet/auth.rs:25-44`) and `open_browser` (`src/wallet/auth.rs:115-135`).

## Root Cause Analysis

- **Suspected Files**:
  - `README.md:342-352` (`cae wallet balance` sample JSON)
  - `README.md:429-471` (`1. Project Ledger (~/.cae/projects/<id>.json)`)
  - `README.md:599-600` (FAQ on headless CI / `CAE_SKIP_BROWSER_OPEN`)
  - Code authorities: `src/wallet/balance.rs:55-81`, `src/ledger.rs:11-24`, `src/git.rs:21-36`, `src/manifest.rs:7-25`, `src/context.rs:8-41`, `src/context.rs:198-211`, `src/wallet/auth.rs:25-44`, `src/wallet/auth.rs:115-135`
- **Cause**: The README JSON examples and FAQ were hand-authored from memory rather than derived directly from the serialized Rust structs and `auth.rs` control flow, introducing field name/type drift (`address` vs `wallet_address`, string vs numeric balances, `path`/`category`/`byte_size` vs `relative_path`/`context_type`/`file_size_bytes`) and misdescribing `CAE_SKIP_BROWSER_OPEN=1` as triggering headless terminal fallback.
- **Adjacent Risks**:
  - Violates the project's strict 1-to-1 canonical JSON contract expectations between `cae-cli`, coding agents, and `AICareerAssetEngine` (`SPEC-031` ingestion / Constitution Principle I & V).
  - Agents parsing `cae wallet balance --json` or `~/.cae/projects/<id>.json` using the README as a contract will fail at runtime with missing keys or type errors.
  - Confidence: **high**.

## Proposed Remediation Plan

**Files likely to change**:
- `README.md`

**Steps**:
1. Update the `cae wallet balance --json` example (`README.md:342-352`) to match `BalanceSummary::to_json()` (`src/wallet/balance.rs:55-81`) verbatim: `"wallet_address"`, numeric floats for `eth_balance`, `usdc_balance`, and `cost_per_run_usdc`, plus the nested `"balances"` (`eth`, `usdc` with `raw` and `formatted`) and `"tailoring"` objects.
2. Update the Project Ledger JSON example (`README.md:429-471`) to match `ProjectLedger`, `GitStatusInfo`, `ManifestInfo`, and `ContextFile` verbatim:
   - Add `"tags"` and `"notes"` to `ProjectLedger`.
   - Include `head_commit_message`, `head_author`, `head_date`, `remote_origin_url`, and `status_summary` in `git`.
   - Replace `"path"` with `"file_name"` and `"relative_path"`, and include `"dependencies_count"`, `"dependencies"`, and `"metadata"` in `manifests`.
   - Replace `"category"`, `"path"`, and `"byte_size"` with `"context_type"`, `"relative_path"`, and `"file_size_bytes"`, strip `#` prefixes from `"headings"`, and include `"word_count"`, `"last_modified"`, `"summary"`, and `"preview"` in `context_files`.
3. Update the FAQ entry (`README.md:599-600`) to accurately distinguish:
   - `--no-browser` or non-TTY / `SSH_CONNECTION` environments automatically select the headless terminal OTP flow (and `cae wallet init --local` runs completely offline with no browser or OTP).
   - `CAE_SKIP_BROWSER_OPEN=1` suppresses launching the OS browser window while keeping the local loopback callback listener active (used for automated loopback tests together with `CAE_FORCE_BROWSER=1`).
4. Verify the updated README JSON blocks can be parsed by `serde_json` and deserialize cleanly into `ProjectLedger` (and match `BalanceSummary::to_json()` key-for-key).
