// SDF Raymarching Shader
// Renders signed distance field primitives with soft shadows and AO

// ============================================================================
// Uniform Buffer
// ============================================================================

struct Uniforms {
    float4x4 invViewProj;
    float4 cameraPosTime;     // xyz = cameraPos, w = time
    float4 lightDirIntensity; // xyz = lightDir, w = lightIntensity
    float4 renderParams;      // xy = resolution, z = aoRadius, w = shadowSoftness
};

[[vk::binding(0, 0)]]
ConstantBuffer<Uniforms> uniforms;

// ============================================================================
// Fullscreen Quad Vertex Shader
// ============================================================================

struct VertexOutput {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

[shader("vertex")]
VertexOutput vs_main(uint vertexID : SV_VertexID) {
    VertexOutput output;

    // Generate fullscreen triangle (3 vertices cover the screen)
    // Vertex 0: (-1, -1), Vertex 1: (3, -1), Vertex 2: (-1, 3)
    float2 pos = float2(
        (vertexID == 1) ? 3.0 : -1.0,
        (vertexID == 2) ? 3.0 : -1.0
    );

    output.position = float4(pos, 0.0, 1.0);
    output.uv = pos * 0.5 + 0.5;

    return output;
}

// ============================================================================
// SDF Primitives
// ============================================================================

float sdSphere(float3 p, float r) {
    return length(p) - r;
}

float sdBox(float3 p, float3 b) {
    float3 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0);
}

float sdCylinder(float3 p, float h, float r) {
    float2 d = abs(float2(length(p.xz), p.y)) - float2(r, h);
    return min(max(d.x, d.y), 0.0) + length(max(d, 0.0));
}

float sdTorus(float3 p, float2 t) {
    float2 q = float2(length(p.xz) - t.x, p.y);
    return length(q) - t.y;
}

float sdPlane(float3 p, float3 n, float h) {
    return dot(p, n) + h;
}

float sdCappedCylinder(float3 p, float h, float r) {
    float2 d = abs(float2(length(p.xz), p.y)) - float2(r, h);
    return min(max(d.x, d.y), 0.0) + length(max(d, 0.0));
}

// Round box for smoother edges
float sdRoundBox(float3 p, float3 b, float r) {
    float3 q = abs(p) - b + r;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

// ============================================================================
// SDF Operations
// ============================================================================

// Smooth minimum (for blending shapes)
float smin(float a, float b, float k) {
    float h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

// Smooth maximum
float smax(float a, float b, float k) {
    return -smin(-a, -b, k);
}

// Union
float opUnion(float d1, float d2) {
    return min(d1, d2);
}

// Subtraction
float opSubtract(float d1, float d2) {
    return max(d1, -d2);
}

// Intersection
float opIntersect(float d1, float d2) {
    return max(d1, d2);
}

// ============================================================================
// Scene Definition
// ============================================================================

struct Hit {
    float dist;
    int materialId;
};

Hit opUnionHit(Hit a, Hit b) {
    if (a.dist < b.dist) return a;
    return b;
}

// Coffee mug SDF - proper hollow mug with handle
float sdMug(float3 p) {
    // Mug body - outer cylinder (radius 0.35, half-height 0.5)
    float outer = sdCappedCylinder(p, 0.5, 0.35);

    // Handle - vertical D-shaped loop on the left side
    // Put handle center at x=-0.40, so with major radius 0.20
    // the inner edge is at x=-0.40+0.20=-0.20, well inside mug
    float3 hp = p - float3(-0.40, 0.0, 0.0);
    // Swap Y and Z for vertical ring
    float handle = sdTorus(float3(hp.x, hp.z, hp.y), float2(0.20, 0.06));
    // Cut off the inner half that's inside the mug
    handle = max(handle, p.x + 0.35);

    // Blend handle with solid cylinder
    float mugWithHandle = smin(outer, handle, 0.15);

    // Hollow out
    float3 innerP = p - float3(0, 0.06, 0);
    float inner = sdCappedCylinder(innerP, 0.5, 0.28);

    return max(mugWithHandle, -inner);
}

Hit mapScene(float3 p) {
    Hit result;
    result.dist = 1e10;
    result.materialId = 0;

    // Ground plane
    Hit ground;
    ground.dist = sdPlane(p, float3(0, 1, 0), 0.5);
    ground.materialId = 1; // Gray ground
    result = opUnionHit(result, ground);

    // Sphere (left)
    Hit sphere;
    sphere.dist = sdSphere(p - float3(-1.5, 0.2, 0.0), 0.7);
    sphere.materialId = 2; // Red
    result = opUnionHit(result, sphere);

    // Box (right)
    Hit box;
    float3 boxPos = p - float3(1.5, 0.0, 0.0);
    // Rotate box slightly
    float angle = 0.3;
    float c = cos(angle);
    float s = sin(angle);
    boxPos.xz = float2(c * boxPos.x - s * boxPos.z, s * boxPos.x + c * boxPos.z);
    box.dist = sdBox(boxPos, float3(0.5, 0.5, 0.5));
    box.materialId = 3; // Blue
    result = opUnionHit(result, box);

    // Coffee mug (center) - positioned on top of ground plane
    Hit mug;
    mug.dist = sdMug(p - float3(0.0, 0.0, 0.0));
    mug.materialId = 4; // Cream/white
    result = opUnionHit(result, mug);

    return result;
}

// ============================================================================
// Materials
// ============================================================================

float3 getMaterialColor(int id) {
    switch (id) {
        case 1: return float3(0.4, 0.4, 0.4);   // Ground - gray
        case 2: return float3(0.9, 0.2, 0.15);  // Sphere - red
        case 3: return float3(0.15, 0.3, 0.9);  // Box - blue
        case 4: return float3(0.95, 0.93, 0.88); // Mug - cream
        default: return float3(0.5, 0.5, 0.5);
    }
}

// ============================================================================
// Normal Calculation
// ============================================================================

float3 calcNormal(float3 p) {
    const float eps = 0.001;
    float3 n;
    n.x = mapScene(p + float3(eps, 0, 0)).dist - mapScene(p - float3(eps, 0, 0)).dist;
    n.y = mapScene(p + float3(0, eps, 0)).dist - mapScene(p - float3(0, eps, 0)).dist;
    n.z = mapScene(p + float3(0, 0, eps)).dist - mapScene(p - float3(0, 0, eps)).dist;
    return normalize(n);
}

// ============================================================================
// Soft Shadows
// ============================================================================

float calcSoftShadow(float3 ro, float3 rd, float mint, float maxt, float k) {
    float res = 1.0;
    float t = mint;

    for (int i = 0; i < 32 && t < maxt; i++) {
        float h = mapScene(ro + rd * t).dist;
        if (h < 0.001) {
            return 0.0;
        }
        res = min(res, k * h / t);
        t += clamp(h, 0.01, 0.2);
    }

    return clamp(res, 0.0, 1.0);
}

// ============================================================================
// Ambient Occlusion
// ============================================================================

float calcAO(float3 pos, float3 nor) {
    float occ = 0.0;
    float sca = 1.0;
    float aoRadius = uniforms.renderParams.z;

    for (int i = 0; i < 5; i++) {
        float h = 0.01 + aoRadius * float(i) / 4.0;
        float d = mapScene(pos + h * nor).dist;
        occ += (h - d) * sca;
        sca *= 0.95;
    }

    return clamp(1.0 - 3.0 * occ, 0.0, 1.0);
}

// ============================================================================
// Raymarching
// ============================================================================

struct RayHit {
    float t;
    int materialId;
    bool hit;
};

RayHit raymarch(float3 ro, float3 rd) {
    RayHit result;
    result.t = 0.0;
    result.materialId = 0;
    result.hit = false;

    const float MAX_DIST = 30.0;
    const int MAX_STEPS = 128;

    float t = 0.0;

    for (int i = 0; i < MAX_STEPS && t < MAX_DIST; i++) {
        float3 p = ro + rd * t;
        Hit h = mapScene(p);

        float eps = 0.001 + t * 0.0002;
        if (h.dist < eps) {
            result.t = t;
            result.materialId = h.materialId;
            result.hit = true;
            return result;
        }

        t += h.dist * 0.9;
    }

    return result;
}

// ============================================================================
// Sky/Background
// ============================================================================

float3 getSkyColor(float3 rd) {
    float t = 0.5 * (rd.y + 1.0);
    return lerp(float3(0.8, 0.85, 0.9), float3(0.4, 0.6, 0.9), t);
}

// ============================================================================
// Main Rendering
// ============================================================================

float3 render(float3 ro, float3 rd) {
    RayHit hit = raymarch(ro, rd);

    if (!hit.hit) {
        return getSkyColor(rd);
    }

    float3 pos = ro + rd * hit.t;

    // Skip normal calc for ground plane - always (0,1,0)
    float3 nor;
    if (hit.materialId == 1) {
        nor = float3(0, 1, 0);
    } else {
        nor = calcNormal(pos);
    }

    float3 col = getMaterialColor(hit.materialId);

    // Unpack uniforms
    float3 lightDir = normalize(uniforms.lightDirIntensity.xyz);
    float lightIntensity = uniforms.lightDirIntensity.w;
    float shadowSoftness = uniforms.renderParams.w;

    // Lighting
    float diff = max(dot(nor, lightDir), 0.0);

    // Soft shadows
    float shadow = calcSoftShadow(pos + nor * 0.01, lightDir, 0.02, 10.0, shadowSoftness);

    // Ambient occlusion
    float ao = calcAO(pos, nor);

    // Ambient light (reduced for more contrast)
    float3 ambient = float3(0.08, 0.09, 0.12);

    // Combine lighting with stronger direct light
    float3 lin = ambient * ao;
    lin += diff * shadow * lightIntensity * 1.5 * float3(1.0, 0.98, 0.95);

    // Apply lighting to color
    col *= lin;

    // Minimal fog
    float fog = 1.0 - exp(-hit.t * 0.003);
    col = lerp(col, getSkyColor(rd) * 0.8, fog);

    return col;
}

// ============================================================================
// Fragment Shader
// ============================================================================

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    // Calculate ray from camera through pixel
    float2 uv = input.uv;

    // Convert to NDC (-1 to 1)
    float2 ndc = uv * 2.0 - 1.0;

    // Unproject ray endpoints
    float4 nearPoint = mul(uniforms.invViewProj, float4(ndc, 0.0, 1.0));
    float4 farPoint = mul(uniforms.invViewProj, float4(ndc, 1.0, 1.0));

    nearPoint /= nearPoint.w;
    farPoint /= farPoint.w;

    float3 ro = uniforms.cameraPosTime.xyz;
    float3 rd = normalize(farPoint.xyz - nearPoint.xyz);

    // Render
    float3 col = render(ro, rd);

    // Gamma correction
    col = pow(col, float3(1.0 / 2.2));

    return float4(col, 1.0);
}
