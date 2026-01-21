// SDF 3D Clay Tablet with Vector SDF Text Relief
// Uses exact Bézier curve SDF with grid acceleration for carved text

struct Uniforms {
    float4x4 invViewProj;
    float4 cameraPosTime;     // xyz = cameraPos, w = time
    float4 lightDirIntensity; // xyz = lightDir, w = intensity
    float4 renderParams;      // xy = resolution, z = textDepth, w = textScale
    float4 textParams;        // x = charCount, y = numLines, z = lineHeight, w = reserved
};

// Grid cell structure
struct GridCell {
    uint curveStartAndCount; // Lower 16 bits = start, upper 8 = count, top 8 = flags
};

// Bézier curve (packed format)
struct BezierCurve {
    float4 points01;    // p0.xy, p1.xy
    float4 points2bbox; // p2.xy, bbox.xy (min)
    float4 bboxFlags;   // bbox.zw (max), flags, padding
};

// Character instance for text layout
struct CharInstance {
    float4 posAndChar;  // xy = position, z = scale, w = charIndex (into glyphData)
};

// Glyph metadata
struct GlyphData {
    float4 bounds;      // left, bottom, right, top in em units
    uint4 gridInfo;     // gridOffset, gridSizeX, gridSizeY, curveCount
};

[[vk::binding(0, 0)]]
ConstantBuffer<Uniforms> uniforms;

[[vk::binding(1, 0)]]
StructuredBuffer<GridCell> gridCells;

[[vk::binding(2, 0)]]
StructuredBuffer<uint> curveIndices;

[[vk::binding(3, 0)]]
StructuredBuffer<BezierCurve> curves;

[[vk::binding(4, 0)]]
StructuredBuffer<GlyphData> glyphData;

[[vk::binding(5, 0)]]
StructuredBuffer<CharInstance> charInstances;

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

// Compute exact distance to quadratic Bézier
float sdQuadraticBezier(float2 p, float2 p0, float2 p1, float2 p2) {
    float2 a = p1 - p0;
    float2 b = p0 - 2.0 * p1 + p2;
    float2 c = a * 2.0;
    float2 d = p0 - p;

    float bb = dot(b, b);
    if (bb < 1e-10) {
        float2 ab = p2 - p0;
        float len2 = dot(ab, ab);
        if (len2 < 1e-10) return length(p - p0);
        float t = clamp(dot(p - p0, ab) / len2, 0.0, 1.0);
        return length(p - (p0 + t * ab));
    }

    float kk = 1.0 / bb;
    float kx = kk * dot(a, b);
    float ky = kk * (2.0 * dot(a, a) + dot(d, b)) / 3.0;
    float kz = kk * dot(d, a);

    float p_coef = ky - kx * kx;
    float q_coef = kx * (2.0 * kx * kx - 3.0 * ky) + kz;
    float p3 = p_coef * p_coef * p_coef;
    float q2 = q_coef * q_coef;
    float h = q2 + 4.0 * p3;

    float dist = 1e10;

    if (h >= 0.0) {
        h = sqrt(h);
        float2 x = (float2(h, -h) - q_coef) * 0.5;
        float2 uv = sign(x) * pow(abs(x), float2(1.0 / 3.0));
        float t = clamp(uv.x + uv.y - kx, 0.0, 1.0);
        float2 q = (1.0 - t) * (1.0 - t) * p0 + 2.0 * (1.0 - t) * t * p1 + t * t * p2;
        dist = length(p - q);
    } else {
        float z = sqrt(-p_coef);
        float v = acos(q_coef / (p_coef * z * 2.0)) / 3.0;
        float m = cos(v);
        float n = sin(v) * 1.732050808;
        float3 t3 = clamp(float3(m + m, -n - m, n - m) * z - kx, 0.0, 1.0);

        for (int i = 0; i < 3; i++) {
            float t = t3[i];
            float2 q = (1.0 - t) * (1.0 - t) * p0 + 2.0 * (1.0 - t) * t * p1 + t * t * p2;
            dist = min(dist, length(p - q));
        }
    }

    return dist;
}

// Compute winding contribution
int curveWindingContribution(float2 point, float2 p0, float2 p1, float2 p2) {
    float px = point.x;
    float py = point.y;

    float y_min = min(min(p0.y, p1.y), p2.y);
    float y_max = max(max(p0.y, p1.y), p2.y);
    if (py < y_min || py >= y_max) return 0;

    float x_max = max(max(p0.x, p1.x), p2.x);
    if (px > x_max) return 0;

    float a_coef = p0.y - 2.0 * p1.y + p2.y;
    float b_coef = 2.0 * (p1.y - p0.y);
    float c_coef = p0.y - py;

    int crossings = 0;

    if (abs(a_coef) < 1e-10) {
        if (abs(b_coef) > 1e-10) {
            float t = -c_coef / b_coef;
            if (t >= 0.0 && t < 1.0) {
                float x = (1.0 - t) * (1.0 - t) * p0.x + 2.0 * (1.0 - t) * t * p1.x + t * t * p2.x;
                if (x > px) {
                    crossings = (p2.y - p0.y > 0.0) ? 1 : -1;
                }
            }
        }
    } else {
        float discriminant = b_coef * b_coef - 4.0 * a_coef * c_coef;
        if (discriminant >= 0.0) {
            float sqrt_d = sqrt(discriminant);
            float t1 = (-b_coef - sqrt_d) / (2.0 * a_coef);
            float t2 = (-b_coef + sqrt_d) / (2.0 * a_coef);

            for (int i = 0; i < 2; i++) {
                float t = (i == 0) ? t1 : t2;
                if (t >= 0.0 && t < 1.0) {
                    float x = (1.0 - t) * (1.0 - t) * p0.x + 2.0 * (1.0 - t) * t * p1.x + t * t * p2.x;
                    if (x > px) {
                        float dy_dt = 2.0 * ((p1.y - p0.y) * (1.0 - t) + (p2.y - p1.y) * t);
                        crossings += (dy_dt > 0.0) ? 1 : -1;
                    }
                }
            }
        }
    }

    return crossings;
}

// Compute SDF for a single glyph at local coordinates
float sdGlyph(float2 localPos, float4 bounds, uint4 gridInfo) {
    uint gridOffset = gridInfo.x;
    uint gridSizeX = gridInfo.y;
    uint gridSizeY = gridInfo.z;

    float2 glyphSize = bounds.zw - bounds.xy;
    float2 gridUV = (localPos - bounds.xy) / glyphSize;

    if (gridUV.x < 0.0 || gridUV.x > 1.0 || gridUV.y < 0.0 || gridUV.y > 1.0) {
        float2 d = max(bounds.xy - localPos, localPos - bounds.zw);
        return length(max(d, 0.0)) + 0.01;
    }

    gridUV = clamp(gridUV, 0.0, 0.9999);

    int cellX = int(gridUV.x * float(gridSizeX));
    int cellY = int(gridUV.y * float(gridSizeY));
    uint cellIdx = gridOffset + cellY * gridSizeX + cellX;

    GridCell cell = gridCells[cellIdx];
    uint curveStart = cell.curveStartAndCount & 0xFFFF;
    uint curveCount = (cell.curveStartAndCount >> 16) & 0xFF;
    uint flags = (cell.curveStartAndCount >> 24) & 0xFF;

    if ((flags & 0x01) != 0) return -0.05;
    if ((flags & 0x02) != 0) return 0.05;

    float minDist = 1e10;
    int winding = 0;

    for (uint i = 0; i < curveCount && i < 16; i++) {
        uint globalCurveIdx = curveIndices[curveStart + i];
        BezierCurve curve = curves[globalCurveIdx];

        float2 p0 = curve.points01.xy;
        float2 p1 = curve.points01.zw;
        float2 p2 = curve.points2bbox.xy;
        float4 bbox = float4(curve.points2bbox.zw, curve.bboxFlags.xy);

        float2 d = max(bbox.xy - localPos, localPos - bbox.zw);
        float bboxDist = length(max(d, 0.0));
        if (bboxDist > minDist) continue;

        float dist = sdQuadraticBezier(localPos, p0, p1, p2);
        minDist = min(minDist, dist);

        winding += curveWindingContribution(localPos, p0, p1, p2);
    }

    float sign_val = (winding != 0) ? -1.0 : 1.0;
    return sign_val * minDist;
}

static const uint MAX_CHARS = 512;

// Compute 2D SDF for all text
float sdText2D(float2 p) {
    float minDist = 1e10;
    uint charCount = uint(uniforms.textParams.x);

    for (uint i = 0; i < charCount && i < MAX_CHARS; i++) {
        CharInstance ch = charInstances[i];
        float2 charPos = ch.posAndChar.xy;
        float charScale = ch.posAndChar.z;
        uint glyphIdx = uint(ch.posAndChar.w);

        GlyphData glyph = glyphData[glyphIdx];

        float2 localP = (p - charPos) / charScale;
        float d = sdGlyph(localP, glyph.bounds, glyph.gridInfo);
        d *= charScale;

        minDist = min(minDist, d);
    }

    return minDist;
}

// 3D extruded text for relief carving
float sdTextRelief(float3 p, float depth) {
    // Text on XZ plane (horizontal tablet surface)
    // Text layout uses +z for front of tablet, shader uses +z for same direction
    float d2d = sdText2D(float2(p.x, p.z));
    float extrusion = abs(p.y) - depth * 0.5;
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

    float textScale = uniforms.renderParams.w;
    float textDepth = uniforms.renderParams.z;

    // Ground plane
    Hit ground;
    ground.dist = sdPlane(p, float3(0, 1, 0), 1.5);
    ground.materialId = 1;
    result = opUnionHit(result, ground);

    // Clay tablet (rounded box)
    Hit tablet;
    float3 tabletP = p - float3(0, 0.3, 0);
    tablet.dist = sdRoundBox(tabletP, float3(2.5, 0.25, 1.8), 0.08);
    tablet.materialId = 2;

    // Carve text into tablet (CSG subtraction)
    float3 textP = (p - float3(0, 0.5, 0)) / textScale;
    float textRelief = sdTextRelief(textP, textDepth / textScale) * textScale;

    // Relief: subtract text from tablet surface
    tablet.dist = max(tablet.dist, -textRelief);

    result = opUnionHit(result, tablet);

    return result;
}

float3 getMaterialColor(int id) {
    switch (id) {
        case 1: return float3(0.25, 0.23, 0.20); // Ground - dark stone
        case 2: return float3(0.78, 0.62, 0.44); // Tablet - clay/terracotta
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

    float3 ambient = float3(0.10, 0.09, 0.08);
    float3 lin = ambient * ao;
    lin += diff * shadow * lightIntensity * 1.5 * float3(1.0, 0.95, 0.9);
    col *= lin;

    float fog = 1.0 - exp(-hit.t * 0.004);
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
