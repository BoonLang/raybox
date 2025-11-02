// Shadow rendering shader for WebGPU
// Renders box shadows as semi-transparent rectangles

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) position: vec2<f32>,      // Shadow position (x, y)
    @location(1) size: vec2<f32>,          // Shadow size (width, height)
    @location(2) color: vec4<f32>,         // RGBA color
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
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
    output.color = input.color;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
