---
slug: pr-4-wallet-temp-file-permission-race
pr_number: 4
comment_id: 4660960209
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960209"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCb0Q"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: src/wallet/profile.rs
line: 188
start_line: 159
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: Predictable temp path and non-exclusive open bypass 0600 permissions and allow symlink/race attacks

- **Slug**: pr-4-wallet-temp-file-permission-race
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960209
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960209
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `src/wallet/profile.rs:159-188`

## Finding

`write_private_file` uses a predictable `<path>.tmp<pid>` filename and opens it with `.create(true).truncate(true).mode(0o600)`. On Unix, `.mode(0o600)` only applies when creating a new file; if `<path>.tmp<pid>` already exists (pre-created with `0o644` or planted as a symlink in a shared dir), `open()` follows symlinks and keeps broader permissions, and concurrent in-process writes collide on the same temp file.

- **Category**: `security`
- **Rule**: Concurrency and Shared State & Security-Sensitive Code

## Diff Context

```diff
+    let mut tmp_name = path.as_os_str().to_owned();
+    tmp_name.push(format!(".tmp{}", std::process::id()));
+    let tmp_path = PathBuf::from(tmp_name);
...
+    fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)
```

## Proposed Direction (Not Applied)

Generate a random suffix via `getrandom`, open with `.create_new(true)` (`O_CREAT | O_EXCL`), enforce `0o600` on the open handle via `file.set_permissions`, and set `0o700` on newly created parent wallet directories.

## Next Step

Run `/speckit-bug-assess slug=pr-4-wallet-temp-file-permission-race` using this intake file.
