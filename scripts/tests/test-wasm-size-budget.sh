#!/usr/bin/env bash
# =============================================================================
# Kora Protocol — Test Harness for WASM Size Budget Checker (#795)
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

TEST_DIR=$(mktemp -d)
trap 'rm -rf "$TEST_DIR"' EXIT

echo "=== Running WASM Size Budget Checker Tests ==="

BUDGET_FILE="$TEST_DIR/budgets.json"
WASM_DIR="$TEST_DIR/wasm"
mkdir -p "$WASM_DIR"

cat << 'EOF' > "$BUDGET_FILE"
{
  "budgets": {
    "access_control": 500,
    "invoice_nft": 1000
  },
  "default_budget_bytes": 500
}
EOF

# Test 1: File under budget passes
echo "Test 1: File under budget..."
head -c 300 /dev/zero > "$WASM_DIR/kora_access_control.wasm"
if bash "$ROOT_DIR/scripts/check-wasm-size.sh" "$BUDGET_FILE" "$WASM_DIR" "" > "$TEST_DIR/out1.log"; then
  echo "✓ Test 1 passed: file under budget accepted"
else
  echo "❌ Test 1 failed"
  cat "$TEST_DIR/out1.log"
  exit 1
fi

# Test 2: File over budget fails without approved label
echo "Test 2: File over budget without label..."
head -c 800 /dev/zero > "$WASM_DIR/kora_access_control.wasm"
if bash "$ROOT_DIR/scripts/check-wasm-size.sh" "$BUDGET_FILE" "$WASM_DIR" "" > "$TEST_DIR/out2.log" 2>&1; then
  echo "❌ Test 2 failed: expected failure on over-budget WASM"
  exit 1
else
  echo "✓ Test 2 passed: over-budget rejected without label"
fi

# Test 3: File over budget passes with approved label override
echo "Test 3: File over budget with wasm-size-increase-approved label..."
if bash "$ROOT_DIR/scripts/check-wasm-size.sh" "$BUDGET_FILE" "$WASM_DIR" "wasm-size-increase-approved documentation" > "$TEST_DIR/out3.log"; then
  echo "✓ Test 3 passed: over-budget accepted with approval label"
else
  echo "❌ Test 3 failed"
  cat "$TEST_DIR/out3.log"
  exit 1
fi

# Dispute escrow must use its explicit budget, including optimized precedence.
head -c 300 /dev/zero > "$WASM_DIR/kora_access_control.wasm"
cat << 'EOF' > "$BUDGET_FILE"
{"budgets":{"access_control":500,"dispute_escrow":600},"default_budget_bytes":1000}
EOF
head -c 601 /dev/zero > "$WASM_DIR/kora_dispute_escrow.wasm"
if bash "$ROOT_DIR/scripts/check-wasm-size.sh" "$BUDGET_FILE" "$WASM_DIR" "" > "$TEST_DIR/escrow.log" 2>&1; then
  echo "ERROR: over-budget dispute escrow accepted"
  exit 1
fi
head -c 600 /dev/zero > "$WASM_DIR/kora_dispute_escrow.optimized.wasm"
bash "$ROOT_DIR/scripts/check-wasm-size.sh" "$BUDGET_FILE" "$WASM_DIR" "" > "$TEST_DIR/escrow-optimized.log"

# A missing required artifact must fail even with a size-approval label.
if REQUIRE_ALL_WASMS=1 bash "$ROOT_DIR/scripts/check-wasm-size.sh" "$BUDGET_FILE" "$WASM_DIR" "wasm-size-increase-approved" > "$TEST_DIR/missing.log" 2>&1; then
  echo "ERROR: missing required release artifacts accepted"
  exit 1
fi

echo "All WASM size budget tests PASSED!"
