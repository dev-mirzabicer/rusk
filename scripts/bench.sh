#!/bin/bash

# Performance benchmarking script for Rusk Task Manager
# Part of Phase 6 Part 17: Rustic Performance Optimization

set -e

echo "=== Rusk Performance Benchmarking Suite ==="
echo "Running comprehensive performance benchmarks..."

# Create results directory
mkdir -p benchmark_results
cd "$(dirname "$0")/.."

# Install required tools if not present
echo "Checking for required tools..."

if ! command -v cargo-flamegraph &> /dev/null; then
    echo "Installing cargo-flamegraph..."
    cargo install flamegraph
fi

if ! command -v cargo-criterion &> /dev/null; then
    echo "Installing cargo-criterion..."
    cargo install cargo-criterion
fi

echo "=== Running Baseline Benchmarks ==="

# Run core benchmarks
echo "Running recurrence benchmarks..."
cargo bench --bench recurrence_benchmarks -- --output-format html --output-dir benchmark_results/recurrence

echo "Running repository benchmarks..."
cargo bench --bench repository_benchmarks -- --output-format html --output-dir benchmark_results/repository

echo "=== Generating Flamegraphs ==="

# Generate flamegraphs for hot paths
echo "Generating flamegraph for recurrence operations..."
cargo flamegraph --bench recurrence_benchmarks --output benchmark_results/recurrence_flamegraph.svg -- --bench

echo "Generating flamegraph for repository operations..."
cargo flamegraph --bench repository_benchmarks --output benchmark_results/repository_flamegraph.svg -- --bench

echo "=== Running Memory Profiling ==="

# Memory usage analysis
echo "Running memory analysis..."
cargo bench --bench recurrence_benchmarks -- --profile-time=5 --output-dir benchmark_results/memory

echo "=== Benchmark Results Summary ==="
echo "Results saved to benchmark_results/ directory:"
echo "  - HTML reports: benchmark_results/*/index.html"
echo "  - Flamegraphs: benchmark_results/*_flamegraph.svg"
echo "  - Memory profiles: benchmark_results/memory/"

echo "Benchmarking complete!"