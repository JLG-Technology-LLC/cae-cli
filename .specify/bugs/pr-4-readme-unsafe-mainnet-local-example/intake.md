---
slug: pr-4-readme-unsafe-mainnet-local-example
pr_number: 4
comment_id: 4660961228
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660961228"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCfzA"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: README.md
line: 290
start_line: 289
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: README recommends unencrypted --local wallet with --force for mainnet

- **Slug**: pr-4-readme-unsafe-mainnet-local-example
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660961228
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660961228
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `README.md:289-290`

## Finding

The `cae wallet init` examples recommend `cae wallet init --local --network base --force` for a production mainnet wallet, even though `--local` writes key material to `~/.cae/wallet.json` and `--force` silently overwrites existing wallets/backups. The `wallet.json` schema section (lines 473-487) also omits security warnings about private key protection and backup permissions.

- **Category**: `security`
- **Rule**: project / *.md — Security & Documentation (BLOCKING)

## Diff Context

```diff
+# Production mainnet wallet with force overwrite
+cae wallet init --local --network base --force
```

## Proposed Direction (Not Applied)

Recommend CDP User Auth (`cae wallet init --network base`) or clearly caveat `--local` security semantics, remove `--force` from the mainnet example, and document key storage & `0600` backup permission handling accurately.

## Next Step

Run `/speckit-bug-assess slug=pr-4-readme-unsafe-mainnet-local-example` using this intake file.
