// MTSDF Text Rendering Shader
//
// Uses Multi-channel True Signed Distance Field for crisp text at any size.
// The MTSDF atlas stores:
// - R, G, B channels: Multi-channel distance for sharp corners
// - A channel: True distance for effects (shadows, outlines)

struct Uniforms {
    resolution: vec2<f32>,
    sdf_range: f32,
    _padding: f32,
}

struct VertexInput {
    @location(0) position: vec2<f32>,  // Screen position
    @location(1) uv: vec2<f32>,         // Atlas UV coordinates
    @location(2) color: vec4<f32>,      // Text color
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var atlas_texture: texture_2d<f32>;

@group(0) @binding(2)
var atlas_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Convert screen coordinates to clip space
    // Screen: (0, 0) top-left, (width, height) bottom-right
    // Clip: (-1, 1) top-left, (1, -1) bottom-right
    let clip_x = (input.position.x / uniforms.resolution.x) * 2.0 - 1.0;
    let clip_y = 1.0 - (input.position.y / uniforms.resolution.y) * 2.0;

    output.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    output.uv = input.uv;
    output.color = input.color;

    return output;
}

// Median of three values - core MSDF algorithm
fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the SDF atlas (fontdue generates single-channel SDF, stored in all RGBA channels)
    let sample = textureSample(atlas_texture, atlas_sampler, input.uv);

    // For fontdue SDF: all channels contain the same distance value
    // 0.5 = edge, >0.5 = inside glyph, <0.5 = outside glyph
    let signed_dist = sample.r;

    // Calculate screen-space derivative for proper anti-aliasing
    let gradient = vec2<f32>(dpdx(signed_dist), dpdy(signed_dist));
    let gradient_length = length(gradient);

    // Anti-alias width: typically 1-2 pixels of smooth transition
    // Use screen-space gradient to scale properly at any zoom level
    let aa_width = 0.5 / max(gradient_length * uniforms.sdf_range, 0.001);

    // Clamp aa_width to reasonable bounds to prevent excessive blur or hard edges
    let clamped_aa = clamp(aa_width, 0.01, 0.25);

    // Smoothstep for anti-aliased edges around the 0.5 threshold
    let alpha = smoothstep(0.5 - clamped_aa, 0.5 + clamped_aa, signed_dist);

    // Discard fully transparent pixels (outside the glyph + padding)
    if (alpha < 0.001) {
        discard;
    }

    // Output color with calculated alpha
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
