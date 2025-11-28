// Raymarching shader for emergent SDF-based UI rendering
//
// This shader renders a full-screen quad and raymarches through
// a scene of SDF primitives to determine the color at each pixel.

// ============================================================================
// Data structures
// ============================================================================

struct Uniforms {
    resolution: vec2<f32>,
    element_count: u32,
    _padding: u32,
    camera_pos: vec4<f32>,
    camera_target: vec4<f32>,
    light_dir: vec4<f32>,
    light_color: vec4<f32>,
    ambient_color: vec4<f32>,
}

struct Element {
    center: vec4<f32>,       // xyz + padding
    half_extents: vec4<f32>, // xyz + padding
    color: vec4<f32>,        // rgb + alpha
    params: vec4<f32>,       // corner_radius, shape_type, reserved, reserved
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> elements: array<Element>;

// ============================================================================
// SDF Primitives (2D for flat UI rendering)
// ============================================================================

// 2D Box SDF (sharp edges)
fn sd_box_2d(p: vec2<f32>, b: vec2<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
}

// 2D Rounded box SDF
fn sd_rounded_box_2d(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + r;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// 2D Circle SDF
fn sd_circle(p: vec2<f32>, r: f32) -> f32 {
    return length(p) - r;
}

// 2D Ring SDF (hollow circle)
fn sd_ring(p: vec2<f32>, outer_radius: f32, thickness: f32) -> f32 {
    let dist = length(p);
    let inner_radius = outer_radius - thickness;
    let middle_radius = (outer_radius + inner_radius) / 2.0;
    let half_thickness = thickness / 2.0;
    return abs(dist - middle_radius) - half_thickness;
}

// ============================================================================
// Procedural SDF Letters (for "todos" title)
// ============================================================================

// Letter 't' - vertical stem with horizontal crossbar at top
fn sd_letter_t(p: vec2<f32>, scale: f32) -> f32 {
    let stem = sd_box_2d(p - vec2<f32>(0.0, -0.05) * scale, vec2<f32>(0.06, 0.35) * scale);
    let cross = sd_box_2d(p - vec2<f32>(0.0, 0.22) * scale, vec2<f32>(0.18, 0.05) * scale);
    return min(stem, cross);
}

// Letter 'o' - ring shape
fn sd_letter_o(p: vec2<f32>, scale: f32) -> f32 {
    return sd_ring(p, 0.22 * scale, 0.06 * scale);
}

// Letter 'd' - vertical stem on RIGHT with bowl on LEFT
fn sd_letter_d(p: vec2<f32>, scale: f32) -> f32 {
    // Tall stem on the right side
    let stem = sd_box_2d(p - vec2<f32>(0.16, 0.05) * scale, vec2<f32>(0.06, 0.4) * scale);
    // Bowl on the left - a ring clipped to show only left half
    let bowl = sd_ring(p - vec2<f32>(0.0, -0.13) * scale, 0.22 * scale, 0.06 * scale);
    return min(stem, bowl);
}

// Letter 's' - S-curve shape using two 3/4 circles
fn sd_letter_s(p: vec2<f32>, scale: f32) -> f32 {
    let r = 0.11 * scale;
    let t = 0.055 * scale;

    // Top arc - remove bottom-left quadrant to form top of S
    let top_c = vec2<f32>(0.0, 0.08 * scale);
    let top_p = p - top_c;
    let d_top = sd_ring(top_p, r, t);
    // Keep where x >= 0 OR y >= 0 (remove bottom-left quadrant)
    let s_top = max(d_top, min(-top_p.x, -top_p.y));

    // Bottom arc - remove top-right quadrant to form bottom of S
    let bot_c = vec2<f32>(0.0, -0.08 * scale);
    let bot_p = p - bot_c;
    let d_bot = sd_ring(bot_p, r, t);
    // Keep where x <= 0 OR y <= 0 (remove top-right quadrant)
    let s_bot = max(d_bot, min(bot_p.x, bot_p.y));

    return min(s_top, s_bot);
}

// Complete "todos" text as a single SDF
fn sd_todos_text(p: vec2<f32>, width: f32) -> f32 {
    // Flip Y to convert from screen coords (Y-down) to SDF coords (Y-up)
    let fp = vec2<f32>(p.x, -p.y);

    let scale = width / 4.5; // Adjust for letter spacing
    let spacing = scale * 0.55;

    // Center the text horizontally
    let start_x = -2.0 * spacing;

    let t1 = sd_letter_t(fp - vec2<f32>(start_x + 0.0 * spacing, 0.0), scale);
    let o1 = sd_letter_o(fp - vec2<f32>(start_x + 0.9 * spacing, -0.08 * scale), scale);
    let d1 = sd_letter_d(fp - vec2<f32>(start_x + 1.85 * spacing, -0.03 * scale), scale);
    let o2 = sd_letter_o(fp - vec2<f32>(start_x + 2.9 * spacing, -0.08 * scale), scale);
    let s1 = sd_letter_s(fp - vec2<f32>(start_x + 3.85 * spacing, -0.03 * scale), scale);

    return min(min(min(min(t1, o1), d1), o2), s1);
}

// Legacy 3D functions (kept for compatibility)
fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

fn sd_rounded_box(p: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let q = abs(p) - b + r;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

fn sd_sphere(p: vec3<f32>, r: f32) -> f32 {
    return length(p) - r;
}

// ============================================================================
// SDF Boolean Operations
// ============================================================================

fn op_union(d1: f32, d2: f32) -> f32 {
    return min(d1, d2);
}

fn op_subtract(d1: f32, d2: f32) -> f32 {
    return max(d1, -d2);
}

fn op_intersect(d1: f32, d2: f32) -> f32 {
    return max(d1, d2);
}

// Smooth union for bevels
fn op_smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}

// ============================================================================
// Scene evaluation (2D with z-ordering)
// ============================================================================

struct HitInfo {
    dist: f32,
    color: vec3<f32>,
}

// 2D element SDF evaluation (ignores z for shape, uses z for layering)
fn get_element_sdf_2d(p: vec2<f32>, elem: Element) -> f32 {
    let local_p = p - elem.center.xy;
    let shape_type = u32(elem.params.y);
    let corner_radius = elem.params.x; // Also used as ring thickness for Ring, or text width for TodosText

    switch shape_type {
        case 0u: { // Box
            return sd_box_2d(local_p, elem.half_extents.xy);
        }
        case 1u: { // RoundedBox
            return sd_rounded_box_2d(local_p, elem.half_extents.xy, corner_radius);
        }
        case 2u: { // Circle (was Sphere)
            return sd_circle(local_p, elem.half_extents.x);
        }
        case 3u: { // Ring (hollow circle)
            return sd_ring(local_p, elem.half_extents.x, corner_radius);
        }
        case 4u: { // TodosText (procedural "todos" text)
            return sd_todos_text(local_p, corner_radius);
        }
        default: {
            return sd_box_2d(local_p, elem.half_extents.xy);
        }
    }
}

// 2D scene evaluation with z-ordering (painter's algorithm)
// Elements with higher z are drawn on top
fn scene_sdf_2d(p: vec2<f32>) -> HitInfo {
    var result: HitInfo;
    result.dist = 1e10;
    result.color = vec3<f32>(0.95, 0.95, 0.95); // Background color

    let count = uniforms.element_count;
    var top_z = -1e10;

    // Find all elements that contain this point, pick the one with highest z
    for (var i = 0u; i < count; i++) {
        let elem = elements[i];
        let d = get_element_sdf_2d(p, elem);

        // If point is inside (or on edge of) this element
        if d <= 0.0 {
            // Use element with highest z value (front-most)
            if elem.center.z > top_z {
                top_z = elem.center.z;
                result.dist = d;
                result.color = elem.color.rgb;
            }
        }
    }

    // If no element hit, check if we're close to any edge (for anti-aliasing later)
    if top_z < -1e9 {
        for (var i = 0u; i < count; i++) {
            let elem = elements[i];
            let d = get_element_sdf_2d(p, elem);
            if d < result.dist {
                result.dist = d;
            }
        }
    }

    return result;
}

// Legacy 3D functions (kept for compatibility)
fn get_element_sdf(p: vec3<f32>, elem: Element) -> f32 {
    let local_p = p - elem.center.xyz;
    let shape_type = u32(elem.params.y);
    let corner_radius = elem.params.x;

    switch shape_type {
        case 0u: { // Box
            return sd_box(local_p, elem.half_extents.xyz);
        }
        case 1u: { // RoundedBox
            return sd_rounded_box(local_p, elem.half_extents.xyz, corner_radius);
        }
        case 2u: { // Sphere
            return sd_sphere(local_p, elem.half_extents.x);
        }
        default: {
            return sd_box(local_p, elem.half_extents.xyz);
        }
    }
}

fn scene_sdf(p: vec3<f32>) -> HitInfo {
    var result: HitInfo;
    result.dist = 1e10;
    result.color = vec3<f32>(0.9, 0.9, 0.9);

    let count = uniforms.element_count;

    for (var i = 0u; i < count; i++) {
        let elem = elements[i];
        let d = get_element_sdf(p, elem);

        if d < result.dist {
            result.dist = d;
            result.color = elem.color.rgb;
        }
    }

    return result;
}

fn scene_distance(p: vec3<f32>) -> f32 {
    var min_dist = 1e10;
    let count = uniforms.element_count;

    for (var i = 0u; i < count; i++) {
        let elem = elements[i];
        let d = get_element_sdf(p, elem);
        min_dist = min(min_dist, d);
    }

    return min_dist;
}

// ============================================================================
// Normal calculation (via gradient)
// ============================================================================

fn calc_normal(p: vec3<f32>) -> vec3<f32> {
    let eps = 0.001;
    let k = vec2<f32>(1.0, -1.0);
    return normalize(
        k.xyy * scene_distance(p + k.xyy * eps) +
        k.yyx * scene_distance(p + k.yyx * eps) +
        k.yxy * scene_distance(p + k.yxy * eps) +
        k.xxx * scene_distance(p + k.xxx * eps)
    );
}

// ============================================================================
// Soft shadows
// ============================================================================

fn soft_shadow(ro: vec3<f32>, rd: vec3<f32>, mint: f32, maxt: f32, k: f32) -> f32 {
    var res = 1.0;
    var t = mint;

    for (var i = 0; i < 32; i++) {
        let h = scene_distance(ro + rd * t);
        res = min(res, k * h / t);
        t += clamp(h, 0.02, 0.2);
        if h < 0.001 || t > maxt {
            break;
        }
    }

    return clamp(res, 0.0, 1.0);
}

// ============================================================================
// Raymarching
// ============================================================================

struct RayResult {
    hit: bool,
    pos: vec3<f32>,
    color: vec3<f32>,
    steps: u32,
}

fn raymarch(ro: vec3<f32>, rd: vec3<f32>) -> RayResult {
    var result: RayResult;
    result.hit = false;
    result.pos = vec3<f32>(0.0);
    result.color = vec3<f32>(0.95);
    result.steps = 0u;

    var t = 0.0;
    let max_dist = 2000.0;
    let max_steps = 256u;
    let hit_threshold = 0.1;

    for (var i = 0u; i < max_steps; i++) {
        result.steps = i;
        let p = ro + rd * t;
        let hit = scene_sdf(p);

        if hit.dist < hit_threshold {
            result.hit = true;
            result.pos = p;
            result.color = hit.color;
            break;
        }

        if t > max_dist {
            break;
        }

        t += hit.dist * 0.9; // Slightly conservative stepping
    }

    return result;
}

// ============================================================================
// Lighting
// ============================================================================

fn shade(pos: vec3<f32>, normal: vec3<f32>, base_color: vec3<f32>) -> vec3<f32> {
    // Flat UI shading - no shadows, just ambient + subtle directional
    let light_dir = normalize(uniforms.light_dir.xyz);
    let ambient = uniforms.ambient_color.rgb;

    // Very subtle directional light for depth cues
    let ndotl = max(dot(normal, light_dir), 0.0);
    let directional = vec3<f32>(0.1) * ndotl;

    // No shadows - pure flat UI look
    let lit = ambient + directional;

    return base_color * clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0));
}

// ============================================================================
// Vertex shader - full-screen triangle
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Generate a full-screen triangle (covers entire NDC space)
    // Vertex 0: (-1, -1)
    // Vertex 1: (3, -1)
    // Vertex 2: (-1, 3)
    let x = f32((vertex_index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vertex_index & 2u) * 2.0 - 1.0;

    output.position = vec4<f32>(x, y, 0.0, 1.0);
    // UV: map from [-1,1] to [0,1], flip y for standard screen coordinates
    output.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);

    return output;
}

// ============================================================================
// Fragment shader
// ============================================================================

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv;
    let pixel = uv * uniforms.resolution;

    // 2D SDF evaluation (no raymarching for flat UI)
    let result = scene_sdf_2d(pixel);

    // Gamma correction
    let gamma_color = pow(result.color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(gamma_color, 1.0);
}
