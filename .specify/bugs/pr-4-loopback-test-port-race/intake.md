---
slug: pr-4-loopback-test-port-race
pr_number: 4
comment_id: 4660961122
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660961122"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCfYg"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: tests/wallet_tests.rs
line: 624
start_line: 602
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: TOCTOU ephemeral port release race and unjoined callback thread in loopback tests

- **Slug**: pr-4-loopback-test-port-race
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660961122
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660961122
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `tests/wallet_tests.rs:602-624`

## Finding

Loopback auth tests in `tests/wallet_tests.rs:602-624`, `658-678` and `tests/e2e_all_commands.rs:705-724` bind `127.0.0.1:0`, drop the `TcpListener` to guess a free port, and spawn a detached thread (dropping its `JoinHandle`) that sleeps 300ms and sends `GET /callback` to that port. Under parallel `cargo test`, another test can claim the port before `cae` binds it, and detached thread panics go unobserved.

- **Category**: `concurrency`
- **Rule**: project / tests/**/*.rs — Concurrency and Shared State & Async Safety

## Diff Context

```diff
+    let listener = TcpListener::bind("127.0.0.1:0")?;
+    let port = listener.local_addr()?.port();
+    drop(listener);
+    std::thread::spawn(move || { ... });
```

## Proposed Direction (Not Applied)

Support `CAE_AUTH_PORT=0` (printing the OS-assigned bound URL/port + state token to stderr for test discovery) and join the callback thread `JoinHandle` while reading the HTTP response before closing the socket.

## Next Step

Run `/speckit-bug-assess slug=pr-4-loopback-test-port-race` using this intake file.
