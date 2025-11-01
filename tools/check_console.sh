#!/usr/bin/env bash
# Check browser console for errors via Chrome DevTools Protocol
#
# Usage: ./check_console.sh [url]
# Example: ./check_console.sh http://localhost:8000

set -euo pipefail

URL="${1:-http://localhost:8000}"
CDP_PORT=9222
TIMEOUT=10

echo "🔍 Checking console logs for: $URL"
echo ""

# Get the WebSocket debugger URL
WS_URL=$(curl -s "http://localhost:$CDP_PORT/json" | jq -r '.[0].webSocketDebuggerUrl' 2>/dev/null || echo "")

if [ -z "$WS_URL" ]; then
    echo "❌ ERROR: Could not connect to Chrome DevTools Protocol"
    echo "   Make sure Chrome is running with --remote-debugging-port=$CDP_PORT"
    exit 1
fi

echo "✅ Connected to Chrome CDP"
echo "   WebSocket: $WS_URL"
echo ""

# Note: Full implementation would use websocat or a Rust CDP client
# to listen for Runtime.consoleAPICalled and Runtime.exceptionThrown events
#
# For now, we can check the page title and basic connectivity
PAGE_TITLE=$(curl -s "http://localhost:$CDP_PORT/json" | jq -r '.[0].title')
PAGE_URL=$(curl -s "http://localhost:$CDP_PORT/json" | jq -r '.[0].url')

echo "📄 Page Info:"
echo "   Title: $PAGE_TITLE"
echo "   URL: $PAGE_URL"
echo ""

# Check if page matches expected URL
if [[ "$PAGE_URL" == *"$URL"* ]]; then
    echo "✅ Page loaded successfully"
else
    echo "⚠️  Warning: Page URL doesn't match expected URL"
    echo "   Expected: $URL"
    echo "   Actual: $PAGE_URL"
fi

echo ""
echo "📝 To implement full console monitoring:"
echo "   1. Connect to WebSocket: $WS_URL"
echo "   2. Enable Runtime domain"
echo "   3. Listen for Runtime.consoleAPICalled events"
echo "   4. Listen for Runtime.exceptionThrown events"
echo "   5. Filter and report errors"
echo ""
echo "   See: https://chromedevtools.github.io/devtools-protocol/tot/Runtime/"
