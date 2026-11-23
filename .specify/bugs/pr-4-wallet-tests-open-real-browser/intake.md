---
slug: pr-4-wallet-tests-open-real-browser
pr_number: 4
comment_id: 4660960941
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960941"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCerQ"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: tests/wallet_tests.rs
line: 633
start_line: 626
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: Loopback auth integration tests launch real system browser windows

- **Slug**: pr-4-wallet-tests-open-real-browser
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960941
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960941
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `tests/wallet_tests.rs:626-633`

## Finding

`cdp_init_loopback_captures_auth_and_persists_embedded_profile` (lines 626-633) and `cdp_login_backs_up_active_local_wallet` (lines 680-687) set `CAE_FORCE_BROWSER=1` without `CAE_SKIP_BROWSER_OPEN=1`, causing `cargo test` to launch real browser tabs on developer machines and race against the loopback test listener.

- **Category**: `correctness`
- **Rule**: project / tests/**/*.rs — Concurrency and Shared State / Test Standards

## Diff Context

```diff
+    let out = Command::new(env!("CARGO_BIN_EXE_cae"))
+        .args(["wallet", "init", "--json"])
+        .env("CAE_HOME", &sb.home)
+        .env("CAE_AUTH_PORT", port.to_string())
+        .env("CAE_FORCE_BROWSER", "1")
```

## Proposed Direction (Not Applied)

Set `.env("CAE_SKIP_BROWSER_OPEN", "1")` on both `Command` invocations in `tests/wallet_tests.rs`.

## Next Step

Run `/speckit-bug-assess slug=pr-4-wallet-tests-open-real-browser` using this intake file.
