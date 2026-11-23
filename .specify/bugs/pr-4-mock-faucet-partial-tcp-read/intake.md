---
slug: pr-4-mock-faucet-partial-tcp-read
pr_number: 4
comment_id: 4660961024
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660961024"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCfAA"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: tests/wallet_tests.rs
line: 520
start_line: 515
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: Mock faucet servers in wallet_tests and e2e_all_commands perform a single partial TCP read

- **Slug**: pr-4-mock-faucet-partial-tcp-read
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660961024
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660961024
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `tests/wallet_tests.rs:515-520`

## Finding

`spawn_mock_faucet` in both `tests/wallet_tests.rs:515-520` and `tests/e2e_all_commands.rs:296-299` performs a single 1024-byte `stream.read()` call instead of looping until `\r\n\r\n` and `Content-Length` body bytes arrive (unlike `spawn_mock_rpc`). When `reqwest`/`hyper` sends headers and POST body in separate TCP writes, the mock drops the socket with unread bytes, causing flaky missing-body assertions and TCP `RST` errors.

- **Category**: `concurrency`
- **Rule**: project / tests/**/*.rs — Concurrency and Shared State / Test Standards

## Diff Context

```diff
+            let mut buf = Vec::new();
+            let mut chunk = [0u8; 1024];
+            let n = stream.read(&mut chunk).unwrap_or(0);
+            buf.extend_from_slice(&chunk[..n]);
```

## Proposed Direction (Not Applied)

Loop reading from `stream` until `\r\n\r\n` and full `Content-Length` body bytes are received (matching `spawn_mock_rpc`) in both `tests/wallet_tests.rs` and `tests/e2e_all_commands.rs`.

## Next Step

Run `/speckit-bug-assess slug=pr-4-mock-faucet-partial-tcp-read` using this intake file.
