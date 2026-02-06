// SDF Towers Scene - Abstract cityscape with stacked boxes

struct Uniforms {
    float4x4 invViewProj;
    float4 cameraPosTime;     // xyz = cameraPos, w = time
    float4 lightDirIntensity; // xyz = lightDir, w = lightIntensity
    float4 renderParams;      // xy = resolution, z = aoRadius, w = shadowSoftness
};

[[vk::binding(0, 0)]]
ConstantBuffer<Uniforms> uniforms;

struct VertexOutput {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

[shader("vertex")]
VertexOutput vs_main(uint vertexID : SV_VertexID) {
    VertexOutput output;
    float2 pos = float2(
        (vertexID == 1) ? 3.0 : -1.0,
        (vertexID == 2) ? 3.0 : -1.0
    );
    output.position = float4(pos, 0.0, 1.0);
    output.uv = pos * 0.5 + 0.5;
    return output;
}

float sdBox(float3 p, float3 b) {
    float3 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0);
}

float sdPlane(float3 p, float3 n, float h) {
    return dot(p, n) + h;
}

// Simple hash for pseudo-random heights
float hash(float2 p) {
    return fract(sin(dot(p, float2(127.1, 311.7))) * 43758.5453);
}

struct Hit {
    float dist;
    int materialId;
};

Hit opUnionHit(Hit a, Hit b) {
    if (a.dist < b.dist) return a;
    return b;
}

Hit mapScene(float3 p) {
    Hit result;
    result.dist = 1e10;
    result.materialId = 0;

    // Ground plane
    Hit ground;
    ground.dist = sdPlane(p, float3(0, 1, 0), 0.5);
    ground.materialId = 1;
    result = opUnionHit(result, ground);

    // 3x3 spatial lookup with clamped center (was 7x7)
    int cx = clamp(int(round(p.x)), -3, 3);
    int cz = clamp(int(round(p.z)), -3, 3);
    for (int di = -1; di <= 1; di++) {
        for (int dj = -1; dj <= 1; dj++) {
            int i = cx + di;
            int j = cz + dj;
            if (i < -3 || i > 3 || j < -3 || j > 3) continue;

            float2 gridPos = float2(float(i), float(j));
            float h = hash(gridPos) * 2.0 + 0.5;  // Height: 0.5 to 2.5
            float w = 0.3 + hash(gridPos + 100.0) * 0.2;  // Width: 0.3 to 0.5

            float3 towerPos = float3(i * 1.0, h * 0.5 - 0.5, j * 1.0);
            float3 towerSize = float3(w, h, w);

            Hit tower;
            tower.dist = sdBox(p - towerPos, towerSize * 0.5);
            tower.materialId = 2 + int(hash(gridPos + 50.0) * 5.99);
            result = opUnionHit(result, tower);
        }
    }

    return result;
}

// Coarse scene for AO - only ground + nearest tower
float mapSceneAO(float3 p) {
    float d = sdPlane(p, float3(0, 1, 0), 0.5);
    int cx = clamp(int(round(p.x)), -3, 3);
    int cz = clamp(int(round(p.z)), -3, 3);
    float2 gridPos = float2(float(cx), float(cz));
    float h = hash(gridPos) * 2.0 + 0.5;
    float w = 0.3 + hash(gridPos + 100.0) * 0.2;
    float3 towerPos = float3(cx * 1.0, h * 0.5 - 0.5, cz * 1.0);
    d = min(d, sdBox(p - towerPos, float3(w, h, w) * 0.5));
    return d;
}

float3 getMaterialColor(int id) {
    switch (id) {
        case 1: return float3(0.25, 0.25, 0.3);  // Ground - dark gray
        case 2: return float3(0.8, 0.4, 0.3);    // Terracotta
        case 3: return float3(0.3, 0.5, 0.7);    // Steel blue
        case 4: return float3(0.6, 0.6, 0.5);    // Concrete
        case 5: return float3(0.4, 0.6, 0.4);    // Sage green
        case 6: return float3(0.7, 0.5, 0.6);    // Mauve
        case 7: return float3(0.5, 0.4, 0.3);    // Brown
        default: return float3(0.5, 0.5, 0.5);
    }
}

float3 calcNormal(float3 p) {
    const float eps = 0.001;
    float3 n;
    n.x = mapScene(p + float3(eps, 0, 0)).dist - mapScene(p - float3(eps, 0, 0)).dist;
    n.y = mapScene(p + float3(0, eps, 0)).dist - mapScene(p - float3(0, eps, 0)).dist;
    n.z = mapScene(p + float3(0, 0, eps)).dist - mapScene(p - float3(0, 0, eps)).dist;
    return normalize(n);
}

float calcSoftShadow(float3 ro, float3 rd, float mint, float maxt, float k) {
    float res = 1.0;
    float t = mint;
    for (int i = 0; i < 32 && t < maxt; i++) {
        float h = mapScene(ro + rd * t).dist;
        if (h < 0.001) return 0.0;
        res = min(res, k * h / t);
        t += clamp(h, 0.01, 0.2);
    }
    return clamp(res, 0.0, 1.0);
}

float calcAO(float3 pos, float3 nor) {
    float occ = 0.0;
    float sca = 1.0;
    float aoRadius = uniforms.renderParams.z;
    for (int i = 0; i < 5; i++) {
        float h = 0.01 + aoRadius * float(i) / 4.0;
        float d = mapSceneAO(pos + h * nor);
        occ += (h - d) * sca;
        sca *= 0.95;
    }
    return clamp(1.0 - 3.0 * occ, 0.0, 1.0);
}

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

    float t = 0.0;

    for (int i = 0; i < 128 && t < 40.0; i++) {
        float3 p = ro + rd * t;
        Hit h = mapScene(p);

        float eps = 0.001 + t * 0.0002;
        if (h.dist < eps) {
            result.t = t;
            result.materialId = h.materialId;
            result.hit = true;
            return result;
        }
        t += h.dist * 0.95;
    }
    return result;
}

float3 getSkyColor(float3 rd) {
    float t = 0.5 * (rd.y + 1.0);
    return lerp(float3(0.9, 0.85, 0.8), float3(0.5, 0.6, 0.8), t);
}

float3 render(float3 ro, float3 rd) {
    RayHit hit = raymarch(ro, rd);
    if (!hit.hit) return getSkyColor(rd);

    float3 pos = ro + rd * hit.t;

    // Skip normal calc for ground plane - always (0,1,0)
    float3 nor;
    if (hit.materialId == 1) {
        nor = float3(0, 1, 0);
    } else {
        nor = calcNormal(pos);
    }

    float3 col = getMaterialColor(hit.materialId);

    float3 lightDir = normalize(uniforms.lightDirIntensity.xyz);
    float lightIntensity = uniforms.lightDirIntensity.w;
    float shadowSoftness = uniforms.renderParams.w;

    float diff = max(dot(nor, lightDir), 0.0);
    float shadow = calcSoftShadow(pos + nor * 0.01, lightDir, 0.02, 10.0, shadowSoftness);
    float ao = calcAO(pos, nor);

    float3 ambient = float3(0.08, 0.09, 0.12);
    float3 lin = ambient * ao;
    lin += diff * shadow * lightIntensity * 1.5 * float3(1.0, 0.95, 0.9);
    col *= lin;

    float fog = 1.0 - exp(-hit.t * 0.003);
    col = lerp(col, getSkyColor(rd) * 0.8, fog);

    return col;
}

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    float2 uv = input.uv;
    float2 ndc = uv * 2.0 - 1.0;

    float4 nearPoint = mul(uniforms.invViewProj, float4(ndc, 0.0, 1.0));
    float4 farPoint = mul(uniforms.invViewProj, float4(ndc, 1.0, 1.0));
    nearPoint /= nearPoint.w;
    farPoint /= farPoint.w;

    float3 ro = uniforms.cameraPosTime.xyz;
    float3 rd = normalize(farPoint.xyz - nearPoint.xyz);

    float3 col = render(ro, rd);
    col = pow(col, float3(1.0 / 2.2));

    return float4(col, 1.0);
}
