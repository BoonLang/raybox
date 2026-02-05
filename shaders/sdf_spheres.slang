// SDF Spheres Scene - Multiple colorful spheres

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

float sdSphere(float3 p, float r) {
    return length(p) - r;
}

float sdPlane(float3 p, float3 n, float h) {
    return dot(p, n) + h;
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

    float time = uniforms.cameraPosTime.w;

    // Ground plane
    Hit ground;
    ground.dist = sdPlane(p, float3(0, 1, 0), 0.5);
    ground.materialId = 1;
    result = opUnionHit(result, ground);

    // 3x3 neighborhood around nearest grid cell (spacing = 1.2, radius = 0.4)
    float sphereRadius = 0.4;
    float spacing = 1.2;
    int cx = int(round(p.x / spacing));
    int cz = int(round(p.z / spacing));
    for (int di = -1; di <= 1; di++) {
        for (int dj = -1; dj <= 1; dj++) {
            int i = cx + di;
            int j = cz + dj;
            if (i < -2 || i > 2 || j < -2 || j > 2) continue;

            float3 offset = float3(i * spacing, 0.0, j * spacing);
            float bounce = abs(sin(time * 2.0 + float(i + j) * 0.5)) * 0.8;
            float3 spherePos = p - offset - float3(0, bounce - 0.1, 0);

            Hit sphere;
            sphere.dist = sdSphere(spherePos, sphereRadius);
            sphere.materialId = 2 + ((i + 2) * 5 + (j + 2)) % 6;
            result = opUnionHit(result, sphere);
        }
    }

    return result;
}

float3 getMaterialColor(int id) {
    switch (id) {
        case 1: return float3(0.3, 0.3, 0.35);  // Ground
        case 2: return float3(0.9, 0.2, 0.2);   // Red
        case 3: return float3(0.2, 0.9, 0.2);   // Green
        case 4: return float3(0.2, 0.2, 0.9);   // Blue
        case 5: return float3(0.9, 0.9, 0.2);   // Yellow
        case 6: return float3(0.9, 0.2, 0.9);   // Magenta
        case 7: return float3(0.2, 0.9, 0.9);   // Cyan
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
    for (int i = 0; i < 64 && t < maxt; i++) {
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
        float d = mapScene(pos + h * nor).dist;
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

    for (int i = 0; i < 128 && t < 100.0; i++) {
        float3 p = ro + rd * t;
        Hit h = mapScene(p);

        if (h.dist < 0.001) {
            result.t = t;
            result.materialId = h.materialId;
            result.hit = true;
            return result;
        }
        t += h.dist * 0.9;
    }
    return result;
}

float3 getSkyColor(float3 rd) {
    float t = 0.5 * (rd.y + 1.0);
    return lerp(float3(0.7, 0.8, 0.9), float3(0.3, 0.5, 0.9), t);
}

float3 render(float3 ro, float3 rd) {
    RayHit hit = raymarch(ro, rd);
    if (!hit.hit) return getSkyColor(rd);

    float3 pos = ro + rd * hit.t;
    float3 nor = calcNormal(pos);
    float3 col = getMaterialColor(hit.materialId);

    float3 lightDir = normalize(uniforms.lightDirIntensity.xyz);
    float lightIntensity = uniforms.lightDirIntensity.w;
    float shadowSoftness = uniforms.renderParams.w;

    float diff = max(dot(nor, lightDir), 0.0);
    float shadow = calcSoftShadow(pos + nor * 0.01, lightDir, 0.02, 10.0, shadowSoftness);
    float ao = calcAO(pos, nor);

    float3 ambient = float3(0.06, 0.07, 0.10);
    float3 lin = ambient * ao;
    lin += diff * shadow * lightIntensity * 1.6 * float3(1.0, 0.98, 0.95);
    col *= lin;

    float fog = 1.0 - exp(-hit.t * 0.002);
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
