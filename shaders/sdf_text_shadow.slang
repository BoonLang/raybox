// SDF 3D Text with Shadows
// Floating 3D letters casting soft shadows on a ground plane

struct Uniforms {
    float4x4 invViewProj;
    float4 cameraPosTime;     // xyz = cameraPos, w = time
    float4 lightDirIntensity; // xyz = lightDir, w = lightIntensity
    float4 renderParams;      // xy = resolution, z = aoRadius, w = shadowSoftness
    float4 atlasParams;       // xy = atlasSize, z = glyphSize, w = sdfRange
};

[[vk::binding(0, 0)]]
ConstantBuffer<Uniforms> uniforms;

[[vk::binding(1, 0)]]
Texture2D<float4> msdfAtlas;

[[vk::binding(2, 0)]]
SamplerState atlasSampler;

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

// 3D SDF primitives
float sdPlane(float3 p, float3 n, float h) {
    return dot(p, n) + h;
}

float sdBox(float3 p, float3 b) {
    float3 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Median of three values - core MSDF operation
float median(float r, float g, float b) {
    return max(min(r, g), min(max(r, g), b));
}

// Sample MSDF atlas and return signed distance
float sampleMsdf(float2 uv) {
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return 1.0;
    }

    float4 msdf = msdfAtlas.Sample(atlasSampler, uv);
    float sd = 0.5 - median(msdf.r, msdf.g, msdf.b);
    return sd * 2.0 * uniforms.atlasParams.w;
}

// Get UV for a character in the atlas
// Atlas is 10x10 grid of 64x64 glyphs (for ASCII 32-126)
// 640x640 total size, yOrigin is bottom
float2 getGlyphUV(int charCode, float2 localUV) {
    int glyphIndex = charCode - 32;
    if (glyphIndex < 0 || glyphIndex >= 95) {
        return float2(-1, -1);
    }

    float glyphsPerRow = 10.0;
    float col = float(glyphIndex % 10);
    float row = float(glyphIndex / 10);

    float cellSize = 1.0 / glyphsPerRow;

    // Map local UV [0,1] to glyph cell in atlas
    // yOrigin is bottom, so row 0 is at top of texture (flip Y)
    float2 atlasUV;
    atlasUV.x = (col + localUV.x) * cellSize;
    atlasUV.y = 1.0 - (row + 1.0 - localUV.y) * cellSize;

    return atlasUV;
}

// Character data for "RAYBOX"
static const int TEXT1_LEN = 6;
static const int TEXT1_CHARS[6] = {82, 65, 89, 66, 79, 88}; // R A Y B O X

// Character widths (normalized)
float getCharWidth(int c) {
    if (c == 73 || c == 105) return 0.3;
    if (c == 77 || c == 109 || c == 87 || c == 119) return 0.9;
    if (c == 32) return 0.3;
    return 0.65;
}

// 2D SDF for text on XZ plane
float sdText2D(float2 p) {
    float d = 1e10;
    float charHeight = 0.5;
    float totalWidth = 0.0;

    // Calculate total width first
    for (int i = 0; i < TEXT1_LEN; i++) {
        totalWidth += getCharWidth(TEXT1_CHARS[i]) * charHeight + 0.05;
    }

    float startX = -totalWidth * 0.5;
    float xPos = startX;

    for (int i = 0; i < TEXT1_LEN; i++) {
        int charCode = TEXT1_CHARS[i];
        float charWidth = getCharWidth(charCode) * charHeight;

        float2 localP = p - float2(xPos + charWidth * 0.5, charHeight * 0.5);
        float2 localUV = localP / float2(charWidth, charHeight) + 0.5;

        float2 atlasUV = getGlyphUV(charCode, localUV);
        if (atlasUV.x >= 0.0) {
            float charDist = sampleMsdf(atlasUV);
            charDist *= charHeight / uniforms.atlasParams.z;
            d = min(d, charDist);
        }

        xPos += charWidth + 0.05;
    }

    return d;
}

// 3D extruded text
float sdText3D(float3 p, float depth) {
    // Text is on XZ plane, Y is up
    float d2d = sdText2D(float2(p.x, -p.z));

    // Extrusion with rounded edges
    float extrusion = abs(p.y - depth * 0.5) - depth * 0.5;

    // Combine 2D shape with extrusion
    return max(d2d, extrusion);
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

    // Floating 3D text
    Hit text;
    float3 textP = p - float3(0, 0.6, 0.3);  // Float above ground
    text.dist = sdText3D(textP, 0.15);
    text.materialId = 2;
    result = opUnionHit(result, text);

    return result;
}

float3 getMaterialColor(int id) {
    switch (id) {
        case 1: return float3(0.85, 0.85, 0.82); // Ground - light gray
        case 2: return float3(0.15, 0.35, 0.65); // Text - blue
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
    for (int i = 0; i < 5; i++) {
        float h = 0.01 + 0.12 * float(i);
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
    for (int i = 0; i < 128 && t < 50.0; i++) {
        float3 p = ro + rd * t;
        Hit h = mapScene(p);
        if (h.dist < 0.001) {
            result.t = t;
            result.materialId = h.materialId;
            result.hit = true;
            return result;
        }
        t += h.dist * 0.8;
    }
    return result;
}

float3 getSkyColor(float3 rd) {
    float t = 0.5 * (rd.y + 1.0);
    return lerp(float3(0.95, 0.95, 0.95), float3(0.6, 0.75, 0.95), t);
}

float3 render(float3 ro, float3 rd) {
    RayHit hit = raymarch(ro, rd);
    if (!hit.hit) return getSkyColor(rd);

    float3 pos = ro + rd * hit.t;
    float3 nor = calcNormal(pos);
    float3 col = getMaterialColor(hit.materialId);

    float3 lightDir = normalize(uniforms.lightDirIntensity.xyz);
    float lightIntensity = uniforms.lightDirIntensity.w;

    float diff = max(dot(nor, lightDir), 0.0);

    // Soft shadows with higher softness for smoother shadows
    float shadow = calcSoftShadow(pos + nor * 0.01, lightDir, 0.02, 10.0, 16.0);
    float ao = calcAO(pos, nor);

    // Specular for text
    float3 viewDir = normalize(ro - pos);
    float3 halfDir = normalize(lightDir + viewDir);
    float spec = pow(max(dot(nor, halfDir), 0.0), 32.0) * 0.3;
    if (hit.materialId != 2) spec = 0.0;

    float3 ambient = float3(0.25, 0.25, 0.28);
    float3 lin = ambient * ao;
    lin += diff * shadow * lightIntensity * 1.2 * float3(1.0, 0.98, 0.95);
    col *= lin;
    col += spec * shadow;

    // Light fog
    float fog = 1.0 - exp(-hit.t * 0.015);
    col = lerp(col, getSkyColor(rd), fog);

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
