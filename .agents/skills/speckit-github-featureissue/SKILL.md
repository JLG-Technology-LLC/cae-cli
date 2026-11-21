---
name: speckit-github-featureissue
description: Create a GitHub feature issue after /speckit-specify or after /speckit-assess-decide returns a go verdict
compatibility: Requires spec-kit project structure with .specify/ directory and GitHub remote (gh CLI or GitHub MCP)
metadata:
  author: project-local
  source: github:commands/speckit.github.featureissue.md
  version: 1.0.0
---

# Create GitHub Feature Issue from Spec or Go Assessment

Create (or update) a GitHub feature issue labeled `enhancement` after:
- **`/speckit-assess-decide`** produces a **`go`** verdict in `.specify/assessments/<ASSESS_SLUG>/decision.md`, OR
- **`/speckit-specify`** produces a feature specification in `specs/<FEATURE_DIR>/spec.md`.

## User Input

```text
$ARGUMENTS
```

## Prerequisites & Safety

1. Check the Git remote URL:
   ```bash
   git config --get remote.origin.url
   ```
   > [!CAUTION]
   > ONLY PROCEED IF `remote.origin.url` IS A GITHUB REPOSITORY. NEVER CREATE ISSUES IN A DIFFERENT REPOSITORY.

## Source Detection & Gate

Determine which workflow triggered this skill (from conversation context or `$ARGUMENTS`):

### Case A: Triggered after `/speckit-assess-decide` (`after_assess_decide`)
1. Read `.specify/assessments/<ASSESS_SLUG>/decision.md`.
2. Inspect `- **Verdict**: ...`:
   - **If `Verdict` is `go`**: proceed to deduplication and issue creation using the Handoff summary (Problem, Chosen approach, In/out of scope, Success metrics, Open questions).
   - **If `Verdict` is `needs-clarification` or `kill`**: **STOP** and report:
     `Skipping GitHub feature issue creation: assessment '<ASSESS_SLUG>' verdict is '<verdict>' (must be 'go').`
3. Set `FEATURE_KEY = <ASSESS_SLUG>` and `SOURCE_PATH = .specify/assessments/<ASSESS_SLUG>/decision.md`.

### Case B: Triggered after `/speckit-specify` (`after_specify`)
1. Resolve `SPEC_FILE` from `.specify/feature.json` or the active `/speckit-specify` session (`specs/<NNN>-<slug>/spec.md`).
2. Read `SPEC_FILE` to extract the feature title, summary/goals, user stories/requirements, and success criteria.
3. Set `FEATURE_KEY = <slug>` (without the leading `<NNN>-` prefix so it matches any issue already created during `after_assess_decide`) and `SOURCE_PATH = specs/<NNN>-<slug>/spec.md`.

## Execution

1. **Deduplicate**:
   - Search existing open and closed issues for `<FEATURE_KEY>` using `gh issue list --state all --search "<FEATURE_KEY>" --json number,title,url` (or GitHub MCP `list_issues`).
   - If a matching feature issue already exists:
     - If triggered from `after_specify` and the existing issue came from `after_assess_decide`, post a comment linking the new spec (`Specification created at \`<SOURCE_PATH>\``) instead of creating a duplicate issue.
     - Otherwise report the existing issue URL and skip creation.
2. **Create the GitHub Feature Issue**:
   - Title: `feat(<FEATURE_KEY>): <short title>`
   - Body: Structured summary from `decision.md` or `spec.md`, link to `<SOURCE_PATH>`, and a hidden marker `<!-- speckit-feature:<FEATURE_KEY> -->` for reliable deduplication.
   - Labels: `enhancement` (verify the label exists or omit if unavailable).
   - Command (via `gh` CLI, or equivalent GitHub MCP `create_issue`):
     ```bash
     gh issue create --title "feat(<FEATURE_KEY>): <short title>" --label "enhancement" --body-file "<temp_body_file>"
     ```
3. **Output**:
   - Report the created (or updated) GitHub issue number and URL so downstream branches/PRs can reference `Closes #<number>`.
