#!/usr/bin/env bash
# Helper script for renaming canvas_3d_6 → raybox
# Run this from the project root: /home/martinkavik/repos/canvas_3d_6

set -e  # Exit on error

echo "🔧 Raybox Rename Helper Script"
echo "=============================="
echo ""

# Verify we're in the right directory
if [[ ! -f "Cargo.toml" ]] || [[ ! -d "tools" ]] || [[ ! -d "renderer" ]]; then
    echo "❌ Error: Not in project root directory!"
    echo "   Please run this from /home/martinkavik/repos/canvas_3d_6"
    exit 1
fi

echo "✅ Verified project root directory"
echo ""

# Function to replace in file
replace_in_file() {
    local file="$1"
    local search="$2"
    local replace="$3"

    if [[ -f "$file" ]]; then
        # Use sed for in-place replacement
        sed -i "s|${search}|${replace}|g" "$file"
        echo "   ✏️  Updated: $file"
    else
        echo "   ⚠️  Skipped (not found): $file"
    fi
}

echo "Phase 1: Updating documentation..."
echo "-----------------------------------"

# Update path references
replace_in_file "CLAUDE.md" "/home/martinkavik/repos/canvas_3d_6" "/home/martinkavik/repos/raybox"
replace_in_file "README.md" "canvas_3d_6" "raybox"
replace_in_file "specs.md" "canvas_3d_6" "raybox"
replace_in_file "WORKFLOW_ANALYSIS.md" "canvas_3d_6" "raybox"
replace_in_file "RUST_ONLY_ARCHITECTURE.md" "canvas_3d_6" "raybox"
replace_in_file "docs/CHROME_SETUP.md" "canvas_3d_6" "raybox"
replace_in_file "docs/DOM_EXTRACTION.md" "canvas_3d_6" "raybox"
replace_in_file "reference/REFERENCE_METADATA.md" "canvas_3d_6" "raybox"
replace_in_file "tools/README.md" "canvas-tools" "raybox-tools"

echo ""
echo "Phase 2: Updating Cargo.toml files..."
echo "-------------------------------------"

# Update authors in root Cargo.toml
sed -i 's/authors = \["TodoMVC Renderer Team"\]/authors = ["Raybox Team"]/' Cargo.toml
echo "   ✏️  Updated: Cargo.toml (authors)"

# Update binary name in tools/Cargo.toml
sed -i 's/name = "canvas-tools"/name = "raybox-tools"/' tools/Cargo.toml
echo "   ✏️  Updated: tools/Cargo.toml (binary name)"

echo ""
echo "Phase 3: Updating source code..."
echo "--------------------------------"

# Update CLI command name in tools/src/main.rs (if present)
if grep -q '#\[command(name = "canvas-tools")\]' tools/src/main.rs 2>/dev/null; then
    sed -i 's/#\[command(name = "canvas-tools")\]/#[command(name = "raybox-tools")]/' tools/src/main.rs
    echo "   ✏️  Updated: tools/src/main.rs (command name)"
else
    echo "   ℹ️  No command name found in tools/src/main.rs (might be auto-derived)"
fi

# Update web/index.html title
sed -i 's/<title>TodoMVC - Canvas WebGPU Renderer<\/title>/<title>TodoMVC - Raybox WebGPU Renderer<\/title>/' web/index.html
echo "   ✏️  Updated: web/index.html (title)"

# Update Justfile if it exists and has canvas-tools references
if [[ -f "Justfile" ]]; then
    if grep -q "canvas-tools" Justfile; then
        sed -i 's/canvas-tools/raybox-tools/g' Justfile
        echo "   ✏️  Updated: Justfile (binary references)"
    else
        echo "   ℹ️  No canvas-tools references in Justfile"
    fi
fi

echo ""
echo "✅ All file contents updated!"
echo ""
echo "Phase 4: Next steps..."
echo "---------------------"
echo "1. Review changes: git diff (or jj diff)"
echo "2. Build project: cargo build --all"
echo "3. Verify binary: ls target/debug/raybox-tools"
echo "4. Test: cargo run -p tools -- --help"
echo "5. If all looks good, rename directory:"
echo "   cd /home/martinkavik/repos"
echo "   mv canvas_3d_6 raybox"
echo "   cd raybox"
echo "6. Commit changes: jj commit -m 'Rename project to raybox'"
echo ""
echo "See RENAME_CHECKLIST.md for detailed verification steps."
echo ""
echo "🎉 Helper script complete!"
