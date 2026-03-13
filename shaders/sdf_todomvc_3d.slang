// Shared retained UiPhysical / stacked-card SDF scene with vector text support
// Based on sdf_clay_vector.slang with theme uniforms and per-character color

struct Uniforms {
    float4x4 invViewProj;
    float4 cameraPosTime;     // xyz = cameraPos, w = time
    float4 lightDirIntensity; // xyz = lightDir, w = intensity
    float4 renderParams;      // xy = resolution, z = textDepth, w = textScale
    float4 textParams;        // x = charCount, y = uiPrimCount, z = windowScaleFactor, w = classicDecalPrimStart
    float4 charGridParams;    // xy = gridDims, zw = cellSize
    float4 charGridBounds;    // xy = gridMin, zw = gridMax
    float4 layoutParams0;     // xy = layoutCenterPx (Y-up), z = pixelToWorld, w = geometryMode (0=generic,1=stackedCard)
    float4 layoutParams1;     // x = generic cornerRadiusPx, y = elevationPx, z = depthPx
    float4 layoutParams2;     // generic fill rgba
    float4 layoutParams3;     // generic outline rgba
    float4 layoutParams4;     // generic shadow rgba
    float4 layoutParams5;     // generic shadow offset xy in px
    float4 layoutParams6;     // generic shadow extra size xy in px
    float4 layoutParams7;     // generic outline width px, content inset px
    float4 layoutParams8;     // generic accent rgba
    float4 layoutParams9;     // generic detail rgba
    float4 layoutBounds;      // xy = layoutMinPx, zw = layoutMaxPx
};

struct ThemeUniforms {
    float4 materialColors[16];  // per materialId: rgb + alpha
    float4 materialProps[16];   // [gloss, metalness, glow, transparency]
    float4 geometryParams;      // edgeRadius, cardElevation, itemElevation, inputDepth
    float4 ambientColor;        // rgb + intensity
    float4 extraParams;         // textReliefDepth, textReliefMode(0=raised,1=carved), themeId, darkMode
};

// Grid cell structure
struct GridCell {
    uint curveStartAndCount;
};

// Bézier curve (packed format)
struct BezierCurve {
    float4 points01;
    float4 points2bbox;
    float4 bboxFlags;
};

// Glyph metadata
struct GlyphData {
    float4 bounds;
    uint4 gridInfo;
    uint4 curveInfo;
};

// Character instance with color
struct CharInstanceEx {
    float4 posAndChar;   // x = pixel x, y = pixel y (Y-up), z = fontSizePx, w = charIndex
    float4 colorFlags;   // rgb = color, w = flags (0=active, 1=completed, 2=heading, 3=placeholder)
};

// Character grid cell
struct CharGridCellData {
    uint offset;
    uint count;
};

struct UiPrimitive {
    float4 posSize;
    float4 color;
    float4 params;
    float4 extra;
};

[[vk::binding(0, 0)]]
ConstantBuffer<Uniforms> uniforms;

[[vk::binding(1, 0)]]
ConstantBuffer<ThemeUniforms> theme;

[[vk::binding(2, 0)]]
StructuredBuffer<GridCell> gridCells;

[[vk::binding(3, 0)]]
StructuredBuffer<uint> curveIndices;

[[vk::binding(4, 0)]]
StructuredBuffer<BezierCurve> curves;

[[vk::binding(5, 0)]]
StructuredBuffer<GlyphData> glyphData;

[[vk::binding(6, 0)]]
StructuredBuffer<CharInstanceEx> charInstances;

[[vk::binding(7, 0)]]
StructuredBuffer<CharGridCellData> charGridCells;

[[vk::binding(8, 0)]]
StructuredBuffer<uint> charGridIndices;

[[vk::binding(9, 0)]]
StructuredBuffer<UiPrimitive> uiPrimitives;

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

static const float THEME_CLASSIC2D = 0.0;
static const float THEME_GLASSMORPHISM = 3.0;

bool isClassicTheme() {
    return abs(theme.extraParams.z - THEME_CLASSIC2D) < 0.5;
}

bool isGlassTheme() {
    return abs(theme.extraParams.z - THEME_GLASSMORPHISM) < 0.5;
}

float layoutCenterX() { return uniforms.layoutParams0.x; }
float layoutCenterY() { return uniforms.layoutParams0.y; }
float layoutPixelToWorld() { return max(uniforms.layoutParams0.z, 1e-5); }
bool useStackedCardGeometry() { return uniforms.layoutParams0.w > 0.5; }
// Shared UiPhysical scenes are still authored in the original floor/XZ scene
// model. Rotate world space into that legacy scene model at the raymarch
// boundary so the rendered result becomes wall-facing in user space.
float3 worldToLegacyScene(float3 p) { return float3(p.x, p.z, -p.y); }
float3 worldToLegacyDirection(float3 d) { return float3(d.x, d.z, -d.y); }
float genericCardCornerRadiusPx() { return max(uniforms.layoutParams1.x, 0.0); }
float genericCardElevationPx() { return max(uniforms.layoutParams1.y, 0.0); }
float genericCardDepthPx() { return max(uniforms.layoutParams1.z, 0.0); }
float4 genericCardFillColor() { return uniforms.layoutParams2; }
float4 genericCardOutlineColor() { return uniforms.layoutParams3; }
float genericCardOutlineWidthPx() { return max(uniforms.layoutParams7.x, 0.0); }
float genericCardContentInsetPx() { return max(uniforms.layoutParams7.y, 0.0); }
float4 genericCardShadowColor() { return uniforms.layoutParams4; }
float2 genericCardShadowOffsetPx() { return uniforms.layoutParams5.xy; }
float2 genericCardShadowExtraSizePx() { return uniforms.layoutParams6.xy; }
float4 genericCardAccentColor() { return uniforms.layoutParams8; }
float4 genericCardDetailColor() { return uniforms.layoutParams9; }

float2 genericCardHalfExtents() {
    float4 bounds = uniforms.layoutBounds;
    float minHalf = max(
        genericCardCornerRadiusPx() * layoutPixelToWorld() +
        max(
            genericCardOutlineWidthPx() * layoutPixelToWorld() * 0.12,
            genericCardCornerRadiusPx() * layoutPixelToWorld() * 0.01
        ),
        0.0
    );
    float halfW = max((bounds.z - bounds.x) * 0.5 * layoutPixelToWorld(), minHalf);
    float halfD = max((bounds.w - bounds.y) * 0.5 * layoutPixelToWorld(), minHalf);
    return float2(halfW, halfD);
}

float genericCardCornerRadius() {
    float2 halfExtents = genericCardHalfExtents();
    float radius = genericCardCornerRadiusPx() * layoutPixelToWorld();
    float maxRadius = max(min(halfExtents.x, halfExtents.y) - 0.002, 0.0);
    return clamp(radius, 0.0, maxRadius);
}

float genericCardElevation() {
    float semanticLift = max(
        genericCardOutlineWidthPx() * layoutPixelToWorld() * 0.08,
        genericCardCornerRadiusPx() * layoutPixelToWorld() * 0.015
    );
    return max(genericCardElevationPx() * layoutPixelToWorld(), genericCardHalfHeight() + semanticLift);
}

float genericCardHalfHeight() {
    float semanticMinHeight = max(
        genericCardOutlineWidthPx() * layoutPixelToWorld() * 0.22,
        genericCardCornerRadiusPx() * layoutPixelToWorld() * 0.045
    );
    return max(genericCardDepthPx() * layoutPixelToWorld() * 0.5, semanticMinHeight);
}

float genericCardShadowDrop() {
    float spreadDrop = min(genericCardShadowSpreadWorld().x, genericCardShadowSpreadWorld().y) * 0.25;
    float fillAlpha = saturate(genericCardFillColor().w);
    float alphaDrop = lerp(0.55, 1.0, fillAlpha * fillAlpha);
    return max((genericCardDepthPx() * layoutPixelToWorld() * 0.25 + spreadDrop) * alphaDrop, 0.0);
}

float3 genericCardShadowCenter() {
    float2 shadowOffset = genericCardShadowOffsetPx() * layoutPixelToWorld();
    float shadowY = genericCardElevation() - genericCardHalfHeight() - genericCardShadowDrop();
    return float3(shadowOffset.x, shadowY, -shadowOffset.y);
}

float2 genericCardShadowSpreadWorld() {
    return max(genericCardShadowExtraSizePx() * layoutPixelToWorld(), float2(0.0, 0.0));
}

float2 genericCardShadowHalfExtents() {
    return genericCardHalfExtents() + genericCardShadowSpreadWorld();
}

float genericCardShadowHalfHeight() {
    float spreadThickness = max(min(genericCardShadowSpreadWorld().x, genericCardShadowSpreadWorld().y) * 0.35, 0.0);
    float semanticMinHeight = max(
        genericCardOutlineWidthPx() * layoutPixelToWorld() * 0.12,
        genericCardCornerRadiusPx() * layoutPixelToWorld() * 0.02
    );
    float fillAlpha = saturate(genericCardFillColor().w);
    float alphaThickness = lerp(0.65, 1.0, fillAlpha * fillAlpha);
    return max((genericCardHalfHeight() * 0.2 + spreadThickness) * alphaThickness, semanticMinHeight * alphaThickness);
}

float genericCardShadowCornerRadius() {
    return min(
        genericCardCornerRadius() + min(genericCardShadowSpreadWorld().x, genericCardShadowSpreadWorld().y),
        max(min(genericCardShadowHalfExtents().x, genericCardShadowHalfExtents().y) - 0.002, 0.0)
    );
}

// ---- SDF primitives ----

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

float sdRoundedBox2D(float2 p, float2 center, float2 halfSize, float radius) {
    float2 d = abs(p - center) - halfSize + radius;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - radius;
}

float sdCircle2D(float2 p, float2 center, float radius) {
    return length(p - center) - radius;
}

float sdLineSegment2D(float2 p, float2 a, float2 b) {
    float2 pa = p - a;
    float2 ba = b - a;
    float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// ---- Hash function for glass noise ----

float3 hash33(float3 p) {
    p = frac(p * float3(0.1031, 0.1030, 0.0973));
    p += dot(p, p.yxz + 33.33);
    return frac((p.xxy + p.yxx) * p.zyx);
}

// ---- Quadratic Bézier SDF ----

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

float sdGlyph(float2 localPos, float4 bounds, uint4 gridInfo, uint4 curveInfo) {
    uint glyphCurveOffset = curveInfo.x;
    uint glyphCurveCount = curveInfo.y;

    float2 glyphSize = bounds.zw - bounds.xy;
    float2 gridUV = (localPos - bounds.xy) / glyphSize;

    if (gridUV.x < 0.0 || gridUV.x > 1.0 || gridUV.y < 0.0 || gridUV.y > 1.0) {
        float2 d = max(bounds.xy - localPos, localPos - bounds.zw);
        return length(max(d, 0.0)) + 0.01;
    }

    float minDist = 1e10;
    int winding = 0;
    for (uint i = 0u; i < glyphCurveCount && i < 64u; i++) {
        BezierCurve curve = curves[glyphCurveOffset + i];
        float2 p0 = curve.points01.xy;
        float2 p1 = curve.points01.zw;
        float2 p2 = curve.points2bbox.xy;

        winding += curveWindingContribution(localPos, p0, p1, p2);

        float2 bboxMin = curve.points2bbox.zw;
        float2 bboxMax = curve.bboxFlags.xy;
        float2 dd = max(bboxMin - localPos, localPos - bboxMax);
        float bboxDist = length(max(dd, 0.0));
        if (bboxDist <= minDist) {
            minDist = min(minDist, sdQuadraticBezier(localPos, p0, p1, p2));
        }
    }

    float sign_val = (winding != 0) ? -1.0 : 1.0;
    return sign_val * minDist;
}

// Brute-force glyph SDF — evaluates ALL curves, no bounds approximation.
// Avoids the +0.01 discontinuity in sdGlyph's bounds check that causes
// rectangle artifacts with smoothstep AA and rounded extrusion.
float sdGlyphBruteForce(float2 localPos, uint4 curveInfo) {
    uint offset = curveInfo.x;
    uint count = curveInfo.y;
    float minDist = 1e10;
    int winding = 0;
    for (uint i = 0u; i < count && i < 64u; i++) {
        BezierCurve curve = curves[offset + i];
        float2 p0 = curve.points01.xy;
        float2 p1 = curve.points01.zw;
        float2 p2 = curve.points2bbox.xy;
        minDist = min(minDist, sdQuadraticBezier(localPos, p0, p1, p2));
        winding += curveWindingContribution(localPos, p0, p1, p2);
    }
    return (winding != 0 ? -1.0 : 1.0) * minDist;
}

// ---- Character grid lookup ----

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

// ---- Extrusion ----

float opExtrusion(float sdf2d, float zDist, float depth) {
    float2 w = float2(sdf2d, abs(zDist) - depth);
    return min(max(w.x, w.y), 0.0) + length(max(w, 0.0));
}

// Rounded extrusion — softens text edges for better readability
float opExtrusionRound(float sdf2d, float zDist, float depth, float r) {
    float2 w = float2(sdf2d, abs(zDist) - depth);
    return min(max(w.x, w.y), 0.0) + length(max(w, 0.0)) - r;
}

// ---- Hit struct ----

struct Hit {
    float dist;
    int materialId;
    float3 textColor;  // filled when hitting text
};

Hit opUnionHit(Hit a, Hit b) {
    if (a.dist < b.dist) return a;
    return b;
}

// ---- Scene geometry constants ----
// Card dimensions in world space (card is on XZ plane, Y is up)
// In 3D, Z maps as: p.z = (pixel_y - 302) / 100
// Card top (py=130) → Z = -1.72, card bottom (py=474.2) → Z = +1.72
static const float CARD_W = 2.75;  // half-width (X)
static const float CARD_D = 1.72;  // half-depth (Z)
static const float CARD_Y = 0.0;   // card center Y

// World Z positions derived from pixel Y coords (center=302, scale=1/100)
// Negative Z = top of card, positive Z = bottom of card
static const float INPUT_TOP_Z = -1.72;    // py=130, same as card top
static const float INPUT_BOT_Z = -1.062;   // py=195.8
static const float SEP1_Z = -0.466;        // py=255.4
static const float SEP2_Z = 0.130;         // py=315.0
static const float SEP3_Z = 0.726;         // py=374.6
static const float FOOTER_Z = 1.314;       // py=433.4
// Item center Z positions (between borders)
static const float ITEM1_Z = -0.764;       // between INPUT_BOT and SEP1
static const float ITEM2_Z = -0.168;       // between SEP1 and SEP2
static const float ITEM3_Z = 0.428;        // between SEP2 and SEP3
static const float ITEM4_Z = 1.020;        // between SEP3 and FOOTER
static const float ITEM_HALF_H = 0.28;     // item half-height in Z (slightly smaller than spacing)
// Checkbox position
static const float CB_X = -2.49;           // pixel x=101: (101-350)/100
static const float CB_RADIUS = 0.17;       // 17 pixels
static const float CB_TUBE_R = 0.006;      // tube radius for torus ring

// ---- Text relief with per-character color ----

struct TextHit {
    float dist;
    float3 color;
    bool valid;
};

TextHit sdTextRelief3D(float3 p, float depth) {
    TextHit result;
    result.dist = 1e10;
    result.color = float3(0.5);
    result.valid = false;

    float halfDepth = depth * 0.5;

    // Text AABB early-exit (with padded Y extent for conservative raymarching)
    float2 gridMin = uniforms.charGridBounds.xy;
    float2 gridMax = uniforms.charGridBounds.zw;
    float aabbPadY = 0.3; // extra Y padding so raymarcher sees text slab from further away
    float3 textBoxMin = float3(
        (gridMin.x - layoutCenterX()) * layoutPixelToWorld(),
        -halfDepth - aabbPadY,
        -(gridMax.y - layoutCenterY()) * layoutPixelToWorld()
    );
    float3 textBoxMax = float3(
        (gridMax.x - layoutCenterX()) * layoutPixelToWorld(),
        halfDepth + aabbPadY,
        -(gridMin.y - layoutCenterY()) * layoutPixelToWorld()
    );
    float3 textBoxCenter = (textBoxMin + textBoxMax) * 0.5;
    float3 textBoxHalf = (textBoxMax - textBoxMin) * 0.5;
    float textBoxDist = sdBox(p - textBoxCenter, textBoxHalf);
    if (textBoxDist > 0.1) {
        result.dist = textBoxDist;
        return result;
    }

    // Convert scene XZ to the shared 2D layout pixel space (Y-up)
    float2 textPos = float2(
        p.x / layoutPixelToWorld() + layoutCenterX(),
        -p.z / layoutPixelToWorld() + layoutCenterY()
    );

    // Check 3x3 neighborhood of grid cells to avoid missing chars at cell boundaries
    int2 centerCell = charGridCellCoord(textPos);
    int2 gridDims = int2(uniforms.charGridParams.xy);

    for (int dy = -1; dy <= 1; dy++) {
        if (result.dist < 0.005) break;
        for (int dx = -1; dx <= 1; dx++) {
            if (result.dist < 0.005) break;
            int2 cell = centerCell + int2(dx, dy);
            if (cell.x < 0 || cell.y < 0 || cell.x >= gridDims.x || cell.y >= gridDims.y)
                continue;

            uint cellIdx = charGridCellIndex(cell);
            CharGridCellData cgCell = charGridCells[cellIdx];
            uint start = cgCell.offset;
            uint count = cgCell.count;

            for (uint i = 0u; i < count && i < 16u; i++) {
                uint charIdx = charGridIndices[start + i];
                CharInstanceEx ch = charInstances[charIdx];

                // Glass theme: only 3D-render heading chars (flag==2), body text is decal
                bool glassTheme = isGlassTheme();
                if (glassTheme && ch.colorFlags.w != 2.0) continue;

                float2 charPos = ch.posAndChar.xy;
                float charScalePx = ch.posAndChar.z;
                float charScaleWorld = charScalePx * layoutPixelToWorld();
                uint glyphIdx = uint(ch.posAndChar.w);

                GlyphData glyph = glyphData[glyphIdx];
                float4 bounds = glyph.bounds;

                float2 localP = (textPos - charPos) / charScalePx;

                if (localP.x >= bounds.x - 0.15 && localP.x <= bounds.z + 0.15 &&
                    localP.y >= bounds.y - 0.15 && localP.y <= bounds.w + 0.15) {
                    float d2d = glassTheme
                        ? sdGlyphBruteForce(localP, glyph.curveInfo) * charScaleWorld
                        : sdGlyph(localP, bounds, glyph.gridInfo, glyph.curveInfo) * charScaleWorld;
                    // Glass heading: subtle rounding for smooth edges
                    float roundR = 0.0;
                    if (glassTheme && ch.colorFlags.w == 2.0) {
                        roundR = 0.015;
                    }
                    float d3d = (roundR > 0.0)
                        ? opExtrusionRound(d2d, p.y, max(halfDepth - roundR, 0.001), roundR)
                        : opExtrusion(d2d, p.y, halfDepth);
                    if (d3d < result.dist) {
                        result.dist = d3d;
                        result.color = ch.colorFlags.xyz;
                        result.valid = true;
                    }
                }
            }
        }
    }

    // If inside the text AABB but no character found, return slab distance
    // to prevent the raymarcher from taking huge steps through the text region
    if (!result.valid) {
        float slabDist = max(abs(p.y) - halfDepth, 0.0);
        // Use distance to nearest cell boundary as XZ component
        float2 cellSize = uniforms.charGridParams.zw;
        float cellMinDist = min(cellSize.x, cellSize.y) * 0.5;
        result.dist = max(slabDist, cellMinDist);
    }

    return result;
}

// ---- Coarse scene (for shadow/AO) ----

Hit mapSceneCoarse(float3 p) {
    Hit result;
    result.dist = 1e10;
    result.materialId = 0;
    result.textColor = float3(0.5);

    float edgeR = theme.geometryParams.x;
    float cardElev = theme.geometryParams.y;

    if (!useStackedCardGeometry()) {
        float2 halfExtents = genericCardHalfExtents();

        if (genericCardShadowColor().w > 0.001) {
            Hit shadow;
            float3 shadowP = p - genericCardShadowCenter();
            shadow.dist = sdRoundBox(
                shadowP,
                float3(genericCardShadowHalfExtents().x, genericCardShadowHalfHeight(), genericCardShadowHalfExtents().y),
                genericCardShadowCornerRadius()
            );
            shadow.materialId = 6;
            shadow.textColor = float3(0);
            result = opUnionHit(result, shadow);
        }

        Hit card;
        float3 cardP = p - float3(0, genericCardElevation(), 0);
        card.dist = sdRoundBox(
            cardP,
            float3(halfExtents.x, genericCardHalfHeight(), halfExtents.y),
            genericCardCornerRadius()
        );
        card.materialId = 2;
        card.textColor = float3(0);
        result = opUnionHit(result, card);

        return result;
    }

    if (isClassicTheme()) {
        Hit ground;
        ground.dist = sdPlane(p, float3(0, 1, 0), 0.5);
        ground.materialId = 1;
        ground.textColor = float3(0);
        result = opUnionHit(result, ground);

        Hit stack1;
        float3 stack1P = p - float3(0, cardElev - 0.005, 0.04);
        stack1.dist = sdRoundBox(stack1P, float3(CARD_W - 0.02, 0.035, CARD_D), 0.0);
        stack1.materialId = 6;
        stack1.textColor = float3(0);
        result = opUnionHit(result, stack1);

        Hit stack2;
        float3 stack2P = p - float3(0, cardElev - 0.01, 0.08);
        stack2.dist = sdRoundBox(stack2P, float3(CARD_W - 0.04, 0.03, CARD_D), 0.0);
        stack2.materialId = 6;
        stack2.textColor = float3(0);
        result = opUnionHit(result, stack2);

        Hit card;
        float3 cardP = p - float3(0, cardElev, 0);
        card.dist = sdRoundBox(cardP, float3(CARD_W, 0.04, CARD_D), 0.0);
        card.materialId = 2;
        card.textColor = float3(0);
        result = opUnionHit(result, card);

        return result;
    }

    // Ground plane
    Hit ground;
    ground.dist = sdPlane(p, float3(0, 1, 0), 0.5);
    ground.materialId = 1;
    ground.textColor = float3(0);
    result = opUnionHit(result, ground);

    // Main card box
    Hit card;
    float3 cardP = p - float3(0, cardElev, 0);
    card.dist = sdRoundBox(cardP, float3(CARD_W, 0.04, CARD_D), edgeR);
    card.materialId = 2;
    card.textColor = float3(0);
    result = opUnionHit(result, card);

    return result;
}

// ---- Full scene ----

// Thread-local for text color passing from mapScene to shading
static float3 g_textColor = float3(0.5);

Hit mapScene(float3 p) {
    Hit result;
    result.dist = 1e10;
    result.materialId = 0;
    result.textColor = float3(0.5);

    float textScale = uniforms.renderParams.w;
    float textDepth = uniforms.renderParams.z;
    float edgeR = theme.geometryParams.x;
    float cardElev = theme.geometryParams.y;
    float itemElev = theme.geometryParams.z;
    float inputDepth = theme.geometryParams.w;
    float textReliefDepth = theme.extraParams.x;
    float textReliefMode = theme.extraParams.y;

    if (!useStackedCardGeometry()) {
        float2 halfExtents = genericCardHalfExtents();

        if (genericCardShadowColor().w > 0.001) {
            Hit shadow;
            float3 shadowP = p - genericCardShadowCenter();
            shadow.dist = sdRoundBox(
                shadowP,
                float3(genericCardShadowHalfExtents().x, genericCardShadowHalfHeight(), genericCardShadowHalfExtents().y),
                genericCardShadowCornerRadius()
            );
            shadow.materialId = 6;
            shadow.textColor = float3(0);
            result = opUnionHit(result, shadow);
        }

        Hit card;
        float3 cardP = p - float3(0, genericCardElevation(), 0);
        card.dist = sdRoundBox(
            cardP,
            float3(halfExtents.x, genericCardHalfHeight(), halfExtents.y),
            genericCardCornerRadius()
        );
        card.materialId = 2;
        card.textColor = float3(0);
        result = opUnionHit(result, card);

        return result;
    }

    if (isClassicTheme()) {
        Hit ground;
        ground.dist = sdPlane(p, float3(0, 1, 0), 0.5);
        ground.materialId = 1;
        ground.textColor = float3(0);
        result = opUnionHit(result, ground);

        Hit stack1;
        float3 stack1P = p - float3(0, cardElev - 0.005, 0.04);
        stack1.dist = sdRoundBox(stack1P, float3(CARD_W - 0.02, 0.035, CARD_D), 0.0);
        stack1.materialId = 6;
        stack1.textColor = float3(0);
        result = opUnionHit(result, stack1);

        Hit stack2;
        float3 stack2P = p - float3(0, cardElev - 0.01, 0.08);
        stack2.dist = sdRoundBox(stack2P, float3(CARD_W - 0.04, 0.03, CARD_D), 0.0);
        stack2.materialId = 6;
        stack2.textColor = float3(0);
        result = opUnionHit(result, stack2);

        Hit card;
        float3 cardP = p - float3(0, cardElev, 0);
        card.dist = sdRoundBox(cardP, float3(CARD_W, 0.04, CARD_D), 0.0);
        card.materialId = 2;
        card.textColor = float3(0);
        result = opUnionHit(result, card);

        return result;
    }

    // Ground plane
    Hit ground;
    ground.dist = sdPlane(p, float3(0, 1, 0), 0.5);
    ground.materialId = 1;
    ground.textColor = float3(0);
    result = opUnionHit(result, ground);

    // Main card
    float3 cardP = p - float3(0, cardElev, 0);
    float cardDist = sdRoundBox(cardP, float3(CARD_W, 0.04, CARD_D), edgeR);

    // Input well (CSG subtract from top of card)
    // Input area: pixel Y=130 to 195.8 → Z = -1.72 to -1.062
    float inputCenterZ = (INPUT_TOP_Z + INPUT_BOT_Z) * 0.5;  // -1.391
    float inputHalfZ = (INPUT_BOT_Z - INPUT_TOP_Z) * 0.5;    // 0.329
    float3 inputP = cardP - float3(0, 0.02, inputCenterZ);
    float inputWell = sdRoundBox(inputP, float3(CARD_W - 0.04, inputDepth, inputHalfZ), edgeR * 0.5);
    cardDist = max(cardDist, -inputWell);  // CSG subtract

    // Raised stacked-card item panels (4 items)
    float itemCenters[4] = { ITEM1_Z, ITEM2_Z, ITEM3_Z, ITEM4_Z };
    for (int i = 0; i < 4; i++) {
        float3 itemP = cardP - float3(0, 0, itemCenters[i]);
        float itemDist = sdRoundBox(itemP, float3(CARD_W - 0.04, 0.04 + itemElev, ITEM_HALF_H), edgeR * 0.3);
        Hit item;
        item.dist = itemDist;
        item.materialId = 3;  // todo item material
        item.textColor = float3(0);
        result = opUnionHit(result, item);

        // Checkbox circle ring (torus lying flat on card surface)
        float3 cbP = cardP - float3(CB_X, 0.04, itemCenters[i]);
        float2 torusQ = float2(length(cbP.xz) - CB_RADIUS, cbP.y);
        float cbDist = length(torusQ) - CB_TUBE_R;
        Hit cbHit;
        cbHit.dist = cbDist;
        cbHit.materialId = 4;  // checkbox ring
        cbHit.textColor = float3(0);
        result = opUnionHit(result, cbHit);
    }

    // Separator lines between items
    float sepZs[3] = { SEP1_Z, SEP2_Z, SEP3_Z };
    for (int s = 0; s < 3; s++) {
        float3 sepP = cardP - float3(0, 0.04, sepZs[s]);
        float sepDist = sdRoundBox(sepP, float3(CARD_W - 0.06, 0.002, 0.002), 0.001);
        Hit sep;
        sep.dist = sepDist;
        sep.materialId = 5;  // separator
        sep.textColor = float3(0);
        result = opUnionHit(result, sep);
    }

    // Input/list border (between input area and first todo item)
    {
        float3 borderP = cardP - float3(0, 0.04, INPUT_BOT_Z);
        float borderDist = sdRoundBox(borderP, float3(CARD_W - 0.06, 0.002, 0.002), 0.001);
        Hit border;
        border.dist = borderDist;
        border.materialId = 5;
        border.textColor = float3(0);
        result = opUnionHit(result, border);
    }

    // Footer border (between last item and footer)
    {
        float3 footerP = cardP - float3(0, 0.04, FOOTER_Z);
        float footerDist = sdRoundBox(footerP, float3(CARD_W - 0.04, 0.002, 0.002), 0.001);
        Hit footer;
        footer.dist = footerDist;
        footer.materialId = 5;
        footer.textColor = float3(0);
        result = opUnionHit(result, footer);
    }

    // Stacked decorative cards (behind and below main card, peek at bottom = +Z)
    Hit stack1;
    float3 stack1P = p - float3(0, cardElev - 0.005, 0.04);
    stack1.dist = sdRoundBox(stack1P, float3(CARD_W - 0.02, 0.035, CARD_D), edgeR);
    stack1.materialId = 6;  // stack card
    stack1.textColor = float3(0);
    result = opUnionHit(result, stack1);

    Hit stack2;
    float3 stack2P = p - float3(0, cardElev - 0.01, 0.08);
    stack2.dist = sdRoundBox(stack2P, float3(CARD_W - 0.04, 0.03, CARD_D), edgeR);
    stack2.materialId = 6;
    stack2.textColor = float3(0);
    result = opUnionHit(result, stack2);

    // Main card body
    Hit card;
    card.dist = cardDist;
    card.materialId = 2;  // main card
    card.textColor = float3(0);
    result = opUnionHit(result, card);

    // Text relief — always include text distance in scene SDF to prevent
    // the raymarcher from stepping through the thin text slab
    float3 textP = (p - float3(0, cardElev + 0.04, 0)) / textScale;
    TextHit textHit = sdTextRelief3D(textP, textReliefDepth / textScale);
    float textDist = textHit.dist * textScale;

    if (textHit.valid) {
        if (textReliefMode < 0.5) {
            // Raised text: union above card surface
            Hit text;
            text.dist = textDist;
            text.materialId = 7;  // text material
            text.textColor = textHit.color;
            result = opUnionHit(result, text);
            if (textDist < result.dist + 0.001) {
                g_textColor = textHit.color;

            }
        } else {
            // Carved text: subtract from card
            if (textDist < 0.1) {
                result.dist = max(result.dist, -textDist);
                if (-textDist > result.dist - 0.002) {
                    result.materialId = 7;
                    g_textColor = textHit.color;
    
                }
            }
        }
    } else {
        // No specific character found, but include text AABB/slab distance
        // so the raymarcher slows down near the text region
        result.dist = min(result.dist, textDist);
    }

    return result;
}

// Geometry-only scene distance (for normals on non-text materials)
// Reuses mapScene but ignores text relief to avoid expensive text SDF in normal calc
float mapSceneGeometryDist(float3 p) {
    float result = 1e10;

    float edgeR = theme.geometryParams.x;
    float cardElev = theme.geometryParams.y;
    float itemElev = theme.geometryParams.z;
    float inputDepth = theme.geometryParams.w;

    if (!useStackedCardGeometry()) {
        if (genericCardShadowColor().w > 0.001) {
            result = min(result, sdRoundBox(
                p - genericCardShadowCenter(),
                float3(genericCardShadowHalfExtents().x, genericCardShadowHalfHeight(), genericCardShadowHalfExtents().y),
                genericCardShadowCornerRadius()
            ));
        }
        float2 halfExtents = genericCardHalfExtents();
        float genericElev = genericCardElevation();
        float genericHalfHeight = genericCardHalfHeight();
        result = min(result, sdRoundBox(
            p - float3(0, genericElev, 0),
            float3(halfExtents.x, genericHalfHeight, halfExtents.y),
            genericCardCornerRadius()
        ));
        return result;
    }

    if (isClassicTheme()) {
        result = min(result, sdPlane(p, float3(0, 1, 0), 0.5));
        result = min(result, sdRoundBox(p - float3(0, cardElev - 0.005, 0.04), float3(CARD_W - 0.02, 0.035, CARD_D), 0.0));
        result = min(result, sdRoundBox(p - float3(0, cardElev - 0.01, 0.08), float3(CARD_W - 0.04, 0.03, CARD_D), 0.0));
        result = min(result, sdRoundBox(p - float3(0, cardElev, 0), float3(CARD_W, 0.04, CARD_D), 0.0));
        return result;
    }

    // Ground plane
    result = min(result, sdPlane(p, float3(0, 1, 0), 0.5));

    // Main card
    float3 cardP = p - float3(0, cardElev, 0);
    float cardDist = sdRoundBox(cardP, float3(CARD_W, 0.04, CARD_D), edgeR);
    float inputCenterZ = (INPUT_TOP_Z + INPUT_BOT_Z) * 0.5;
    float inputHalfZ = (INPUT_BOT_Z - INPUT_TOP_Z) * 0.5;
    float3 inputP = cardP - float3(0, 0.02, inputCenterZ);
    float inputWell = sdRoundBox(inputP, float3(CARD_W - 0.04, inputDepth, inputHalfZ), edgeR * 0.5);
    cardDist = max(cardDist, -inputWell);

    // Items + checkboxes
    float itemCenters[4] = { ITEM1_Z, ITEM2_Z, ITEM3_Z, ITEM4_Z };
    for (int i = 0; i < 4; i++) {
        result = min(result, sdRoundBox(cardP - float3(0, 0, itemCenters[i]), float3(CARD_W - 0.04, 0.04 + itemElev, ITEM_HALF_H), edgeR * 0.3));
        float3 cbP = cardP - float3(CB_X, 0.04, itemCenters[i]);
        float2 torusQ = float2(length(cbP.xz) - CB_RADIUS, cbP.y);
        result = min(result, length(torusQ) - CB_TUBE_R);
    }

    // Separators
    float sepZs[3] = { SEP1_Z, SEP2_Z, SEP3_Z };
    for (int s = 0; s < 3; s++)
        result = min(result, sdRoundBox(cardP - float3(0, 0.04, sepZs[s]), float3(CARD_W - 0.06, 0.002, 0.002), 0.001));
    result = min(result, sdRoundBox(cardP - float3(0, 0.04, INPUT_BOT_Z), float3(CARD_W - 0.06, 0.002, 0.002), 0.001));
    result = min(result, sdRoundBox(cardP - float3(0, 0.04, FOOTER_Z), float3(CARD_W - 0.04, 0.002, 0.002), 0.001));

    // Stacked cards
    result = min(result, sdRoundBox(p - float3(0, cardElev - 0.005, 0.04), float3(CARD_W - 0.02, 0.035, CARD_D), edgeR));
    result = min(result, sdRoundBox(p - float3(0, cardElev - 0.01, 0.08), float3(CARD_W - 0.04, 0.03, CARD_D), edgeR));

    // Main card body
    result = min(result, cardDist);

    return result;
}

// ---- Normal calculation ----

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

float3 calcNormalGeometry(float3 p) {
    const float h = 0.0002;
    const float2 k = float2(1, -1);
    return normalize(
        k.xyy * mapSceneGeometryDist(p + k.xyy * h) +
        k.yyx * mapSceneGeometryDist(p + k.yyx * h) +
        k.yxy * mapSceneGeometryDist(p + k.yxy * h) +
        k.xxx * mapSceneGeometryDist(p + k.xxx * h)
    );
}

float calcSoftShadow(float3 ro, float3 rd, float mint, float maxt, float k) {
    float res = 1.0;
    float t = mint;
    for (int i = 0; i < 32 && t < maxt; i++) {
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

// ---- Raymarching ----

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
    // No over-relaxation: thin floating text slabs (0.06 units) get overshot with omega>1
    float omega = 1.0;
    float prevRadius = 0.0;

    for (int i = 0; i < 128 && t < 20.0; i++) {
        float3 p = ro + rd * t;
        Hit h = mapScene(p);
        float radius = abs(h.dist);

        bool sorFail = omega > 1.0 && radius < (omega - 1.0) * prevRadius;
        if (sorFail) {
            t -= (omega - 1.0) * prevRadius;
            omega = 1.0;
            prevRadius = radius;
            continue;
        }

        if (h.dist < 0.001 + t * 0.00005) {
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

// ---- Shading ----

float3 shade(float3 pos, float3 nor, float3 rd, int materialId) {
    float3 baseColor;

    if (materialId == 7) {
        // Text: use per-character color from g_textColor
        baseColor = g_textColor;
    } else if (!useStackedCardGeometry() && materialId == 6) {
        baseColor = genericCardShadowColor().xyz;
    } else if (!useStackedCardGeometry() && materialId == 2) {
        baseColor = genericCardFillColor().xyz;
    } else {
        baseColor = theme.materialColors[materialId].xyz;
    }

    // For glassmorphism, tint ground with the position-based gradient
    bool glassTheme = isGlassTheme();
    if (glassTheme && materialId == 1) {
        baseColor = lerp(baseColor, glassGradient(pos.xz), 0.90);
    }

    // Glass: blend base color with gradient BEFORE lighting so transparency is visible
    bool isGlassSurface = glassTheme && (materialId == 2 || materialId == 3 || materialId == 5 || materialId == 6);
    float3 glassBg = float3(0);
    if (isGlassSurface) {
        for (int tap = 0; tap < 3; tap++) {
            float3 jitter = hash33(pos * 31.7 + float(tap) * 17.3) * 2.0 - 1.0;
            glassBg += glassGradient(pos.xz + jitter.xz * 0.25);
        }
        glassBg /= 3.0;
        float alpha = theme.materialColors[materialId].w;
        baseColor = lerp(glassBg, baseColor, alpha);
    }

    float gloss = theme.materialProps[materialId].x;
    float metalness = theme.materialProps[materialId].y;

    float3 lightDir = normalize(worldToLegacyDirection(uniforms.lightDirIntensity.xyz));
    float lightIntensity = uniforms.lightDirIntensity.w;

    float diff = max(dot(nor, lightDir), 0.0);
    float shadowK = glassTheme ? 8.0 : 24.0;
    float shadowMaxT = glassTheme ? 5.0 : 10.0;
    float shadow = calcSoftShadow(pos + nor * 0.01, lightDir, 0.02, shadowMaxT, shadowK);
    if (glassTheme) shadow = lerp(1.0, shadow, 0.3);
    // Glass heading: moderate shadow/AO for 3D depth
    if (glassTheme && materialId == 7) shadow = lerp(1.0, shadow, 0.5);
    float ao = calcAO(pos, nor);
    if (glassTheme) ao = lerp(1.0, ao, 0.5);
    if (glassTheme && materialId == 7) ao = lerp(1.0, ao, 0.5);

    // Ambient
    float3 ambient = theme.ambientColor.xyz * theme.ambientColor.w;

    // Diffuse
    float3 lin = ambient * ao;
    lin += diff * shadow * lightIntensity * float3(1.0, 0.98, 0.95);

    // Specular (Blinn-Phong)
    float3 halfVec = normalize(lightDir - rd);
    float spec = pow(max(dot(nor, halfVec), 0.0), 8.0 + gloss * 120.0);
    float3 specColor = lerp(float3(0.04), baseColor, metalness);
    lin += spec * specColor * shadow * lightIntensity * gloss;

    // Fresnel
    float fresnel = pow(1.0 - max(dot(nor, -rd), 0.0), 5.0);
    lin += fresnel * metalness * 0.15 * specColor;

    float3 col = baseColor * lin;

    if (!useStackedCardGeometry() && materialId == 6) {
        float fillAlpha = saturate(genericCardFillColor().w);
        float alpha = saturate(genericCardShadowColor().w) * lerp(0.25, 1.0, fillAlpha * fillAlpha);
        float edgeDist = -sdRoundedBox2D(
            pos.xz,
            genericCardShadowCenter().xz,
            genericCardShadowHalfExtents(),
            genericCardShadowCornerRadius()
        );
        float2 spreadWorld = genericCardShadowSpreadWorld();
        float semanticCornerSoftness = genericCardShadowCornerRadius() * 0.18;
        float edgeSoftness = max(
            max(spreadWorld.x, spreadWorld.y),
            max(genericCardShadowHalfHeight() * 0.75, semanticCornerSoftness)
        );
        float alphaEdgeScale = lerp(0.85, 1.0, fillAlpha * fillAlpha);
        float footprintFade = smoothstep(
            -edgeSoftness * alphaEdgeScale,
            edgeSoftness * 0.65 * alphaEdgeScale,
            edgeDist
        );
        float verticalSoftness = genericCardShadowHalfHeight() * 1.75
            + max(spreadWorld.x, spreadWorld.y) * 0.25
            + semanticCornerSoftness * 0.35;
        float verticalFade = 1.0 - smoothstep(0.0, verticalSoftness, abs(pos.y - genericCardShadowCenter().y));
        return baseColor * alpha * footprintFade * verticalFade;
    }

    if (!useStackedCardGeometry() && materialId == 2) {
        float fillAlpha = saturate(genericCardFillColor().w);
        float surfaceOpacity = lerp(0.25, 1.0, fillAlpha * fillAlpha);
        if (fillAlpha < 0.999) {
            float3 baseFill = genericCardFillColor().xyz;
            float accentBackdrop = saturate(genericCardAccentColor().w);
            float detailBackdrop = saturate(genericCardDetailColor().w);
            float semanticBackdropWeight = saturate(
                max(
                    genericCardCornerRadiusPx() * 0.03,
                    max(genericCardOutlineWidthPx() * 0.12, genericCardContentInsetPx() * 0.025)
                )
            );
            float3 liftedFill = lerp(
                baseFill * float3(1.02, 1.02, 1.03),
                genericCardAccentColor().xyz,
                accentBackdrop * (0.10 + semanticBackdropWeight * 0.20)
            );
            float3 shadedFill = lerp(
                baseFill * float3(0.94, 0.95, 0.98),
                genericCardDetailColor().xyz,
                detailBackdrop * (0.10 + semanticBackdropWeight * 0.20)
            );
            float3 backdrop = lerp(
                liftedFill,
                shadedFill,
                saturate(0.42 + semanticBackdropWeight * 0.08 + (detailBackdrop - accentBackdrop) * 0.08)
            );
            float retainedAlpha = fillAlpha * fillAlpha;
            col = lerp(backdrop * lin, col, retainedAlpha);
        }

        float edgeDist = -sdRoundedBox2D(
            pos.xz,
            float2(0.0, 0.0),
            genericCardHalfExtents(),
            genericCardCornerRadius()
        );
        float outlineWidth = max(genericCardOutlineWidthPx() * layoutPixelToWorld(), 0.0);
        float outlineBand = outlineWidth > 0.0
            ? 1.0 - smoothstep(0.0, outlineWidth, edgeDist)
            : 0.0;
        col = lerp(
            col,
            genericCardOutlineColor().xyz * lin,
            outlineBand * genericCardOutlineColor().w * surfaceOpacity
        );

        float semanticCornerBand = genericCardCornerRadius() * 0.22;
        float semanticDepthBand = genericCardHalfHeight() * 0.28;
        float bevelWidth = max(outlineWidth * 1.35, max(semanticCornerBand, semanticDepthBand));
        float bevelBand = 1.0 - smoothstep(0.0, bevelWidth, edgeDist);
        float semanticOutlineThreshold = max(outlineWidth, genericCardCornerRadius() * 0.06);
        float outlineInfluence = saturate(genericCardOutlineColor().w)
            * smoothstep(0.0, semanticOutlineThreshold, outlineWidth);
        float bevelPresence = lerp(0.30, 1.0, outlineInfluence);
        float semanticDepthScalePx = max(
            genericCardContentInsetPx() * 0.45,
            max(genericCardCornerRadiusPx() * 1.1, genericCardOutlineWidthPx() * 6.0)
        );
        float bevelStrength = saturate(genericCardDepthPx() / max(semanticDepthScalePx, 1.0))
            * surfaceOpacity
            * bevelPresence;
        float3 rimTint = lerp(genericCardFillColor().xyz, genericCardOutlineColor().xyz, 0.28 * outlineInfluence);

        float cardMinY = genericCardElevation() - genericCardHalfHeight();
        float cardMaxY = genericCardElevation() + genericCardHalfHeight();
        float localHeight = saturate((pos.y - cardMinY) / max(cardMaxY - cardMinY, 1e-4));
        float depthInfluence = saturate(genericCardDepthPx() / max(semanticDepthScalePx, 1.0));
        float semanticEdgeWeight = lerp(0.30, 1.0, outlineInfluence) * lerp(0.7, 1.0, depthInfluence);
        col += rimTint * (bevelBand * bevelStrength * 0.08 * semanticEdgeWeight);
        float topLight = lerp(0.94, 1.0 + 0.08 * depthInfluence, localHeight);
        float sideShade = lerp(1.0, 1.0 - 0.08 * depthInfluence, bevelBand * bevelStrength);
        col *= topLight * sideShade;

        float semanticAlphaWeight = fillAlpha * fillAlpha;

        float semanticVariationScale = saturate(
            max(
                genericCardCornerRadiusPx() * 0.04,
                max(genericCardOutlineWidthPx() * 0.18, genericCardContentInsetPx() * 0.04)
            )
        );
        float accentStrength = saturate(genericCardAccentColor().w)
            * localHeight
            * (0.02 + semanticVariationScale * 0.12 + depthInfluence * 0.04)
            * surfaceOpacity
            * semanticAlphaWeight;
        col = lerp(col, genericCardAccentColor().xyz * lin, accentStrength);

        float detailStrength = saturate(genericCardDetailColor().w)
            * (
                (0.015 + semanticVariationScale * 0.08 + depthInfluence * 0.025)
                + bevelBand * bevelStrength * (0.035 + semanticVariationScale * 0.10 + depthInfluence * 0.04)
            )
            * (1.0 - localHeight * 0.35)
            * surfaceOpacity
            * semanticAlphaWeight;
        col = lerp(col, genericCardDetailColor().xyz * lin, detailStrength);
    }

    // Glass heading: glass pipe material with text color tint
    if (glassTheme && materialId == 7) {
        float3 glassBgText = float3(0);
        for (int tap = 0; tap < 3; tap++) {
            float3 jitter = hash33(pos * 31.7 + float(tap) * 17.3) * 2.0 - 1.0;
            glassBgText += glassGradient(pos.xz + jitter.xz * 0.25);
        }
        glassBgText /= 3.0;
        float3 tintedBase = lerp(glassBgText, baseColor, 0.35);
        col = tintedBase * lin;
        // Blue glass tint
        col *= float3(0.85, 0.90, 1.10);
        // Fresnel rim highlight
        float NdotV_text = max(dot(nor, -rd), 0.0);
        float textFresnel = 0.04 + 0.96 * pow(1.0 - NdotV_text, 5.0);
        col += textFresnel * float3(0.30, 0.40, 0.80) * 0.8;
    }

    // Glass: blue tint + fresnel rim + border glow
    if (isGlassSurface) {
        float2 cardHalfExtents = useStackedCardGeometry() ? float2(CARD_W, CARD_D) : genericCardHalfExtents();
        // Blue glass tint — stronger blue cast matching reference
        col *= float3(0.85, 0.90, 1.10);

        // Normal-based fresnel rim (for physical rounded edges)
        float NdotV = max(dot(nor, -rd), 0.0);
        float fresnelSchlick = 0.04 + 0.96 * pow(1.0 - NdotV, 5.0);
        float edgeRim = 1.0 - abs(dot(nor, float3(0, 1, 0)));
        float rimFactor = max(fresnelSchlick, edgeRim * 0.8);
        col += rimFactor * float3(0.40, 0.50, 0.90) * 1.5;
        col *= lerp(1.0, 0.80, edgeRim * 0.5);

        // Card border: wide blue glow + sharp inner rim line
        float2 cardInset = cardHalfExtents - abs(pos.xz);
        float edgeDist = min(cardInset.x, cardInset.y);
        // Wide outer glow
        float outerGlow = 1.0 - smoothstep(0.0, 0.80, edgeDist);
        col.xy *= lerp(float2(1.0), float2(0.50), outerGlow);
        col.z += outerGlow * 0.40;
        // Sharp thin rim line (the defined blue border visible in reference)
        float rimLine = smoothstep(0.02, 0.04, edgeDist) * (1.0 - smoothstep(0.04, 0.10, edgeDist));
        col += rimLine * float3(0.15, 0.22, 0.55);
        // Very edge highlight
        float edgeHighlight = 1.0 - smoothstep(0.0, 0.04, edgeDist);
        col += edgeHighlight * float3(0.10, 0.15, 0.35);

        // Item panel borders (glass panels with thin blue rim + glow)
        if (useStackedCardGeometry() && materialId == 3) {
            float itemCenters[4] = { ITEM1_Z, ITEM2_Z, ITEM3_Z, ITEM4_Z };
            float itemHW = CARD_W - 0.04;
            float minItemEdge = 1e10;
            for (int i = 0; i < 4; i++) {
                float2 itemLocal = float2(abs(pos.x), abs(pos.z - itemCenters[i]));
                float2 itemInset = float2(itemHW, ITEM_HALF_H) - itemLocal;
                minItemEdge = min(minItemEdge, min(itemInset.x, itemInset.y));
            }
            // Soft glow near edges
            float itemGlow = 1.0 - smoothstep(0.0, 0.20, minItemEdge);
            col.xy *= lerp(float2(1.0), float2(0.65), itemGlow);
            col.z += itemGlow * 0.20;
            // Sharp thin border line
            float itemRim = smoothstep(0.01, 0.03, minItemEdge) * (1.0 - smoothstep(0.03, 0.07, minItemEdge));
            col += itemRim * float3(0.08, 0.12, 0.30);
        }

        // Input field glass panel border
        if (useStackedCardGeometry() && materialId == 2) {
            float inputCenterZ = (INPUT_TOP_Z + INPUT_BOT_Z) * 0.5;
            float inputHalfZ = (INPUT_BOT_Z - INPUT_TOP_Z) * 0.5;
            float2 inputLocal = float2(abs(pos.x), abs(pos.z - inputCenterZ));
            float2 inputInset = float2(CARD_W - 0.04, inputHalfZ) - inputLocal;
            float inputEdge = min(inputInset.x, inputInset.y);
            if (inputEdge > -0.1 && inputEdge < 0.25) {
                // Soft glow
                float inputGlow = 1.0 - smoothstep(0.0, 0.20, max(inputEdge, 0.0));
                col.xy *= lerp(float2(1.0), float2(0.65), inputGlow);
                col.z += inputGlow * 0.20;
                // Sharp thin border line
                float inputRim = smoothstep(0.01, 0.03, max(inputEdge, 0.0)) * (1.0 - smoothstep(0.03, 0.07, max(inputEdge, 0.0)));
                col += inputRim * float3(0.08, 0.12, 0.30);
            }
        }
    }

    // Glass theme: darken ground near card edges (drop shadow)
    if (glassTheme && materialId == 1) {
        float2 cardDist2 = abs(pos.xz) - (useStackedCardGeometry() ? float2(CARD_W, CARD_D) : genericCardHalfExtents());
        float outsideDist = length(max(cardDist2, 0.0));
        float cardProximity = 1.0 - smoothstep(0.0, 0.8, outsideDist);
        col *= lerp(1.0, 0.75, cardProximity);
    }

    return col;
}

// Position-based gradient for glassmorphism (used for glass background AND sky)
float3 glassGradient(float2 worldXZ) {
    // Normalize to card-centered UV roughly [-1,1]
    float2 uv = worldXZ * float2(1.5, 0.5);
    bool darkMode = theme.extraParams.w > 0.5;
    float3 base;
    if (darkMode) {
        base = float3(0.04, 0.06, 0.14);
        // Blue-purple upper-left
        base += float3(-0.02, 0.01, 0.16) * exp(-dot(uv - float2(-1.2, 0.8), uv - float2(-1.2, 0.8)) * 0.5);
        // Pink/warm right
        base += float3(0.14, 0.05, -0.03) * exp(-dot(uv - float2(1.2, 0.0), uv - float2(1.2, 0.0)) * 0.5);
        // Warm amber lower-right
        base += float3(0.12, 0.07, -0.05) * exp(-dot(uv - float2(0.5, -1.0), uv - float2(0.5, -1.0)) * 0.6);
    } else {
        base = float3(0.78, 0.80, 0.88);
        // Blue-purple blob — upper-left quadrant
        base += float3(-0.18, -0.04, 0.30) * exp(-dot(uv - float2(-1.0, 0.6), uv - float2(-1.0, 0.6)) * 0.35);
        // Pink/warm blob — right side
        base += float3(0.22, 0.06, -0.14) * exp(-dot(uv - float2(1.3, 0.3), uv - float2(1.3, 0.3)) * 0.35);
        // Warm peach blob — lower area
        base += float3(0.20, 0.12, -0.08) * exp(-dot(uv - float2(0.4, -1.0), uv - float2(0.4, -1.0)) * 0.45);
        // Cool blue accent — top-center
        base += float3(-0.10, 0.02, 0.22) * exp(-dot(uv - float2(0.0, 1.2), uv - float2(0.0, 1.2)) * 0.35);
    }
    return saturate(base);
}

float3 getSkyColor(float3 rd) {
    bool isGlass = isGlassTheme();
    if (isGlass) {
        // Project ray direction to XZ world plane for sky gradient
        float2 skyXZ = rd.xz * 3.0 / (abs(rd.y) + 0.3);
        return glassGradient(skyXZ);
    }

    float t = 0.5 * (rd.y + 1.0);
    float3 skyLow = theme.materialColors[0].xyz;
    float3 skyHigh = lerp(skyLow, float3(0.6, 0.7, 0.85), 0.3);
    return lerp(skyLow, skyHigh, t);
}

// ---- Decal text helpers (glass theme) ----

float4 blendOver(float4 bg, float4 fg) {
    float outA = fg.a + bg.a * (1.0 - fg.a);
    if (outA < 0.001) return float4(0);
    float3 outRGB = (fg.rgb * fg.a + bg.rgb * bg.a * (1.0 - fg.a)) / outA;
    return float4(outRGB, outA);
}

float3 srgbToLinear(float3 c) {
    return float3(
        c.x <= 0.04045 ? c.x / 12.92 : pow((c.x + 0.055) / 1.055, 2.4),
        c.y <= 0.04045 ? c.y / 12.92 : pow((c.y + 0.055) / 1.055, 2.4),
        c.z <= 0.04045 ? c.z / 12.92 : pow((c.z + 0.055) / 1.055, 2.4));
}

float4 evaluateTextDecal(float2 textPos, float aaScale, bool skipHeading) {
    float4 result = float4(0.0);
    int2 gridDims = int2(uniforms.charGridParams.xy);
    int2 centerCell = charGridCellCoord(textPos);

    for (int dy = -1; dy <= 1; dy++) {
        for (int dx = -1; dx <= 1; dx++) {
            int2 cell = centerCell + int2(dx, dy);
            if (cell.x < 0 || cell.y < 0 || cell.x >= gridDims.x || cell.y >= gridDims.y)
                continue;
            uint cellIdx = charGridCellIndex(cell);
            CharGridCellData cgCell = charGridCells[cellIdx];

            for (uint i = 0u; i < cgCell.count && i < 16u; i++) {
                uint charIdx = charGridIndices[cgCell.offset + i];
                CharInstanceEx ch = charInstances[charIdx];

                if (skipHeading && ch.colorFlags.w == 2.0) continue;

                float2 charPos = ch.posAndChar.xy;
                float charScale = ch.posAndChar.z;
                uint glyphIdx = uint(ch.posAndChar.w);
                GlyphData glyph = glyphData[glyphIdx];
                float4 bounds = glyph.bounds;
                float2 localP = (textPos - charPos) / charScale;

                if (localP.x >= bounds.x - 0.15 && localP.x <= bounds.z + 0.15 &&
                    localP.y >= bounds.y - 0.15 && localP.y <= bounds.w + 0.15) {
                    float d2d = sdGlyphBruteForce(localP, glyph.curveInfo) * charScale;
                    float alpha = smoothstep(aaScale, -aaScale, d2d);
                    if (alpha > 0.001) {
                        result = blendOver(result, float4(ch.colorFlags.xyz, alpha));
                    }
                }
            }
        }
    }
    return result;
}

float4 renderUiPrimitives2D(float2 p, uint startIndex, uint primCount) {
    float4 result = float4(0.0);

    for (uint i = startIndex; i < primCount && i < 64u; i++) {
        UiPrimitive prim = uiPrimitives[i];
        float primType = prim.params.w;
        float4 col = prim.color;

        float d = 1e10;
        float alpha = 0.0;

        if (primType < 0.5) {
            float2 center = prim.posSize.xy + prim.posSize.zw * 0.5;
            float2 halfSize = prim.posSize.zw * 0.5;
            float radius = prim.params.x;
            d = sdRoundedBox2D(p, center, halfSize, radius);
            alpha = smoothstep(0.5, -0.5, d);
        } else if (primType < 1.5) {
            float2 center = prim.posSize.xy + prim.posSize.zw * 0.5;
            float2 halfSize = prim.posSize.zw * 0.5;
            float radius = prim.params.x;
            float borderW = prim.params.y;
            d = sdRoundedBox2D(p, center, halfSize, radius);
            alpha = smoothstep(0.5, -0.5, abs(d) - borderW * 0.5);
        } else if (primType < 2.5) {
            float2 center = prim.posSize.xy;
            float radius = prim.posSize.z;
            d = sdCircle2D(p, center, radius);
            alpha = smoothstep(0.5, -0.5, d);
        } else if (primType < 3.5) {
            float2 center = prim.posSize.xy;
            float radius = prim.posSize.z;
            float borderW = prim.params.y;
            d = sdCircle2D(p, center, radius);
            alpha = smoothstep(0.5, -0.5, abs(d) - borderW * 0.5);
        } else if (primType < 4.5) {
            float2 a = prim.posSize.xy;
            float2 b = prim.posSize.zw;
            float lineW = prim.params.y;
            d = sdLineSegment2D(p, a, b) - lineW * 0.5;
            alpha = smoothstep(0.5, -0.5, d);
        } else if (primType < 5.5) {
            float2 center = prim.posSize.xy + prim.posSize.zw * 0.5;
            float2 halfSize = prim.posSize.zw * 0.5;
            float2 shadowOffset = prim.extra.xy;
            float spread = prim.extra.z;
            float blur = prim.params.z;
            float radius = prim.params.x;
            float2 shadowCenter = center + shadowOffset;
            float2 shadowHalfSize = halfSize + spread;
            d = sdRoundedBox2D(p, shadowCenter, shadowHalfSize, radius);
            alpha = col.a * smoothstep(blur, -blur * 0.1, d);
            col.a = 1.0;
        } else if (primType < 6.5) {
            float2 a = prim.posSize.xy;
            float2 b = prim.posSize.zw;
            float2 c = prim.extra.xy;
            float lineW = prim.params.y;
            float d1 = sdLineSegment2D(p, a, b) - lineW * 0.5;
            float d2 = sdLineSegment2D(p, b, c) - lineW * 0.5;
            d = min(d1, d2);
            alpha = smoothstep(0.5, -0.5, d);
        }

        result = blendOver(result, float4(col.rgb, col.a * alpha));
    }

    return result;
}

float4 renderClassicCardDecal(float2 textPos, float aaScale) {
    float4 color = float4(0.0);
    uint primCount = uint(uniforms.textParams.y);
    uint overlayStart = uint(uniforms.textParams.w);
    color = blendOver(color, renderUiPrimitives2D(textPos, overlayStart, primCount));
    color = blendOver(color, evaluateTextDecal(textPos, aaScale, false));
    return color;
}

float4 renderGenericCardDecal(float2 textPos, float aaScale) {
    float4 bounds = uniforms.layoutBounds;
    float2 center = float2((bounds.x + bounds.z) * 0.5, (bounds.y + bounds.w) * 0.5);
    float2 halfSize = float2((bounds.z - bounds.x) * 0.5, (bounds.w - bounds.y) * 0.5);
    float radius = genericCardCornerRadiusPx();
    float outlinePad = genericCardOutlineWidthPx() * 0.85;
    float depthPad = genericCardDepthPx() * 0.04;
    float contentInsetPad = genericCardContentInsetPx() * 0.5;
    float semanticPad = max(radius * 0.06, max(outlinePad * 0.5 + depthPad, contentInsetPad));
    float clipPad = max(outlinePad, semanticPad);
    float2 clipHalfSize = max(halfSize - clipPad, float2(0.0, 0.0));
    float clipRadius = max(radius - clipPad, 0.0);
    float clipDist = sdRoundedBox2D(textPos, center, clipHalfSize, clipRadius);
    float clipFade = max(
        aaScale * 1.5,
        max(
            radius * 0.02,
            max(outlinePad * 0.35 + depthPad * 0.5, contentInsetPad * 0.12)
        )
    );
    if (clipDist > clipFade) {
        return float4(0.0);
    }
    float clipAlpha = 1.0 - smoothstep(0.0, clipFade, max(clipDist, 0.0));

    float4 color = float4(0.0);
    uint primCount = uint(uniforms.textParams.y);
    color = blendOver(color, renderUiPrimitives2D(textPos, 0u, primCount));
    color = blendOver(color, evaluateTextDecal(textPos, aaScale, false));
    color.a *= clipAlpha;
    return color;
}

// ---- Main render ----

float3 render(float3 ro, float3 rd) {
    // Reset text color globals
    g_textColor = float3(0.5);

    bool isGlass = isGlassTheme();
    bool isClassic = isClassicTheme();
    RayHit hit = raymarch(ro, rd);

    float3 col;
    if (!hit.hit) {
        col = getSkyColor(rd);
    } else {
        float3 pos = ro + rd * hit.t;

        // Material-aware normal calculation
        float3 nor;
        if (hit.materialId == 1) {
            nor = float3(0, 1, 0); // ground is always flat
        } else if (hit.materialId == 7) {
            nor = calcNormal(pos);
            mapScene(pos); // recapture text color after calcNormal overwrites globals
        } else {
            nor = calcNormalGeometry(pos);
        }

        col = shade(pos, nor, rd, hit.materialId);

        // Fog
        float fog = 1.0 - exp(-hit.t * 0.003);
        col = lerp(col, getSkyColor(rd) * 0.9, fog);
    }

    // Card-plane decals for themes that project 2D UI onto the card surface.
    if (!useStackedCardGeometry() || isGlass || isClassic) {
        float genericTextPlaneLift = max(
            max(
                genericCardDepthPx() * layoutPixelToWorld() * 0.035,
                genericCardOutlineWidthPx() * layoutPixelToWorld() * 0.18
            ),
            genericCardCornerRadius() * 0.02
        );
        float textPlaneY = !useStackedCardGeometry()
            ? genericCardElevation() + genericCardHalfHeight() + genericTextPlaneLift
            : theme.geometryParams.y + 0.04;

        if (abs(rd.y) > 0.001) {
            float tPlane = (textPlaneY - ro.y) / rd.y;

            // Visible: in front of camera AND in front of (or near) scene hit
            if (tPlane > 0.0 && (!hit.hit || tPlane < hit.t + 0.1)) {
                float3 planePos = ro + rd * tPlane;
                float2 textPos2D = float2(
                    planePos.x / layoutPixelToWorld() + layoutCenterX(),
                    -planePos.z / layoutPixelToWorld() + layoutCenterY()
                );

                // AA scale from pixel footprint at plane distance
                float pixelWorldSize = tPlane * 2.0 / uniforms.renderParams.y;
                float aaScale = pixelWorldSize / layoutPixelToWorld();

                float4 decal = !useStackedCardGeometry()
                    ? renderGenericCardDecal(textPos2D, aaScale)
                    : (isGlass
                        ? evaluateTextDecal(textPos2D, aaScale, true)
                        : renderClassicCardDecal(textPos2D, aaScale));
                if (decal.a > 0.01) {
                    float decalOpacity = 1.0;
                    if (!useStackedCardGeometry()) {
                        float fillAlpha = saturate(genericCardFillColor().w);
                        float outlinePresence = saturate(genericCardOutlineColor().w) *
                            saturate(genericCardOutlineWidthPx() / 3.0);
                        float semanticOutlineLift = saturate(
                            genericCardOutlineWidthPx() * 0.035 + genericCardCornerRadiusPx() * 0.003
                        );
                        decalOpacity = lerp(0.70, 1.0, fillAlpha * fillAlpha);
                        decalOpacity = lerp(
                            decalOpacity,
                            min(decalOpacity + semanticOutlineLift, 1.0),
                            outlinePresence
                        );
                    }
                    // Convert sRGB text color to linear for correct blending
                    float3 textLinear = srgbToLinear(decal.rgb);
                    col = lerp(col, textLinear, decal.a * decalOpacity);
                }
            }
        }
    }

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

    float3 ro = worldToLegacyScene(uniforms.cameraPosTime.xyz);
    float3 rd = normalize(worldToLegacyDirection(farPoint.xyz - nearPoint.xyz));

    float3 col = render(ro, rd);
    col = pow(col, float3(1.0 / 2.2));

    return float4(col, 1.0);
}
