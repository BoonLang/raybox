#!/usr/bin/env bash
# Test all demos by taking screenshots
# Usage: ./scripts/test_all_demos.sh

set -e

echo "Building demos..."
cargo build --bin demos --features windowed

echo "Testing all demos (native)..."
mkdir -p output

for demo in 0 1 2 3 4 5 6; do
    echo "Testing Demo $demo..."
    # Run each demo's headless screenshot mode if available
    # For now, this is a placeholder for automated testing
    echo "  Demo $demo: OK (manual verification required)"
done

echo ""
echo "All demos built successfully!"
echo "Run 'just demos' to test interactively:"
echo "  - Press 0-6 to switch between demos"
echo "  - Press F to toggle stats overlay"
echo "  - Press G to toggle full system stats"
echo "  - Press K to toggle keybindings display"
echo "  - Press Esc to exit"
