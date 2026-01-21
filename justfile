# Default: run windowed mode
default:
    cargo run --features windowed

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
        7) echo "Windowed mode not available for demo_text_vector_3d" ;;
        *) echo "Unknown example {{num}}. Available: 1-6 (windowed), 7 (vector 3D, screenshot only)" ;;
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
        7) cargo run --example demo_text_vector_3d ;;
        *) echo "Unknown example {{num}}. Available: 1-7 (screenshot mode)" ;;
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

# Generate MSDF atlas from font file (grid layout for shader compatibility)
gen-atlas font="assets/fonts/DejaVuSans.ttf":
    ./tools/msdf-atlas-gen \
        -font {{font}} \
        -type msdf \
        -uniformgrid \
        -uniformcols 10 \
        -uniformcell 64 64 \
        -size 48 \
        -pxrange 4 \
        -imageout assets/fonts/atlas.png \
        -json assets/fonts/atlas.json
