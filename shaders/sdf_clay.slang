// SDF 3D Clay Tablet with Wikipedia-style Relief Text

struct Uniforms {
    float4x4 invViewProj;
    float4 cameraPosTime;
    float4 lightDirIntensity;
    float4 renderParams;
};

[[vk::binding(0, 0)]]
ConstantBuffer<Uniforms> uniforms;

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

// 2D SDF primitives for text
float sdCircle2D(float2 p, float r) { return length(p) - r; }
float sdBox2D(float2 p, float2 b) {
    float2 d = abs(p) - b;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}
float sdSeg2D(float2 p, float2 a, float2 b) {
    float2 pa = p - a, ba = b - a;
    float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// Stroke width for glyphs
static const float SW = 0.04;

// Lowercase letters
float sdA(float2 p) {
    float d = abs(sdCircle2D(p - float2(0, 0.15), 0.15)) - SW;
    d = min(d, sdBox2D(p - float2(0.15, 0.15), float2(SW, 0.15)));
    return d;
}
float sdB(float2 p) {
    float d = sdBox2D(p - float2(-0.12, 0.2), float2(SW, 0.2));
    d = min(d, abs(sdCircle2D(p - float2(0, 0.15), 0.15)) - SW);
    d = max(d, -p.x - 0.12);
    return d;
}
float sdC(float2 p) {
    float d = abs(sdCircle2D(p - float2(0, 0.15), 0.15)) - SW;
    d = max(d, p.x - 0.08);
    return d;
}
float sdD(float2 p) {
    float d = sdBox2D(p - float2(0.12, 0.2), float2(SW, 0.2));
    d = min(d, abs(sdCircle2D(p - float2(0, 0.15), 0.15)) - SW);
    d = max(d, p.x - 0.12);
    return d;
}
float sdE(float2 p) {
    float d = abs(sdCircle2D(p - float2(0, 0.15), 0.15)) - SW;
    d = max(d, p.x - 0.1);
    d = min(d, sdBox2D(p - float2(0.05, 0.15), float2(0.1, SW)));
    return d;
}
float sdF(float2 p) {
    float d = sdBox2D(p - float2(-0.06, 0.2), float2(SW, 0.2));
    float hook = abs(sdCircle2D(p - float2(0.05, 0.35), 0.08)) - SW;
    hook = max(hook, -p.y + 0.35);
    d = min(d, hook);
    d = min(d, sdBox2D(p - float2(0.01, 0.2), float2(0.08, SW)));
    return d;
}
float sdG(float2 p) {
    float d = abs(sdCircle2D(p - float2(0, 0.15), 0.15)) - SW;
    d = min(d, sdBox2D(p - float2(0.12, -0.05), float2(SW, 0.1)));
    float curve = abs(sdCircle2D(p - float2(0, -0.1), 0.12)) - SW;
    curve = max(curve, p.y + 0.1);
    curve = max(curve, -p.x);
    d = min(d, curve);
    return d;
}
float sdH(float2 p) {
    float d = sdBox2D(p - float2(-0.11, 0.2), float2(SW, 0.2));
    d = min(d, sdBox2D(p - float2(0.11, 0.15), float2(SW, 0.15)));
    float arc = abs(sdCircle2D(p - float2(0, 0.15), 0.11)) - SW;
    arc = max(arc, -p.y + 0.15);
    d = min(d, arc);
    return d;
}
float sdI(float2 p) {
    float d = sdBox2D(p - float2(0, 0.15), float2(SW, 0.15));
    d = min(d, sdCircle2D(p - float2(0, 0.38), SW));
    return d;
}
float sdK(float2 p) {
    float d = sdBox2D(p - float2(-0.11, 0.2), float2(SW, 0.2));
    d = min(d, sdSeg2D(p, float2(-0.06, 0.14), float2(0.14, 0.3)) - SW);
    d = min(d, sdSeg2D(p, float2(-0.03, 0.15), float2(0.14, 0.0)) - SW);
    return d;
}
float sdL(float2 p) {
    return sdBox2D(p - float2(-0.04, 0.2), float2(SW, 0.2));
}
float sdM(float2 p) {
    float d = sdBox2D(p - float2(-0.15, 0.15), float2(SW, 0.15));
    d = min(d, sdBox2D(p - float2(0.15, 0.15), float2(SW, 0.15)));
    float arc1 = abs(sdCircle2D(p - float2(-0.075, 0.19), 0.075)) - SW;
    arc1 = max(arc1, -p.y + 0.19);
    d = min(d, arc1);
    float arc2 = abs(sdCircle2D(p - float2(0.075, 0.19), 0.075)) - SW;
    arc2 = max(arc2, -p.y + 0.19);
    d = min(d, arc2);
    d = min(d, sdBox2D(p - float2(0, 0.15), float2(SW, 0.08)));
    return d;
}
float sdN(float2 p) {
    float d = sdBox2D(p - float2(-0.11, 0.15), float2(SW, 0.15));
    d = min(d, sdBox2D(p - float2(0.11, 0.15), float2(SW, 0.15)));
    float arc = abs(sdCircle2D(p - float2(0, 0.17), 0.11)) - SW;
    arc = max(arc, -p.y + 0.17);
    d = min(d, arc);
    return d;
}
float sdO(float2 p) {
    return abs(sdCircle2D(p - float2(0, 0.15), 0.15)) - SW;
}
float sdP(float2 p) {
    float d = sdBox2D(p - float2(-0.11, 0.05), float2(SW, 0.2));
    float arc = abs(sdCircle2D(p - float2(0, 0.15), 0.11)) - SW;
    arc = max(arc, -p.x - 0.11);
    d = min(d, arc);
    return d;
}
float sdR(float2 p) {
    float d = sdBox2D(p - float2(-0.08, 0.15), float2(SW, 0.15));
    float arc = abs(sdCircle2D(p - float2(0.02, 0.22), 0.08)) - SW;
    arc = max(arc, -p.y + 0.22);
    d = min(d, arc);
    return d;
}
float sdS(float2 p) {
    float top = abs(sdCircle2D(p - float2(0.01, 0.22), 0.08)) - SW;
    top = max(top, p.y - 0.30);
    top = max(top, -p.y + 0.17);
    float bot = abs(sdCircle2D(p - float2(-0.01, 0.08), 0.08)) - SW;
    bot = max(bot, p.y - 0.12);
    return min(top, bot);
}
float sdT(float2 p) {
    float d = sdBox2D(p - float2(0, 0.15), float2(SW, 0.15));
    d = min(d, sdBox2D(p - float2(0, 0.30), float2(0.11, SW)));
    return d;
}
float sdU(float2 p) {
    float d = sdBox2D(p - float2(-0.11, 0.19), float2(SW, 0.11));
    d = min(d, sdBox2D(p - float2(0.11, 0.19), float2(SW, 0.11)));
    float arc = abs(sdCircle2D(p - float2(0, 0.11), 0.11)) - SW;
    arc = max(arc, p.y - 0.11);
    d = min(d, arc);
    return d;
}
float sdW(float2 p) {
    float d = sdSeg2D(p, float2(-0.19, 0.30), float2(-0.1, 0)) - SW;
    d = min(d, sdSeg2D(p, float2(-0.1, 0), float2(0, 0.19)) - SW);
    d = min(d, sdSeg2D(p, float2(0, 0.19), float2(0.1, 0)) - SW);
    d = min(d, sdSeg2D(p, float2(0.1, 0), float2(0.19, 0.30)) - SW);
    return d;
}
float sdX(float2 p) {
    float d = sdSeg2D(p, float2(-0.12, 0), float2(0.12, 0.30)) - SW;
    d = min(d, sdSeg2D(p, float2(0.12, 0), float2(-0.12, 0.30)) - SW);
    return d;
}
float sdY(float2 p) {
    float d = sdSeg2D(p, float2(-0.11, 0.30), float2(0, 0.13)) - SW;
    d = min(d, sdSeg2D(p, float2(0.11, 0.30), float2(0, 0.13)) - SW);
    d = min(d, sdBox2D(p - float2(0, -0.03), float2(SW, 0.12)));
    return d;
}

// Uppercase letters (taller)
float sdCapR(float2 p) {
    p.y -= 0.07;
    float d = sdBox2D(p - float2(-0.12, 0.2), float2(SW, 0.2));
    float arc = abs(sdCircle2D(p - float2(0, 0.27), 0.12)) - SW;
    arc = max(arc, -p.x - 0.12);
    arc = max(arc, -p.y + 0.15);
    d = min(d, arc);
    d = min(d, sdSeg2D(p, float2(0, 0.15), float2(0.15, 0)) - SW);
    return d;
}
float sdCapA(float2 p) {
    p.y -= 0.07;
    float d = sdSeg2D(p, float2(-0.15, 0), float2(0, 0.4)) - SW;
    d = min(d, sdSeg2D(p, float2(0.15, 0), float2(0, 0.4)) - SW);
    d = min(d, sdBox2D(p - float2(0, 0.13), float2(0.1, SW)));
    return d;
}
float sdCapY(float2 p) {
    p.y -= 0.07;
    float d = sdSeg2D(p, float2(-0.13, 0.4), float2(0, 0.19)) - SW;
    d = min(d, sdSeg2D(p, float2(0.13, 0.4), float2(0, 0.19)) - SW);
    d = min(d, sdBox2D(p - float2(0, 0.1), float2(SW, 0.1)));
    return d;
}
float sdCapB(float2 p) {
    p.y -= 0.07;
    float d = sdBox2D(p - float2(-0.11, 0.2), float2(SW, 0.2));
    float top = abs(sdCircle2D(p - float2(0, 0.3), 0.1)) - SW;
    top = max(top, -p.x - 0.11);
    d = min(d, top);
    float bot = abs(sdCircle2D(p - float2(0, 0.1), 0.1)) - SW;
    bot = max(bot, -p.x - 0.11);
    d = min(d, bot);
    return d;
}

float sdSpace(float2 p) { return 1.0; }
float sdPeriod(float2 p) { return sdCircle2D(p - float2(0, 0.03), SW); }

// Character dispatch
float sdChar(float2 p, int c) {
    if (c == 0) return sdSpace(p);
    if (c == 27) return sdPeriod(p);
    if (c >= 100) {
        int u = c - 100;
        if (u == 0) return sdCapA(p);
        if (u == 1) return sdCapB(p);
        if (u == 17) return sdCapR(p);
        if (u == 24) return sdCapY(p);
        return sdSpace(p);
    }
    if (c == 1) return sdA(p);
    if (c == 2) return sdB(p);
    if (c == 3) return sdC(p);
    if (c == 4) return sdD(p);
    if (c == 5) return sdE(p);
    if (c == 6) return sdF(p);
    if (c == 7) return sdG(p);
    if (c == 8) return sdH(p);
    if (c == 9) return sdI(p);
    if (c == 11) return sdK(p);
    if (c == 12) return sdL(p);
    if (c == 13) return sdM(p);
    if (c == 14) return sdN(p);
    if (c == 15) return sdO(p);
    if (c == 16) return sdP(p);
    if (c == 18) return sdR(p);
    if (c == 19) return sdS(p);
    if (c == 20) return sdT(p);
    if (c == 21) return sdU(p);
    if (c == 23) return sdW(p);
    if (c == 24) return sdX(p);
    if (c == 25) return sdY(p);
    return sdSpace(p);
}

// Text content (similar to 2D version)
static const int TITLE_LEN = 6;
static const int TITLE[6] = {117, 1, 25, 2, 15, 24}; // Raybox

static const int L1_LEN = 19;
static const int L1[19] = {19,9,7,14,5,4,0,4,9,19,20,1,14,3,5,0,6,9,5}; // signed distance fie

static const int L2_LEN = 18;
static const int L2[18] = {12,4,0,18,5,14,4,5,18,9,14,7,0,21,19,9,14,7}; // ld rendering using

static const int L3_LEN = 16;
static const int L3[16] = {19,16,8,5,18,5,0,20,18,1,3,9,14,7,27,0}; // sphere tracing.

static const int L4_LEN = 18;
static const int L4[18] = {19,15,6,20,0,19,8,1,4,15,23,19,0,1,14,4,0,1}; // soft shadows and a

static const int L5_LEN = 17;
static const int L5[17] = {13,2,9,5,14,20,0,15,3,3,12,21,19,9,15,14,27}; // mbient occlusion.

float getCharWidth(int c) {
    if (c == 0) return 0.14;
    if (c == 9 || c == 12) return 0.14;
    if (c == 13 || c == 23) return 0.35;
    return 0.25;
}

float sdLine(float2 p, int lineIdx) {
    float d = 1e10;
    float xpos = 0.0;

    int len = 0;
    if (lineIdx == 0) len = TITLE_LEN;
    else if (lineIdx == 1) len = L1_LEN;
    else if (lineIdx == 2) len = L2_LEN;
    else if (lineIdx == 3) len = L3_LEN;
    else if (lineIdx == 4) len = L4_LEN;
    else if (lineIdx == 5) len = L5_LEN;

    for (int i = 0; i < 20 && i < len; i++) {
        int c = 0;
        if (lineIdx == 0 && i < TITLE_LEN) c = TITLE[i];
        else if (lineIdx == 1 && i < L1_LEN) c = L1[i];
        else if (lineIdx == 2 && i < L2_LEN) c = L2[i];
        else if (lineIdx == 3 && i < L3_LEN) c = L3[i];
        else if (lineIdx == 4 && i < L4_LEN) c = L4[i];
        else if (lineIdx == 5 && i < L5_LEN) c = L5[i];

        float charD = sdChar(p - float2(xpos, 0), c);
        d = min(d, charD);
        xpos += getCharWidth(c);
    }

    return d;
}

// Combined 2D text SDF for all lines
float sdText2D(float2 p) {
    float d = 1e10;
    float lineHeight = 0.55;
    float leftMargin = -1.8;
    float topMargin = 1.1;

    // Title (larger)
    {
        float2 titleP = p;
        titleP.x -= leftMargin;
        titleP.y -= topMargin;
        titleP *= 0.7;
        d = min(d, sdLine(titleP, 0));
    }

    // Body text
    for (int line = 1; line < 6; line++) {
        float2 lineP = p;
        lineP.x -= leftMargin;
        lineP.y -= topMargin - 0.7 - float(line - 1) * lineHeight;

        d = min(d, sdLine(lineP, line));
    }

    return d;
}

// Extrude 2D text into 3D relief
float sdTextRelief(float3 p, float depth) {
    float d2d = sdText2D(float2(p.x, -p.z));
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

    // Ground
    Hit ground;
    ground.dist = sdPlane(p, float3(0, 1, 0), 1.5);
    ground.materialId = 1;
    result = opUnionHit(result, ground);

    // Clay tablet (rounded box)
    Hit tablet;
    float3 tabletP = p - float3(0, 0.3, 0);
    tablet.dist = sdRoundBox(tabletP, float3(2.5, 0.25, 1.8), 0.08);
    tablet.materialId = 2;

    // Carve text into tablet
    float3 textP = p - float3(0, 0.50, 0);
    float textRelief = sdTextRelief(textP, 0.08);

    tablet.dist = max(tablet.dist, -textRelief);

    result = opUnionHit(result, tablet);

    return result;
}

float3 getMaterialColor(int id) {
    switch (id) {
        case 1: return float3(0.25, 0.23, 0.20);
        case 2: return float3(0.78, 0.62, 0.44);
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
