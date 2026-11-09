.PHONY: help format lint test trivy-scan clean

help:
	@echo "Available commands:"
	@echo "  make format     - Check Rust formatting with rustfmt"
	@echo "  make lint       - Run Clippy lints with warnings denied"
	@echo "  make test       - Run hermetic unit and integration tests"
	@echo "  make trivy-scan - Run Trivy filesystem scan (vuln, secret, misconfig)"
	@echo "  make clean      - Clean Cargo target directory"

format:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test: format lint
	cargo test --all-targets --all-features

trivy-scan:
	@echo "Running Trivy filesystem security scan..."
	@if command -v trivy >/dev/null 2>&1; then \
		trivy fs . --scanners vuln,secret,misconfig --severity CRITICAL,HIGH --ignore-unfixed --skip-dirs target --exit-code 1; \
	else \
		docker run --rm -v "$(PWD):/workspace" -w /workspace aquasec/trivy:latest fs . --scanners vuln,secret,misconfig --severity CRITICAL,HIGH --ignore-unfixed --skip-dirs target --exit-code 1; \
	fi

clean:
	cargo clean
