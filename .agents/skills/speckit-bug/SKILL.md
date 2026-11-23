---
name: speckit-bug
description: Parallel end-to-end PR review and bug resolution workflow. Runs multi-agent OpenCodeReview (ocr delegate) discovery or imports existing PR review threads, then fans out parallel Spec Kit stages (assess -> fix -> test) under .specify/bugs/<slug>/ with strict 1-commit-per-bug Conventional Commit isolation, GitHub issue hooks, and live PR thread replies.
---

# Parallel OCR + Spec Kit Bug Triage & PR Thread Workflow

Combine **multi-agent OpenCodeReview (`ocr delegate`)** discovery with **parallel Spec Kit (`assess` -> `fix` -> `test`)** remediation. Every discovered bug gets an isolated `.specify/bugs/<slug>/` workspace, maps 1-to-1 to a PR review comment thread (and validated GitHub bug issue), and lands as **its own separate Conventional Commit**.

## Parallelism & Commit Isolation Rules (Non-Negotiable)

1. **Parallel from Stage 0**: Discovery, assessment, fixing, and verification all fan out across multiple subagents whenever more than one file batch or bug exists.
2. **One Commit Per Bug**: Every validated bug fix MUST be committed as **its own isolated Conventional Commit** (`fix(<scope>): <summary>`) passing `.git/hooks/commit-msg` (`commitlint`). Never bundle multiple bugs into one commit and never run `git add -A` across concurrent bugs.
3. **Zero Concurrent Edit / Index Races**:
   - Bugs with **disjoint file ownership** fix in parallel; bugs sharing any source/test file are automatically scheduled into **sequential waves**.
   - Git staging/committing is always isolated (either via per-bug `git worktree` or serialized coordinator commits staging only explicit file paths).

---

## Entry Routing

1. **Stage 0 (`discover` / default when no `.specify/bugs/<slug>/intake.md` exists):**
   - **Path A (Fresh Discovery):** Partition reviewable files and launch parallel discovery subagents using `ocr delegate`, post one inline PR review thread per `High`/`Medium` finding, and scaffold `.specify/bugs/<slug>/intake.md`.
   - **Path B (Import Existing PR OCR Threads):** If OCR review threads already exist on the target PR (e.g., `ocr delegate` was already run), import all unresolved OCR threads directly into `.specify/bugs/<slug>/intake.md` without posting duplicate PR comments.
2. **Stage 1 (`assess`):** Fan out parallel read-only assessment subagents (1 per bug) to diagnose root cause, write `assessment.md`, run `after_bug_assess` (`speckit-github-bugissue` when `Verdict: valid`), and reply to the PR thread.
3. **Stage 2 (`fix`):** Partition validated bugs into conflict-free parallel waves by file ownership, apply minimal fixes + regression tests in parallel, create **exactly one Conventional Commit per bug**, write `fix.md`, and reply to each PR thread with the commit SHA.
4. **Stage 3 (`test`):** Fan out parallel verification subagents per bug (plus one final combined suite/linter pass for cross-bug regression safety), write `test.md`, run `after_bug_test` (`speckit-github-bugverify`), and post the final verdict to each PR thread.
5. **End-to-End (`full`):** Run `discover` -> `assess` -> `fix` -> `test` across all discovered/imported bugs using the parallel wave orchestration below.

---

## Stage 0: `discover` (Parallel Multi-Agent Review or Existing PR Thread Import)

**Constraint:** Strictly read-only for application source files. Do not auto-fix code during discovery.

### Path A: Fresh Parallel `ocr delegate` Discovery
1. **Preview & Partition (Coordinator):**
   - Run `ocr delegate preview [user-args]` once (install via `npm i -g @alibaba-group/open-code-review` if missing) to enumerate all reviewable files.
   - Detect PR number and head commit SHA (e.g., `gh pr view --json number,headRefOid`).
   - Partition reviewable files into balanced batches by module/directory (e.g., 5–10 files per batch, or split by layer such as `core/`, `features/`, `test/`).
2. **Parallel Discovery Subagents (Read-Only):**
   - Launch **one subagent per file batch** concurrently.
   - Each subagent runs `ocr delegate rule <batch-files>`, inspects diffs for its assigned files using `git --no-pager diff --no-ext-diff --no-textconv --no-color -- <batch-files>`, and returns structured `High` and `Medium` findings (`file_path`, `line_number`, `severity`, `title`, `description`, `suggested_direction`). Discard `Low` findings silently.
3. **Deduplicate, Post PR Threads & Scaffold Intake (Coordinator):**
   - Merge and deduplicate overlapping findings across subagents.
   - For each unique `High`/`Medium` finding:
     1. Derive `<slug>` (e.g., `pr-<num>-<short-slug>`).
     2. Post **one inline PR review comment thread** on `<file_path>` at `<line_number>` via `gh api` and capture `comment_id`.
     3. Write `.specify/bugs/<slug>/intake.md`.

### Path B: Import Existing OCR Threads from a PR (e.g., PR already reviewed by `ocr delegate`)
1. Fetch existing PR review comments via `gh api repos/{owner}/{repo}/pulls/<pr_number>/comments --paginate` and filter to top-level unresolved OCR findings (`in_reply_to_id == null`).
2. Skip re-running `ocr delegate` and do **not** post duplicate review threads.
3. Derive a unique `<slug>` per finding and scaffold `.specify/bugs/<slug>/intake.md` (parallelizable across batches).

### Intake File Schema (`.specify/bugs/<slug>/intake.md`)
```markdown
# Bug Intake: <slug>
- **Severity:** High | Medium
- **PR Number:** <pr_number>
- **Head SHA:** <head_sha>
- **Thread / Comment ID:** <comment_id>
- **File & Line:** `<file_path>:<line_number>`

## OCR Finding
<Clear description of the defect and diff context>

## Suggested Direction
<Initial remediation hint from OCR>
```

**Summary Report:** Output `total_files`, `reviewed_files`, `skipped_files` (with reasons), `coverage_rate` (for Path A), and the full list of `<slug>` items ready for Stage 1.

---

## Stage 1: `assess` (Parallel Multi-Agent Assessment + GitHub Issue Hook)

**Constraint:** Strictly read-only for application source files. Each subagent writes only `.specify/bugs/<slug>/assessment.md`.

1. **Parallel Fan-Out (Coordinator -> Subagents):**
   - Launch **parallel read-only subagents** (1 subagent per `<slug>`, or bounded concurrency batches of 5–8 if there are many bugs).
   - Each subagent reads `.specify/bugs/<slug>/intake.md`, `.specify/memory/constitution.md`, relevant `specs/`, and traces callers, state mutations, and edge cases to decide whether the finding is a **valid bug**, **needs reproduction**, or **invalid / false positive**.
2. **Write `.specify/bugs/<slug>/assessment.md`:**
   ```markdown
   # Bug Assessment: <slug>
   - **Slug**: <slug>
   - **Created**: <ISO 8601 date>
   - **Source**: PR #<pr_number> (Comment ID: `<comment_id>`)
   - **Verdict**: valid | likely valid, needs reproduction | invalid
   - **Status**: Assessed (Awaiting Fix) | Invalid / Not a Bug
   - **Severity**: critical | high | medium | low
   - **PR Thread**: PR #<pr_number> (Comment ID: `<comment_id>`)
   - **GitHub Issue**: #<issue_number> (or "None")
   - **Related Spec**: <path to spec.md or "None">

   ## Symptom & Reproduction
   <Observed vs. expected behavior and reproduction steps/command>

   ## Root Cause Analysis
   - **Suspected Files**: `<file_path>:<line_number>`
   - **Cause**: <Technical explanation of why the defect occurs>
   - **Adjacent Risks**: <Side effects, edge cases, or dependent modules>

   ## Proposed Remediation Plan
   **Files likely to change**:
   - `<path/to/source_file>`
   - `<path/to/test_file>`

   **Steps**:
   1. <Minimal code change aligned with constitution.md>
   2. <Regression test to add or update>
   ```
3. **Post-Assessment Hooks & PR Thread Reply (Only if `Verdict: valid`):**
   - Trigger `.specify/extensions.yml` `hooks.after_bug_assess` (**`speckit-github-bugissue`**) so validated bugs automatically get a deduplicated `bug`-labeled GitHub issue; record `GitHub Issue: #<number>` in `assessment.md`.
   - Reply to `comment_id` on PR `#<pr_number>`:
     ```bash
     gh api repos/{owner}/{repo}/pulls/<pr_number>/comments/<comment_id>/replies -f body="..."
     ```
     > **SpecKit Assessment (`<slug>`):** Confirmed valid `<Severity>` bug (Issue `#<issue_number>`).
     > **Root Cause:** <1–2 sentence cause>
     > **Planned Fix:** <1–2 sentence plan>
   - If `Verdict: invalid`, do **not** create a GitHub issue, mark `Status: Invalid / Not a Bug`, and skip Stage 2–3.

---

## Stage 2: `fix` (Parallel Conflict-Free Waves + Strict 1-Commit-Per-Bug Isolation)

**Precondition:** `.specify/bugs/<slug>/assessment.md` has `Verdict: valid` and `Status: Assessed (Awaiting Fix)`.

### 1. Build Conflict-Free Parallel Wave Plan (Coordinator)
- Collect `Files likely to change` from every validated `assessment.md`.
- Group bugs into **waves** such that no two bugs in the same wave share any source or test file:
  - **Wave 1:** Maximal set of bugs with mutually disjoint file sets (run concurrently).
  - **Wave 2+:** Remaining bugs that overlapped with earlier waves (run after Wave 1 commits land).
- If working on a single standalone bug (not batch PR triage), trigger `hooks.before_bug_fix` (`speckit-gitflow-bugfix-start`) to branch `bugfix/<slug>` from `dev`.

### 2. Parallel Fix Execution (Choose Supported Isolation Mode)

- **Mode A — Per-Bug Git Worktree (Preferred when `git worktree` is available):**
  1. For each bug `<slug>` in the current wave, create an isolated worktree:
     ```bash
     git worktree add -b "bugfix/<slug>" "../.worktrees/<slug>" HEAD
     ```
  2. Launch **one subagent per bug** in parallel inside its own worktree. Each subagent applies the minimal fix + regression test, writes `.specify/bugs/<slug>/fix.md`, and creates **exactly 1 Conventional Commit** inside its worktree.
  3. After all subagents in the wave finish, the coordinator cherry-picks each bug's single commit onto the active branch in deterministic order (`git cherry-pick <sha>`), resolves any trivial context if needed, updates `Commit SHA` in `fix.md`, and cleans up the worktrees (`git worktree remove` + `git branch -d`).

- **Mode B — Shared Workspace Disjoint-File Waves + Serialized Per-Bug Commit (Fallback when worktrees are unavailable):**
  1. Launch **one subagent per bug** in the current wave concurrently, giving each subagent an explicit allowlist of its disjoint files (`Files likely to change` + `.specify/bugs/<slug>/fix.md`).
  2. **Subagents MUST NOT run `git add` or `git commit`** in Mode B (prevents `.git/index.lock` races and cross-bug commit mixing).
  3. Once all subagents in the wave complete, the coordinator commits **one bug at a time** sequentially, staging **only** that bug's exact modified files + `.specify/bugs/<slug>/` files (never `git add -A`):
     ```bash
     git add -- <bug_file_1> <bug_file_2> ".specify/bugs/<slug>/"
     git commit -F "<temp_commit_msg_file>"
     ```

### 3. Mandatory Conventional Commit Format (1 Commit Per Bug)
Every bug commit MUST satisfy `.git/hooks/commit-msg` (`commitlint`):
```text
fix(<scope>): <concise imperative summary of the bug fix>

Bug: <slug>
Fixes #<issue_number>
PR-Thread: #<pr_number> (comment <comment_id>)
```
*(Omit `Fixes #<issue_number>` if no GitHub issue was created; omit `PR-Thread` if not from a PR.)*

### 4. Write `.specify/bugs/<slug>/fix.md` & Reply to PR Thread
```markdown
# Bug Fix: <slug>
- **Status:** Fixed (Awaiting Verification)
- **Based On:** `.specify/bugs/<slug>/assessment.md`
- **Commit SHA:** `<short_sha>`
- **GitHub Issue:** `#<issue_number>` (or "None")

## Summary of Changes
- `<modified_file>`: <What changed and why>
- `<test_file>`: <Regression test added/updated>

## Deviation from Assessment
<Any adjustments made during implementation, or "None">
```
Post a reply to the bug's PR review thread (`comment_id`):
> **SpecKit Fix Applied (`<slug>`):**
> - **Commit:** `<short_sha>`
> - **Files Changed:** `<modified_file>`, `<test_file>`
> - **Summary:** <Concise explanation of the patch>
> - Proceeding to parallel verification.

---

## Stage 3: `test` (Parallel Verification + Final Shared Gate + Hooks)

**Precondition:** `.specify/bugs/<slug>/fix.md` exists and the bug's fix commit has landed. **Strictly read-only for application source code.**

1. **Parallel Per-Bug Verification (Subagents):**
   - Launch **parallel read-only subagents** (1 per fixed bug `<slug>`) concurrently.
   - Each subagent:
     - Audits the exact bug commit diff (`git show <short_sha>`) against `.specify/bugs/<slug>/intake.md` and `assessment.md` to confirm the defect is fully resolved with zero scope creep.
     - Runs bug-specific reproduction checks and targeted unit/widget tests (for toolchains where concurrent test runners contend on shared build artifacts like Cargo `target/` or Flutter `.dart_tool/`, run targeted file-level checks where safe or defer test execution to Step 2).
     - Writes `.specify/bugs/<slug>/test.md`.
2. **Final Combined Regression Gate (Coordinator):**
   - Run one full project test suite + linter pass (e.g., `flutter test && flutter analyze`, `cargo test --all-targets --all-features && cargo clippy`, or `uv run pytest && uv run ruff check .`) after all waves complete to catch cross-bug regressions.
3. **Determine Verdict:**
   - `verified`: Reproduction passes, regression tests pass, zero regressions.
   - `partial`: Primary bug fixed, but minor edge cases or unrelated flaky checks remain.
   - `failed`: Reproduction still fails or regressions were introduced.
4. **Write `.specify/bugs/<slug>/test.md`:**
   ````markdown
   # Bug Verification: <slug>
   - **Slug**: <slug>
   - **Tested**: <ISO 8601 date>
   - **Assessment**: ./assessment.md
   - **Fix**: ./fix.md
   - **Commit SHA**: `<short_sha>`
   - **Result**: verified | partial | failed
   - **Verdict**: verified | partial | failed

   ## Commands Executed
   ```bash
   <exact commands run>
   ```

   ## Results
   - **Reproduction Check:** Pass | Fail
   - **Regression Suite:** <Pass/Fail summary>
   - **Notes:** <Caveats or follow-up items>
   ````
5. **Post-Verification Hooks, PR Thread Reply & Clean Commit:**
   - Trigger `.specify/extensions.yml` `hooks.after_bug_test`:
     - **`speckit-github-bugverify`**: Posts verification results to the linked GitHub bug issue and updates labels (`needs-triage` -> `verified`).
     - **`speckit-gitflow-bugfix-finish`** *(optional, for standalone `bugfix/*` branches)*: Opens PR into `dev` with `Fixes #<issue>` or finishes via `git flow bugfix finish`.
   - Reply to the PR review thread (`comment_id`):
     > **SpecKit Verification (`<slug>`):** `<VERDICT>` (Commit `<short_sha>`)
     > - **Reproduction:** Pass/Fail
     > - **Tests Run:** `<command>` -> <Pass/Fail summary>
   - If `verified`: Update `assessment.md` and `fix.md` status headers to `Verified` and commit each bug's verification artifact cleanly (`docs(bug): record verification for <slug>`) without squashing separate bug commits together.
   - If `failed`: Loop back to Stage 2 (`fix`) once with the failure logs.
