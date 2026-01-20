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

# Take screenshot with headless Chromium
screenshot:
    mkdir -p output
    chromium \
        --headless=new \
        --user-data-dir=./chromium_data \
        --no-first-run \
        --test-type \
        --enable-unsafe-webgpu \
        --enable-features=Vulkan,WebGPU,UseSkiaRenderer \
        --use-angle=vulkan \
        --screenshot=output/web_screenshot.png \
        --window-size=800,600 \
        http://localhost:8000

# Build, serve, and open browser
web: build-web
    pkill miniserve || true
    (sleep 1 && just open-browser) &
    miniserve . --port 8000 --index index.html

# Build, serve, take screenshot (for verification)
web-screenshot: build-web
    pkill miniserve || true
    miniserve . --port 8000 --index index.html &
    sleep 2
    just screenshot
    pkill miniserve || true

# Install required tools
setup:
    cargo install wasm-bindgen-cli
    cargo install miniserve
    rustup target add wasm32-unknown-unknown
