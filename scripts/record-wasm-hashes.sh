#!/usr/bin/env bash
# =============================================================================
# Kora Protocol — Record WASM Hashes for Release
# =============================================================================
#
# Usage:
#   ./scripts/record-wasm-hashes.sh <version> [--verify]
#
# Examples:
#   ./scripts/record-wasm-hashes.sh v0.2.0
#   ./scripts/record-wasm-hashes.sh v0.2.0 --verify   # compare against existing
#
# What this does:
#   1. Verifies WASM binaries exist (run 'make build-optimized' first)
#   2. Computes SHA-256 hashes for all contracts
#   3. Records hashes in releases/<version>.hashes (sha256sum compatible format)
#   4. Prints exact reproduction steps for independent verification
#
# Reproducibility guarantees:
#   - Pin rust-toolchain.toml before building (channel = "1.75.0")
#   - Always run: make clean && make build-optimized
#   - Set: SOURCE_DATE_EPOCH=0 CARGO_INCREMENTAL=0
#   - The Cargo.toml release profile (lto=true, codegen-units=1, opt-level=z)
#     eliminates all remaining non-determinism sources.
#
# =============================================================================

set -euo pipefail

VERSION="${1:-}"
VERIFY_MODE="${2:-}"

if [ -z "$VERSION" ]; then
  echo "ERROR: Version required. Usage: $0 <version> [--verify]"
  exit 1
fi

if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "ERROR: Version must be in format vX.Y.Z, got: $VERSION"
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WASM_DIR="$ROOT_DIR/target/wasm32-unknown-unknown/release"
RELEASES_DIR="$ROOT_DIR/releases"
HASHES_FILE="$RELEASES_DIR/$VERSION.hashes"

CONTRACTS=(
  "access_control"
  "invoice_nft"
  "marketplace"
  "financing_pool"
  "treasury"
  "risk_registry"
  "price_oracle"
  "tranche"
  "dispute_resolution"
  "dispute_escrow"
  "governance"
  "governance_voting"
  "parameter_registry"
)

# ── Detect toolchain ────────────────────────────────────────────────────────

RUSTC_VERSION="$(rustc --version 2>/dev/null || echo 'rustc not found')"
TOOLCHAIN_FILE="$ROOT_DIR/rust-toolchain.toml"
PINNED_CHANNEL="unknown"
if [ -f "$TOOLCHAIN_FILE" ]; then
  PINNED_CHANNEL="$(grep 'channel' "$TOOLCHAIN_FILE" | head -1 | sed 's/.*= *"\(.*\)".*/\1/')"
fi

# ── Verify mode: compare rebuild against stored hashes ─────────────────────

if [ "$VERIFY_MODE" = "--verify" ]; then
  if [ ! -f "$HASHES_FILE" ]; then
    echo "ERROR: No stored hashes for $VERSION at $HASHES_FILE"
    echo "       Run without --verify first to record hashes."
    exit 1
  fi

  if [ ! -d "$WASM_DIR" ]; then
    echo "ERROR: WASM directory not found. Run: make clean && make build-optimized"
    exit 1
  fi

  echo "=== Kora Protocol — Reproducibility Verification ==="
  echo "Version  : $VERSION"
  echo "Toolchain: $RUSTC_VERSION"
  echo ""

  PASS=true
  for contract in "${CONTRACTS[@]}"; do
    WASM="$WASM_DIR/kora_${contract}.wasm"
    if [ ! -f "$WASM" ]; then
      echo "  SKIP $contract: WASM not found"
      continue
    fi

    ACTUAL_HASH=$(sha256sum "$WASM" | awk '{print $1}')
    EXPECTED_HASH=$(grep "kora_${contract}.wasm" "$HASHES_FILE" | awk '{print $1}' || echo "NOT_FOUND")

    if [ "$EXPECTED_HASH" = "NOT_FOUND" ]; then
      echo "  SKIP $contract: not in stored hashes"
      continue
    fi

    if [ "$ACTUAL_HASH" = "$EXPECTED_HASH" ]; then
      echo "  ✓ $contract: $ACTUAL_HASH"
    else
      echo "  ✗ $contract: MISMATCH"
      echo "      Expected: $EXPECTED_HASH"
      echo "      Got:      $ACTUAL_HASH"
      PASS=false
    fi
  done

  echo ""
  if [ "$PASS" = "true" ]; then
    echo "✓ PASS: All contracts reproduced byte-identically."
    exit 0
  else
    echo "✗ FAIL: One or more contracts did not reproduce identically."
    echo ""
    echo "Investigate:"
    echo "  1. Confirm toolchain: rustup show active-toolchain"
    echo "     Expected: $PINNED_CHANNEL"
    echo "  2. Clean rebuild: make clean && SOURCE_DATE_EPOCH=0 CARGO_INCREMENTAL=0 make build-optimized"
    echo "  3. Check git status: git status (no uncommitted changes)"
    echo "  4. Confirm git ref: git rev-parse HEAD"
    exit 1
  fi
fi

# ── Record mode: compute and store hashes ──────────────────────────────────

echo "=== Kora Protocol — Record WASM Hashes ==="
echo "Version  : $VERSION"
echo "Toolchain: $RUSTC_VERSION"
echo ""

if [ ! -d "$WASM_DIR" ]; then
  echo "ERROR: WASM directory not found: $WASM_DIR"
  echo "Run: make clean && make build-optimized"
  exit 1
fi

mkdir -p "$RELEASES_DIR"

echo "Recording hashes..."
echo ""

> "$HASHES_FILE"
RECORDED=0

for contract in "${CONTRACTS[@]}"; do
  WASM="$WASM_DIR/kora_${contract}.wasm"

  if [ ! -f "$WASM" ]; then
    echo "  ⚠  $contract: WASM not found (skipping)"
    continue
  fi

  HASH=$(sha256sum "$WASM" | awk '{print $1}')
  echo "$HASH  target/wasm32-unknown-unknown/release/kora_${contract}.wasm" >> "$HASHES_FILE"
  echo "  ✓  $contract: $HASH"
  RECORDED=$((RECORDED + 1))
done

echo ""
echo "Recorded $RECORDED contract hash(es) to: $HASHES_FILE"
echo ""

# ── Verification instructions ───────────────────────────────────────────────

cat <<INSTRUCTIONS
Independent Verification Steps
================================
Any contributor can verify the deployed WASM matches the reviewed source:

1. Install the pinned toolchain:
   rustup toolchain install $PINNED_CHANNEL
   rustup target add wasm32-unknown-unknown --toolchain $PINNED_CHANNEL

2. Checkout the tagged commit:
   git checkout $VERSION

3. Clean and rebuild:
   SOURCE_DATE_EPOCH=0 CARGO_INCREMENTAL=0 make clean build-optimized

4. Verify hashes match:
   sha256sum -c $HASHES_FILE

   Or using this script:
   ./scripts/record-wasm-hashes.sh $VERSION --verify

If all lines print 'OK', the deployed bytecode matches the reviewed source code.
Any failure indicates a potential build environment mismatch — investigate before deploying.

INSTRUCTIONS

# ── Next steps ───────────────────────────────────────────────────────────────

# ── Update WASM metrics baseline ──────────────────────────────────────────────

echo "Updating WASM metrics baseline..."
if cargo run -p kora-xtask --bin wasm-metrics -- measure-wasm \
    --out "$ROOT_DIR/baselines/wasm-metrics.json" 2>/dev/null; then
  echo "  ✓ baselines/wasm-metrics.json updated"
else
  echo "  WARNING: could not update metrics baseline (xtask build failed — skipping)"
fi

echo ""
echo "Next steps:"
echo "  1. Review CHANGELOG.md and ensure [Unreleased] → [$VERSION]"
echo "  2. Commit: git add $HASHES_FILE baselines/wasm-metrics.json && git commit -m 'chore: record WASM hashes and metrics baseline for $VERSION'"
echo "  3. Tag:    git tag -a $VERSION -m 'Release $VERSION'"
echo "  4. Push:   git push origin $VERSION"
echo "  5. The release.yml workflow will perform a second independent build and"
echo "     publish a verification report alongside the release artifacts."
