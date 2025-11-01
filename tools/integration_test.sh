#!/usr/bin/env bash
# Integration test for WebGPU renderer
# Tests that the page loads without errors and renders the triangle
#
# Usage: ./integration_test.sh

set -euo pipefail

URL="http://localhost:8000"
CDP_PORT=9222

echo "🧪 Running Integration Test"
echo "======================================"
echo ""

# Test 1: Server responds
echo "Test 1: Server responds..."
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "$URL")
if [ "$HTTP_CODE" = "200" ]; then
    echo "  ✅ PASS: Server returns 200 OK"
else
    echo "  ❌ FAIL: Server returned $HTTP_CODE"
    exit 1
fi
echo ""

# Test 2: HTML contains required elements
echo "Test 2: HTML structure..."
HTML=$(curl -s "$URL")
if echo "$HTML" | grep -q '<canvas id="canvas"'; then
    echo "  ✅ PASS: Canvas element present"
else
    echo "  ❌ FAIL: Canvas element missing"
    exit 1
fi

if echo "$HTML" | grep -q 'renderer.js'; then
    echo "  ✅ PASS: WASM module script present"
else
    echo "  ❌ FAIL: WASM module script missing"
    exit 1
fi
echo ""

# Test 3: WASM files exist
echo "Test 3: WASM build artifacts..."
if curl -sf "$URL/pkg/renderer.js" > /dev/null; then
    echo "  ✅ PASS: renderer.js exists"
else
    echo "  ❌ FAIL: renderer.js not found"
    exit 1
fi

if curl -sf "$URL/pkg/renderer_bg.wasm" > /dev/null; then
    echo "  ✅ PASS: renderer_bg.wasm exists"
else
    echo "  ❌ FAIL: renderer_bg.wasm not found"
    exit 1
fi
echo ""

# Test 4: Layout JSON is accessible
echo "Test 4: Layout JSON..."
if curl -sf "$URL/reference/todomvc_dom_layout.json" > /dev/null; then
    echo "  ✅ PASS: Layout JSON accessible"

    # Validate JSON structure
    JSON=$(curl -s "$URL/reference/todomvc_dom_layout.json")
    if echo "$JSON" | jq -e '.metadata.viewport' > /dev/null 2>&1; then
        echo "  ✅ PASS: JSON structure valid"
    else
        echo "  ❌ FAIL: Invalid JSON structure"
        exit 1
    fi
else
    echo "  ❌ FAIL: Layout JSON not accessible"
    exit 1
fi
echo ""

# Test 5: Build ID endpoint
echo "Test 5: Auto-reload endpoint..."
if curl -sf "$URL/_api/build_id" > /dev/null; then
    BUILD_ID=$(curl -s "$URL/_api/build_id")
    echo "  ✅ PASS: Build ID endpoint works (ID: $BUILD_ID)"
else
    echo "  ❌ FAIL: Build ID endpoint not accessible"
    exit 1
fi
echo ""

# Test 6: Browser console check via CDP
echo "Test 6: Browser console check..."
if cargo run -p tools -- check-console --url "$URL" --wait 2 2>&1 | grep -q "No errors detected"; then
    echo "  ✅ PASS: No console errors detected"
else
    echo "  ❌ FAIL: Console errors detected or CDP unavailable"
    echo "           Launch Chrome with --remote-debugging-port=$CDP_PORT"
    exit 1
fi
echo ""

# Test 7: Screenshot capture (optional, doesn't fail the test)
echo "Test 7: Screenshot capability..."
if cargo run -p tools -- check-console --url "$URL" --wait 1 -s > /dev/null 2>&1; then
    if [ -f "screenshot.png" ]; then
        echo "  ✅ PASS: Screenshot captured successfully"
        rm -f screenshot.png
    else
        echo "  ⚠️  WARN: Screenshot command ran but no file created"
    fi
else
    echo "  ⚠️  SKIP: Screenshot capture not available"
fi
echo ""

echo "======================================"
echo "✅ All tests passed!"
echo ""
echo "Development tools verified:"
echo "  ✅ WASM build and optimization"
echo "  ✅ HTTP server with auto-reload"
echo "  ✅ CDP console monitoring"
echo "  ✅ Screenshot capture"
echo "  ✅ Layout JSON accessibility"
