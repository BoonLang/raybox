struct Globals {
    screen_size: vec2<f32>;
    light_dir_deg: vec2<f32>;
    ambient: f32;
    add_rim: f32;
    ao_strength: f32;
    _pad: f32;
};
@group(0) @binding(0)
var<uniform> globals: Globals;

struct VsIn {
    @location(0) pos: vec3<f32>;
    @location(1) normal: vec3<f32>;
    @location(2) rect: vec4<f32>;
    @location(3) depth: f32;
    @location(4) elevation: f32;
    @location(5) color: vec4<f32>;
};

struct VsOut {
    @builtin(position) position: vec4<f32>;
    @location(0) normal: vec3<f32>;
    @location(1) color: vec4<f32>;
    @location(2) uv: vec2<f32>;
    @location(3) height: f32;
};

fn to_clip(x: f32, y: f32, z: f32) -> vec4<f32> {
    let sx = globals.screen_size.x;
    let sy = globals.screen_size.y;
    let ndc_x = (x / sx) * 2.0 - 1.0;
    let ndc_y = 1.0 - (y / sy) * 2.0;
    let ndc_z = z / 200.0;
    return vec4<f32>(ndc_x, ndc_y, ndc_z, 1.0);
}

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    let x = input.rect.x + input.pos.x * input.rect.z;
    let y = input.rect.y + input.pos.y * input.rect.w;
    let z = input.elevation + input.pos.z * input.depth;
    out.position = to_clip(x, y, z);
    out.normal = input.normal;
    out.color = input.color;
    out.uv = input.pos.xy;
    out.height = z;
    return out;
}

fn deg2rad(d: f32) -> f32 { return d * 0.017453292519943295; }

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let az = deg2rad(globals.light_dir_deg.x);
    let alt = deg2rad(globals.light_dir_deg.y);
    let L = normalize(vec3<f32>(cos(alt) * cos(az), cos(alt) * sin(az), sin(alt)));
    // Fake bevel by biasing face normals toward +Z near edges
    let edge = min(min(input.uv.x, 1.0 - input.uv.x), min(input.uv.y, 1.0 - input.uv.y));
    let bevel = clamp(edge * 6.0, 0.0, 1.0); // steeper near edges
    let blended_normal = normalize(vec3(input.normal.xy * (0.4 + 0.6 * bevel), input.normal.z + (1.0 - bevel) * 0.6));
    let N = blended_normal;
    let diff = max(dot(N, L), 0.0);
    let ambient = globals.ambient;

    // Simple rim light to keep silhouettes visible in top-down view
    let V = vec3<f32>(0.0, 0.0, 1.0);
    let rim = globals.add_rim * pow(1.0 - max(dot(N, V), 0.0), 2.0);

    let facing = clamp(N.z * 0.35 + 0.65, 0.4, 1.1);

    // Slight height-based darkening to ground tall stacks
    let ground_dark = 1.0 - clamp(input.height / 120.0, 0.0, 0.12);

    // Edge/corner darkening (cheap AO)
    let edge = min(min(input.uv.x, 1.0 - input.uv.x), min(input.uv.y, 1.0 - input.uv.y));
    let edge_ao = 1.0 - globals.ao_strength * clamp((0.22 - edge) * 4.0, 0.0, 1.0);

    let lighting = (ambient + diff * (1.0 - ambient) + rim * 0.15) * facing * ground_dark * edge_ao;
    let rgb = input.color.rgb * lighting;
    return vec4<f32>(rgb, input.color.a);
}
