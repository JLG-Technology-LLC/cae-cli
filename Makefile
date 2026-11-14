.PHONY: help format lint test e2e docker-build-test docker-e2e semgrep-scan trivy-scan security clean

help:
	@echo "Available commands:"
	@echo "  make format            - Check Rust formatting with rustfmt"
	@echo "  make lint              - Run Clippy lints with warnings denied"
	@echo "  make test              - Run hermetic unit and integration tests"
	@echo "  make e2e               - Run comprehensive end-to-end pipeline test suite locally"
	@echo "  make docker-build-test - Build the lightweight trixie test container image"
	@echo "  make docker-e2e        - Run the E2E test suite inside trixie container"
	@echo "  make semgrep-scan      - Run Semgrep Rust SAST static analysis"
	@echo "  make trivy-scan        - Run Trivy filesystem scan (vuln, secret, misconfig)"
	@echo "  make security          - Run both Semgrep SAST and Trivy filesystem scans"
	@echo "  make clean             - Clean Cargo target directory"

format:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test: format lint
	cargo test --all-targets --all-features

e2e:
	cargo test --test e2e_all_commands --all-features -- --nocapture

docker-build-test:
	docker build -f Dockerfile.test -t cae-cli:test-trixie .

docker-e2e: docker-build-test
	docker run --rm \
		-v "$(CURDIR):/workspace:ro" \
		-v cae_cargo_target:/target \
		-v cae_cargo_cache:/usr/local/cargo/registry \
		-e CAE_SKIP_BROWSER_OPEN=1 \
		-e CARGO_TARGET_DIR=/target \
		-w /workspace \
		cae-cli:test-trixie \
		cargo test --test e2e_all_commands --all-features -- --nocapture

semgrep-scan:
	@echo "Running Semgrep SAST security scan for Rust..."
	@if command -v semgrep >/dev/null 2>&1; then \
		semgrep scan --config auto --severity=WARNING --error; \
	else \
		docker run --rm -v "$(CURDIR):/src" semgrep/semgrep:1.98.0 semgrep scan --config auto --severity=WARNING --error; \
	fi

trivy-scan:
	@echo "Running Trivy filesystem security scan..."
	@if command -v trivy >/dev/null 2>&1; then \
		trivy fs . --scanners vuln,secret,misconfig --severity CRITICAL,HIGH --ignore-unfixed --skip-dirs target --exit-code 1; \
	else \
		docker run --rm -v "$(CURDIR):/workspace" -w /workspace aquasec/trivy:0.58.2 fs . --scanners vuln,secret,misconfig --severity CRITICAL,HIGH --ignore-unfixed --skip-dirs target --exit-code 1; \
	fi

security: semgrep-scan trivy-scan

clean:
	cargo clean
