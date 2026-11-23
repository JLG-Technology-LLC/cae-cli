---
slug: pr-4-ipv6-loopback-url-check-broken
pr_number: 4
comment_id: 4660960540
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960540"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCdHA"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: src/wallet/balance.rs
line: 137
start_line: 133
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: is_loopback_url splits on ':' making IPv6 [::1] detection unreachable

- **Slug**: pr-4-ipv6-loopback-url-check-broken
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960540
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960540
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `src/wallet/balance.rs:133-137`

## Finding

`is_loopback_url` splits on `['/', ':']`, so an IPv6 loopback URL like `http://[::1]:8545` produces `host = "["` and never matches `"[::1]"`. It also misses uppercase `LOCALHOST` and userinfo URLs. The exact same broken helper is duplicated at `src/wallet/faucet.rs:113-117`.

- **Category**: `correctness`
- **Rule**: Security-Sensitive Code — URL parsing/validation

## Diff Context

```diff
+fn is_loopback_url(url: &str) -> bool {
+    let rest = url.split("://").nth(1).unwrap_or(url);
+    let host = rest.split(['/', ':']).next().unwrap_or("");
+    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
+}
```

## Proposed Direction (Not Applied)

Parse URLs with `reqwest::Url::parse` and inspect `url.host()` (`Domain` case-insensitive `localhost`, `Ipv4`/`Ipv6` `.is_loopback()`), sharing one helper between `balance.rs` and `faucet.rs`.

## Next Step

Run `/speckit-bug-assess slug=pr-4-ipv6-loopback-url-check-broken` using this intake file.
