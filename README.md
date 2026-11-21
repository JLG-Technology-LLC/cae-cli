# Career Asset Engine CLI (`cae-cli`)

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/JLG-Technology-LLC/cae-cli/actions)
[![Language](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Proprietary-red.svg)](#license)
[![EVM Networks](https://img.shields.io/badge/networks-Base%20%7C%20Base%20Sepolia-blue.svg)](https://base.org)

Standalone Rust CLI tool and automation agent for tracking software projects, discovering ecosystem manifests, extracting agent context files, maintaining a durable local ledger, and managing candidate non-custodial EVM wallets for AI tailoring micropayments.

---

## Table of Contents

- [Features Overview](#features-overview)
- [Installation & Build](#installation--build)
- [Quick Start](#quick-start)
- [Complete CLI Command Reference](#complete-cli-command-reference)
  - [`cae track`](#1-cae-track)
  - [`cae status`](#2-cae-status)
  - [`cae record-commit`](#3-cae-record-commit)
  - [`cae untrack`](#4-cae-untrack)
  - [`cae dir`](#5-cae-dir)
  - [`cae wallet` (Candidate Identity & Quota)](#6-cae-wallet-candidate-identity--quota)
    - [`cae wallet init`](#cae-wallet-init)
    - [`cae wallet login` & `logout`](#cae-wallet-login--logout)
    - [`cae wallet show` & `address`](#cae-wallet-show--address)
    - [`cae wallet balance`](#cae-wallet-balance)
    - [`cae wallet fund`](#cae-wallet-fund)
    - [`cae wallet set-network`](#cae-wallet-set-network)
    - [`cae wallet limit`](#cae-wallet-limit)
    - [`cae wallet export-key`](#cae-wallet-export-key)
- [Process Exit Codes & Scripting Contract](#process-exit-codes--scripting-contract)
- [Storage & Ledger File Formats](#storage--ledger-file-formats)
- [Security & Static Analysis](#security--static-analysis)
- [Testing & Containerized Workflows](#testing--containerized-workflows)
- [Makefile Reference](#makefile-reference)
- [Troubleshooting & FAQs](#troubleshooting--faqs)
- [License](#license)

---

## Features Overview

* **Automatic Project Discovery (`cae track`)**: Recursively scans directories to detect software project roots, identifies supported package manifests, inspects Git repository state, extracts agent context files, and serializes a durable ledger record.
* **Manifest Parsing Engine (`src/manifest.rs`)**:
  * **Flutter / Dart**: `pubspec.yaml` (name, version, description, SDK constraints)
  * **Node.js / TypeScript**: `package.json` (name, version, scripts, dependencies)
  * **Python**: `pyproject.toml` (PEP 621 & Poetry metadata)
  * **Rust**: `Cargo.toml` (package name, version, edition, workspace members)
* **Agent Context Extraction (`src/context.rs`)**:
  * Automatically discovers and extracts `ARCHITECTURE`, `README`, `DESIGN`, and `MEMORY` markdown files across project roots and `.agents/`, `docs/`, `memory/` folders.
  * Computes file metrics: line counts, byte sizes, SHA-256 hashes, heading outlines, and content previews.
* **Git Repository Intelligence (`src/git.rs`)**:
  * Captures active branch, HEAD commit hash (full & short), remote origin URLs, dirty/clean status, and staged, modified, and untracked file counts.
* **Commit Ledger Recording (`cae record-commit`)**:
  * Appends commit metadata events (author, timestamp, commit message, changed files, optional developer/agent notes) to durable project ledgers.
* **Candidate Non-Custodial Wallet Lifecycle (`cae wallet`)**:
  * **EVM Web3 Identity**: Implements candidate wallet management on Base Mainnet (`eip155:8453`) and Base Sepolia (`eip155:84532`).
  * **CDP User Auth & Local EOA**: Supports both seamless Coinbase Developer Platform (CDP) User Auth (browser loopback or terminal OTP) and offline, device-local private key generation.
  * **Balance & Quota Computing**: Queries native ETH and ERC-20 USDC balances and calculates exact available resume/asset tailoring runs ($0.03 USDC per run).
  * **Testnet Faucet Funding**: Requests testnet USDC and ETH directly from the CDP faucet on Base Sepolia, with strict guardrails preventing accidental mainnet faucet requests.
  * **Daily Spending Mandates**: Client-side spending cap tracking matching the Flutter client `MandateService` ($0.15 USDC default daily limit).
  * **Strict Permissions**: Enforces POSIX `0600` file permissions on `~/.cae/wallet.json` and creates atomic `.bak` backups on rotation.
* **Hermetic Container Testing & Quality Gates**:
  * Official Debian Trixie test container (`rust:1-slim-trixie`) with isolated volumes and zero host pollution.
  * Pre-commit hooks orchestrating `cargo fmt`, `cargo clippy`, `cargo test`, `Semgrep` Rust SAST, and `Trivy` filesystem vulnerability scans.

---

## Installation & Build

### Prerequisites

* Rust 1.75+ (with `cargo` and `rustfmt`)
* Git 2.30+
* (Optional) Docker for containerized test execution
* (Optional) `pre-commit`, `semgrep`, and `trivy` for local security scans

### Build from Source

```bash
# Clone the repository
git clone https://github.com/JLG-Technology-LLC/cae.git
cd cae/cae-cli

# Build release binary
cargo build --release

# Binary is available at target/release/cae
./target/release/cae --version
```

### Install Globally

Install `cae` into your Cargo binary path (`~/.cargo/bin`):

```bash
cargo install --path .
```

Verify global availability:

```bash
cae --help
```

---

## Quick Start

```bash
# 1. Track the current project
cae track . --tag ai --tag flutter

# 2. Inspect project status
cae status

# 3. Initialize candidate tailoring wallet on Base Sepolia
cae wallet init --local

# 4. Check wallet address & balance
cae wallet address
cae wallet balance

# 5. Fund testnet wallet with USDC
cae wallet fund

# 6. Record a commit after making changes
cae record-commit --note "Implemented core candidate tailoring feature"
```

---

## Complete CLI Command Reference

### 1. `cae track`

Scans and registers a directory or repository into the local CAE ledger (`~/.cae/projects/<id>.json`).

```bash
cae track [PATH] [OPTIONS]
```

#### Arguments & Options

| Parameter | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `PATH` | Positional | `.` | Directory path to track |
| `--id <ID>` | Option | Derived | Custom unique identifier for the project ledger |
| `--name <NAME>` | Option | Manifest name | Custom human-readable project display name |
| `-t, --tag <TAG>` | Option (Repeatable) | `[]` | Categorical tags (e.g. `--tag rust --tag cli`) |
| `--notes <NOTES>` | Option | `None` | Freeform text notes describing the project |

#### Examples

```bash
# Track current directory
cae track .

# Track a specific path with custom ID and tags
cae track ../fastapi_agent --id backend-gateway --tag api --tag python --notes "Staging gateway"

# Track frontend client
cae track ../AICareerAssetEngine --name "Career Asset Engine App" --tag flutter --tag desktop
```

---

### 2. `cae status`

Inspects tracked projects, outputs formatted summaries, or returns raw JSON for automated consumption.

```bash
cae status [TARGET] [OPTIONS]
```

#### Options

| Flag | Description |
| :--- | :--- |
| `[TARGET]` | Specific project ID or directory path to inspect (defaults to current directory if omitted) |
| `-a, --all` | List all registered projects in a formatted ASCII table |
| `--json` | Output full project ledger in raw, machine-readable JSON |
| `--dir` | Print the ledger storage directory path (`~/.cae/projects`) |

#### Examples

```bash
# Inspect current project
cae status

# Inspect specific project by ID
cae status career-asset-engine

# List all tracked projects
cae status --all

# Machine-readable output for scripts and AI agents
cae status career-asset-engine --json
```

---

### 3. `cae record-commit`

Captures Git commit history and appends a structured event to the project's commit history ledger.

```bash
cae record-commit [OPTIONS]
```

#### Options

| Flag | Description |
| :--- | :--- |
| `-p, --project <ID>` | Target project ID or path (defaults to auto-detecting current directory) |
| `-m, --message <MSG>` | Commit message override (defaults to current Git HEAD commit message) |
| `-n, --note <NOTE>` | Supplementary developer, agent, or milestone note |
| `-c, --commit <HASH>` | Specific commit SHA or reference to record (defaults to `HEAD`) |

#### Examples

```bash
# Record current HEAD commit
cae record-commit

# Record with an engineering note
cae record-commit --note "Completed unit testing for CDP wallet module"

# Record historical commit for a specific project
cae record-commit --project backend-gateway --commit a1b2c3d --note "Hotfix staging deployment"
```

---

### 4. `cae untrack`

Removes a project from the local ledger directory.

```bash
cae untrack <TARGET>
```

#### Examples

```bash
# Untrack by project ID
cae untrack career-asset-engine

# Untrack by directory path
cae untrack ../fastapi_agent
```

---

### 5. `cae dir`

Prints the absolute path to the local project ledger storage directory:

```bash
cae dir
# Output: /Users/<user>/.cae/projects
```

---

### 6. `cae wallet` (Candidate Identity & Quota)

The `cae wallet` subcommands manage the candidate's non-custodial EVM wallet, testnet faucet tokens, spending mandates, and tailoring run allowances.

#### `cae wallet init`

Initializes a candidate wallet. By default, launches Coinbase Developer Platform (CDP) User Auth via browser loopback. Supports local offline key generation.

```bash
cae wallet init [OPTIONS]
```

* `--local`: Generate an offline, device-local non-custodial EOA wallet (secp256k1) stored with 0600 permissions in `~/.cae/wallet.json`.
* `--network <NETWORK>`: Target default network (`base-sepolia` [default] or `base`).
* `--no-browser`: Use terminal email OTP prompt instead of launching a local web browser.
* `--force`: Overwrite existing wallet (backs up current wallet to `~/.cae/wallet.json.bak`).
* `--json`: Emit JSON-formatted creation receipt.

```bash
# Recommended: Local development wallet on Base Sepolia
cae wallet init --local

# Production mainnet wallet with force overwrite
cae wallet init --local --network base --force

# CDP User Auth via terminal OTP
cae wallet init --no-browser
```

#### `cae wallet login` & `logout`

Synchronize or de-authorize CDP User Auth sessions across development devices:

```bash
# Authenticate with existing CDP account
cae wallet login --email candidate@example.com

# Non-browser login via OTP
cae wallet login --email candidate@example.com --no-browser

# De-authorize active session
cae wallet logout
```

#### `cae wallet show` & `address`

Inspect wallet metadata or extract the 42-character checksummed address:

```bash
# Display full wallet status, network, and spend allowance
cae wallet show

# Emit only the wallet address (ideal for shell piping and scripting)
cae wallet address
# Output: 0x930EBa94562E010050d253D9E46d94cEE66Fe660

# Structured JSON output
cae wallet show --json
```

#### `cae wallet balance`

Queries live RPC nodes for native ETH and ERC-20 USDC balances and computes the candidate's tailoring quota ($0.03 USDC platform fee):

```bash
# Check balances on active network
cae wallet balance

# Query Base Mainnet explicitly
cae wallet balance --network base

# Automation output
cae wallet balance --json
```

**Sample JSON Output:**
```json
{
  "address": "0x4B20993Bc481177ec7E8f571ceCaE8A9e22C02db",
  "network": "base-sepolia",
  "eth_balance": "0.015000000000000000",
  "usdc_balance": "1.500000",
  "tailoring_runs_available": 50,
  "cost_per_run_usdc": "0.030000"
}
```

#### `cae wallet fund`

Requests testnet tokens from the Coinbase Developer Platform faucet on Base Sepolia:

```bash
# Request testnet USDC (default)
cae wallet fund

# Request testnet ETH for gas fees
cae wallet fund --token eth

# Machine-readable distribution receipt
cae wallet fund --json
```

> **Security Note:** `cae wallet fund` strictly rejects faucet requests when the active network is Base Mainnet (exits with code 5).

#### `cae wallet set-network`

Switches the default active network for payments and queries:

```bash
# Switch to Base Mainnet
cae wallet set-network base

# Switch to Base Sepolia testnet
cae wallet set-network base-sepolia
```

#### `cae wallet limit`

Manages client-side daily spending limits (matching the Flutter `MandateService`, default `$0.15` USDC per day):

```bash
# View current daily spend limit and consumed allowance
cae wallet limit
# or: cae wallet limit get

# Update daily allowance to $0.30 USDC
cae wallet limit set 0.30

# JSON output
cae wallet limit get --json
```

#### `cae wallet export-key`

Exports the hex-encoded private key of a local-eoa wallet for import into external tools (e.g. MetaMask, Coinbase Wallet):

```bash
# Requires interactive [y/N] confirmation
cae wallet export-key

# Automated scripts (bypasses prompt)
cae wallet export-key --force
```

---

## Process Exit Codes & Scripting Contract

`cae-cli` implements a deterministic exit code contract for headless CI pipelines and AI agents:

| Exit Code | Error Type | Cause / Recommended Agent Action |
| :---: | :--- | :--- |
| `0` | **Success** | Command completed successfully. |
| `1` | **General Error** | I/O error, network timeout, or invalid argument. Check stderr. |
| `2` | **SpendLimitExceeded** | Daily candidate spend allowance reached. Adjust limit with `cae wallet limit set <amount>`. |
| `3` | **NoWallet** | Operation requires a wallet but none is initialized. Run `cae wallet init`. |
| `5` | **Refused** | Operation refused on active network (e.g. requesting faucet funds on Base Mainnet). |

---

## Storage & Ledger File Formats

### 1. Project Ledger (`~/.cae/projects/<id>.json`)

```json
{
  "id": "career-asset-engine",
  "name": "career_asset_engine",
  "canonical_path": "/Users/developer/AICareerAssetEngine",
  "tracked_at": "2026-11-12T20:00:00.000000+00:00",
  "updated_at": "2026-11-12T22:30:00.000000+00:00",
  "ecosystems": ["Flutter/Dart"],
  "git": {
    "is_git_repo": true,
    "root_path": "/Users/developer/AICareerAssetEngine",
    "current_branch": "dev",
    "head_commit": "8997091339fc558c33c4dab746ac71146e74a6c5",
    "head_short_commit": "8997091",
    "is_dirty": false,
    "modified_count": 0,
    "untracked_count": 0,
    "staged_count": 0
  },
  "manifests": [
    {
      "ecosystem": "Flutter/Dart",
      "path": "pubspec.yaml",
      "name": "career_asset_engine",
      "version": "1.0.0-beta.5+6",
      "description": "Your Job Seeker Assistant."
    }
  ],
  "context_files": [
    {
      "category": "Architecture",
      "path": "ARCHITECTURE.md",
      "line_count": 420,
      "byte_size": 18450,
      "sha256": "4b20993bc481177ec7e8f571cecae8a9e22c02db...",
      "headings": ["# Architecture Overview", "## System Topology"]
    }
  ],
  "commits": []
}
```

### 2. Candidate Wallet Profile (`~/.cae/wallet.json`)

Stored strictly with POSIX permissions `0600` (`-rw-------`):

```json
{
  "wallet_type": "local-eoa",
  "evm_address": "0x4B20993Bc481177ec7E8f571ceCaE8A9e22C02db",
  "network": "base-sepolia",
  "created_at": "2026-11-12T21:00:00.000000+00:00",
  "daily_limit_usdc": 0.15,
  "spent_today_usdc": 0.03,
  "last_spend_date": "2026-11-12"
}
```

---

## Security & Static Analysis

Automated quality gates and vulnerability analysis are integrated into the repository:

### 1. Semgrep SAST Scanning

Scans Rust source files against automated static analysis rulesets:

```bash
make semgrep-scan
```

*(Runs `semgrep scan --config auto --severity=WARNING --error`, automatically falling back to Docker if `semgrep` is not locally installed).*

### 2. Trivy Filesystem Scanning

Audits dependencies, lockfiles, and project configurations for known vulnerabilities and secrets:

```bash
make trivy-scan
```

*(Scans for CRITICAL and HIGH severity issues with `--ignore-unfixed` and exit code 1).*

### 3. Unified Security Gate

Runs both Semgrep SAST and Trivy filesystem scans in sequence:

```bash
make security
```

### 4. Git Pre-Commit Hooks

Enforces formatting, lints, tests, Semgrep SAST, and Trivy scans on every commit:

```bash
# Install git hooks
pre-commit install

# Manually trigger across all files
pre-commit run --all-files
```

---

## Testing & Containerized Workflows

### Run Local Unit & Integration Tests

```bash
# Run unit and integration tests
cargo test

# Run the complete E2E pipeline test suite
make e2e
# or: cargo test --test e2e_all_commands --all-features -- --nocapture
```

### Hermetic Debian Trixie Test Container

Run hermetic tests in an isolated, minimal-footprint container using official `rust:1-slim-trixie`:

```bash
# Option 1: Using Makefile
make docker-e2e

# Option 2: Using Docker Compose
docker compose -f docker-compose.test.yml run --rm e2e

# Option 3: Direct Docker execution
make docker-build-test
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

---

## Makefile Reference

| Target | Description |
| :--- | :--- |
| `make help` | Display available targets and descriptions |
| `make format` | Check Rust source formatting with `rustfmt` |
| `make lint` | Run Clippy lints denying compiler warnings |
| `make test` | Run format checks, Clippy lints, and unit tests |
| `make e2e` | Run comprehensive end-to-end pipeline test suite locally |
| `make docker-build-test` | Build the lightweight `rust:1-slim-trixie` test container image |
| `make docker-e2e` | Run the full E2E test suite inside Debian Trixie container |
| `make semgrep-scan` | Run Semgrep Rust SAST static analysis |
| `make trivy-scan` | Run Trivy filesystem vulnerability and secret scan |
| `make security` | Run both Semgrep SAST and Trivy filesystem scans |
| `make clean` | Clean Cargo target directory and build cache |

---

## Troubleshooting & FAQs

### Q: Why does `cae wallet fund` fail with exit code 5?
**A:** You are on Base Mainnet (`base`). Faucet funding is only supported on test networks (`base-sepolia`). Switch networks using `cae wallet set-network base-sepolia`.

### Q: How do I run tests headlessly in CI without browser prompts?
**A:** Set the environment variable `CAE_SKIP_BROWSER_OPEN=1`. This instructs `cae` to fall back to headless terminal flows.

### Q: How do I back up or restore my wallet?
**A:** Your wallet profile lives at `~/.cae/wallet.json`. When `cae wallet init --force` runs, an automatic backup is created at `~/.cae/wallet.json.bak`. You can restore your profile by copying `wallet.json.bak` back to `wallet.json`.

---

*Last reviewed: November 2026*

## License

Proprietary and confidential. Copyright © 2026 JLG Technology LLC. All rights reserved.
