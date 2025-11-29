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
    // 2x2 Supersampling for smoother anti-aliasing
    // Sample 4 sub-pixel locations and average the results

    // Calculate UV offset for sub-pixel sampling
    // fwidth gives us the UV change across one pixel, use 0.25 for sub-pixel offsets
    let uv_offset = fwidth(input.uv) * 0.25;

    // Sample center to calculate AA width (used for all sub-samples)
    let center_sample = textureSample(atlas_texture, atlas_sampler, input.uv);
    let fw = fwidth(center_sample.r);
    let aa_width = fw * 0.5;  // Tight AA for sharp edges without sharpening filter

    // Sample 4 sub-pixels in a 2x2 pattern
    let sample1 = textureSample(atlas_texture, atlas_sampler, input.uv + vec2<f32>(-uv_offset.x, -uv_offset.y));
    let alpha1 = smoothstep(0.5 - aa_width, 0.5 + aa_width, sample1.r);

    let sample2 = textureSample(atlas_texture, atlas_sampler, input.uv + vec2<f32>(uv_offset.x, -uv_offset.y));
    let alpha2 = smoothstep(0.5 - aa_width, 0.5 + aa_width, sample2.r);

    let sample3 = textureSample(atlas_texture, atlas_sampler, input.uv + vec2<f32>(-uv_offset.x, uv_offset.y));
    let alpha3 = smoothstep(0.5 - aa_width, 0.5 + aa_width, sample3.r);

    let sample4 = textureSample(atlas_texture, atlas_sampler, input.uv + vec2<f32>(uv_offset.x, uv_offset.y));
    let alpha4 = smoothstep(0.5 - aa_width, 0.5 + aa_width, sample4.r);

    // Average the 4 sub-pixel samples
    let alpha = (alpha1 + alpha2 + alpha3 + alpha4) * 0.25;

    // Note: No sharpening filter applied - unsharp mask destroys thin features
    // like the middle leg of 'm'. Tight AA (0.5) provides sufficient sharpness.

    // Discard fully transparent pixels
    if (alpha < 0.001) {
        discard;
    }

    // Output color with calculated alpha
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
