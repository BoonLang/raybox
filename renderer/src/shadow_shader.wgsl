// Shadow rendering shader for WebGPU
// Renders box shadows as semi-transparent rectangles with dual-layer blending

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) position: vec2<f32>,       // Shadow position (x, y)
    @location(1) size: vec2<f32>,           // Shadow size (width, height)
    @location(2) content_size: vec2<f32>,   // Content rectangle size
    @location(3) color1: vec4<f32>,         // Layer 1 RGBA color
    @location(4) blur_radius1: f32,         // Layer 1 blur radius
    @location(5) offset1: vec2<f32>,        // Layer 1 offset
    @location(6) color2: vec4<f32>,         // Layer 2 RGBA color
    @location(7) blur_radius2: f32,         // Layer 2 blur radius
    @location(8) offset2: vec2<f32>,        // Layer 2 offset
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,       // Position within shadow quad
    @location(1) content_size: vec2<f32>,    // Content size for SDF
    @location(2) color1: vec4<f32>,          // Layer 1 color
    @location(3) blur_radius1: f32,          // Layer 1 blur
    @location(4) offset1: vec2<f32>,         // Layer 1 offset
    @location(5) color2: vec4<f32>,          // Layer 2 color
    @location(6) blur_radius2: f32,          // Layer 2 blur
    @location(7) offset2: vec2<f32>,         // Layer 2 offset
}

// Convert screen coordinates to NDC (Normalized Device Coordinates)
// Screen space: (0, 0) = top-left, (width, height) = bottom-right
// NDC space: (-1, -1) = bottom-left, (1, 1) = top-right
fn screen_to_ndc(screen_pos: vec2<f32>, viewport_size: vec2<f32>) -> vec2<f32> {
    let ndc_x = (screen_pos.x / viewport_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (screen_pos.y / viewport_size.y) * 2.0; // Flip Y axis
    return vec2<f32>(ndc_x, ndc_y);
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Hardcoded viewport size (should match canvas size)
    let viewport_size = vec2<f32>(700.0, 700.0);

    // Generate quad vertices based on vertex_index
    // 0-1-2 = first triangle, 2-3-0 = second triangle
    var local_pos: vec2<f32>;
    switch input.vertex_index {
        case 0u: { local_pos = vec2<f32>(0.0, 0.0); }           // Top-left
        case 1u: { local_pos = vec2<f32>(input.size.x, 0.0); }  // Top-right
        case 2u: { local_pos = vec2<f32>(input.size.x, input.size.y); } // Bottom-right
        case 3u: { local_pos = vec2<f32>(input.size.x, input.size.y); } // Bottom-right
        case 4u: { local_pos = vec2<f32>(0.0, input.size.y); }  // Bottom-left
        default: { local_pos = vec2<f32>(0.0, 0.0); }           // Top-left
    }

    // Calculate final screen position
    let screen_pos = input.position + local_pos;

    // Convert to NDC
    let ndc_pos = screen_to_ndc(screen_pos, viewport_size);

    output.position = vec4<f32>(ndc_pos, 0.0, 1.0);
    output.local_pos = local_pos;
    output.content_size = input.content_size;
    output.color1 = input.color1;
    output.blur_radius1 = input.blur_radius1;
    output.offset1 = input.offset1;
    output.color2 = input.color2;
    output.blur_radius2 = input.blur_radius2;
    output.offset2 = input.offset2;

    return output;
}

// SDF for rectangle (returns negative inside, positive outside)
fn sd_box(p: vec2<f32>, size: vec2<f32>) -> f32 {
    let d = abs(p) - size;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate alpha for each shadow layer and blend them together
    // This creates a unified shadow like a physical object

    // Shadow quad is sized based on UNION of both layers' extents.
    // Content is positioned at (union_extent_left, union_extent_top) in the quad.
    // Each shadow layer's gradient is then offset from that content position.

    // Calculate union extents (where content is positioned in the unified quad)
    let extent_top1 = input.blur_radius1 - input.offset1.y;
    let extent_left1 = input.blur_radius1 - input.offset1.x;
    let extent_top2 = input.blur_radius2 - input.offset2.y;
    let extent_left2 = input.blur_radius2 - input.offset2.x;

    let extent_top = max(extent_top1, extent_top2);
    let extent_left = max(extent_left1, extent_left2);

    // Content position in the unified quad
    let content_top_left = vec2<f32>(extent_left, extent_top);

    // Layer 1: Shadow gradient centered at content + offset
    let shadow_center1 = content_top_left + input.content_size * 0.5 + input.offset1;
    let relative_pos1 = input.local_pos - shadow_center1;
    let dist1 = sd_box(relative_pos1, input.content_size * 0.5);
    let alpha1 = (1.0 - smoothstep(0.0, input.blur_radius1, dist1)) * input.color1.a;

    // Layer 2: Shadow gradient centered at content + offset
    let shadow_center2 = content_top_left + input.content_size * 0.5 + input.offset2;
    let relative_pos2 = input.local_pos - shadow_center2;
    let dist2 = sd_box(relative_pos2, input.content_size * 0.5);
    let alpha2 = (1.0 - smoothstep(0.0, input.blur_radius2, dist2)) * input.color2.a;

    // Blend the two layers using "over" compositing (front-to-back)
    // Formula: result = src + dst * (1 - src.a)
    // Layer 1 is the front layer (small sharp shadow)
    let color1_premult = input.color1.rgb * alpha1;
    let color2_premult = input.color2.rgb * alpha2;

    // Composite: layer1 over layer2
    let final_color = color1_premult + color2_premult * (1.0 - alpha1);
    let final_alpha = alpha1 + alpha2 * (1.0 - alpha1);

    return vec4<f32>(final_color / max(final_alpha, 0.001), final_alpha);
}
