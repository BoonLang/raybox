// Exact Signed Distance Field functions for quadratic Bézier curves
// Uses canonical parabola transform for efficient distance computation

// Bézier curve structure matching Rust BezierCurve
struct BezierCurve {
    float4 points01;    // p0.xy, p1.xy
    float4 points2bbox; // p2.xy, bbox.xy (min)
    float4 bboxFlags;   // bbox.zw (max), flags, padding
};

// Compute exact signed distance to a quadratic Bézier curve
// Uses the canonical parabola method for efficiency
float sdQuadraticBezier(float2 p, float2 p0, float2 p1, float2 p2) {
    // Transform to canonical space where curve becomes y = x²
    float2 a = p1 - p0;
    float2 b = p0 - 2.0 * p1 + p2;
    float2 c = a * 2.0;
    float2 d = p0 - p;

    // Handle nearly-linear case
    float bb = dot(b, b);
    if (bb < 1e-10) {
        // Degenerate to line segment
        float2 ab = p2 - p0;
        float t = clamp(dot(p - p0, ab) / dot(ab, ab), 0.0, 1.0);
        return length(p - (p0 + t * ab));
    }

    // Solve cubic: t³ + at² + bt + c = 0
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
        // One real root
        h = sqrt(h);
        float2 x = (float2(h, -h) - q_coef) * 0.5;
        float2 uv = sign(x) * pow(abs(x), float2(1.0 / 3.0, 1.0 / 3.0));
        float t = clamp(uv.x + uv.y - kx, 0.0, 1.0);

        float2 q = (1.0 - t) * (1.0 - t) * p0 + 2.0 * (1.0 - t) * t * p1 + t * t * p2;
        dist = length(p - q);
    } else {
        // Three real roots - use trigonometric solution
        float z = sqrt(-p_coef);
        float v = acos(q_coef / (p_coef * z * 2.0)) / 3.0;
        float m = cos(v);
        float n = sin(v) * 1.732050808; // sqrt(3)

        float3 t3 = clamp(float3(m + m, -n - m, n - m) * z - kx, 0.0, 1.0);

        // Evaluate all three roots and pick minimum distance
        for (int i = 0; i < 3; i++) {
            float t = t3[i];
            float2 q = (1.0 - t) * (1.0 - t) * p0 + 2.0 * (1.0 - t) * t * p1 + t * t * p2;
            float d = length(p - q);
            dist = min(dist, d);
        }
    }

    return dist;
}

// Compute winding number contribution from a quadratic Bézier
// Returns +1 for upward crossing, -1 for downward, 0 for no crossing
int curveWindingContribution(float2 point, float2 p0, float2 p1, float2 p2) {
    float px = point.x;
    float py = point.y;

    // Quick reject using Y bounds
    float y_min = min(min(p0.y, p1.y), p2.y);
    float y_max = max(max(p0.y, p1.y), p2.y);
    if (py < y_min || py >= y_max) return 0;

    // Quick reject if point is to the right of curve
    float x_max = max(max(p0.x, p1.x), p2.x);
    if (px > x_max) return 0;

    // Solve for t where y(t) = py
    // y(t) = (1-t)²y0 + 2(1-t)t·y1 + t²y2
    float a = p0.y - 2.0 * p1.y + p2.y;
    float b = 2.0 * (p1.y - p0.y);
    float c = p0.y - py;

    int crossings = 0;

    if (abs(a) < 1e-10) {
        // Linear case
        if (abs(b) > 1e-10) {
            float t = -c / b;
            if (t >= 0.0 && t < 1.0) {
                float x = (1.0 - t) * (1.0 - t) * p0.x + 2.0 * (1.0 - t) * t * p1.x + t * t * p2.x;
                if (x > px) {
                    float dy = p2.y - p0.y;
                    crossings = (dy > 0.0) ? 1 : -1;
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
                        // Compute dy/dt at intersection
                        float dy_dt = 2.0 * ((p1.y - p0.y) * (1.0 - t) + (p2.y - p1.y) * t);
                        crossings += (dy_dt > 0.0) ? 1 : -1;
                    }
                }
            }
        }
    }

    return crossings;
}

// Compute squared distance to axis-aligned bounding box
float distanceToAABB(float2 p, float4 bbox) {
    float2 d = max(float2(bbox.x, bbox.y) - p, p - float2(bbox.z, bbox.w));
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}

// Extract curve control points from packed format
void unpackCurve(BezierCurve curve, out float2 p0, out float2 p1, out float2 p2, out float4 bbox) {
    p0 = curve.points01.xy;
    p1 = curve.points01.zw;
    p2 = curve.points2bbox.xy;
    bbox = float4(curve.points2bbox.zw, curve.bboxFlags.xy);
}
