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
// SDF Primitives
// ============================================================================

// Box SDF (sharp edges)
fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Rounded box SDF
fn sd_rounded_box(p: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let q = abs(p) - b + r;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

// Sphere SDF
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
// Scene evaluation
// ============================================================================

struct HitInfo {
    dist: f32,
    color: vec3<f32>,
}

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

// Just the distance (for shadows)
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
    let light_dir = normalize(uniforms.light_dir.xyz);
    let light_color = uniforms.light_color.rgb;
    let ambient = uniforms.ambient_color.rgb;

    // Diffuse lighting
    let ndotl = max(dot(normal, light_dir), 0.0);
    let diffuse = light_color * ndotl;

    // Soft shadow
    let shadow = soft_shadow(pos + normal * 1.0, light_dir, 1.0, 500.0, 8.0);

    // Combine
    let lit = ambient + diffuse * shadow;

    return base_color * lit;
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

    // Generate a full-screen triangle
    // Vertex 0: (-1, -1)
    // Vertex 1: (3, -1)
    // Vertex 2: (-1, 3)
    let x = f32(i32(vertex_index) - 1);
    let y = f32(i32(vertex_index & 1u) * 4 - 1);

    output.position = vec4<f32>(x, y, 0.0, 1.0);
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

    // Camera setup (orthographic-like for UI)
    let camera_pos = uniforms.camera_pos.xyz;
    let camera_target = uniforms.camera_target.xyz;

    // For UI, we use a mostly orthographic projection
    // Ray origin is at the pixel position, looking into the screen
    let ro = vec3<f32>(pixel.x, pixel.y, camera_pos.z);
    let rd = normalize(vec3<f32>(0.0, 0.0, -1.0));

    // Raymarch
    let result = raymarch(ro, rd);

    if result.hit {
        let normal = calc_normal(result.pos);
        let color = shade(result.pos, normal, result.color);

        // Gamma correction
        let gamma_color = pow(color, vec3<f32>(1.0 / 2.2));
        return vec4<f32>(gamma_color, 1.0);
    }

    // Background color
    return vec4<f32>(0.95, 0.95, 0.95, 1.0);
}
