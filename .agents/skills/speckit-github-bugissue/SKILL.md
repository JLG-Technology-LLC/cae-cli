---
name: speckit-github-bugissue
description: Create a GitHub bug issue after /speckit-bug-assess when the bug verdict is validated
compatibility: Requires spec-kit project structure with .specify/ directory and GitHub remote (gh CLI or GitHub MCP)
metadata:
  author: project-local
  source: github:commands/speckit.github.bugissue.md
  version: 1.0.0
---

# Create GitHub Bug Issue from Bug Assessment

Create a GitHub issue labeled `bug` from `.specify/bugs/<BUG_SLUG>/assessment.md` **only when the assessment verdict is validated (`valid`)**.

## User Input

```text
$ARGUMENTS
```

## Prerequisites & Safety

1. Resolve `BUG_SLUG` from `$ARGUMENTS` or the active `/speckit-bug-assess` session and verify `.specify/bugs/<BUG_SLUG>/assessment.md` exists.
2. Check the Git remote URL:
   ```bash
   git config --get remote.origin.url
   ```
   > [!CAUTION]
   > ONLY PROCEED IF `remote.origin.url` IS A GITHUB REPOSITORY. NEVER CREATE ISSUES IN A DIFFERENT REPOSITORY.

## Validation Gate

Read `.specify/bugs/<BUG_SLUG>/assessment.md` and inspect `- **Verdict**: ...`:
- **If `Verdict` is `valid`**: proceed to deduplication and issue creation.
- **If `Verdict` is `invalid` or `likely valid, needs reproduction` (not validated)**: **STOP** and report:
  `Skipping GitHub bug issue creation: bug '<BUG_SLUG>' verdict is '<verdict>' (must be 'valid').`

## Execution

1. **Deduplicate**:
   - Check existing open and closed issues in the repository for `[bug:<BUG_SLUG>]` or the bug title using `gh issue list --state all --search "<BUG_SLUG>" --json number,title,url` (or GitHub MCP `list_issues`).
   - If a matching issue already exists, report its URL and skip creating a duplicate.
2. **Create the GitHub Bug Issue**:
   - Title: `bug(<BUG_SLUG>): <short title from assessment.md>`
   - Body: Include Severity, Symptom, Reproduction steps, Suspected Code Paths, Root Cause Hypothesis, Proposed Remediation, and a reference to `.specify/bugs/<BUG_SLUG>/assessment.md` plus a hidden marker `<!-- speckit-bug:<BUG_SLUG> -->` for reliable deduplication.
   - Labels: `bug` (verify the label exists or omit if unavailable).
   - Command (via `gh` CLI, or equivalent GitHub MCP `create_issue`):
     ```bash
     gh issue create --title "bug(<BUG_SLUG>): <short title>" --label "bug" --body-file "<temp_body_file>"
     ```
3. **Output**:
   - Report the created (or existing) GitHub issue number and URL so `/speckit-bug-fix` and PRs can link `Fixes #<number>`.
