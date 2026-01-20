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
    // UVs are already correctly oriented by the atlas loader
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
    // Values stored as [0,1] where 0.5 = glyph edge
    let sd = median(msdf.r, msdf.g, msdf.b);

    // Screen-space anti-aliasing using UV derivatives
    // This approach works better than fwidth(sd) which can be unstable
    let uv_grad = fwidth(in.uv);
    let tex_size = vec2<f32>(textureDimensions(atlas_texture));

    // How many texels per screen pixel
    let texels_per_pixel = max(uv_grad.x * tex_size.x, uv_grad.y * tex_size.y);

    // SDF range in texture is 4 pixels, so the edge transition in normalized units
    // is approximately 1/(4*2) = 0.125 per texel
    // Scale anti-aliasing width by texels per pixel
    let aa_width = 0.5 * texels_per_pixel / uniforms.sdf_params.x;

    // Clamp aa_width to reasonable range to prevent artifacts
    let clamped_aa = clamp(aa_width, 0.01, 0.25);

    // MSDF convention: values > 0.5 = inside glyph, values < 0.5 = outside
    // Smoothstep gives 0 when sd < 0.5-aa (outside) and 1 when sd > 0.5+aa (inside)
    let alpha = smoothstep(0.5 - clamped_aa, 0.5 + clamped_aa, sd);

    // Discard fully transparent pixels
    if (alpha < 0.01) {
        discard;
    }

    return vec4<f32>(in.color.rgb, alpha * in.color.a);
}
