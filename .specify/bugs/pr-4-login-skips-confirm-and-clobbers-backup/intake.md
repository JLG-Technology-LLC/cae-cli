---
slug: pr-4-login-skips-confirm-and-clobbers-backup
pr_number: 4
comment_id: 4660960788
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960788"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCeFA"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: src/wallet/mod.rs
line: 326
start_line: 305
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: handle_login bypasses local-wallet confirmation on --json and clobbers backup files before auth succeeds

- **Slug**: pr-4-login-skips-confirm-and-clobbers-backup
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960788
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960788
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `src/wallet/mod.rs:305-326`

## Finding

Passing `--json` to `cae wallet login` skips the interactive confirmation prompt (`if !args.json && !confirm(&prompt)?`) and silently replaces an active `local-eoa` wallet without requiring `--force`. Moreover, `backup_local_profile(&path)?` (and `backup_profile(&path)?` in `handle_init` line 232) overwrites existing backup files *before* `auth::authenticate_user` succeeds, destroying prior backups if auth fails or times out.

- **Category**: `error handling`
- **Rule**: Error Handling and Panics & Type and API Design

## Diff Context

```diff
+            if !args.json && !confirm(&prompt)? {
+                bail!("Login cancelled. Existing local wallet was kept.");
+            }
+            let local_bak = backup_local_profile(&path)?;
...
+    let auth_res = auth::authenticate_user(args.email, args.no_browser, args.json)?;
```

## Proposed Direction (Not Applied)

Add a `--force` flag to `wallet login` and fail closed in `--json` mode when replacing a `local-eoa` wallet without `--force`; defer `backup_local_profile` (and `backup_profile` in `handle_init`) until after authentication succeeds and immediately before `save_profile`.

## Next Step

Run `/speckit-bug-assess slug=pr-4-login-skips-confirm-and-clobbers-backup` using this intake file.
