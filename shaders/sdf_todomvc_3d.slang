// SDF 3D TodoMVC with Vector SDF Text Relief and Theme System
// Based on sdf_clay_vector.slang with theme uniforms and per-character color

struct Uniforms {
    float4x4 invViewProj;
    float4 cameraPosTime;     // xyz = cameraPos, w = time
    float4 lightDirIntensity; // xyz = lightDir, w = intensity
    float4 renderParams;      // xy = resolution, z = textDepth, w = textScale
    float4 textParams;        // x = charCount, y = numLines, z = lineHeight, w = reserved
    float4 charGridParams;    // xy = gridDims, zw = cellSize
    float4 charGridBounds;    // xy = gridMin, zw = gridMax
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
    float4 posAndChar;   // xy = position, z = scale, w = charIndex
    float4 colorFlags;   // rgb = color, w = flags (0=active, 1=completed, 2=heading, 3=placeholder)
};

// Character grid cell
struct CharGridCellData {
    uint offset;
    uint count;
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
    for (uint i = 0u; i < glyphCurveCount && i < 256u; i++) {
        BezierCurve curve = curves[glyphCurveOffset + i];
        float2 p0 = curve.points01.xy;
        float2 p1 = curve.points01.zw;
        float2 p2 = curve.points2bbox.xy;

        float dist = sdQuadraticBezier(localPos, p0, p1, p2);
        minDist = min(minDist, dist);

        winding += curveWindingContribution(localPos, p0, p1, p2);
    }

    float sign_val = (winding != 0) ? -1.0 : 1.0;
    return sign_val * minDist;
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
static const float CARD_W = 2.75;  // half-width
static const float CARD_D = 1.72;  // half-depth (Z)
static const float CARD_Y = 0.0;   // card center Y

// ---- Text relief with per-character color ----

struct TextHit {
    float dist;
    float3 color;
    float flags;
    bool valid;
};

TextHit sdTextRelief3D(float3 p, float depth) {
    TextHit result;
    result.dist = 1e10;
    result.color = float3(0.5);
    result.flags = 0.0;
    result.valid = false;

    float halfDepth = depth * 0.5;

    // Text AABB early-exit
    float2 gridMin = uniforms.charGridBounds.xy;
    float2 gridMax = uniforms.charGridBounds.zw;
    float3 textBoxMin = float3(gridMin.x, -halfDepth, -gridMax.y);
    float3 textBoxMax = float3(gridMax.x, halfDepth, -gridMin.y);
    float3 textBoxCenter = (textBoxMin + textBoxMax) * 0.5;
    float3 textBoxHalf = (textBoxMax - textBoxMin) * 0.5;
    float textBoxDist = sdBox(p - textBoxCenter, textBoxHalf);
    if (textBoxDist > 0.3) {
        result.dist = textBoxDist;
        return result;
    }

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
        CharInstanceEx ch = charInstances[charIdx];
        float2 charPos = ch.posAndChar.xy;
        float charScale = ch.posAndChar.z;
        uint glyphIdx = uint(ch.posAndChar.w);

        GlyphData glyph = glyphData[glyphIdx];
        float4 bounds = glyph.bounds;

        float2 localP = (textPos - charPos) / charScale;

        if (localP.x >= bounds.x - 0.15 && localP.x <= bounds.z + 0.15 &&
            localP.y >= bounds.y - 0.15 && localP.y <= bounds.w + 0.15) {
            float d2d = sdGlyph(localP, bounds, glyph.gridInfo, glyph.curveInfo) * charScale;
            float d3d = opExtrusion(d2d, p.y, halfDepth);
            if (d3d < result.dist) {
                result.dist = d3d;
                result.color = ch.colorFlags.xyz;
                result.flags = ch.colorFlags.w;
                result.valid = true;
            }
        }
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
static float g_textFlags = 0.0;

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
    // Input area occupies top ~19% of card in Z
    float inputWellZ = CARD_D - 0.33;  // input well center Z offset from card center
    float3 inputP = cardP - float3(0, 0.02, -inputWellZ);
    float inputWell = sdRoundBox(inputP, float3(CARD_W - 0.04, inputDepth, 0.30), edgeR * 0.5);
    cardDist = max(cardDist, -inputWell);  // CSG subtract

    // Todo item raised panels (4 items)
    float itemStartZ = CARD_D - 0.93;  // start of first item
    float itemH = 0.28;  // height per item
    float itemGap = 0.02;

    for (int i = 0; i < 4; i++) {
        float iz = itemStartZ - float(i) * (itemH + itemGap);
        float3 itemP = cardP - float3(0, 0.0, -iz);
        float itemDist = sdRoundBox(itemP, float3(CARD_W - 0.04, 0.04 + itemElev, itemH * 0.5), edgeR * 0.3);
        // Union with main card
        Hit item;
        item.dist = itemDist;
        item.materialId = 3;  // todo item material
        item.textColor = float3(0);
        result = opUnionHit(result, item);

        // Checkbox recess (CSG subtract a circle from item)
        float3 cbP = cardP - float3(-CARD_W + 0.35, 0.0, -iz);
        float cbDist = sdRoundBox(cbP, float3(0.09, 0.06, 0.09), 0.08);
        // Only apply if inside the item area
        if (cbDist < 0.2) {
            // Checkbox border ring (material 4)
            float cbRing = abs(cbDist) - 0.008;
            Hit cbHit;
            cbHit.dist = cbRing;
            cbHit.materialId = 4;  // checkbox ring
            cbHit.textColor = float3(0);
            result = opUnionHit(result, cbHit);
        }
    }

    // Separator lines between items (thin boxes)
    for (int s = 0; s < 3; s++) {
        float sz = itemStartZ - float(s + 1) * (itemH + itemGap) + itemGap * 0.5 + itemH * 0.5;
        float3 sepP = cardP - float3(0, 0.04, -sz);
        float sepDist = sdRoundBox(sepP, float3(CARD_W - 0.06, 0.002, 0.002), 0.001);
        Hit sep;
        sep.dist = sepDist;
        sep.materialId = 5;  // separator
        sep.textColor = float3(0);
        result = opUnionHit(result, sep);
    }

    // Footer area (bottom of card)
    float footerZ = -CARD_D + 0.25;
    float3 footerSepP = cardP - float3(0, 0.04, -footerZ - 0.12);
    float footerSepDist = sdRoundBox(footerSepP, float3(CARD_W - 0.04, 0.002, 0.002), 0.001);
    Hit footerSep;
    footerSep.dist = footerSepDist;
    footerSep.materialId = 5;
    footerSep.textColor = float3(0);
    result = opUnionHit(result, footerSep);

    // Stacked decorative cards behind main card
    Hit stack1;
    float3 stack1P = p - float3(0, cardElev - 0.005, -0.02);
    stack1.dist = sdRoundBox(stack1P, float3(CARD_W - 0.02, 0.035, CARD_D), edgeR);
    stack1.materialId = 6;  // stack card
    stack1.textColor = float3(0);
    result = opUnionHit(result, stack1);

    Hit stack2;
    float3 stack2P = p - float3(0, cardElev - 0.01, -0.04);
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

    // Text relief
    float3 textP = (p - float3(0, cardElev + 0.04, 0)) / textScale;
    TextHit textHit = sdTextRelief3D(textP, textReliefDepth / textScale);

    if (textHit.valid) {
        float textDist = textHit.dist * textScale;
        if (textReliefMode < 0.5) {
            // Raised text: union above card surface
            Hit text;
            text.dist = textDist;
            text.materialId = 7;  // text material
            text.textColor = textHit.color;
            result = opUnionHit(result, text);
            if (textDist < result.dist + 0.001) {
                g_textColor = textHit.color;
                g_textFlags = textHit.flags;
            }
        } else {
            // Carved text: subtract from card
            if (textDist < 0.1) {
                result.dist = max(result.dist, -textDist);
                if (-textDist > result.dist - 0.002) {
                    result.materialId = 7;
                    g_textColor = textHit.color;
                    g_textFlags = textHit.flags;
                }
            }
        }
    }

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
    float omega = 1.6;
    float prevRadius = 0.0;

    for (int i = 0; i < 128 && t < 50.0; i++) {
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

// ---- Shading ----

float3 shade(float3 pos, float3 nor, float3 rd, int materialId) {
    float3 baseColor;

    if (materialId == 7) {
        // Text: use per-character color from g_textColor
        baseColor = g_textColor;
    } else {
        baseColor = theme.materialColors[materialId].xyz;
    }

    float gloss = theme.materialProps[materialId].x;
    float metalness = theme.materialProps[materialId].y;

    float3 lightDir = normalize(uniforms.lightDirIntensity.xyz);
    float lightIntensity = uniforms.lightDirIntensity.w;

    float diff = max(dot(nor, lightDir), 0.0);
    float shadow = calcSoftShadow(pos + nor * 0.01, lightDir, 0.02, 10.0, 24.0);
    float ao = calcAO(pos, nor);

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

    return col;
}

float3 getSkyColor(float3 rd) {
    float t = 0.5 * (rd.y + 1.0);
    float3 skyLow = theme.materialColors[0].xyz;  // materialColors[0] = sky/background
    float3 skyHigh = lerp(skyLow, float3(0.6, 0.7, 0.85), 0.3);
    return lerp(skyLow, skyHigh, t);
}

float3 render(float3 ro, float3 rd) {
    // Reset text color globals
    g_textColor = float3(0.5);
    g_textFlags = 0.0;

    RayHit hit = raymarch(ro, rd);
    if (!hit.hit) return getSkyColor(rd);

    float3 pos = ro + rd * hit.t;

    // Re-evaluate mapScene at hit point to capture text color
    mapScene(pos);

    float3 nor = calcNormal(pos);
    float3 col = shade(pos, nor, rd, hit.materialId);

    // Fog
    float fog = 1.0 - exp(-hit.t * 0.003);
    col = lerp(col, getSkyColor(rd) * 0.9, fog);

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
