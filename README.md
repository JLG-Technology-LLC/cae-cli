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

### 5. Candidate Wallet Lifecycle (`cae wallet`)

Manage persistent candidate non-custodial wallets on Base Sepolia (`eip155:84532`) and Base Mainnet (`eip155:8453`), track balances, compute tailoring quotas, and request testnet funding.

#### Initialize a Wallet

```bash
# Initialize with CDP User Auth (default, hybrid browser loopback or email OTP)
cae wallet init

# Initialize an offline, device-local non-custodial wallet
cae wallet init --local

# Initialize on Base Mainnet with overwrite confirmation
cae wallet init --local --network base --force

# Machine-readable output for coding agents
cae wallet init --local --json
```

#### Inspect Wallet Status and Address

```bash
# Display wallet metadata, network, and spend allowance
cae wallet show

# Emit only the 42-character checksummed address (ideal for shell piping)
cae wallet address

# JSON format
cae wallet show --json
```

#### Check Balance & Tailoring Quota

Queries native ETH and ERC-20 USDC balances and computes the candidate's tailoring quota ($0.03 USDC platform fee):

```bash
# Check current network balances
cae wallet balance

# Query Base Mainnet explicitly
cae wallet balance --network base

# Structured JSON for automation
cae wallet balance --json
```

#### Fund Wallet with Testnet Tokens (Base Sepolia)

```bash
# Request testnet USDC from CDP faucet (default)
cae wallet fund

# Request testnet ETH for manual gas top-ups
cae wallet fund --token eth

# Machine-readable distribution receipt
cae wallet fund --json
```

#### Network Switching

```bash
# Switch default active network to Base Mainnet
cae wallet set-network base

# Switch back to Base Sepolia testnet
cae wallet set-network base-sepolia
```

#### Daily Spend Limit Management

Protects candidate balances against unexpected run depletion with client-side spend allowances (matching Flutter `MandateService`, default `$0.15` USDC):

```bash
# View daily allowance and spend tracker
cae wallet limit

# Update daily allowance
cae wallet limit set 0.30
```

#### Key Export & Multi-Device Sync

```bash
# Export local private key (requires interactive [y/N] confirmation)
cae wallet export-key

# Skip confirmation prompt in automated scripts
cae wallet export-key --force

# Sync across devices via CDP User Auth
cae wallet login --email candidate@example.com

# De-authorize active CDP session
cae wallet logout
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

## Testing

Run all unit, integration, and E2E tests locally:

```bash
# Run unit and integration tests
cargo test

# Run the comprehensive E2E pipeline test suite
make e2e
# or: cargo test --test e2e_all_commands --all-features -- --nocapture
```

### Running Tests in Lightweight Test Container (Debian Trixie)

To run hermetic tests locally inside an isolated container with minimal disk footprint (using the official `rust:1-slim-trixie` image):

```bash
# Option 1: Using Makefile
make docker-e2e

# Option 2: Using Docker Compose
docker compose -f docker-compose.test.yml run --rm e2e

# Option 3: Direct Docker Run
docker build -f Dockerfile.test -t cae-cli:test-trixie .
docker run --rm \
  -v "$(pwd):/workspace:ro" \
  -v cae_cargo_target:/target \
  -v cae_cargo_cache:/usr/local/cargo/registry \
  -e CAE_SKIP_BROWSER_OPEN=1 \
  -e CARGO_TARGET_DIR=/target \
  -w /workspace \
  cae-cli:test-trixie \
  cargo test --test e2e_all_commands --all-features -- --nocapture
```
