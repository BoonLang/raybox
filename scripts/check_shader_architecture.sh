#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

fail=0
TMP_OUT=/tmp/raybox_shader_arch_check.out

filter_allowlist() {
    local input="$1"
    grep -v '^src/hot_reload/shader_loader.rs:' "$input" \
        | grep -v '^src/architecture_guard.rs:' \
        || true
}

check_absent() {
    local description="$1"
    shift
    if rg -n "$@" >"$TMP_OUT" 2>/dev/null; then
        local filtered
        filtered="$(filter_allowlist "$TMP_OUT")"
        if [[ -n "$filtered" ]]; then
            echo "FAIL: ${description}"
            printf '%s\n' "$filtered"
            fail=1
        fi
    fi
}

check_absent \
    "repo-tracked handwritten WGSL remains outside hot-reload/compiler plumbing" \
    'ShaderSource::Wgsl|create_shader_module\(.*Wgsl|const .*SHADER: &str = r#"' \
    --glob '!src/hot_reload/shader_loader.rs' \
    --glob '!src/architecture_guard.rs' \
    src examples

check_absent \
    "implicit min_binding_size: None remains in runtime or examples" \
    'min_binding_size:\s*None' \
    --glob '!src/architecture_guard.rs' \
    src examples

check_absent \
    "removed handwritten GPU ABI mirror structs remain in runtime or examples" \
    'struct (GpuGridCell|AtlasGridCell|GpuBezierCurve|GpuGlyphData|GpuCharInstanceEx|GpuUiPrimitive)\b|\b(grid_cells_buffer|curve_indices_buffer|grid_cells|curve_indices)\b' \
    --glob '!src/architecture_guard.rs' \
    src examples

check_absent \
    "dead glyph-grid shader ABI remains in tracked Slang shaders" \
    'struct GridCell\b|StructuredBuffer<GridCell>|StructuredBuffer<uint> curveIndices|uint4 gridInfo\b' \
    shaders/sdf_text2d_vector.slang \
    shaders/sdf_todomvc.slang \
    shaders/sdf_clay_vector.slang \
    shaders/sdf_text_shadow_vector.slang \
    shaders/sdf_todomvc_3d.slang

if [[ "$fail" -ne 0 ]]; then
    exit 1
fi

echo "Shader architecture checks passed."
