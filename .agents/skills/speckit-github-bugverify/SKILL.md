---
name: speckit-github-bugverify
description: Post bug verification results from /speckit-bug-test to the linked GitHub bug issue and update labels
compatibility: Requires spec-kit project structure with .specify/ directory and GitHub remote (gh CLI or GitHub MCP)
metadata:
  author: project-local
  source: github:commands/speckit.github.bugverify.md
  version: 1.0.0
---

# Sync Bug Verification Outcome to GitHub Issue

Post the verification result (`verified`, `partial`, or `failed`) and summary from `.specify/bugs/<BUG_SLUG>/test.md` to the linked GitHub bug issue, and swap `needs-triage` → `verified` when verification passes.

## User Input

```text
$ARGUMENTS
```

## Prerequisites & Safety

1. Resolve `BUG_SLUG` from `$ARGUMENTS` or the active `/speckit-bug-test` session and verify `.specify/bugs/<BUG_SLUG>/test.md` exists.
2. Check the Git remote URL:
   ```bash
   git config --get remote.origin.url
   ```
   > [!CAUTION]
   > ONLY PROCEED IF `remote.origin.url` IS A GITHUB REPOSITORY. NEVER COMMENT ON OR MODIFY ISSUES IN A DIFFERENT REPOSITORY.

## Execution

1. **Read Verification Report**:
   - Read `.specify/bugs/<BUG_SLUG>/test.md` and extract:
     - `Result` (`verified` | `partial` | `failed`)
     - `Summary`
     - `Checks Performed` and `Recommendation`
2. **Look Up Linked Bug Issue**:
   - Search for the open GitHub issue matching `speckit-bug:${BUG_SLUG}`:
     ```bash
     ISSUE_JSON=$(gh issue list --state open --search "speckit-bug:${BUG_SLUG}" --json number,url --jq '.[0]')
     ISSUE_NUMBER=$(echo "$ISSUE_JSON" | jq -r '.number // empty')
     ```
   - If no open matching issue is found, report `No open GitHub bug issue found for 'speckit-bug:<BUG_SLUG>'; skipping comment.` and stop cleanly.
3. **Post Verification Comment**:
   - Post a comment on `#${ISSUE_NUMBER}` with the verification `Result`, `Summary`, `Checks Performed` table, and `Recommendation` from `.specify/bugs/${BUG_SLUG}/test.md`:
     ```bash
     gh issue comment "$ISSUE_NUMBER" --body-file "<temp_comment_file>"
     ```
     (or equivalent GitHub MCP `add_issue_comment`).
4. **Update Labels on `Result: verified`**:
   - If `Result` is `verified`, swap labels while ignoring missing-label errors:
     ```bash
     gh issue edit "$ISSUE_NUMBER" --remove-label "needs-triage" 2>/dev/null || true
     gh issue edit "$ISSUE_NUMBER" --add-label "verified" 2>/dev/null || true
     ```
5. **Output**:
   - Report the updated GitHub issue number/URL, the posted verification result, and any label changes applied.
