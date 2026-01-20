// SDF 3D Clay Tablet with MSDF Atlas-based Relief Text
// Uses multi-channel signed distance field texture for crisp text

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
float sdBox(float3 p, float3 b) {
    float3 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0);
}

float sdRoundBox(float3 p, float3 b, float r) {
    float3 q = abs(p) - b + r;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

float sdPlane(float3 p, float3 n, float h) {
    return dot(p, n) + h;
}

// Median of three values - core MSDF operation
float median(float r, float g, float b) {
    return max(min(r, g), min(max(r, g), b));
}

// Sample MSDF atlas and return signed distance
float sampleMsdf(float2 uv) {
    // Clamp UVs to valid range
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return 1.0; // Outside atlas
    }

    float4 msdf = msdfAtlas.Sample(atlasSampler, uv);
    float sd = median(msdf.r, msdf.g, msdf.b) - 0.5;
    return sd * uniforms.atlasParams.w; // Scale by SDF range
}

// Get UV for a character in the atlas
// Atlas is 10x10 grid of 64x64 glyphs (for ASCII 32-126)
float2 getGlyphUV(int charCode, float2 localUV) {
    // Map ASCII to atlas position
    // Space (32) is at position 0, so subtract 32
    int glyphIndex = charCode - 32;
    if (glyphIndex < 0 || glyphIndex >= 95) {
        return float2(-1, -1); // Invalid character
    }

    float glyphsPerRow = 10.0;
    float col = float(glyphIndex % 10);
    float row = float(glyphIndex / 10);

    float cellSize = 1.0 / glyphsPerRow;

    // Map local UV [0,1] to glyph cell in atlas
    float2 atlasUV;
    atlasUV.x = (col + localUV.x) * cellSize;
    atlasUV.y = (row + localUV.y) * cellSize;

    return atlasUV;
}

// Text content and layout
// "RAYBOX" title
static const int TEXT_LEN = 6;
static const int TEXT_CHARS[6] = {82, 65, 89, 66, 79, 88}; // R A Y B O X

// Character widths (normalized)
float getCharWidth(int c) {
    // Most characters are 0.6 em width
    if (c == 73 || c == 105) return 0.3; // I, i
    if (c == 77 || c == 109 || c == 87 || c == 119) return 0.9; // M, m, W, w
    if (c == 32) return 0.3; // Space
    return 0.6;
}

// Sample text SDF in 2D
float sdText2D(float2 p) {
    float d = 1e10;

    // Layout parameters
    float charHeight = 0.4;
    float startX = -0.9;

    float xPos = startX;

    for (int i = 0; i < TEXT_LEN; i++) {
        int charCode = TEXT_CHARS[i];
        float charWidth = getCharWidth(charCode) * charHeight;

        // Check if point is in this character's bounding box
        float2 localP = p - float2(xPos + charWidth * 0.5, charHeight * 0.5);

        // Normalize to [0,1] for glyph cell
        float2 localUV = localP / float2(charWidth, charHeight) + 0.5;

        // Sample MSDF
        float2 atlasUV = getGlyphUV(charCode, localUV);
        if (atlasUV.x >= 0.0) {
            float charDist = sampleMsdf(atlasUV);
            // Scale distance back to world space
            charDist *= charHeight / uniforms.atlasParams.z * uniforms.atlasParams.x;
            d = min(d, charDist);
        }

        xPos += charWidth + 0.05; // Advance with spacing
    }

    return d;
}

// Second line: "MSDF TEXT"
static const int LINE2_LEN = 9;
static const int LINE2_CHARS[9] = {77, 83, 68, 70, 32, 84, 69, 88, 84}; // M S D F   T E X T

float sdLine2(float2 p) {
    float d = 1e10;
    float charHeight = 0.25;
    float startX = -0.65;
    float xPos = startX;

    for (int i = 0; i < LINE2_LEN; i++) {
        int charCode = LINE2_CHARS[i];
        float charWidth = getCharWidth(charCode) * charHeight;

        float2 localP = p - float2(xPos + charWidth * 0.5, charHeight * 0.5);
        float2 localUV = localP / float2(charWidth, charHeight) + 0.5;

        float2 atlasUV = getGlyphUV(charCode, localUV);
        if (atlasUV.x >= 0.0) {
            float charDist = sampleMsdf(atlasUV);
            charDist *= charHeight / uniforms.atlasParams.z * uniforms.atlasParams.x;
            d = min(d, charDist);
        }

        xPos += charWidth + 0.03;
    }

    return d;
}

// Third line: "RENDERING"
static const int LINE3_LEN = 9;
static const int LINE3_CHARS[9] = {82, 69, 78, 68, 69, 82, 73, 78, 71}; // R E N D E R I N G

float sdLine3(float2 p) {
    float d = 1e10;
    float charHeight = 0.25;
    float startX = -0.65;
    float xPos = startX;

    for (int i = 0; i < LINE3_LEN; i++) {
        int charCode = LINE3_CHARS[i];
        float charWidth = getCharWidth(charCode) * charHeight;

        float2 localP = p - float2(xPos + charWidth * 0.5, charHeight * 0.5);
        float2 localUV = localP / float2(charWidth, charHeight) + 0.5;

        float2 atlasUV = getGlyphUV(charCode, localUV);
        if (atlasUV.x >= 0.0) {
            float charDist = sampleMsdf(atlasUV);
            charDist *= charHeight / uniforms.atlasParams.z * uniforms.atlasParams.x;
            d = min(d, charDist);
        }

        xPos += charWidth + 0.03;
    }

    return d;
}

// Combined text SDF
float sdAllText2D(float2 p) {
    float d = 1e10;

    // Title "RAYBOX" centered at top
    d = min(d, sdText2D(p - float2(0.0, 0.4)));

    // "MSDF TEXT" below
    d = min(d, sdLine2(p - float2(0.0, -0.1)));

    // "RENDERING" below that
    d = min(d, sdLine3(p - float2(0.0, -0.45)));

    return d;
}

// Extrude 2D text into 3D relief using opExtrusion
float sdTextRelief(float3 p, float depth) {
    // Map XZ plane to text (text faces up)
    float d2d = sdAllText2D(float2(p.x, -p.z));
    // Extrusion: limit in Y direction
    float extrusion = abs(p.y) - depth;
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
    ground.dist = sdPlane(p, float3(0, 1, 0), 1.5);
    ground.materialId = 1;
    result = opUnionHit(result, ground);

    // Clay tablet (rounded box)
    Hit tablet;
    float3 tabletP = p - float3(0, 0.3, 0);
    tablet.dist = sdRoundBox(tabletP, float3(2.0, 0.25, 1.4), 0.08);
    tablet.materialId = 2;

    // Carve text into tablet (subtract relief from tablet)
    float3 textP = p - float3(0, 0.50, 0);
    float textRelief = sdTextRelief(textP, 0.06);
    tablet.dist = max(tablet.dist, -textRelief);

    result = opUnionHit(result, tablet);

    return result;
}

float3 getMaterialColor(int id) {
    switch (id) {
        case 1: return float3(0.25, 0.23, 0.20); // Ground - dark
        case 2: return float3(0.82, 0.68, 0.52); // Tablet - clay
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
    return lerp(float3(0.9, 0.85, 0.8), float3(0.6, 0.7, 0.85), t);
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
    float shadow = calcSoftShadow(pos + nor * 0.01, lightDir, 0.02, 10.0, 24.0);
    float ao = calcAO(pos, nor);

    float3 ambient = float3(0.12, 0.11, 0.10);
    float3 lin = ambient * ao;
    lin += diff * shadow * lightIntensity * 1.4 * float3(1.0, 0.97, 0.92);
    col *= lin;

    float fog = 1.0 - exp(-hit.t * 0.003);
    col = lerp(col, getSkyColor(rd) * 0.85, fog);

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
