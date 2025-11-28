// SDF Text Rendering Shader
//
// Renders SDF fonts with mathematical scaling - one atlas works at any size.
// The scale factor (font_size / glyph_size) is used to calculate proper
// anti-aliasing width, giving crisp edges at any scale.
//
// Key formula: aa_width = pixel_width * 0.05
// where pixel_width = 1 / (sdf_range * scale)

struct Uniforms {
    resolution: vec2<f32>,
    sdf_range: f32,
    _padding: f32,
}

struct VertexInput {
    @location(0) position: vec2<f32>,  // Screen position
    @location(1) uv: vec2<f32>,         // Atlas UV coordinates
    @location(2) color: vec4<f32>,      // Text color
    @location(3) scale_pad: vec2<f32>,  // scale (font_size / glyph_size) + padding
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) scale: f32,
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
    output.scale = input.scale_pad.x;  // Pass scale to fragment shader

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

    // Mathematical SDF scaling for crisp text at any size:
    //
    // pixel_width = how much normalized SDF space 1 screen pixel covers
    // At native size (scale=1): 1 pixel = 1/sdf_range in SDF space
    // At 2x scale: 1 pixel = 1/(sdf_range * 2) in SDF space (finer detail)
    //
    // aa_width controls anti-aliasing: smaller = crisper, larger = smoother
    // 0.2 gives ~0.4 pixel of smoothing - good balance with 128px atlas
    let pixel_width = 1.0 / (uniforms.sdf_range * input.scale);
    let aa_width = pixel_width * 0.2;

    // Threshold at 0.5 (the glyph edge) with tight smoothstep for crisp AA
    let alpha = smoothstep(0.5 - aa_width, 0.5 + aa_width, signed_dist);

    // Discard fully transparent pixels
    if (alpha < 0.001) {
        discard;
    }

    // Output color with calculated alpha
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
