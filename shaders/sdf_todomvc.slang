// TodoMVC SDF Demo
// Renders TodoMVC UI using SDF primitives (rectangles, circles, lines)
// and exact Bézier SDF text computation from vector font data

struct Uniforms {
    float2 screenSize;
    float2 offset;       // pan offset in pixels (x, y)
    float4 textParams;   // x = charCount, y = scale, z = rotation, w = uiPrimCount
    float4 charGridParams;  // xy = gridDims, zw = cellSize
    float4 charGridBounds;  // xy = gridMin, zw = gridMax
};

// Grid cell structure (glyph spatial grid)
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

// Extended character instance with color
struct CharInstanceEx {
    float4 posAndChar;  // x, y, scale, glyphIndex
    float4 colorFlags;  // r, g, b, flags (flags: bit0 = strikethrough)
};

// Character grid cell (offset + count into charGridIndices)
struct CharGridCellData {
    uint offset;
    uint count;
};

// UI Primitive for rectangles, circles, lines, shadows
struct UiPrimitive {
    float4 posSize;    // x, y, w, h (or cx, cy, r, 0 for circles)
    float4 color;      // rgba
    float4 params;     // cornerRadius, borderWidth, blur, type
    float4 extra;      // shadow offset x/y, spread, extra
};

// Types: 0=FilledRect, 1=StrokedRect, 2=FilledCircle, 3=StrokedCircle,
//        4=Line, 5=BoxShadow, 6=CheckmarkV

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
StructuredBuffer<CharInstanceEx> charInstances;

[[vk::binding(6, 0)]]
StructuredBuffer<CharGridCellData> charGridCells;

[[vk::binding(7, 0)]]
StructuredBuffer<uint> charGridIndices;

[[vk::binding(8, 0)]]
StructuredBuffer<UiPrimitive> uiPrimitives;

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

// sRGB to linear conversion (needed because render target is Rgba8UnormSrgb)
float srgbToLinear(float c) {
    return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4);
}

// --- SDF Primitives ---

float sdRoundedBox(float2 p, float2 center, float2 halfSize, float radius) {
    float2 d = abs(p - center) - halfSize + radius;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - radius;
}

float sdCircle(float2 p, float2 center, float radius) {
    return length(p - center) - radius;
}

float sdLineSegment(float2 p, float2 a, float2 b) {
    float2 pa = p - a;
    float2 ba = b - a;
    float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// --- Bézier SDF (reused from text2d) ---

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

int curveWindingContribution(float2 point, float2 p0, float2 p1, float2 p2) {
    float px = point.x;
    float py = point.y;

    const float EPS = 1e-6;

    float y_min = min(min(p0.y, p1.y), p2.y);
    float y_max = max(max(p0.y, p1.y), p2.y);

    if (py < y_min - EPS || py >= y_max - EPS) return 0;

    float x_max = max(max(p0.x, p1.x), p2.x);
    if (px > x_max + EPS) return 0;

    float a = p0.y - 2.0 * p1.y + p2.y;
    float b = 2.0 * (p1.y - p0.y);
    float c = p0.y - py;

    int crossings = 0;

    if (abs(a) < 1e-7) {
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

float sdGlyphAt(float2 localPos, uint glyphIdx, float4 bounds, uint4 gridInfo, uint4 curveInfo) {
    uint gridOffset = gridInfo.x;
    uint gridSizeX = gridInfo.y;
    uint gridSizeY = gridInfo.z;
    uint glyphCurveOffset = curveInfo.x;
    uint glyphCurveCount = curveInfo.y;

    float2 gridUV = (localPos - bounds.xy) / (bounds.zw - bounds.xy);
    gridUV = clamp(gridUV, 0.0, 1.0);

    int cellX = min(int(gridUV.x * float(gridSizeX)), int(gridSizeX) - 1);
    int cellY = min(int(gridUV.y * float(gridSizeY)), int(gridSizeY) - 1);
    uint cellIdx = gridOffset + uint(cellY) * gridSizeX + uint(cellX);

    GridCell cell = gridCells[cellIdx];
    uint cellCurveStart = cell.curveStartAndCount & 0xFFFF;
    uint cellCurveCount = (cell.curveStartAndCount >> 16) & 0xFF;
    uint flags = (cell.curveStartAndCount >> 24) & 0xFF;

    bool cellIsInside = (flags & 0x01u) != 0u;
    bool isUniform = (flags & 0x02u) != 0u;

    float minDist = 1e10;
    for (uint i = 0u; i < cellCurveCount && i < 16u; i++) {
        uint globalCurveIdx = curveIndices[cellCurveStart + i];
        BezierCurve curve = curves[globalCurveIdx];

        float2 p0 = curve.points01.xy;
        float2 p1 = curve.points01.zw;
        float2 p2 = curve.points2bbox.xy;

        float2 mid = (p0 + 2.0 * p1 + p2) * 0.25;
        float midDist = length(localPos - mid);
        float4 bbox = float4(curve.points2bbox.zw, curve.bboxFlags.xy);
        float curveRadius = length(bbox.zw - bbox.xy) * 0.5;
        if (midDist - curveRadius > minDist) continue;

        float dist = sdQuadraticBezier(localPos, p0, p1, p2);
        minDist = min(minDist, dist);
    }

    if (isUniform && cellCurveCount == 0u) {
        return (cellIsInside ? -1.0 : 1.0) * minDist;
    }

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

// Character grid helpers
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

// Composite a color with premultiplied alpha over existing color
float4 blendOver(float4 bg, float4 fg) {
    float outA = fg.a + bg.a * (1.0 - fg.a);
    if (outA < 0.001) return float4(0.0);
    float3 outRGB = (fg.rgb * fg.a + bg.rgb * bg.a * (1.0 - fg.a)) / outA;
    return float4(outRGB, outA);
}

// Render UI primitives (back-to-front compositing)
float4 renderUiPrimitives(float2 p, uint primCount) {
    float4 result = float4(0.0);

    for (uint i = 0u; i < primCount && i < 64u; i++) {
        UiPrimitive prim = uiPrimitives[i];
        float primType = prim.params.w;
        float4 col = prim.color;

        float d = 1e10;
        float alpha = 0.0;

        if (primType < 0.5) {
            // FilledRect
            float2 center = prim.posSize.xy + prim.posSize.zw * 0.5;
            float2 halfSize = prim.posSize.zw * 0.5;
            float radius = prim.params.x;
            d = sdRoundedBox(p, center, halfSize, radius);
            alpha = smoothstep(0.5, -0.5, d);
        }
        else if (primType < 1.5) {
            // StrokedRect
            float2 center = prim.posSize.xy + prim.posSize.zw * 0.5;
            float2 halfSize = prim.posSize.zw * 0.5;
            float radius = prim.params.x;
            float borderW = prim.params.y;
            d = sdRoundedBox(p, center, halfSize, radius);
            alpha = smoothstep(0.5, -0.5, abs(d) - borderW * 0.5);
        }
        else if (primType < 2.5) {
            // FilledCircle
            float2 center = prim.posSize.xy;
            float radius = prim.posSize.z;
            d = sdCircle(p, center, radius);
            alpha = smoothstep(0.5, -0.5, d);
        }
        else if (primType < 3.5) {
            // StrokedCircle
            float2 center = prim.posSize.xy;
            float radius = prim.posSize.z;
            float borderW = prim.params.y;
            d = sdCircle(p, center, radius);
            alpha = smoothstep(0.5, -0.5, abs(d) - borderW * 0.5);
        }
        else if (primType < 4.5) {
            // Line
            float2 a = prim.posSize.xy;
            float2 b = prim.posSize.zw;
            float lineW = prim.params.y;
            d = sdLineSegment(p, a, b) - lineW * 0.5;
            alpha = smoothstep(0.5, -0.5, d);
        }
        else if (primType < 5.5) {
            // BoxShadow
            float2 center = prim.posSize.xy + prim.posSize.zw * 0.5;
            float2 halfSize = prim.posSize.zw * 0.5;
            float2 shadowOffset = prim.extra.xy;
            float spread = prim.extra.z;
            float blur = prim.params.z;
            float radius = prim.params.x;

            float2 shadowCenter = center + shadowOffset;
            float2 shadowHalfSize = halfSize + spread;
            d = sdRoundedBox(p, shadowCenter, shadowHalfSize, radius);
            alpha = col.a * smoothstep(blur, -blur * 0.1, d);
            col.a = 1.0;
        }
        else if (primType < 6.5) {
            // CheckmarkV - two line segments forming a V shape
            float2 a = prim.posSize.xy;  // left point
            float2 b = prim.posSize.zw;  // bottom point (vertex of V)
            float2 c = prim.extra.xy;    // right point
            float lineW = prim.params.y;
            float d1 = sdLineSegment(p, a, b) - lineW * 0.5;
            float d2 = sdLineSegment(p, b, c) - lineW * 0.5;
            d = min(d1, d2);
            alpha = smoothstep(0.5, -0.5, d);
        }

        float4 fg = float4(col.rgb, col.a * alpha);
        result = blendOver(result, fg);
    }

    return result;
}

// Render text with per-character color via spatial grid
float4 renderText(float2 pixelPos, float scale) {
    float4 result = float4(0.0);

    int2 cellCoord = charGridCellCoord(pixelPos);
    uint cellIdx = charGridCellIndex(cellCoord);
    CharGridCellData cgCell = charGridCells[cellIdx];
    uint start = cgCell.offset;
    uint count = cgCell.count;

    float aa = 1.0 / max(scale, 0.5);

    for (uint i = 0u; i < count && i < 16u; i++) {
        uint charIdx = charGridIndices[start + i];
        CharInstanceEx ch = charInstances[charIdx];
        float2 charPos = ch.posAndChar.xy;
        float charScale = ch.posAndChar.z;
        uint glyphIdx = uint(ch.posAndChar.w);

        GlyphData glyph = glyphData[glyphIdx];
        float4 bounds = glyph.bounds;

        float2 localPos = (pixelPos - charPos) / charScale;

        if (localPos.x >= bounds.x - 0.1 && localPos.x <= bounds.z + 0.1 &&
            localPos.y >= bounds.y - 0.1 && localPos.y <= bounds.w + 0.1) {
            float d = sdGlyphAt(localPos, glyphIdx, bounds, glyph.gridInfo, glyph.curveInfo);
            float dist = d * charScale;
            float alpha = smoothstep(aa, -aa, dist);

            if (alpha > 0.001) {
                float3 textColor = ch.colorFlags.rgb;
                float4 fg = float4(textColor, alpha);
                result = blendOver(result, fg);
            }
        }
    }

    return result;
}

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    // Virtual viewport: matches metadata 700x700 capture size
    static const float2 VIRTUAL_SIZE = float2(700.0, 700.0);
    float2 actualSize = uniforms.screenSize;

    // 1:1 pixel mapping — center 700x700 content in actual viewport
    float2 margin = (actualSize - VIRTUAL_SIZE) * 0.5;

    // Convert UV to actual pixels, then to virtual coordinates (no scaling)
    float2 actualPixel = float2(input.uv.x * actualSize.x, (1.0 - input.uv.y) * actualSize.y);
    float2 pixelPos = actualPixel - margin;

    // Apply pan/zoom/rotate transform around virtual center
    float2 center = VIRTUAL_SIZE * 0.5;
    float scale = uniforms.textParams.y;
    float rotation = uniforms.textParams.z;
    float2 offset = uniforms.offset;

    // Rotate around center
    float c = cos(rotation);
    float s = sin(rotation);
    float2 p = pixelPos - center;
    pixelPos = float2(p.x * c - p.y * s, p.x * s + p.y * c) + center;

    // Then apply zoom and offset
    pixelPos = (pixelPos - center) / scale + center + offset;

    uint uiPrimCount = uint(uniforms.textParams.w);

    // Outside the 700x700 virtual viewport: dark gray
    bool outsideViewport = pixelPos.x < 0.0 || pixelPos.x > VIRTUAL_SIZE.x ||
                           pixelPos.y < 0.0 || pixelPos.y > VIRTUAL_SIZE.y;
    // All compositing in sRGB space (matches browser text rendering behavior).
    // Final result converted to linear for the Rgba8UnormSrgb framebuffer.
    float3 finalColor;

    if (outsideViewport) {
        finalColor = float3(0.25, 0.25, 0.25);
    } else {
        // Background: #f5f5f5 (sRGB)
        float3 bgColor = float3(0.96078, 0.96078, 0.96078);
        float4 color = float4(bgColor, 1.0);

        // Render UI primitives (shadows, rects, borders, circles, lines)
        float4 uiColor = renderUiPrimitives(pixelPos, uiPrimCount);
        color = blendOver(color, uiColor);

        // Render text
        float4 textColor = renderText(pixelPos, scale);
        color = blendOver(color, textColor);

        finalColor = color.rgb;
    }

    // Convert sRGB composited result → linear for sRGB framebuffer hardware encode
    return float4(srgbToLinear(finalColor.r), srgbToLinear(finalColor.g), srgbToLinear(finalColor.b), 1.0);
}
