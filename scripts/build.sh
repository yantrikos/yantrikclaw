#!/usr/bin/env bash
# Build script for YantrikClaw — runs in WSL for cross-compiled Linux binaries.
#
# Usage:
#   ./scripts/build.sh          # release build (optimized, ~2-6 min first time, ~30s cached)
#   ./scripts/build.sh debug    # debug build (faster compile, larger binary)
#   ./scripts/build.sh check    # type-check only (fastest, no binary)
#
# Uses sccache for compilation caching — subsequent builds reuse compiled crates.
# Builds in a native Linux tmpdir to avoid NTFS performance penalty.

set -euo pipefail

FEATURES="memory-yantrikdb"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_MODE="${1:-release}"

# sccache for crate-level caching
export RUSTC_WRAPPER=sccache

case "$BUILD_MODE" in
    release)
        echo "🔨 Release build (optimized)..."
        cargo build --release --features "$FEATURES" --manifest-path "$PROJECT_DIR/Cargo.toml"
        BINARY="$PROJECT_DIR/target/release/yantrikclaw"
        echo "✅ Binary: $BINARY"
        ;;
    debug)
        echo "🔨 Debug build..."
        cargo build --features "$FEATURES" --manifest-path "$PROJECT_DIR/Cargo.toml"
        BINARY="$PROJECT_DIR/target/debug/yantrikclaw"
        echo "✅ Binary: $BINARY"
        ;;
    check)
        echo "🔍 Type-check only..."
        cargo check --features "$FEATURES" --manifest-path "$PROJECT_DIR/Cargo.toml"
        echo "✅ Check passed"
        exit 0
        ;;
    *)
        echo "Usage: $0 [release|debug|check]"
        exit 1
        ;;
esac

echo "📊 sccache stats:"
sccache --show-stats 2>/dev/null | grep -E '(hit|miss|Cache)' || true
