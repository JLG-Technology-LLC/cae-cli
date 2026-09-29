# Career Asset Engine CLI (`cae-cli`)

Standalone Rust CLI tool for tracking projects, parsing ecosystem manifests, extracting agent context files, and maintaining a durable project ledger.

## Features

- **Project Tracking (`cae track`)**: Automatically scans directories, discovers ecosystem manifests, extracts agent context documents, inspects Git status, and creates a local ledger.
- **Manifest Parsers (`cae-cli/src/manifest.rs`)**:
  - `pubspec.yaml` (Flutter / Dart)
  - `package.json` (Node / JS / TS)
  - `pyproject.toml` (Python - PEP 621 & Poetry)
  - `Cargo.toml` (Rust)
- **Agent Context Extraction (`cae-cli/src/context.rs`)**:
  - Discovers and extracts `ARCHITECTURE`, `README`, `DESIGN`, and `MEMORY` files across root and `docs/`, `memory/`, and `.agents/` directories.
  - Extracts line counts, file sizes, SHA-256 hashes, markdown headings, summaries, and previews.
- **Git State & History Tracking (`cae-cli/src/git.rs`)**:
  - Captures repository branch, HEAD commit, remote origin, clean/dirty state, and staged/modified/untracked file counts.
- **Commit Ledger Recording (`cae record-commit`)**:
  - Appends commit events with author, timestamp, message, changed files, and optional notes to the local ledger.
- **Local Ledger Management (`cae-cli/src/ledger.rs`)**:
  - Persists JSON ledgers to `~/.cae/projects/<id>.json`.
  - Supports listing, untracking, searching by ID or canonical path, and JSON output mode.

---

## Installation & Build

Ensure Rust (1.75+) is installed, then build:

```bash
cd cae-cli
cargo build --release
```

The compiled binary will be located at `target/release/cae`.

You can install it to your Cargo bin path for global access:

```bash
cargo install --path .
```

---

## Usage

### 1. Track a Project

Track the current directory or a specific path:

```bash
# Track current directory
cae track .

# Track a specific project path
cae track ../AICareerAssetEngine

# Track with custom ID and tags
cae track ../fastapi_agent --id backend-agent --tag api --tag python
```

### 2. Check Project Status

```bash
# View detailed status of the project in the current working directory
cae status

# View status of a specific tracked project by ID or path
cae status career-asset-engine

# List all tracked projects in a formatted table
cae status --all

# Output raw JSON format (ideal for agents and automation)
cae status career-asset-engine --json
```

### 3. Record a Git Commit

Record the latest commit or a specific commit into the project ledger:

```bash
# Record commit from current project
cae record-commit

# Record with a developer or agent note
cae record-commit --note "Implemented job search filtering"

# Record commit for a specific tracked project
cae record-commit --project career-asset-engine --note "Frontend release v1.0.0-beta.3"
```

### 4. Untrack a Project

Remove a project from the CAE ledger:

```bash
# Untrack by project ID
cae untrack career-asset-engine

# Untrack by path
cae untrack ../fastapi_agent
```

---

## Ledger Format (`~/.cae/projects/<id>.json`)

Each tracked project has a corresponding JSON file in `~/.cae/projects/`:

```json
{
  "id": "career-asset-engine",
  "name": "career_asset_engine",
  "canonical_path": "/Users/.../AICareerAssetEngine",
  "tracked_at": "2026-09-27T02:25:48.103282+00:00",
  "updated_at": "2026-09-27T02:26:10.929400+00:00",
  "ecosystems": [
    "Flutter/Dart"
  ],
  "git": {
    "is_git_repo": true,
    "root_path": "/Users/.../AICareerAssetEngine",
    "current_branch": "staging",
    "head_commit": "69a921f...",
    "head_short_commit": "69a921f",
    "is_dirty": true,
    "modified_count": 2,
    "untracked_count": 1,
    "staged_count": 1
  },
  "manifests": [ ... ],
  "context_files": [ ... ],
  "commits": [ ... ]
}
```

---

## Running Tests

Run all unit and integration tests:

```bash
cargo test
```
