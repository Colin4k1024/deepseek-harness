#!/bin/bash
# Generate unit test coverage report for dsh-rust.
#
# Prerequisites:
#   cargo install cargo-llvm-cov
#
# Usage:
#   ./scripts/coverage.sh           # Generate HTML report
#   ./scripts/coverage.sh --lcov    # Generate LCOV report (for CI)
#   ./scripts/coverage.sh --open    # Generate and open HTML report

set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "Error: cargo-llvm-cov is not installed."
    echo "Install it with: cargo install cargo-llvm-cov"
    exit 1
fi

# Clean previous coverage data
cargo llvm-cov clean --workspace

if [[ "${1:-}" == "--lcov" ]]; then
    echo "Generating LCOV coverage report..."
    cargo llvm-cov --workspace --lcov --output-path coverage/lcov.info \
        --ignore-filename-regex='.*/tests/.*' \
        --ignore-filename-regex='.*/test-support/.*'
    echo "LCOV report written to coverage/lcov.info"
elif [[ "${1:-}" == "--open" ]]; then
    echo "Generating and opening HTML coverage report..."
    cargo llvm-cov --workspace --open \
        --ignore-filename-regex='.*/tests/.*' \
        --ignore-filename-regex='.*/test-support/.*'
else
    echo "Generating HTML coverage report..."
    cargo llvm-cov --workspace --html \
        --ignore-filename-regex='.*/tests/.*' \
        --ignore-filename-regex='.*/test-support/.*'
    echo "HTML report written to target/llvm-cov/html/index.html"
fi

# Print summary
echo ""
echo "=== Coverage Summary ==="
cargo llvm-cov --workspace --no-run --ignore-filename-regex='.*/tests/.*' 2>/dev/null || true
