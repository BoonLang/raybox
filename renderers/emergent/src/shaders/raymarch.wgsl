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
// Procedural SDF Shapes
// ============================================================================

// Checkmark shape for completed todos
fn sd_checkmark(p: vec2<f32>, scale: f32) -> f32 {
    let thickness = 0.06 * scale;

    // Short stroke going down-left (the small part of checkmark)
    let p1 = p - vec2<f32>(-0.08, -0.02) * scale;
    let angle1 = -0.65;
    let c1 = cos(angle1);
    let s1 = sin(angle1);
    let p1_rot = vec2<f32>(p1.x * c1 + p1.y * s1, -p1.x * s1 + p1.y * c1);
    let stroke1 = sd_box_2d(p1_rot, vec2<f32>(0.10 * scale, thickness));

    // Long stroke going up-right (the long part of checkmark)
    let p2 = p - vec2<f32>(0.06, 0.06) * scale;
    let angle2 = 0.5;
    let c2 = cos(angle2);
    let s2 = sin(angle2);
    let p2_rot = vec2<f32>(p2.x * c2 + p2.y * s2, -p2.x * s2 + p2.y * c2);
    let stroke2 = sd_box_2d(p2_rot, vec2<f32>(0.16 * scale, thickness));

    return min(stroke1, stroke2);
}

// Chevron/arrow shape for toggle-all (pointing down)
fn sd_chevron(p: vec2<f32>, scale: f32) -> f32 {
    let thickness = 0.03 * scale;

    // Left arm of chevron
    let p1 = p - vec2<f32>(-0.10, 0.05) * scale;
    let angle1 = -0.75;
    let c1 = cos(angle1);
    let s1 = sin(angle1);
    let p1_rot = vec2<f32>(p1.x * c1 + p1.y * s1, -p1.x * s1 + p1.y * c1);
    let arm1 = sd_box_2d(p1_rot, vec2<f32>(0.14 * scale, thickness));

    // Right arm of chevron
    let p2 = p - vec2<f32>(0.10, 0.05) * scale;
    let angle2 = 0.75;
    let c2 = cos(angle2);
    let s2 = sin(angle2);
    let p2_rot = vec2<f32>(p2.x * c2 + p2.y * s2, -p2.x * s2 + p2.y * c2);
    let arm2 = sd_box_2d(p2_rot, vec2<f32>(0.14 * scale, thickness));

    return min(arm1, arm2);
}

// ============================================================================
// SDF Letters for "todos" title (raymarched text - infinite resolution)
// ============================================================================

// Letter 't' - vertical stem with crossbar
fn sd_letter_t(p: vec2<f32>, scale: f32) -> f32 {
    let w = 0.08 * scale;  // stroke width

    // Vertical stem (slightly off-center to left for better balance)
    let stem = sd_box_2d(p - vec2<f32>(-0.02 * scale, 0.0), vec2<f32>(w, 0.4 * scale));

    // Crossbar at top
    let crossbar = sd_box_2d(p - vec2<f32>(0.0, 0.25 * scale), vec2<f32>(0.2 * scale, w));

    return min(stem, crossbar);
}

// Letter 'o' - circle (ring)
fn sd_letter_o(p: vec2<f32>, scale: f32) -> f32 {
    let outer_r = 0.3 * scale;
    let inner_r = 0.14 * scale;

    let outer = sd_circle(p, outer_r);
    let inner = sd_circle(p, inner_r);

    return max(outer, -inner);  // Subtract inner from outer
}

// Letter 'd' - vertical stem on right + bowl on left
fn sd_letter_d(p: vec2<f32>, scale: f32) -> f32 {
    let w = 0.08 * scale;

    // Vertical stem on right (negative x in shader coords = right on screen)
    let stem = sd_box_2d(p - vec2<f32>(-0.15 * scale, 0.1 * scale), vec2<f32>(w, 0.5 * scale));

    // Bowl (circle) on left, positioned lower
    let bowl_outer = sd_circle(p - vec2<f32>(0.0, -0.1 * scale), 0.3 * scale);
    let bowl_inner = sd_circle(p - vec2<f32>(0.0, -0.1 * scale), 0.14 * scale);
    let bowl = max(bowl_outer, -bowl_inner);

    return min(stem, bowl);
}

// Letter 's' - stylized S shape using curved segments
fn sd_letter_s(p: vec2<f32>, scale: f32) -> f32 {
    let w = 0.08 * scale;
    let r = 0.18 * scale;

    // Top arc (right-facing C)
    let top_center = vec2<f32>(0.0, 0.15 * scale);
    let top_outer = sd_circle(p - top_center, r + w);
    let top_inner = sd_circle(p - top_center, r - w);
    let top_arc = max(top_outer, -top_inner);
    // Cut off right side of top arc
    let top_cut = max(top_arc, p.x - 0.05 * scale);

    // Bottom arc (left-facing C)
    let bot_center = vec2<f32>(0.0, -0.15 * scale);
    let bot_outer = sd_circle(p - bot_center, r + w);
    let bot_inner = sd_circle(p - bot_center, r - w);
    let bot_arc = max(bot_outer, -bot_inner);
    // Cut off left side of bottom arc
    let bot_cut = max(bot_arc, -p.x - 0.05 * scale);

    return min(top_cut, bot_cut);
}

// Complete "todos" word as single SDF - positions letters with proper kerning
fn sd_todos_word(p: vec2<f32>, scale: f32) -> f32 {
    let spacing = 0.55 * scale;  // Letter spacing

    // Position letters from left to right: t(-2), o(-1), d(0), o(+1), s(+2)
    // Negative offset = left side of word, Positive offset = right side
    // Each letter needs mirrored X for correct orientation

    // t - leftmost letter (negative x offset)
    let t_pos = p - vec2<f32>(-2.0 * spacing, 0.0);
    let t = sd_letter_t(vec2<f32>(-t_pos.x, t_pos.y), scale);  // Mirror X

    // o - second letter (symmetric, no mirror needed)
    let o1_pos = p - vec2<f32>(-1.0 * spacing, -0.1 * scale);
    let o1 = sd_letter_o(o1_pos, scale);

    // d - center letter (no mirror - stem stays on right side)
    let d_pos = p - vec2<f32>(0.0, 0.0);
    let d = sd_letter_d(d_pos, scale);

    // o - fourth letter (symmetric, no mirror needed)
    let o2_pos = p - vec2<f32>(1.0 * spacing, -0.1 * scale);
    let o2 = sd_letter_o(o2_pos, scale);

    // s - rightmost letter (positive x offset)
    let s_pos = p - vec2<f32>(2.0 * spacing, 0.0);
    let s = sd_letter_s(vec2<f32>(-s_pos.x, s_pos.y), scale);  // Mirror X

    return min(min(min(min(t, o1), d), o2), s);
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
// Anti-aliasing helper
// ============================================================================

// Calculate smooth alpha for SDF edge anti-aliasing using screen-space derivatives
// fwidth() gives us the rate of change of distance across neighboring pixels,
// which is exactly what we need for mathematically correct AA at any scale.
fn sdf_alpha(d: f32) -> f32 {
    // fwidth(d) = abs(dFdx(d)) + abs(dFdy(d)) - how much d changes per pixel
    // Multiply by 0.75 for crisp AA (0.5 = very crisp, 1.0 = softer, 1.5+ = blurry)
    let aa_width = fwidth(d) * 0.75;
    // Clamp to prevent issues with very small/large values
    let clamped_aa = clamp(aa_width, 0.0001, 2.0);
    // smoothstep: d < -aa = 1.0 (inside), d > +aa = 0.0 (outside), smooth transition between
    return 1.0 - smoothstep(-clamped_aa, clamped_aa, d);
}

// ============================================================================
// Anti-aliasing Techniques
// ============================================================================

// APPROACH 1: Sharpen - Analytical edge enhancement using SDF
// AGGRESSIVE: Strength increased from 0.25 to 3.0 for visible effect
fn sharpen_at_edge(color: vec3<f32>, sdf_dist: f32, edge_proximity: f32) -> vec3<f32> {
    let SHARPEN_STRENGTH: f32 = 3.0;  // WAS 0.25 - now very aggressive
    let fw = fwidth(sdf_dist);
    let edge_width = fw * 4.0;  // WAS 2.0 - wider edge detection
    let edge_factor = 1.0 - smoothstep(0.0, edge_width, abs(sdf_dist));
    let luminance = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    let gray = vec3<f32>(luminance);
    let high_pass = color - gray;
    let sharpen_amount = edge_factor * SHARPEN_STRENGTH * edge_proximity;
    let sharpened = color + high_pass * sharpen_amount * 2.0;  // Extra boost
    return clamp(sharpened, vec3<f32>(0.0), vec3<f32>(1.0));
}

// APPROACH 1: EXTREME SHARPEN - Creates visible halos around edges
// EXTREME strength to make text look "etched" with dark outlines
fn sharpen_blend(pixel: vec2<f32>, center_color: vec3<f32>) -> vec3<f32> {
    // Sample neighbors at 1px distance
    let n = scene_sdf_2d(pixel + vec2<f32>(0.0, -1.0)).color;
    let s = scene_sdf_2d(pixel + vec2<f32>(0.0, 1.0)).color;
    let e = scene_sdf_2d(pixel + vec2<f32>(1.0, 0.0)).color;
    let w = scene_sdf_2d(pixel + vec2<f32>(-1.0, 0.0)).color;

    // Calculate blur (average of neighbors)
    let blur = (n + s + e + w) * 0.25;

    // EXTREME unsharp mask - strength 4.0 creates obvious halos
    let SHARPEN_STRENGTH: f32 = 4.0;
    let high_pass = center_color - blur;
    let sharpened = center_color + high_pass * SHARPEN_STRENGTH;

    return clamp(sharpened, vec3<f32>(0.0), vec3<f32>(1.0));
}

// APPROACH 2: HEAVY BLUR - 2px radius blur for visibly soft/blurry text
// Much larger sampling radius = obviously blurry result
fn subpixel_blend(pixel: vec2<f32>, center_color: vec3<f32>) -> vec3<f32> {
    // Sample at 2px distance for visible blur
    let n1 = scene_sdf_2d(pixel + vec2<f32>(0.0, -1.0)).color;
    let s1 = scene_sdf_2d(pixel + vec2<f32>(0.0, 1.0)).color;
    let e1 = scene_sdf_2d(pixel + vec2<f32>(1.0, 0.0)).color;
    let w1 = scene_sdf_2d(pixel + vec2<f32>(-1.0, 0.0)).color;
    let n2 = scene_sdf_2d(pixel + vec2<f32>(0.0, -2.0)).color;
    let s2 = scene_sdf_2d(pixel + vec2<f32>(0.0, 2.0)).color;
    let e2 = scene_sdf_2d(pixel + vec2<f32>(2.0, 0.0)).color;
    let w2 = scene_sdf_2d(pixel + vec2<f32>(-2.0, 0.0)).color;

    // Heavy blur - average ALL samples with equal weight
    // This creates very visible softening
    return (center_color + n1 + s1 + e1 + w1 + n2 + s2 + e2 + w2) / 9.0;
}

// APPROACH 3: EXTREME HORIZONTAL BLUR - Always blurs horizontally
// Creates visible horizontal smearing/streaking effect
fn fxaa_blend(pixel: vec2<f32>, center_color: vec3<f32>) -> vec3<f32> {
    // Sample horizontal neighbors at 1px and 2px for strong horizontal blur
    let e1 = scene_sdf_2d(pixel + vec2<f32>(1.0, 0.0)).color;
    let w1 = scene_sdf_2d(pixel + vec2<f32>(-1.0, 0.0)).color;
    let e2 = scene_sdf_2d(pixel + vec2<f32>(2.0, 0.0)).color;
    let w2 = scene_sdf_2d(pixel + vec2<f32>(-2.0, 0.0)).color;

    // EXTREME: 100% horizontal blur - completely replaces center
    // This creates very obvious horizontal smearing
    return (center_color + e1 + w1 + e2 + w2) / 5.0;
}

// APPROACH 4: EXTREME 5x5 GAUSSIAN BLUR - Maximum softness/blur
// Full 5x5 gaussian kernel for extremely soft/blurry result
fn cmaa_blend(pixel: vec2<f32>, center_color: vec3<f32>) -> vec3<f32> {
    // Sample a 5x5 neighborhood - this is EXTREMELY heavy blur
    // Row -2
    let r2_c2 = scene_sdf_2d(pixel + vec2<f32>(-2.0, -2.0)).color;
    let r2_c1 = scene_sdf_2d(pixel + vec2<f32>(-1.0, -2.0)).color;
    let r2_c0 = scene_sdf_2d(pixel + vec2<f32>( 0.0, -2.0)).color;
    let r2_c3 = scene_sdf_2d(pixel + vec2<f32>( 1.0, -2.0)).color;
    let r2_c4 = scene_sdf_2d(pixel + vec2<f32>( 2.0, -2.0)).color;
    // Row -1
    let r1_c2 = scene_sdf_2d(pixel + vec2<f32>(-2.0, -1.0)).color;
    let r1_c1 = scene_sdf_2d(pixel + vec2<f32>(-1.0, -1.0)).color;
    let r1_c0 = scene_sdf_2d(pixel + vec2<f32>( 0.0, -1.0)).color;
    let r1_c3 = scene_sdf_2d(pixel + vec2<f32>( 1.0, -1.0)).color;
    let r1_c4 = scene_sdf_2d(pixel + vec2<f32>( 2.0, -1.0)).color;
    // Row 0
    let r0_c2 = scene_sdf_2d(pixel + vec2<f32>(-2.0,  0.0)).color;
    let r0_c1 = scene_sdf_2d(pixel + vec2<f32>(-1.0,  0.0)).color;
    let r0_c3 = scene_sdf_2d(pixel + vec2<f32>( 1.0,  0.0)).color;
    let r0_c4 = scene_sdf_2d(pixel + vec2<f32>( 2.0,  0.0)).color;
    // Row +1
    let r3_c2 = scene_sdf_2d(pixel + vec2<f32>(-2.0,  1.0)).color;
    let r3_c1 = scene_sdf_2d(pixel + vec2<f32>(-1.0,  1.0)).color;
    let r3_c0 = scene_sdf_2d(pixel + vec2<f32>( 0.0,  1.0)).color;
    let r3_c3 = scene_sdf_2d(pixel + vec2<f32>( 1.0,  1.0)).color;
    let r3_c4 = scene_sdf_2d(pixel + vec2<f32>( 2.0,  1.0)).color;
    // Row +2
    let r4_c2 = scene_sdf_2d(pixel + vec2<f32>(-2.0,  2.0)).color;
    let r4_c1 = scene_sdf_2d(pixel + vec2<f32>(-1.0,  2.0)).color;
    let r4_c0 = scene_sdf_2d(pixel + vec2<f32>( 0.0,  2.0)).color;
    let r4_c3 = scene_sdf_2d(pixel + vec2<f32>( 1.0,  2.0)).color;
    let r4_c4 = scene_sdf_2d(pixel + vec2<f32>( 2.0,  2.0)).color;

    // Simple box blur of ALL 25 samples for MAXIMUM blur
    let total = r2_c2 + r2_c1 + r2_c0 + r2_c3 + r2_c4 +
                r1_c2 + r1_c1 + r1_c0 + r1_c3 + r1_c4 +
                r0_c2 + r0_c1 + center_color + r0_c3 + r0_c4 +
                r3_c2 + r3_c1 + r3_c0 + r3_c3 + r3_c4 +
                r4_c2 + r4_c1 + r4_c0 + r4_c3 + r4_c4;

    return total / 25.0;
}

// ============================================================================
// Scene evaluation (2D with z-ordering)
// ============================================================================

struct HitInfo {
    dist: f32,
    color: vec3<f32>,
    alpha: f32,  // For edge anti-aliasing
}

// 2D element SDF evaluation (ignores z for shape, uses z for layering)
fn get_element_sdf_2d(p: vec2<f32>, elem: Element) -> f32 {
    let local_p = p - elem.center.xy;
    let shape_type = u32(elem.params.y);
    let corner_radius = elem.params.x; // Also used as ring thickness for Ring, or size for shapes

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
        case 4u: { // Checkmark
            return sd_checkmark(local_p, corner_radius);
        }
        case 5u: { // Chevron
            return sd_chevron(local_p, corner_radius);
        }
        case 6u: { // TodosWord (raymarched "todos" title)
            return sd_todos_word(local_p, corner_radius);
        }
        default: {
            return sd_box_2d(local_p, elem.half_extents.xy);
        }
    }
}

// 2D scene evaluation with z-ordering and proper alpha compositing
// Elements with higher z are drawn on top, with smooth anti-aliased edges
fn scene_sdf_2d(p: vec2<f32>) -> HitInfo {
    var result: HitInfo;
    result.dist = 1e10;
    result.color = vec3<f32>(0.95, 0.95, 0.95); // Background color
    result.alpha = 1.0; // Start with opaque background

    let count = uniforms.element_count;

    // Simple approach: find the top-most element with significant alpha
    // and blend its anti-aliased edge with the background/layer below
    var top_z = -1e10;
    var top_alpha = 0.0;
    var top_color = vec3<f32>(0.0);
    var min_dist = 1e10;

    for (var i = 0u; i < count; i++) {
        let elem = elements[i];
        let d = get_element_sdf_2d(p, elem);

        // Track minimum distance for debugging
        if d < min_dist {
            min_dist = d;
        }

        // Calculate smooth alpha for this element
        let alpha = sdf_alpha(d);

        // Only consider elements with non-zero alpha contribution
        if alpha > 0.001 {
            // Higher z wins (painter's algorithm with alpha)
            if elem.center.z > top_z {
                // If new layer has higher z, it goes on top
                // Blend: new layer over old accumulated result
                top_z = elem.center.z;
                top_alpha = alpha;
                top_color = elem.color.rgb;
            } else if elem.center.z == top_z && alpha > top_alpha {
                // Same z-level, stronger alpha wins
                top_alpha = alpha;
                top_color = elem.color.rgb;
            }
        }
    }

    result.dist = min_dist;

    // Composite the top element over background
    if top_alpha > 0.001 {
        // Alpha blending: result = foreground * alpha + background * (1 - alpha)
        result.color = top_color * top_alpha + result.color * (1.0 - top_alpha);
        result.alpha = 1.0; // Final output is always opaque (composited onto background)
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

    // ========================================
    // APPROACH 4: EXTREME BOX BLUR/CMAA (YELLOW indicator)
    // Always applies 5x5 box blur for visible effect
    // ========================================
    let result = scene_sdf_2d(pixel);
    let center_color = result.color;
    let final_color = cmaa_blend(pixel, center_color);

    // Add YELLOW indicator bar at top (50px height)
    var output_color = final_color;
    if pixel.y < 50.0 {
        output_color = vec3<f32>(1.0, 1.0, 0.0); // YELLOW for CMAA
    }

    // Gamma correction
    let gamma_color = pow(output_color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(gamma_color, 1.0);
}
