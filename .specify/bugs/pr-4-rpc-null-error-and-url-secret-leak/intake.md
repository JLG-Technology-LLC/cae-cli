---
slug: pr-4-rpc-null-error-and-url-secret-leak
pr_number: 4
comment_id: 4660960615
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960615"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCdZw"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: src/wallet/balance.rs
line: 130
start_line: 116
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: rpc_call fails on JSON-RPC "error": null and leaks private RPC URL credentials in error logs

- **Slug**: pr-4-rpc-null-error-and-url-secret-leak
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960615
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960615
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `src/wallet/balance.rs:116-130`

## Finding

`rpc_call` treats `"error": null` (returned by many JSON-RPC providers on success) as a fatal error because it does not check `!err.is_null()`. It also interpolates the full RPC `url` verbatim into error messages, leaking secret API keys embedded in Alchemy/Infura/QuickNode URL paths, query strings, or userinfo (same URL leak occurs in `src/wallet/faucet.rs:66, 80`).

- **Category**: `error handling`
- **Rule**: Security-Sensitive Code (no secret logging) & Error Handling

## Diff Context

```diff
+    if let Some(err) = response.get("error") {
+        bail!("RPC error from {}: {}", url, err);
+    }
```

## Proposed Direction (Not Applied)

Filter `response.get("error").filter(|v| !v.is_null())` and sanitize URLs to `scheme://host[:port]` (stripping userinfo, path, and query) in all error messages in `balance.rs` and `faucet.rs`.

## Next Step

Run `/speckit-bug-assess slug=pr-4-rpc-null-error-and-url-secret-leak` using this intake file.
