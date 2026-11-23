---
slug: pr-4-silent-csprng-zero-fallback
pr_number: 4
comment_id: 4660960349
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960349"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCcXQ"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: src/wallet/auth.rs
line: 294
start_line: 290
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: crypto_random_bytes silently swallows CSPRNG errors and returns all zeros

- **Slug**: pr-4-silent-csprng-zero-fallback
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960349
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960349
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `src/wallet/auth.rs:290-294`

## Finding

`crypto_random_bytes` calls `getrandom::getrandom(&mut b).unwrap_or_default()`, silently ignoring OS CSPRNG failures and returning `[0u8; 16]`, which produces predictable all-zero tokens instead of failing closed.

- **Category**: `error handling`
- **Rule**: Error Handling and Panics & Security-Sensitive Code

## Diff Context

```diff
+fn crypto_random_bytes() -> [u8; 16] {
+    let mut b = [0u8; 16];
+    getrandom::getrandom(&mut b).unwrap_or_default();
+    b
+}
```

## Proposed Direction (Not Applied)

Return `Result<[u8; 16]>` and propagate `getrandom` failures with context.

## Next Step

Run `/speckit-bug-assess slug=pr-4-silent-csprng-zero-fallback` using this intake file.
