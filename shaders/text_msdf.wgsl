// MSDF Text Rendering Shader
// Uses multi-channel signed distance field for crisp text at any size

struct TextUniforms {
    screen_size: vec2<f32>,
    sdf_params: vec2<f32>,  // x = px_range, y = reserved
}

@group(0) @binding(0) var<uniform> uniforms: TextUniforms;
@group(0) @binding(1) var atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct InstanceInput {
    @location(2) offset: vec2<f32>,
    @location(3) size: vec2<f32>,
    @location(4) uv_min: vec2<f32>,
    @location(5) uv_max: vec2<f32>,
    @location(6) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // Calculate screen position
    let world_pos = instance.offset + vertex.position * instance.size;

    // Convert to NDC (-1 to 1)
    let ndc = (world_pos / uniforms.screen_size) * 2.0 - 1.0;

    // Flip Y for screen coordinates (top-left origin)
    out.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);

    // Interpolate UVs within glyph cell
    out.uv = mix(instance.uv_min, instance.uv_max, vertex.uv);
    out.color = instance.color;

    return out;
}

// Median of three values - core MSDF operation
fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample MSDF texture
    let msdf = textureSample(atlas_texture, atlas_sampler, in.uv);

    // Reconstruct signed distance from MSDF channels
    // Values are stored as [0,1], convert to [-0.5, 0.5] range
    let sd = median(msdf.r, msdf.g, msdf.b) - 0.5;

    // Calculate screen-space distance field gradient for anti-aliasing
    let px_range = uniforms.sdf_params.x;

    // Use screen-space derivatives for resolution-independent AA
    let dxuv = dpdx(in.uv);
    let dyuv = dpdy(in.uv);

    // Approximate pixel size in UV space
    let tex_size = vec2<f32>(textureDimensions(atlas_texture));
    let px_size = sqrt(dot(dxuv, dxuv) + dot(dyuv, dyuv)) * length(tex_size);

    // Scale the distance by the pixel range
    let screen_px_distance = sd * px_range / max(px_size, 0.001);

    // Smooth step for anti-aliased edge
    let alpha = clamp(screen_px_distance + 0.5, 0.0, 1.0);

    // Output with premultiplied alpha
    return vec4<f32>(in.color.rgb * alpha, alpha * in.color.a);
}
