#!/bin/bash
# Security audit for dsh-rust.
#
# Checks:
# 1. cargo-deny (license, advisories, sources)
# 2. cargo-audit (RustSec advisory database)
# 3. Dependency tree analysis
# 4. Unsafe code detection
#
# Prerequisites:
#   cargo install cargo-deny cargo-audit
#
# Usage:
#   ./scripts/security-audit.sh

set -euo pipefail
cd "$(dirname "$0")/.."

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

PASS=0
FAIL=0
WARN=0

check() {
    local name="$1"
    shift
    echo -e "\n${YELLOW}=== $name ===${NC}"
    if "$@"; then
        echo -e "${GREEN}PASS${NC}: $name"
        ((PASS++))
    else
        echo -e "${RED}FAIL${NC}: $name"
        ((FAIL++))
    fi
}

warn_check() {
    local name="$1"
    shift
    echo -e "\n${YELLOW}=== $name ===${NC}"
    if "$@"; then
        echo -e "${GREEN}PASS${NC}: $name"
        ((PASS++))
    else
        echo -e "${YELLOW}WARN${NC}: $name (non-blocking)"
        ((WARN++))
    fi
}

# 1. cargo-deny
echo "Running security audit..."

check "cargo-deny (advisories)" cargo deny check advisories
check "cargo-deny (licenses)" cargo deny check licenses
check "cargo-deny (sources)" cargo deny check sources

# 2. cargo-audit
if command -v cargo-audit &> /dev/null; then
    check "cargo-audit" cargo audit
else
    echo -e "${YELLOW}SKIP${NC}: cargo-audit not installed (cargo install cargo-audit)"
fi

# 3. Unsafe code detection
echo -e "\n${YELLOW}=== Unsafe Code Detection ===${NC}"
UNSAFE_COUNT=$(grep -r "unsafe" --include="*.rs" crates/ apps/ | grep -v "// SAFETY" | grep -v "deny(unsafe" | wc -l || true)
if [ "$UNSAFE_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}WARN${NC}: Found $UNSAFE_COUNT unsafe blocks (review recommended)"
    grep -rn "unsafe" --include="*.rs" crates/ apps/ | grep -v "// SAFETY" | grep -v "deny(unsafe" | head -20
    ((WARN++))
else
    echo -e "${GREEN}PASS${NC}: No unsafe code found"
    ((PASS++))
fi

# 4. Dependency count
echo -e "\n${YELLOW}=== Dependency Analysis ===${NC}"
DEP_COUNT=$(cargo metadata --format-version 1 2>/dev/null | python3 -c "import sys,json; print(len(json.load(sys.stdin)['packages']))" 2>/dev/null || echo "unknown")
echo "Total dependencies: $DEP_COUNT"

# 5. Check for known vulnerable patterns
echo -e "\n${YELLOW}=== Pattern Checks ===${NC}"

# Check for unwrap() in non-test code
UNWRAP_COUNT=$(grep -r "\.unwrap()" --include="*.rs" crates/ apps/src/ | grep -v "#[cfg(test)]" | grep -v "#[test]" | wc -l || true)
if [ "$UNWRAP_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}WARN${NC}: Found $UNWRAP_COUNT unwrap() calls in non-test code"
    ((WARN++))
else
    echo -e "${GREEN}PASS${NC}: No unwrap() in non-test code"
    ((PASS++))
fi

# Check for expect() in non-test code
EXPECT_COUNT=$(grep -r "\.expect(" --include="*.rs" crates/ apps/src/ | grep -v "#[cfg(test)]" | grep -v "#[test]" | wc -l || true)
if [ "$EXPECT_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}WARN${NC}: Found $EXPECT_COUNT expect() calls in non-test code"
    ((WARN++))
else
    echo -e "${GREEN}PASS${NC}: No expect() in non-test code"
    ((PASS++))
fi

# Summary
echo -e "\n${YELLOW}=== Summary ===${NC}"
echo -e "Passed: ${GREEN}$PASS${NC}"
echo -e "Failed: ${RED}$FAIL${NC}"
echo -e "Warnings: ${YELLOW}$WARN${NC}"

if [ "$FAIL" -gt 0 ]; then
    echo -e "\n${RED}Security audit FAILED${NC}"
    exit 1
else
    echo -e "\n${GREEN}Security audit PASSED${NC}"
    exit 0
fi
