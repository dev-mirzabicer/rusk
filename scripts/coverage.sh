#!/bin/bash

# Coverage script for local development
# Requires: cargo install cargo-llvm-cov

set -e

echo "🧹 Cleaning previous coverage data..."
cargo llvm-cov clean --workspace

echo "📊 Running tests with coverage instrumentation..."
cargo llvm-cov --all-features --workspace --html --open

echo "📈 Generating summary report..."
cargo llvm-cov --all-features --workspace --summary-only

echo "📊 Coverage report generated in target/llvm-cov/html/"
echo "🎯 Target: 95%+ line coverage"