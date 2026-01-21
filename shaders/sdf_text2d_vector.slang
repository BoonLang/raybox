// 2D Vector SDF Text Rendering
// Renders text as a flat 2D layer using exact Bézier SDF computation

struct Uniforms {
    float2 screenSize;
    float2 padding;
    float4 textParams;   // x = charCount, y = scale, z = offsetY, w = reserved
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

// Glyph data
struct GlyphData {
    float4 bounds;      // min_x, min_y, max_x, max_y
    uint4 gridInfo;     // gridOffset, gridSizeX, gridSizeY, unused
    uint4 curveInfo;    // curveOffset, curveCount, unused, unused
};

// Character instance
struct CharInstance {
    float4 posAndChar;  // x, y, scale, glyphIndex
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
VertexOutput vs_main(uint vertexId : SV_VertexID) {
    float2 positions[3] = {
        float2(-1.0, -1.0),
        float2(3.0, -1.0),
        float2(-1.0, 3.0)
    };
    float2 uvs[3] = {
        float2(0.0, 1.0),
        float2(2.0, 1.0),
        float2(0.0, -1.0)
    };

    VertexOutput output;
    output.position = float4(positions[vertexId], 0.0, 1.0);
    output.uv = uvs[vertexId];
    return output;
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
// Uses half-open interval [y_min, y_max) to avoid double-counting at shared endpoints
int curveWindingContribution(float2 point, float2 p0, float2 p1, float2 p2) {
    float px = point.x;
    float py = point.y;

    // Small epsilon for numerical stability at boundaries
    const float EPS = 1e-6;

    float y_min = min(min(p0.y, p1.y), p2.y);
    float y_max = max(max(p0.y, p1.y), p2.y);

    // Use half-open interval [y_min, y_max) but with epsilon tolerance
    if (py < y_min - EPS || py >= y_max - EPS) return 0;

    float x_max = max(max(p0.x, p1.x), p2.x);
    if (px > x_max + EPS) return 0;

    float a = p0.y - 2.0 * p1.y + p2.y;
    float b = 2.0 * (p1.y - p0.y);
    float c = p0.y - py;

    int crossings = 0;

    if (abs(a) < 1e-7) {
        // Nearly linear case
        if (abs(b) > 1e-7) {
            float t = -c / b;
            if (t > -EPS && t < 1.0 - EPS) {
                float x = (1.0 - t) * (1.0 - t) * p0.x + 2.0 * (1.0 - t) * t * p1.x + t * t * p2.x;
                if (x > px) {
                    crossings = (p2.y - p0.y > 0.0) ? 1 : -1;
                }
            }
        }
    } else {
        float discriminant = b * b - 4.0 * a * c;
        if (discriminant >= 0.0) {
            float sqrt_d = sqrt(discriminant);
            float t1 = (-b - sqrt_d) / (2.0 * a);
            float t2 = (-b + sqrt_d) / (2.0 * a);

            for (int i = 0; i < 2; i++) {
                float t = (i == 0) ? t1 : t2;
                // Use slightly open interval to avoid endpoint issues
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

// Compute SDF for a single glyph
float sdGlyphAt(float2 localPos, uint glyphIdx, float4 bounds, uint4 gridInfo, uint4 curveInfo) {
    uint gridOffset = gridInfo.x;
    uint gridSizeX = gridInfo.y;
    uint gridSizeY = gridInfo.z;
    uint glyphCurveOffset = curveInfo.x;
    uint glyphCurveCount = curveInfo.y;

    // Determine grid cell - clamp to valid range to avoid out-of-bounds access
    float2 gridUV = (localPos - bounds.xy) / (bounds.zw - bounds.xy);
    gridUV = clamp(gridUV, 0.0, 1.0);

    // Compute cell indices, ensuring we don't exceed grid dimensions
    int cellX = min(int(gridUV.x * float(gridSizeX)), int(gridSizeX) - 1);
    int cellY = min(int(gridUV.y * float(gridSizeY)), int(gridSizeY) - 1);
    uint cellIdx = gridOffset + uint(cellY) * gridSizeX + uint(cellX);

    GridCell cell = gridCells[cellIdx];
    uint cellCurveStart = cell.curveStartAndCount & 0xFFFF;
    uint cellCurveCount = (cell.curveStartAndCount >> 16) & 0xFF;
    uint flags = (cell.curveStartAndCount >> 24) & 0xFF;

    // Flag interpretation:
    // Bit 0 (0x01): cell center is INSIDE the glyph
    // Bit 1 (0x02): cell is UNIFORM (all corners same as center) - safe for early return
    bool cellIsInside = (flags & 0x01u) != 0u;
    bool isUniform = (flags & 0x02u) != 0u;

    // Early out for uniform cells - use larger distances for cleaner AA
    if (isUniform && cellCurveCount == 0u) {
        return cellIsInside ? -1.0 : 1.0;
    }

    // Compute minimum distance to curves in this cell
    float minDist = 1e10;
    for (uint i = 0u; i < cellCurveCount && i < 16u; i++) {
        uint globalCurveIdx = curveIndices[cellCurveStart + i];
        BezierCurve curve = curves[globalCurveIdx];

        float2 p0 = curve.points01.xy;
        float2 p1 = curve.points01.zw;
        float2 p2 = curve.points2bbox.xy;
        float4 bbox = float4(curve.points2bbox.zw, curve.bboxFlags.xy);

        // Early reject using bounding box
        float2 d = max(bbox.xy - localPos, localPos - bbox.zw);
        float bboxDist = length(max(d, 0.0));
        if (bboxDist > minDist) continue;

        float dist = sdQuadraticBezier(localPos, p0, p1, p2);
        minDist = min(minDist, dist);
    }

    // Compute winding using ALL glyph curves (not just cell curves)
    // This gives correct inside/outside determination
    int winding = 0;
    for (uint i = 0u; i < glyphCurveCount && i < 256u; i++) {
        BezierCurve curve = curves[glyphCurveOffset + i];
        float2 p0 = curve.points01.xy;
        float2 p1 = curve.points01.zw;
        float2 p2 = curve.points2bbox.xy;
        winding += curveWindingContribution(localPos, p0, p1, p2);
    }

    float sign = (winding != 0) ? -1.0 : 1.0;
    return sign * minDist;
}

// Compute SDF for all text characters
float sdText(float2 pixelPos) {
    float minDist = 1e10;
    int charCount = int(uniforms.textParams.x);
    float globalScale = uniforms.textParams.y;

    for (int i = 0; i < charCount && i < 4000; i++) {
        CharInstance ch = charInstances[i];
        float2 charPos = ch.posAndChar.xy;
        float charScale = ch.posAndChar.z * globalScale;
        uint glyphIdx = uint(ch.posAndChar.w);

        GlyphData glyph = glyphData[glyphIdx];
        float4 bounds = glyph.bounds;

        // Check if pixel is near this glyph (screen space bounding box)
        // Account for glyph bounds offset (descenders have negative Y bounds)
        float2 glyphScreenMin = charPos + float2(bounds.x, bounds.y) * charScale;
        float2 glyphScreenMax = charPos + float2(bounds.z, bounds.w) * charScale;

        if (pixelPos.x < glyphScreenMin.x - 5.0 || pixelPos.x > glyphScreenMax.x + 5.0 ||
            pixelPos.y < glyphScreenMin.y - 5.0 || pixelPos.y > glyphScreenMax.y + 5.0) {
            continue;
        }

        // Transform from screen pixels to glyph local coordinates
        // Match the 3D shader approach - just scale, don't add bounds
        float2 localPos = (pixelPos - charPos) / charScale;

        // Compute SDF if inside bounds (with margin)
        if (localPos.x >= bounds.x - 0.1 && localPos.x <= bounds.z + 0.1 &&
            localPos.y >= bounds.y - 0.1 && localPos.y <= bounds.w + 0.1) {
            float d = sdGlyphAt(localPos, glyphIdx, bounds, glyph.gridInfo, glyph.curveInfo);
            minDist = min(minDist, d * charScale);
        }
    }

    return minDist;
}

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    // Convert UV to pixel coordinates, flip Y to match font coordinate system
    float2 pixelPos = float2(input.uv.x, 1.0 - input.uv.y) * uniforms.screenSize;

    // Compute SDF
    float d = sdText(pixelPos);

    // Anti-aliasing
    float aa = 1.0; // 1 pixel anti-alias
    float alpha = smoothstep(aa, -aa, d);

    // Text color (white on dark background)
    float3 bgColor = float3(0.1, 0.1, 0.12);
    float3 textColor = float3(0.95, 0.95, 0.95);

    float3 color = lerp(bgColor, textColor, alpha);
    return float4(color, 1.0);
}
