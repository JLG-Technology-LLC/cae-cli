---
name: speckit-gitflow-release
description: Start or finish a Git Flow release branch from dev to main with SemVer and Conventional Commits
compatibility: Requires spec-kit project structure with .specify/ directory and git flow config
metadata:
  author: project-local
  source: gitflow:commands/speckit.gitflow.release.md
  version: 1.0.0
---

# Git Flow Release Workflow

Start or finish a Git Flow release branch (`release/vX.Y.Z`) promoting `dev` (`gitflow.branch.develop`) to `main` (`gitflow.branch.master`) with Conventional Commit messages (`chore(release): vX.Y.Z`) compatible with `commitlint`.

## User Input

```text
$ARGUMENTS
```

Expected usage:
- `start <version>` (e.g. `start 1.2.0` — `gitflow.prefix.versiontag` `v` is applied automatically)
- `finish <version>` (e.g. `finish 1.2.0`)
- If `<version>` is omitted on `start`, inspect commits since the last tag (`git log $(git describe --tags --abbrev=0)..dev --oneline`) and propose the next SemVer version (`feat!` / `BREAKING CHANGE` → major, `feat` → minor, `fix` → patch).

## Execution

### Start a Release (`start <version>`)
1. Ensure `dev` is clean and up to date with `origin/dev`.
2. **If `git flow` CLI is installed**:
   ```bash
   git flow release start "<version>"
   ```
   *(Creates `release/<version>` from `dev`.)*
3. **Fallback (if `git flow` CLI is not installed)**:
   ```bash
   git checkout -b "release/v<version>" "$(git config --get gitflow.branch.develop || echo dev)"
   ```
4. Bump version metadata (e.g., `pubspec.yaml`, `Cargo.toml`, or `pyproject.toml`) and commit using Conventional Commits:
   ```bash
   git commit -m "chore(release): prepare v<version>"
   ```

### Finish a Release (`finish <version>`)
Ask the user whether to open a PR into `main` (recommended to trigger `.github/workflows/release-trigger.yml` and CI) or finish locally:
1. **Via Pull Request (Recommended)**:
   ```bash
   git push -u origin "release/v<version>"
   gh pr create --base main --title "chore(release): v<version>" --body "Release v<version> from dev."
   ```
2. **Locally via `git flow`**:
   - **If `git flow` CLI is installed**:
     ```bash
     git flow release finish -m "chore(release): v<version>" "<version>"
     ```
   - **Fallback**:
     Merge `release/v<version>` into `main` with `chore(release): v<version>`, tag `v<version>`, merge back into `dev` with `chore(release): merge v<version> into dev`, and delete the release branch.
