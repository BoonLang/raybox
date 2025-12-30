#!/bin/bash
# compare_physical.sh - Compare emergent (physical) renderer against reference
#
# PURPOSE: Final verification script for user/CI
# For granular debugging, use: check-console, screenshot, compare-layouts, etc.
#
# CURRENT LIMITATION: wasm-start only builds classic renderer.
# To test emergent renderer, build it manually first:
#   cargo build --target wasm32-unknown-unknown --package emergent-renderer
#   wasm-bindgen target/wasm32-unknown-unknown/debug/emergent_renderer.wasm --out-dir web/pkg --target web
# Then serve with: cargo run -p tools -- serve web --port 8001
# And navigate to: http://localhost:8001/emergent.html
#
# TODO: Add --renderer emergent flag to wasm-start
#
# EXIT CODES:
#   0 - PASS (visual match within threshold)
#   1 - FAIL (visual difference exceeds threshold)

set -e

# Configuration
# NOTE: Port 9222 is used by boon tools server, use 9333 for raybox
# NOTE: Port 8081 is used by boon MoonZoon playground, use 8001 for raybox
REFERENCE="reference/screenshots/screenshot.png"
OUTPUT="/tmp/emergent_screenshot.png"
SSIM_THRESHOLD="0.97"
SERVER_PORT=8001
CHROME_DEBUG_PORT=9333
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHROME_DATA_DIR="$SCRIPT_DIR/.chrome-data"

# Choose which renderer to test:
# - emergent.html: Pure HTML/CSS mockup (for visual reference)
# - emergent_wasm.html: WebGPU renderer (requires emergent-renderer build)
# - index.html: Classic WebGPU renderer
#
# Now using WASM WebGPU version (emergent-renderer is built by wasm-start --renderer emergent)
RENDERER_URL="http://localhost:$SERVER_PORT/emergent_wasm.html"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=== Physical Renderer Comparison ==="
echo ""

# Ensure chrome data dir exists
mkdir -p "$CHROME_DATA_DIR"

#-------------------------------------------------------------------------------
# 1. Check if server is running, start if not
#-------------------------------------------------------------------------------
if curl -s "http://localhost:$SERVER_PORT" > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} Server already running on port $SERVER_PORT"
else
    echo -e "${YELLOW}→${NC} Starting emergent renderer server on port $SERVER_PORT..."
    echo "  Run in another terminal: cargo run -p tools -- wasm-start --port $SERVER_PORT"
    echo ""
    echo -e "${RED}ERROR:${NC} Server not running. Please start it first:"
    echo "  cd $SCRIPT_DIR"
    echo "  cargo run -p tools -- wasm-start --port $SERVER_PORT"
    exit 1
fi

#-------------------------------------------------------------------------------
# 2. Take screenshot (tool will launch its own Chrome with WebGPU flags)
#-------------------------------------------------------------------------------
echo ""
echo "Taking screenshot via screenshot command..."

# Build tools if needed (release for faster execution)
if [ ! -f "$SCRIPT_DIR/target/release/raybox-tools" ]; then
    echo "  Building tools (release)..."
    cargo build --release -p tools
fi

# Take screenshot using the screenshot command
# The tool launches its own headed Chrome with WebGPU flags
"$SCRIPT_DIR/target/release/raybox-tools" screenshot \
    --url "$RENDERER_URL" \
    --output "$OUTPUT" \
    --width 700 \
    --height 700

echo -e "${GREEN}✓${NC} Screenshot saved: $OUTPUT"

#-------------------------------------------------------------------------------
# 4. Check if reference exists
#-------------------------------------------------------------------------------
echo ""
if [ ! -f "$REFERENCE" ]; then
    echo -e "${YELLOW}WARNING:${NC} Reference screenshot not found: $REFERENCE"
    echo "  Creating baseline from current screenshot..."
    cp "$OUTPUT" "$REFERENCE"
    echo -e "${GREEN}✓${NC} Baseline created: $REFERENCE"
    echo ""
    echo "Run this script again to compare against the baseline."
    exit 0
fi

#-------------------------------------------------------------------------------
# 5. Compare screenshots using pixel-diff
#-------------------------------------------------------------------------------
echo "Comparing against reference..."
echo "  Reference: $REFERENCE"
echo "  Current:   $OUTPUT"
echo ""

# Run pixel-diff and capture output
set +e  # Don't exit on error (we want to capture the exit code)
DIFF_OUTPUT=$("$SCRIPT_DIR/target/release/raybox-tools" pixel-diff \
    --reference "$REFERENCE" \
    --current "$OUTPUT" \
    --output "/tmp/emergent_diff.png" \
    --threshold "$SSIM_THRESHOLD" 2>&1)
DIFF_EXIT_CODE=$?
set -e

echo "$DIFF_OUTPUT"

#-------------------------------------------------------------------------------
# 6. Report results
#-------------------------------------------------------------------------------
echo ""
echo "=== Results ==="

# Extract SSIM from output (format: "SSIM: 0.9774")
SSIM=$(echo "$DIFF_OUTPUT" | grep -oP 'SSIM:\s*\K[0-9.]+' || echo "")

if [ -n "$SSIM" ]; then
    echo "SSIM Score: $SSIM"
    echo "Threshold:  $SSIM_THRESHOLD"
    echo ""

    # Use bc for floating point comparison
    PASS=$(echo "$SSIM >= $SSIM_THRESHOLD" | bc -l)

    if [ "$PASS" = "1" ]; then
        echo -e "${GREEN}✓ PASS${NC} - Visual match within threshold"
        echo ""
        echo "The emergent renderer matches the reference!"
        exit 0
    else
        echo -e "${RED}✗ FAIL${NC} - Visual difference exceeds threshold"
        echo ""
        echo "Diff image saved to: /tmp/emergent_diff.png"
        echo ""
        echo "Debug tips:"
        echo "  1. View diff: feh /tmp/emergent_diff.png"
        echo "  2. Check console: cargo run -p tools -- check-console --url http://localhost:$SERVER_PORT --port $CHROME_DEBUG_PORT"
        echo "  3. Compare layouts: cargo run -p tools -- compare-layouts --reference reference/layouts/layout.json --actual /tmp/renderer_layout.json"
        exit 1
    fi
else
    # Couldn't parse SSIM, use exit code from pixel-diff
    if [ $DIFF_EXIT_CODE -eq 0 ]; then
        echo -e "${GREEN}✓ PASS${NC} - Images match"
        exit 0
    else
        echo -e "${RED}✗ FAIL${NC} - Images differ"
        exit 1
    fi
fi
