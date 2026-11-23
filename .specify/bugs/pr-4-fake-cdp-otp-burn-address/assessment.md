# Bug Assessment: Terminal OTP and fallback auth fabricate CDP verification and produce unspendable burn addresses

- **Slug**: pr-4-fake-cdp-otp-burn-address
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: 4660959904)
- **GitHub Issue**: #7
- **Verdict**: valid
- **Status**: Verified
- **Severity**: high

## Report (verbatim or summarized)

From PR #4 review comment (`https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660959904`, commit `f82e11d94db3dacd2ae0db526d2d973e0877af09`):

> `run_terminal_otp` claims to dispatch and verify a 6-digit CDP OTP but makes zero network/CDP API calls: it accepts any 6 ASCII digits, generates a random local `cdp_sess_<hex>` string, and computes `evm_address` via `derive_address_for_email(&email)` = `Keccak256(email)[12..]` (also used as fallback in `handle_loopback_stream:198`). A raw Keccak-256 hash of an email is not derived from any secp256k1 public key or Coinbase enclave wallet—no private key exists for it, so any deposited ETH or USDC is permanently burned.

## Symptom

When a user runs `cae wallet init` or `cae wallet login` in headless mode, with `--no-browser`, or when browser loopback fails, `run_terminal_otp` pretends to email and verify a 6-digit CDP code without contacting any server, accepts any arbitrary 6 digits, and persists a `cdp-embedded` profile whose `evm_address` is `Keccak256(email)[12..]`. The same unspendable hash-derived address is also synthesized as a fallback in `handle_loopback_stream`. Because no secp256k1 private key or Coinbase enclave wallet corresponds to `Keccak256(email)[12..]`, any ETH or USDC deposited to the displayed wallet address is permanently lost.

## Reproduction

1. Run `printf "user@example.com\n000000\n" | cae wallet init --no-browser --json` (even with network completely disabled).
2. Observe that the CLI prints `Dispatching 6-digit verification code...` and `Verifying code...`, exits `0`, and writes `~/.cae/wallet.json` with `wallet_type: "cdp-embedded"` and `evm_address` equal to the EIP-55 checksum of `Keccak256("user@example.com")[12..]`.
3. Observe that no CDP enclave wallet exists for that address and `cae wallet export-key` refuses export because it claims keys are held in Coinbase's secure enclave.

## Suspected Code Paths

- `src/wallet/auth.rs:245-288` (`run_terminal_otp`) — prompts for email and 6 digits, performs zero HTTP/CDP requests, accepts any 6 ASCII digits, generates a fake local `cdp_sess_<hex>`, and assigns `evm_address = derive_address_for_email(&email)`.
- `src/wallet/auth.rs:196-199` (`handle_loopback_stream`) — falls back to `derive_address_for_email(&email)` whenever the callback `address` parameter is absent or invalid.
- `src/wallet/auth.rs:296-300` (`derive_address_for_email`) — hashes `email.as_bytes()` with `Keccak256` and formats the last 20 bytes as an EIP-55 EVM address with no backing private key.
- `src/wallet/auth.rs:322-327` (`tests::derives_deterministic_valid_checksummed_address`) — unit test pinning the burn-address derivation helper.
- `tests/wallet_tests.rs:710-732` (`cdp_init_terminal_otp`) and `tests/e2e_all_commands.rs:675-695` — integration tests exercising `--no-browser` terminal OTP against the fake offline stub.

## Root Cause Hypothesis

`run_terminal_otp` and `derive_address_for_email` were written as an offline mock stub in production code to satisfy `--no-browser` tests before real Coinbase CDP end-user OTP endpoints were integrated. Treating the hash of an email string as a valid EVM address violates both cryptographic fundamentals (no corresponding private key exists) and `SPEC-005` SC-003 (100% of addresses must match the real CDP wallet). **Confidence: high** (directly verified in `src/wallet/auth.rs:196-199,245-300`).

## Proposed Remediation

**Preferred**:
1. **Delete `derive_address_for_email` completely** from `src/wallet/auth.rs` (including its fallback call in `handle_loopback_stream` and its unit test) so production code can never mint an unspendable hash-derived address under any code path.
2. **Wire `run_terminal_otp` to a real HTTP verification contract with fail-closed default** (following the exact same env-override + `reqwest::blocking::Client` pattern already used in `src/wallet/faucet.rs` via `CAE_CDP_FAUCET_URL` and `src/wallet/balance.rs` via `CAE_RPC_URL_*`, requiring zero new crates):
   - Read the CDP auth base URL from `CAE_CDP_AUTH_URL` (trimmed, non-empty).
   - If `CAE_CDP_AUTH_URL` is unset or empty (default until production CDP end-user OTP service is configured), **fail closed immediately** before printing "Dispatching..." or prompting for an OTP, returning an actionable error explaining that terminal CDP OTP authentication requires a configured CDP auth endpoint (`CAE_CDP_AUTH_URL`) and directing the user to run `cae wallet init --local` for a cryptographically backed local EOA wallet.
   - When `CAE_CDP_AUTH_URL` is set, validate the email input, build a `reqwest::blocking::Client` with timeout (e.g., 15s) and `.no_proxy()` for loopback URLs (`localhost` / `127.0.0.1` / `[::1]`):
     - `POST {base_url}/otp/initiate` with `{"email": email}`; require a 2xx response (`error_for_status()`) and extract optional `flow_id`.
     - Prompt for the 6-digit numeric OTP and validate format (`len == 6` and ASCII digits).
     - `POST {base_url}/otp/verify` with `{"email": email, "otp": trimmed_otp, "flow_id": flow_id}`; require a 2xx response and parse JSON.
     - Strictly validate the server JSON response: require a valid 42-char `evm_address` passing `crate::wallet::crypto::is_valid_address`, a non-empty `session_ref`, and a valid `email` (falling back only to the verified request email if omitted by server). Never fabricate `evm_address` or `session_ref` locally.
3. **Hermetic mock-server integration tests** (matching `spawn_mock_rpc` and `spawn_mock_faucet` in `tests/wallet_tests.rs` and `tests/e2e_all_commands.rs`):
   - Add a `spawn_mock_cdp_auth` helper binding `127.0.0.1:0` that serves `POST /otp/initiate` and `POST /otp/verify` (verifying the expected OTP and returning a real checksummed EVM address + `session_ref`).
   - Update `cdp_init_terminal_otp` in `tests/wallet_tests.rs` and section 14A in `tests/e2e_all_commands.rs` to pass `CAE_CDP_AUTH_URL` pointing to the mock server and assert the returned `evm_address` and `session_ref` match the server response.
   - Add negative tests verifying that without `CAE_CDP_AUTH_URL`, `cae wallet init --no-browser` fails closed with guidance to use `cae wallet init --local` and writes NO `wallet.json`, and that an invalid OTP (`401`) or missing/invalid `evm_address` from the server also fails closed without writing `wallet.json`.

**Alternatives**:
- Unconditionally return an "unimplemented" error from `run_terminal_otp` with no HTTP client support until Coinbase's production CDP endpoint URL is finalized. *Trade-off*: Simpler code, but leaves the `--no-browser` OTP workflow completely untestable end-to-end and requires removing/ignoring existing OTP integration tests (`cdp_init_terminal_otp` and E2E step 14A). Using `CAE_CDP_AUTH_URL` with fail-closed default gives identical production safety while keeping the OTP contract cleanly verified in CI.

**Files likely to change**:
- `src/wallet/auth.rs`
- `tests/wallet_tests.rs`
- `tests/e2e_all_commands.rs`

**Tests to add or update**:
- Remove `tests::derives_deterministic_valid_checksummed_address` in `src/wallet/auth.rs`.
- Add unit/integration tests verifying:
  - `cae wallet init --no-browser` with no `CAE_CDP_AUTH_URL` fails closed (non-zero exit), stderr mentions `cae wallet init --local`, and `~/.cae/wallet.json` is not created.
  - `cae wallet init --no-browser` with `CAE_CDP_AUTH_URL` pointing to a mock CDP auth server succeeds only when OTP verification returns HTTP 200 with a valid `evm_address` and `session_ref`, and persists those exact values.
  - Mock server returning HTTP 401 (bad OTP) or HTTP 200 with malformed/missing `evm_address` fails closed and creates no `wallet.json`.
- Update `cdp_init_terminal_otp` (`tests/wallet_tests.rs`) and section 14A (`tests/e2e_all_commands.rs`) to use the hermetic mock CDP auth server via `CAE_CDP_AUTH_URL`.

## Risks & Considerations

- **Coordination with `pr-4-loopback-auth-csrf-hijack`**: Both bugs modify `src/wallet/auth.rs`, `tests/wallet_tests.rs`, and `tests/e2e_all_commands.rs`. Ensure `handle_loopback_stream` no longer references `derive_address_for_email` in either branch.
- **No fake local fallback**: Ensure `authenticate_user` does NOT silently mask loopback errors by creating a fake wallet; when loopback fails and falls back to `run_terminal_otp`, `run_terminal_otp` must likewise fail closed if `CAE_CDP_AUTH_URL` is unset.
- **Zero new dependencies**: Uses `reqwest::blocking` and `serde_json` already in `Cargo.toml`.

## Open Questions

- None — fail-closed default with explicit `CAE_CDP_AUTH_URL` endpoint contract cleanly resolves the fund-loss vulnerability while keeping all CI tests hermetic and green.
