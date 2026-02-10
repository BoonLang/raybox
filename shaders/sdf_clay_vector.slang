// SDF 3D Clay Tablet with Vector SDF Text Relief
// Uses exact Bézier curve SDF with grid acceleration for carved text

struct Uniforms {
    float4x4 invViewProj;
    float4 cameraPosTime;     // xyz = cameraPos, w = time
    float4 lightDirIntensity; // xyz = lightDir, w = intensity
    float4 renderParams;      // xy = resolution, z = textDepth, w = textScale
    float4 textParams;        // x = charCount, y = numLines, z = lineHeight, w = reserved
    float4 charGridParams;    // xy = gridDims, zw = cellSize
    float4 charGridBounds;    // xy = gridMin, zw = gridMax
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
    uint4 gridInfo;     // gridOffset, gridSizeX, gridSizeY, unused
    uint4 curveInfo;    // curveOffset, curveCount, unused, unused
};

// Character grid cell (offset + count into charGridIndices)
struct CharGridCellData {
    uint offset;
    uint count;
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

[[vk::binding(6, 0)]]
StructuredBuffer<CharGridCellData> charGridCells;

[[vk::binding(7, 0)]]
StructuredBuffer<uint> charGridIndices;

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
// Uses epsilon to avoid boundary issues at curve junctions
int curveWindingContribution(float2 point, float2 p0, float2 p1, float2 p2) {
    const float EPS = 1e-6;

    float px = point.x;
    float py = point.y;

    float y_min = min(min(p0.y, p1.y), p2.y);
    float y_max = max(max(p0.y, p1.y), p2.y);
    if (py < y_min - EPS || py >= y_max - EPS) return 0;

    float x_max = max(max(p0.x, p1.x), p2.x);
    if (px > x_max + EPS) return 0;

    float a_coef = p0.y - 2.0 * p1.y + p2.y;
    float b_coef = 2.0 * (p1.y - p0.y);
    float c_coef = p0.y - py;

    int crossings = 0;

    if (abs(a_coef) < 1e-7) {
        if (abs(b_coef) > 1e-7) {
            float t = -c_coef / b_coef;
            if (t > -EPS && t < 1.0 - EPS) {
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
                if (t > EPS && t < 1.0 - EPS) {
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
float sdGlyph(float2 localPos, float4 bounds, uint4 gridInfo, uint4 curveInfo) {
    uint gridOffset = gridInfo.x;
    uint gridSizeX = gridInfo.y;
    uint gridSizeY = gridInfo.z;
    uint glyphCurveOffset = curveInfo.x;
    uint glyphCurveCount = curveInfo.y;

    float2 glyphSize = bounds.zw - bounds.xy;
    float2 gridUV = (localPos - bounds.xy) / glyphSize;

    if (gridUV.x < 0.0 || gridUV.x > 1.0 || gridUV.y < 0.0 || gridUV.y > 1.0) {
        float2 d = max(bounds.xy - localPos, localPos - bounds.zw);
        return length(max(d, 0.0)) + 0.01;
    }

    // Fix: clamp to [0,1] and use min() to avoid out-of-bounds
    gridUV = clamp(gridUV, 0.0, 1.0);
    int cellX = min(int(gridUV.x * float(gridSizeX)), int(gridSizeX) - 1);
    int cellY = min(int(gridUV.y * float(gridSizeY)), int(gridSizeY) - 1);
    uint cellIdx = gridOffset + uint(cellY) * gridSizeX + uint(cellX);

    GridCell cell = gridCells[cellIdx];
    uint cellCurveStart = cell.curveStartAndCount & 0xFFFF;
    uint cellCurveCount = (cell.curveStartAndCount >> 16) & 0xFF;
    uint flags = (cell.curveStartAndCount >> 24) & 0xFF;

    // Flag interpretation: bit 0 = inside, bit 1 = uniform
    bool cellIsInside = (flags & 0x01u) != 0u;
    bool isUniform = (flags & 0x02u) != 0u;

    // Compute minimum distance to curves in this cell
    float minDist = 1e10;
    for (uint i = 0u; i < cellCurveCount && i < 16u; i++) {
        uint globalCurveIdx = curveIndices[cellCurveStart + i];
        BezierCurve curve = curves[globalCurveIdx];

        float2 p0 = curve.points01.xy;
        float2 p1 = curve.points01.zw;
        float2 p2 = curve.points2bbox.xy;

        // Quick culling: skip curves whose bounding sphere is farther than current best
        float2 mid = (p0 + 2.0 * p1 + p2) * 0.25;
        float midDist = length(localPos - mid);
        float curveRadius = length(curve.bboxFlags.xy - curve.points2bbox.zw) * 0.5;
        if (midDist - curveRadius > minDist) continue;

        float dist = sdQuadraticBezier(localPos, p0, p1, p2);
        minDist = min(minDist, dist);
    }

    // For uniform cells with no curves, sign is already known — skip expensive winding loop
    if (isUniform && cellCurveCount == 0u) {
        return (cellIsInside ? -1.0 : 1.0) * minDist;
    }

    // Boundary cells and uniform cells with curves need winding from ALL glyph curves
    int winding = 0;
    for (uint i = 0u; i < glyphCurveCount && i < 256u; i++) {
        BezierCurve curve = curves[glyphCurveOffset + i];
        float2 p0 = curve.points01.xy;
        float2 p1 = curve.points01.zw;
        float2 p2 = curve.points2bbox.xy;
        winding += curveWindingContribution(localPos, p0, p1, p2);
    }

    float sign_val = (winding != 0) ? -1.0 : 1.0;
    return sign_val * minDist;
}

// Look up characters overlapping a 2D position using the character spatial grid
// Returns charGridCells index for the cell. Caller reads offset/count.
// p is in the text's 2D coordinate space (XZ for clay).
int2 charGridCellCoord(float2 p) {
    float2 gridMin = uniforms.charGridBounds.xy;
    float2 cellSize = uniforms.charGridParams.zw;
    int2 gridDims = int2(uniforms.charGridParams.xy);
    int2 cell = int2((p - gridMin) / cellSize);
    return clamp(cell, int2(0), gridDims - 1);
}

uint charGridCellIndex(int2 cell) {
    int gridW = int(uniforms.charGridParams.x);
    return uint(cell.y * gridW + cell.x);
}

// Exact extrusion operation (from Inigo Quilez)
// Produces mathematically exact 3D SDF from exact 2D SDF
float opExtrusion(float sdf2d, float zDist, float depth) {
    float2 w = float2(sdf2d, abs(zDist) - depth);
    return min(max(w.x, w.y), 0.0) + length(max(w, 0.0));
}

// 3D extruded text for relief carving with per-character extrusion
// Uses character spatial grid for O(1) lookup instead of O(n) linear scan
float sdTextRelief(float3 p, float depth) {
    float minDist = 1e10;
    float halfDepth = depth * 0.5;

    // Text AABB early-exit
    float2 gridMin = uniforms.charGridBounds.xy;
    float2 gridMax = uniforms.charGridBounds.zw;
    float3 textBoxMin = float3(gridMin.x, -halfDepth, -gridMax.y);
    float3 textBoxMax = float3(gridMax.x, halfDepth, -gridMin.y);
    float3 textBoxCenter = (textBoxMin + textBoxMax) * 0.5;
    float3 textBoxHalf = (textBoxMax - textBoxMin) * 0.5;
    float textBoxDist = sdBox(p - textBoxCenter, textBoxHalf);
    if (textBoxDist > 0.2) return textBoxDist;

    // Convert 3D position to text 2D coordinates (text on XZ plane, Z negated)
    float2 textPos = float2(p.x, -p.z);

    // Look up character grid cell
    int2 cellCoord = charGridCellCoord(textPos);
    uint cellIdx = charGridCellIndex(cellCoord);
    CharGridCellData cgCell = charGridCells[cellIdx];
    uint start = cgCell.offset;
    uint count = cgCell.count;

    for (uint i = 0u; i < count && i < 16u; i++) {
        uint charIdx = charGridIndices[start + i];
        CharInstance ch = charInstances[charIdx];
        float2 charPos = ch.posAndChar.xy;
        float charScale = ch.posAndChar.z;
        uint glyphIdx = uint(ch.posAndChar.w);

        GlyphData glyph = glyphData[glyphIdx];
        float4 bounds = glyph.bounds;

        // Transform to glyph local 2D coordinates
        float2 localP = (textPos - charPos) / charScale;

        if (localP.x >= bounds.x - 0.15 && localP.x <= bounds.z + 0.15 &&
            localP.y >= bounds.y - 0.15 && localP.y <= bounds.w + 0.15) {
            float d2d = sdGlyph(localP, bounds, glyph.gridInfo, glyph.curveInfo) * charScale;
            float d3d = opExtrusion(d2d, p.y, halfDepth);
            minDist = min(minDist, d3d);
        }
    }

    if (minDist < 1e9) {
        return minDist;
    }
    return 1e10;
}

struct Hit {
    float dist;
    int materialId;
};

Hit opUnionHit(Hit a, Hit b) {
    if (a.dist < b.dist) return a;
    return b;
}

// Coarse scene for shadow/AO rays — skip expensive text SDF
Hit mapSceneCoarse(float3 p) {
    Hit result;
    result.dist = 1e10;
    result.materialId = 0;

    // Ground plane
    Hit ground;
    ground.dist = sdPlane(p, float3(0, 1, 0), 1.5);
    ground.materialId = 1;
    result = opUnionHit(result, ground);

    // Clay tablet (rounded box only, no text carving)
    Hit tablet;
    float3 tabletP = p - float3(0, 0.3, 0);
    tablet.dist = sdRoundBox(tabletP, float3(3.5, 0.25, 2.5), 0.08);
    tablet.materialId = 2;
    result = opUnionHit(result, tablet);

    return result;
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
    tablet.dist = sdRoundBox(tabletP, float3(3.5, 0.25, 2.5), 0.08);
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

// Tetrahedron normal calculation (4 mapScene calls instead of 6)
float3 calcNormal(float3 p) {
    const float h = 0.0002;
    const float2 k = float2(1, -1);
    return normalize(
        k.xyy * mapScene(p + k.xyy * h).dist +
        k.yyx * mapScene(p + k.yyx * h).dist +
        k.yxy * mapScene(p + k.yxy * h).dist +
        k.xxx * mapScene(p + k.xxx * h).dist
    );
}

float calcSoftShadow(float3 ro, float3 rd, float mint, float maxt, float k) {
    float res = 1.0;
    float t = mint;
    for (int i = 0; i < 64 && t < maxt; i++) {
        float h = mapSceneCoarse(ro + rd * t).dist;
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
        float d = mapSceneCoarse(pos + h * nor).dist;
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
    float omega = 1.6;  // Over-relaxation factor
    float prevRadius = 0.0;

    for (int i = 0; i < 128 && t < 50.0; i++) {
        float3 p = ro + rd * t;
        Hit h = mapScene(p);
        float radius = abs(h.dist);

        // Over-relaxation with fallback: check if spheres don't overlap
        bool sorFail = omega > 1.0 && radius < (omega - 1.0) * prevRadius;
        if (sorFail) {
            t -= (omega - 1.0) * prevRadius;  // Revert overstep
            omega = 1.0;                       // Fall back to conservative
            prevRadius = radius;
            continue;
        }

        if (h.dist < 0.001) {
            result.t = t;
            result.materialId = h.materialId;
            result.hit = true;
            return result;
        }

        t += h.dist * omega;
        prevRadius = radius;
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
