#!/bin/bash

# Setup script for coverage tools
set -e

echo "🔧 Installing cargo-llvm-cov..."
cargo install cargo-llvm-cov

echo "🔧 Installing llvm-tools-preview component..."
rustup component add llvm-tools-preview

echo "✅ Coverage tools installed successfully!"
echo "📋 Usage:"
echo "  cargo llvm-cov --html --open    # Generate and open HTML report"
echo "  ./scripts/coverage.sh           # Run full coverage analysis"
echo "  cargo llvm-cov --summary-only   # Quick coverage summary"