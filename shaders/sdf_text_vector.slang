// Vector SDF Text Rendering Shader
// Computes exact signed distance to Bézier curves using grid acceleration
//
// Uses instanced quad rendering where per-instance data is passed via vertex attributes

struct Uniforms {
    float2 screenSize;      // Screen dimensions in pixels
    float2 sdfParams;       // x = grid_resolution, y = reserved
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

[[vk::binding(0, 0)]]
ConstantBuffer<Uniforms> uniforms;

[[vk::binding(1, 0)]]
StructuredBuffer<GridCell> gridCells;

[[vk::binding(2, 0)]]
StructuredBuffer<uint> curveIndices;

[[vk::binding(3, 0)]]
StructuredBuffer<BezierCurve> curves;

struct VertexOutput {
    float4 position : SV_Position;
    float2 glyphUV : TEXCOORD0;
    nointerpolation float4 glyphBounds : TEXCOORD1;
    nointerpolation uint4 gridInfo : TEXCOORD2;  // gridOffset, gridSizeX, gridSizeY, curveCount
    nointerpolation float4 color : COLOR;
};

// Vertex input from vertex buffer + instance attributes
struct VertexInput {
    // Per-vertex
    float2 position : POSITION;
    float2 uv : TEXCOORD0;
    // Per-instance (via instance buffer as vertex attributes)
    float2 offset : TEXCOORD1;
    float2 size : TEXCOORD2;
    float4 glyphBounds : TEXCOORD3;
    uint4 gridInfo : TEXCOORD4;  // gridOffset, gridSizeX, gridSizeY, curveCount
    float4 color : TEXCOORD5;
};

[shader("vertex")]
VertexOutput vs_main(VertexInput input) {
    // Transform unit quad to screen position
    float2 screenPos = input.offset + input.position * input.size;

    // Convert to NDC
    float2 ndc = (screenPos / uniforms.screenSize) * 2.0 - 1.0;
    ndc.y = -ndc.y; // Flip Y for screen coordinates

    VertexOutput output;
    output.position = float4(ndc, 0.0, 1.0);
    output.glyphUV = input.uv;
    output.glyphBounds = input.glyphBounds;
    output.gridInfo = input.gridInfo;
    output.color = input.color;

    return output;
}

// Compute exact distance to quadratic Bézier (canonical parabola method)
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

    float a = p0.y - 2.0 * p1.y + p2.y;
    float b = 2.0 * (p1.y - p0.y);
    float c = p0.y - py;

    int crossings = 0;

    if (abs(a) < 1e-10) {
        if (abs(b) > 1e-10) {
            float t = -c / b;
            if (t >= 0.0 && t < 1.0) {
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

// Compute SDF for a glyph at local UV coordinates
float sdGlyph(float2 localUV, float4 glyphBounds, uint4 gridInfo) {
    uint gridOffset = gridInfo.x;
    uint gridSizeX = gridInfo.y;
    uint gridSizeY = gridInfo.z;

    // Map UV [0,1] to glyph bounds in em units
    float2 localPos = float2(
        lerp(glyphBounds.x, glyphBounds.z, localUV.x),
        lerp(glyphBounds.w, glyphBounds.y, localUV.y) // Flip Y
    );

    // Determine grid cell
    float2 gridUV = (localPos - glyphBounds.xy) / (glyphBounds.zw - glyphBounds.xy);
    gridUV = clamp(gridUV, 0.0, 0.9999);

    int cellX = int(gridUV.x * float(gridSizeX));
    int cellY = int(gridUV.y * float(gridSizeY));
    uint cellIdx = gridOffset + cellY * gridSizeX + cellX;

    GridCell cell = gridCells[cellIdx];
    uint curveStart = cell.curveStartAndCount & 0xFFFF;
    uint curveCount = (cell.curveStartAndCount >> 16) & 0xFF;
    uint flags = (cell.curveStartAndCount >> 24) & 0xFF;

    // Early out for uniform cells
    if ((flags & 0x01) != 0) return -0.05; // Inside
    if ((flags & 0x02) != 0) return 0.05;  // Outside

    // Compute distance to curves in this cell
    float minDist = 1e10;
    int winding = 0;

    for (uint i = 0; i < curveCount && i < 16; i++) {
        uint globalCurveIdx = curveIndices[curveStart + i];
        BezierCurve curve = curves[globalCurveIdx];

        float2 p0 = curve.points01.xy;
        float2 p1 = curve.points01.zw;
        float2 p2 = curve.points2bbox.xy;
        float4 bbox = float4(curve.points2bbox.zw, curve.bboxFlags.xy);

        // Bounding box early rejection
        float2 d = max(bbox.xy - localPos, localPos - bbox.zw);
        float bboxDist = length(max(d, 0.0));
        if (bboxDist > minDist) continue;

        float dist = sdQuadraticBezier(localPos, p0, p1, p2);
        minDist = min(minDist, dist);

        winding += curveWindingContribution(localPos, p0, p1, p2);
    }

    // Sign from winding number
    float sign = (winding != 0) ? -1.0 : 1.0;
    return sign * minDist;
}

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    // Compute SDF value
    float d = sdGlyph(input.glyphUV, input.glyphBounds, input.gridInfo);

    // Scale distance for anti-aliasing
    float2 dUV = fwidth(input.glyphUV);
    float glyphEmWidth = input.glyphBounds.z - input.glyphBounds.x;
    float glyphEmHeight = input.glyphBounds.w - input.glyphBounds.y;
    float pixelSize = max(dUV.x * glyphEmWidth, dUV.y * glyphEmHeight);

    // Anti-aliased edge
    float alpha = smoothstep(pixelSize, -pixelSize, d);

    // Output with premultiplied alpha
    return float4(input.color.rgb * alpha, alpha) * input.color.a;
}
