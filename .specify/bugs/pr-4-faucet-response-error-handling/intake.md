---
slug: pr-4-faucet-response-error-handling
pr_number: 4
comment_id: 4660960706
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960706"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCdwg"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: src/wallet/faucet.rs
line: 85
start_line: 71
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: Faucet handler ignores non-429 HTTP errors, misclassifies 200 OK message/null error, and drops error objects

- **Slug**: pr-4-faucet-response-error-handling
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960706
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960706
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `src/wallet/faucet.rs:71-85`

## Finding

`request_faucet` only checks `StatusCode::TOO_MANY_REQUESTS` and never checks `!status.is_success()` for 400/401/403/500 responses. On HTTP 200 OK, `res_json.get("error").or_else(|| res_json.get("message"))` falsely fails when `"error": null` or an informational `"message"` is present, and `.as_str().unwrap_or("Unknown faucet error")` discards structured JSON error objects.

- **Category**: `error handling`
- **Rule**: Error Handling and Panics

## Diff Context

```diff
+    if let Some(err) = res_json.get("error").or_else(|| res_json.get("message")) {
+        let err_str = err.as_str().unwrap_or("Unknown faucet error");
+        bail!("CDP Faucet refused request: {}", err_str);
+    }
```

## Proposed Direction (Not Applied)

Check `!status.is_success()` explicitly and extract structured `error.message`/`message`/serialized error details; on 2xx responses only fail when `"error"` is present and non-null.

## Next Step

Run `/speckit-bug-assess slug=pr-4-faucet-response-error-handling` using this intake file.
