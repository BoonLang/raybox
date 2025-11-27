# TodoMVC Canvas Renderer - Development Commands

# Default: show available commands
default:
    @just --list

# Build everything in release mode
build:
    cargo build --release --all

# Build tools crate only
build-tools:
    cargo build --release -p tools

# Build WASM renderer (development)
build-wasm:
    cargo run --release -p tools -- wasm-build

# Build WASM renderer (release with optimization)
build-wasm-release:
    cargo run --release -p tools -- wasm-build --release

# Start WASM dev server with auto-rebuild and live reload
start-wasm:
    cargo run --release -p tools -- wasm-start

# Start WASM dev server in release mode
start-wasm-release:
    cargo run --release -p tools -- wasm-start --release

# Start WASM dev server and open browser
start-wasm-open:
    cargo run --release -p tools -- wasm-start --open

# Run all tests
test:
    cargo test --all

# Run tests with output
test-verbose:
    cargo test --all -- --nocapture

# Watch and auto-rebuild on changes
watch:
    cargo watch -x "build --release"

# Watch and auto-test on changes
watch-test:
    cargo watch -x test

# Extract DOM layout from CSS analysis
extract-dom OUTPUT="reference/layouts/layout.json":
    cargo run --release -p tools -- extract-dom --output {{OUTPUT}}

# Compare two layout JSON files
compare REF="reference/layouts/layout.json" ACTUAL="output/renderer_layout.json":
    cargo run --release -p tools -- compare-layouts --reference {{REF}} --actual {{ACTUAL}}

# Compare with diff export
compare-diff REF="reference/layouts/layout.json" ACTUAL="output/renderer_layout.json" DIFF="output/diff.json":
    cargo run --release -p tools -- compare-layouts --reference {{REF}} --actual {{ACTUAL}} --diff-output {{DIFF}}

# Visualize layout as HTML (TODO: implement)
visualize INPUT="reference/layouts/layout.json" OUTPUT="output/viz.html":
    cargo run --release -p tools -- visualize-layout --input {{INPUT}} --output {{OUTPUT}}

# Start HTTP server (TODO: implement)
serve DIR="dist" PORT="8080":
    cargo run --release -p tools -- serve {{DIR}} --port {{PORT}}

# Take screenshot via Chrome CDP (TODO: implement)
screenshot URL="http://localhost:8080" OUTPUT="output/screenshot.png":
    cargo run --release -p tools -- screenshot --url {{URL}} --output {{OUTPUT}}

# Clean build artifacts
clean:
    cargo clean

# Format code
fmt:
    cargo fmt --all

# Check code with clippy
clippy:
    cargo clippy --all -- -D warnings

# Full check: format, clippy, test
check:
    just fmt
    just clippy
    just test

# Development workflow: watch + auto-rebuild + auto-test
dev:
    #!/usr/bin/env bash
    echo "Starting development workflow..."
    echo "Terminal 1: Auto-rebuild"
    cargo watch -s "just build" &
    echo "Terminal 2: Auto-test"
    cargo watch -x test

# Serve web directory (no auto-reload)
serve-web PORT="8000":
    cargo run --release -p tools -- serve web --port {{PORT}}

# Install required tools
install-tools:
    #!/usr/bin/env bash
    echo "Installing required tools..."
    cargo install cargo-watch
    cargo install just
    cargo install wasm-pack
    echo "✓ Tools installed"

# Verify setup
verify:
    #!/usr/bin/env bash
    echo "Verifying setup..."
    echo -n "Rust: "
    rustc --version
    echo -n "Cargo: "
    cargo --version
    echo -n "cargo-watch: "
    cargo watch --version 2>/dev/null || echo "NOT INSTALLED - run 'just install-tools'"
    echo -n "just: "
    just --version
    echo -n "jj: "
    jj --version 2>/dev/null || echo "NOT INSTALLED"
    echo ""
    echo "Building tools..."
    cargo build --release -p tools
    echo "✓ Build successful"
    echo ""
    echo "Testing extract-dom..."
    ./target/release/raybox-tools extract-dom --output /tmp/test_layout.json
    echo "✓ extract-dom works"
    echo ""
    echo "Testing compare-layouts..."
    ./target/release/raybox-tools compare-layouts \
        --reference /tmp/test_layout.json \
        --actual /tmp/test_layout.json
    echo "✓ compare-layouts works"
    echo ""
    echo "✓ All checks passed!"
