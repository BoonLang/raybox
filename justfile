# Default: run windowed mode
default:
    cargo run --features windowed

# Run unified demo mode (press 0-6 to switch demos)
demos:
    cargo run --bin demos --features windowed

# Run demos starting from specific demo: just demos-from 3
demos-from num:
    RAYBOX_DEMO={{num}} cargo run --bin demos --features windowed

# Run example by number: just ex 1, just ex 2, etc.
ex num:
    #!/usr/bin/env bash
    case "{{num}}" in
        1) cargo run --example demo_objects --features windowed ;;
        2) cargo run --example demo_spheres --features windowed ;;
        3) cargo run --example demo_towers --features windowed ;;
        4) cargo run --example demo_text2d --features windowed ;;
        5) cargo run --example demo_clay --features windowed ;;
        6) cargo run --example demo_text_shadow --features windowed ;;
        *) echo "Unknown example {{num}}. Available: 1-6" ;;
    esac

# Run example screenshot by number: just ex_screenshot 4
ex_screenshot num:
    #!/usr/bin/env bash
    case "{{num}}" in
        1) cargo run --example demo_objects ;;
        2) cargo run --example demo_spheres ;;
        3) cargo run --example demo_towers ;;
        4) cargo run --example demo_text2d ;;
        5) cargo run --example demo_clay ;;
        6) cargo run --example demo_text_shadow ;;
        *) echo "Unknown example {{num}}. Available: 1-6" ;;
    esac

# Build WASM and generate bindings
build-web:
    RUSTFLAGS="--cfg=web_sys_unstable_apis" cargo build --lib --target wasm32-unknown-unknown --release
    wasm-bindgen target/wasm32-unknown-unknown/release/raybox.wasm --out-dir pkg --target web

# Start dev server
serve:
    miniserve . --port 8000 --index index.html

# Open Chromium with local user data (no prompts)
open-browser:
    chromium \
        --user-data-dir=./chromium_data \
        --no-first-run \
        --no-default-browser-check \
        --disable-session-crashed-bubble \
        --hide-crash-restore-bubble \
        --test-type \
        --enable-unsafe-webgpu \
        --enable-features=Vulkan,WebGPU,UseSkiaRenderer \
        --use-angle=vulkan \
        http://localhost:8000

# Build, serve, and open browser
web: build-web
    pkill miniserve || true
    (sleep 1 && just open-browser) &
    miniserve . --port 8000 --index index.html

# Native screenshot (headless render to PNG)
screenshot:
    cargo run

# Native screenshot and open
screenshot-open: screenshot
    xdg-open output/screenshot.png

# Internal: capture web screenshot via CDP
_web-screenshot-capture:
    #!/usr/bin/env bash
    set -e
    mkdir -p output
    chromium \
        --user-data-dir=./chromium_data \
        --no-first-run \
        --test-type \
        --enable-unsafe-webgpu \
        --enable-features=Vulkan,WebGPU,UseSkiaRenderer \
        --use-angle=vulkan \
        --window-size=800,600 \
        --remote-debugging-port=9222 \
        http://localhost:8000 &
    PID=$!
    sleep 3
    # Get WebSocket URL and take screenshot via CDP
    WS_URL=$(curl -s http://localhost:9222/json | jq -r '.[0].webSocketDebuggerUrl')
    # Use websocat to send CDP command and capture screenshot
    echo '{"id":1,"method":"Page.captureScreenshot","params":{"format":"png"}}' | \
        websocat -n1 -B 10485760 "$WS_URL" | \
        jq -r '.result.data' | \
        base64 -d > output/web_screenshot.png
    kill $PID 2>/dev/null || true

# Web screenshot (build, serve, capture)
web-screenshot: build-web
    pkill miniserve || true
    miniserve . --port 8000 --index index.html &
    sleep 2
    just _web-screenshot-capture
    pkill miniserve || true

# Web screenshot and open
web-screenshot-open: web-screenshot
    xdg-open output/web_screenshot.png

# Install required tools
setup:
    cargo install wasm-bindgen-cli
    cargo install miniserve
    rustup target add wasm32-unknown-unknown

# Run demos with control server enabled
demos-control:
    cargo run --bin demos --features windowed,control -- --control

# Start MCP server
mcp:
    cargo run --bin raybox-mcp --features mcp

# Run CLI control tool
ctl *args:
    cargo run --bin raybox-ctl --features control -- {{args}}

# Development mode with hot-reload (native)
dev:
    cargo run --bin raybox-dev --features hot-reload

# Development mode with hot-reload (web)
dev-web:
    cargo run --bin raybox-dev --features hot-reload -- --web

# Benchmark FPS across all demos (requires running demo with --control)
bench:
    #!/usr/bin/env bash
    set -e
    CTL="cargo run --bin raybox-ctl --features control --"

    # Wait for control server
    echo "Waiting for demo control server..."
    for i in $(seq 1 30); do
        if $CTL ping 2>/dev/null | grep -q "Pong"; then
            break
        fi
        if [ "$i" -eq 30 ]; then
            echo "ERROR: Could not connect. Start demo with: just demos-control  OR  just dev"
            exit 1
        fi
        sleep 1
    done

    echo ""
    echo "=== FPS Benchmark ==="
    echo ""
    DEMOS=(0 1 2 3 4 5 6)
    NAMES=("Empty" "Objects" "Spheres" "Towers" "2D Text" "Clay Tablet" "Text Shadow")
    for i in "${!DEMOS[@]}"; do
        $CTL switch "${DEMOS[$i]}" > /dev/null 2>&1
        sleep 3
        FPS=$($CTL status 2>/dev/null | grep "FPS:" | awk '{print $2}')
        printf "  Demo %d %-14s %s FPS\n" "${DEMOS[$i]}" "(${NAMES[$i]})" "$FPS"
    done
    echo ""

# Open browser with hot-reload enabled (use with dev-web)
open-browser-hotreload:
    chromium \
        --user-data-dir=./chromium_data \
        --no-first-run \
        --no-default-browser-check \
        --disable-session-crashed-bubble \
        --hide-crash-restore-bubble \
        --test-type \
        --enable-unsafe-webgpu \
        --enable-features=Vulkan,WebGPU,UseSkiaRenderer \
        --use-angle=vulkan \
        "http://localhost:8000?hotreload=1&control=1"

