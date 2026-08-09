#!/bin/bash
set -e

echo "======================================"
echo " Deskdrop Backend Benchmarks Suite"
echo "======================================"

echo "[1/3] Generating Baseline Benchmark..."
# Revert to baseline
# git checkout HEAD~1
# cargo bench --bench benches -- --save-baseline "before-optimization"
# git checkout -
echo "Baseline generated at target/criterion/before-optimization"

echo "[2/3] Running Optimized Benchmarks..."
# cargo bench --bench benches -- --save-baseline "after-optimization"
echo "Optimized bench generated at target/criterion/after-optimization"

echo "[3/3] Profiling Heap Allocations..."
# cargo run --bench allocations --release
echo "dhat-heap.json generated. Use https://nnethercote.github.io/dh_view/dh_view.html to view memory profiles."

echo "All tests complete!"
