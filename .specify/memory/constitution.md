<!--
Sync Impact Report
- Version change: 1.0.0 → 1.1.0 (MINOR, 2026-11-11): Principle I relaxed from "strictly offline"
  to "no telemetry / no cloud sync of ledger data", permitting explicit user-initiated network
  calls from `cae wallet` (spec 005-cdp-wallet-cli). Rationale: balance, faucet, and CDP auth are
  inherently networked; project-tracking commands stay offline.
- Previous change: [CONSTITUTION_VERSION] (uninitialized template) → 1.0.0
- Modified principles:
  - [PRINCIPLE_1_NAME] → I. Local-First Ledger Integrity & Atomic Persistence
  - [PRINCIPLE_2_NAME] → II. Deterministic Manifest & Agent Context Extraction
  - [PRINCIPLE_3_NAME] → III. Read-Only Git Introspection & Zero Repository Mutation
  - [PRINCIPLE_4_NAME] → IV. Hermetic Tempdir Testing (NON-NEGOTIABLE)
  - [PRINCIPLE_5_NAME] → V. Idiomatic Rust, Dual Output Contract & Zero-Warning Quality Gates
- Added sections:
  - Technology Stack & Operational Constraints
  - Development Workflow & Quality Gates
  - Governance (populated from template placeholders)
- Removed sections: None
- Templates requiring updates:
  - ✅ .specify/templates/plan-template.md (verified compatible with Constitution Check gates)
  - ✅ .specify/templates/spec-template.md (verified compatible)
  - ✅ .specify/templates/tasks-template.md (verified compatible)
- Follow-up TODOs: None
-->
# Career Asset Engine CLI (`cae-cli`) Constitution

## Core Principles

### I. Local-First Ledger Integrity & Atomic Persistence
All tracked project metadata, manifest snapshots, extracted agent context summaries, and commit
histories MUST be persisted locally under `~/.cae/projects/<id>.json` (`src/ledger.rs`). `cae-cli`
MUST NOT emit outbound network telemetry, synchronize ledger or project data to cloud storage, or
exfiltrate secrets. Project-tracking commands (`track`, `untrack`, `status`, `record-commit`) MUST
remain fully offline. The `cae wallet` command group MAY make outbound requests ONLY when the user
explicitly invokes a command whose purpose requires it (public JSON-RPC balance reads, testnet
faucet requests, CDP authentication); such requests MUST NOT carry ledger/project data, MUST NOT
transmit private keys or seeds, and MUST NOT run implicitly in the background. Ledger schemas MUST remain backward-compatible with `AICareerAssetEngine`
(`SPEC-031` project ingestion contracts) so the Flutter client and coding agents can reliably read
`~/.cae/projects/`.

**Rationale**: Candidate engineering portfolios and local repository metadata are sensitive local
assets that underpin evidence-grounded resume tailoring across the CAE ecosystem.

### II. Deterministic Manifest & Agent Context Extraction
Ecosystem manifest parsers (`src/manifest.rs` covering `pubspec.yaml`, `package.json`,
`pyproject.toml`, and `Cargo.toml`) and agent context extractors (`src/context.rs` covering
`ARCHITECTURE`, `README`, `DESIGN`, and `MEMORY` documents) MUST produce deterministic, reproducible
outputs, including accurate SHA-256 content hashes, byte sizes, line counts, and Markdown section
headings. Directory traversal (`walkdir`) MUST skip heavy build/dependency directories (`target/`,
`node_modules/`, `.git/`, `.dart_tool/`, `build/`, `.venv/`) and enforce bounded file-size limits.

**Rationale**: Deterministic hashes and fast directory scanning prevent redundant re-indexing and
protect CLI responsiveness on large monorepos.

### III. Read-Only Git Introspection & Zero Repository Mutation
When inspecting tracked repositories (`src/git.rs`), `cae-cli` MUST execute strictly read-only Git
queries (branch detection, HEAD SHA resolution, remote origin lookup, working-tree status counts,
and commit log extraction). `cae-cli` MUST NEVER mutate a tracked project's working tree, staging
area, Git config, or commit history during `track`, `status`, `record-commit`, or `untrack`
operations.

**Rationale**: Developers and autonomous agents run `cae` alongside active coding sessions; the
tracker must be a zero-side-effect observer of target repositories.

### IV. Hermetic Tempdir Testing (NON-NEGOTIABLE)
Every new subcommand, manifest parser, context extractor, or ledger mutation MUST be accompanied by
automated unit and integration tests (`src/` and `tests/`). Tests MUST be completely hermetic,
executing against isolated temporary directories and overriding `CAE_HOME` (or passing explicit
ledger paths) so test runs NEVER read from or write to the developer's live `~/.cae/` directory.

**Rationale**: Hermetic tests ensure deterministic CI runs and protect local developer ledgers from
corruption during `cargo test`.

### V. Idiomatic Rust, Dual Output Contract & Zero-Warning Quality Gates
All CLI commands (`src/main.rs`, `src/models.rs`) MUST support both human-readable terminal output
(`colored`) and machine-parseable `--json` (`serde_json`) output for agent and pipeline consumption,
with operational errors written to `stderr` via `anyhow::Result` and non-zero exit codes. Production
code paths MUST avoid `.unwrap()` and `.expect()` on fallible I/O or user input, preferring explicit
`anyhow::Context` propagation. All code MUST pass `cargo fmt --check` and
`cargo clippy --all-targets -- -D warnings` with zero warnings.

**Rationale**: Dual-mode CLI output enables seamless integration with both human developers and
Spec Kit / Antigravity coding agents while Rust's type system enforces runtime reliability.

## Technology Stack & Operational Constraints

- **Language & Toolchain**: Rust (2021 edition, compatible with stable Rust `>=1.75`), managed via
  `Cargo.toml` and `Cargo.lock`.
- **Core Crates**: `clap` (v4 derive CLI parsing), `serde` / `serde_json` / `serde_yaml` / `toml`
  (manifest and ledger serialization), `sha2` + `hex` (content hashing), `walkdir` (bounded
  filesystem traversal), `chrono` (UTC ISO-8601 timestamps), and `anyhow` (structured error context).
- **Storage Boundary**: Local filesystem ledger at `~/.cae/projects/<id>.json` (configurable via
  environment/path overrides for hermetic tests).

## Development Workflow & Quality Gates

1. **Spec-Driven Development (SDD)**: Substantial features follow the Spec Kit Agentic SDLC
   (`assess` → `specify` → `plan` → `tasks` → `implement` → `converge`), while bounded fixes use
   either the `bugfix` workflow (`speckit-bug-assess` → `fix` → `test`) or standard issue/PR review.
2. **Formatting & Linting Gate**: All changes MUST pass `cargo fmt --all -- --check` and
   `cargo clippy --all-targets --all-features -- -D warnings`.
3. **Automated Test Gate**: All unit and integration tests MUST pass via `cargo test --all-targets`.
4. **Code Review Boundary**: All changes must comply with `.opencodereview/rule.json` and preserve
   JSON ledger compatibility with `AICareerAssetEngine`.

## Governance

This Constitution is the architectural authority for `cae-cli`. Every specification
(`/speckit-specify`), implementation plan (`/speckit-plan`), task breakdown (`/speckit-tasks`),
and pull request review MUST verify compliance with Core Principles I–V.

- **Amendment Procedure**: Amendments require a documented rationale, an updated Sync Impact Report,
  and a Semantic Version bump in `.specify/memory/constitution.md`.
- **Versioning Policy**: Follows Semantic Versioning (`MAJOR.MINOR.PATCH`):
  - `MAJOR`: Backward-incompatible ledger schema changes or removal of a Core Principle.
  - `MINOR`: New Core Principle or expanded ecosystem support requirements.
  - `PATCH`: Clarifications, formatting refinements, or non-semantic updates.
- **Compliance Review**: Any PR violating local-first privacy, read-only Git introspection, or
  hermetic test isolation MUST be rejected until remediated.

**Version**: 1.1.0 | **Ratified**: 2026-11-09 | **Last Amended**: 2026-11-11
