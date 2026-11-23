---
slug: pr-4-loopback-auth-csrf-hijack
pr_number: 4
comment_id: 4660959818
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660959818"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCaSg"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: high
file: src/wallet/auth.rs
line: 200
start_line: 145
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: Unauthenticated loopback auth callback allows CSRF wallet hijacking and completes on arbitrary paths

- **Slug**: pr-4-loopback-auth-csrf-hijack
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660959818
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660959818
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: High
- **Location**: `src/wallet/auth.rs:145-200`

## Finding

`run_loopback_auth` / `handle_loopback_stream` listens on predictable `127.0.0.1:3000` with no CSRF `state` nonce, no HTTP method or callback route check, and no percent-decoding of query values. Any malicious webpage open in the user's browser can trigger a cross-origin request (`<img src="http://localhost:3000/callback?address=0xATTACKER...&email=a@b.com&session_ref=x">`) to silently hijack `evm_address` in `~/.cae/wallet.json`. Moreover, line 151 only ignores `/`, `/index.html`, and `/favicon.ico` when `query_str` is empty, so any ancillary browser request (`/robots.txt`, `/apple-touch-icon.png`, `/favicon.ico?v=1`) immediately completes auth with fallback email `"candidate@example.com"` and an unspendable burn address.

- **Category**: `security`
- **Rule**: Security-Sensitive Code — validate URL/serialized input & explicit authentication verification

## Diff Context

```diff
+    let first_line = request_str.lines().next().unwrap_or("");
+    let path = first_line.split_whitespace().nth(1).unwrap_or("/");
+    let (route, query_str) = path.split_once('?').unwrap_or((path, ""));
+    if (route == "/" || route == "/index.html" || route == "/favicon.ico") && query_str.is_empty() {
...
+    let email = query_params.get("email").cloned().or(email_hint).unwrap_or_else(|| "candidate@example.com".to_string());
+    let evm_address = match raw_address {
+        Some(addr) if crate::wallet::crypto::is_valid_address(addr) => addr.clone(),
+        _ => derive_address_for_email(&email),
+    };
```

## Proposed Direction (Not Applied)

Generate a per-flow cryptographically random `state` nonce, strictly enforce `GET /callback` with constant-time `state` validation and URL percent-decoding, return `404`/`400` for unrecognized routes or missing/invalid callback parameters, and never silently fall back to synthetic email/address values.

## Next Step

Run `/speckit-bug-assess slug=pr-4-loopback-auth-csrf-hijack` using this intake file.
