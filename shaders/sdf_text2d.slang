// SDF 2D Text Rendering - Clean readable block letters
// Simple sans-serif style procedural glyphs

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

// 2D SDF primitives
float sdBox(float2 p, float2 b) {
    float2 d = abs(p) - b;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}

float sdCircle(float2 p, float r) {
    return length(p) - r;
}

// Simple block-style uppercase letters
// Each letter: width ~0.5, height ~0.7
// Stroke width W = 0.08

static const float W = 0.08;

// Letter A - two verticals + top + crossbar
float sdA(float2 p) {
    float left = sdBox(p - float2(-0.14, 0.0), float2(W, 0.35));
    float right = sdBox(p - float2(0.14, 0.0), float2(W, 0.35));
    float top = sdBox(p - float2(0.0, 0.27), float2(0.14, W));
    float mid = sdBox(p - float2(0.0, -0.02), float2(0.14, W));
    return min(min(left, right), min(top, mid));
}

// Letter B
float sdB(float2 p) {
    float stem = sdBox(p - float2(-0.14, 0.0), float2(W, 0.35));
    float top = sdBox(p - float2(0.0, 0.27), float2(0.14, W));
    float mid = sdBox(p - float2(0.0, 0.0), float2(0.12, W));
    float bot = sdBox(p - float2(0.0, -0.27), float2(0.14, W));
    float rightTop = sdBox(p - float2(0.10, 0.135), float2(W, 0.135));
    float rightBot = sdBox(p - float2(0.12, -0.135), float2(W, 0.135));
    return min(stem, min(min(top, mid), min(bot, min(rightTop, rightBot))));
}

// Letter C
float sdC(float2 p) {
    float d = abs(sdCircle(p, 0.26)) - W;
    d = max(d, p.x - 0.06);
    return d;
}

// Letter D
float sdD(float2 p) {
    float stem = sdBox(p - float2(-0.14, 0.0), float2(W, 0.35));
    float arc = abs(sdCircle(p - float2(-0.04, 0.0), 0.30)) - W;
    arc = max(arc, -p.x - 0.14);
    return min(stem, arc);
}

// Letter E
float sdE(float2 p) {
    float stem = sdBox(p - float2(-0.14, 0.0), float2(W, 0.35));
    float top = sdBox(p - float2(0.02, 0.27), float2(0.18, W));
    float mid = sdBox(p - float2(-0.01, 0.0), float2(0.14, W));
    float bot = sdBox(p - float2(0.02, -0.27), float2(0.18, W));
    return min(stem, min(top, min(mid, bot)));
}

// Letter F
float sdF(float2 p) {
    float stem = sdBox(p - float2(-0.14, 0.0), float2(W, 0.35));
    float top = sdBox(p - float2(0.02, 0.27), float2(0.18, W));
    float mid = sdBox(p - float2(-0.01, 0.02), float2(0.14, W));
    return min(stem, min(top, mid));
}

// Letter G
float sdG(float2 p) {
    float arc = abs(sdCircle(p, 0.26)) - W;
    arc = max(arc, p.x - 0.10);
    float bar = sdBox(p - float2(0.06, -0.04), float2(0.12, W));
    bar = max(bar, p.y - 0.0);
    return min(arc, bar);
}

// Letter H
float sdH(float2 p) {
    float left = sdBox(p - float2(-0.14, 0.0), float2(W, 0.35));
    float right = sdBox(p - float2(0.14, 0.0), float2(W, 0.35));
    float mid = sdBox(p - float2(0.0, 0.0), float2(0.14, W));
    return min(left, min(right, mid));
}

// Letter I
float sdI(float2 p) {
    float stem = sdBox(p, float2(W, 0.35));
    float top = sdBox(p - float2(0.0, 0.27), float2(0.12, W));
    float bot = sdBox(p - float2(0.0, -0.27), float2(0.12, W));
    return min(stem, min(top, bot));
}

// Letter L
float sdL(float2 p) {
    float stem = sdBox(p - float2(-0.14, 0.0), float2(W, 0.35));
    float bot = sdBox(p - float2(0.02, -0.27), float2(0.18, W));
    return min(stem, bot);
}

// Letter M
float sdM(float2 p) {
    float left = sdBox(p - float2(-0.18, 0.0), float2(W, 0.35));
    float right = sdBox(p - float2(0.18, 0.0), float2(W, 0.35));
    float mid = sdBox(p - float2(0.0, 0.10), float2(W, 0.25));
    float topL = sdBox(p - float2(-0.09, 0.24), float2(0.11, W));
    float topR = sdBox(p - float2(0.09, 0.24), float2(0.11, W));
    return min(min(left, right), min(mid, min(topL, topR)));
}

// Letter N - two verticals + top bar (like H but top connected)
float sdN(float2 p) {
    float left = sdBox(p - float2(-0.14, 0.0), float2(W, 0.35));
    float right = sdBox(p - float2(0.14, 0.0), float2(W, 0.35));
    float top = sdBox(p - float2(0.0, 0.27), float2(0.14, W));
    return min(left, min(right, top));
}

// Letter O
float sdO(float2 p) {
    return abs(sdCircle(p, 0.26)) - W;
}

// Letter P
float sdP(float2 p) {
    float stem = sdBox(p - float2(-0.14, 0.0), float2(W, 0.35));
    float top = sdBox(p - float2(0.0, 0.27), float2(0.14, W));
    float mid = sdBox(p - float2(-0.02, 0.04), float2(0.12, W));
    float rightTop = sdBox(p - float2(0.10, 0.155), float2(W, 0.115));
    return min(stem, min(top, min(mid, rightTop)));
}

// Letter R
float sdR(float2 p) {
    float pPart = sdP(p);
    float leg = sdBox(p - float2(0.10, -0.16), float2(W, 0.14));
    return min(pPart, leg);
}

// Letter S - simple three horizontal bars connected
float sdS(float2 p) {
    float top = sdBox(p - float2(0.0, 0.27), float2(0.16, W));
    float mid = sdBox(p - float2(0.0, 0.0), float2(0.16, W));
    float bot = sdBox(p - float2(0.0, -0.27), float2(0.16, W));
    float rightTop = sdBox(p - float2(-0.12, 0.135), float2(W, 0.135));
    float leftBot = sdBox(p - float2(0.12, -0.135), float2(W, 0.135));
    return min(min(top, mid), min(bot, min(rightTop, leftBot)));
}

// Letter T
float sdT(float2 p) {
    float stem = sdBox(p - float2(0.0, -0.04), float2(W, 0.31));
    float top = sdBox(p - float2(0.0, 0.27), float2(0.20, W));
    return min(stem, top);
}

// Letter U
float sdU(float2 p) {
    float left = sdBox(p - float2(-0.14, 0.08), float2(W, 0.27));
    float right = sdBox(p - float2(0.14, 0.08), float2(W, 0.27));
    float arc = abs(sdCircle(p - float2(0.0, -0.14), 0.14)) - W;
    arc = max(arc, p.y + 0.14);
    return min(left, min(right, arc));
}

// Letter W
float sdW(float2 p) {
    float l = sdBox(p - float2(-0.20, 0.0), float2(W, 0.35));
    float ml = sdBox(p - float2(-0.07, -0.08), float2(W, 0.27));
    float mr = sdBox(p - float2(0.07, -0.08), float2(W, 0.27));
    float r = sdBox(p - float2(0.20, 0.0), float2(W, 0.35));
    float bot = sdBox(p - float2(0.0, -0.24), float2(0.10, W));
    return min(min(l, r), min(min(ml, mr), bot));
}

// Letter X
float sdX(float2 p) {
    float c = 0.76, s = 0.65;
    float2 r1 = float2(p.x*c - p.y*s, p.x*s + p.y*c);
    float2 r2 = float2(p.x*c + p.y*s, -p.x*s + p.y*c);
    float d1 = sdBox(r1, float2(W, 0.38));
    float d2 = sdBox(r2, float2(W, 0.38));
    return min(d1, d2);
}

// Letter Y
float sdY(float2 p) {
    float stem = sdBox(p - float2(0.0, -0.16), float2(W, 0.19));
    float armL = sdBox(p - float2(-0.10, 0.16), float2(W, 0.16));
    float armR = sdBox(p - float2(0.10, 0.16), float2(W, 0.16));
    float join = sdBox(p - float2(0.0, 0.05), float2(0.12, W));
    return min(stem, min(min(armL, armR), join));
}

// Space
float sdSpace(float2 p) { return 1.0; }

// Period
float sdPeriod(float2 p) {
    return length(p - float2(0.0, -0.27)) - 0.06;
}

// Character lookup: 0=space, 1-26=A-Z, 27=period
float sdChar(float2 p, int c) {
    if (c == 0) return sdSpace(p);
    if (c == 1) return sdA(p);
    if (c == 2) return sdB(p);
    if (c == 3) return sdC(p);
    if (c == 4) return sdD(p);
    if (c == 5) return sdE(p);
    if (c == 6) return sdF(p);
    if (c == 7) return sdG(p);
    if (c == 8) return sdH(p);
    if (c == 9) return sdI(p);
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
    if (c == 27) return sdPeriod(p);
    return sdSpace(p);
}

// Text content
// "RAYBOX"
static const int TITLE[6] = {18, 1, 25, 2, 15, 24};

// "SIGNED DISTANCE FIELD"
static const int L1[21] = {19,9,7,14,5,4,0,4,9,19,20,1,14,3,5,0,6,9,5,12,4};

// "RENDERING ENGINE"
static const int L2[16] = {18,5,14,4,5,18,9,14,7,0,5,14,7,9,14,5};

// "BUILT IN RUST"
static const int L3[13] = {2,21,9,12,20,0,9,14,0,18,21,19,20};

// "SOFT SHADOWS AND AO"
static const int L4[18] = {19,15,6,20,0,19,8,1,4,15,23,19,0,1,14,4,0,1};

float getWidth(int c) {
    if (c == 0) return 0.30;
    if (c == 9) return 0.38;
    if (c == 13 || c == 23) return 0.55;
    return 0.44;
}

float renderLine(float2 p, int lineNum, int len) {
    float d = 1e10;
    float x = 0.0;

    for (int i = 0; i < 22 && i < len; i++) {
        int c = 0;
        if (lineNum == 0 && i < 6) c = TITLE[i];
        else if (lineNum == 1 && i < 21) c = L1[i];
        else if (lineNum == 2 && i < 16) c = L2[i];
        else if (lineNum == 3 && i < 13) c = L3[i];
        else if (lineNum == 4 && i < 18) c = L4[i];

        d = min(d, sdChar(p - float2(x, 0.0), c));
        x += getWidth(c);
    }
    return d;
}

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    float2 uv = input.uv;
    float2 res = uniforms.renderParams.xy;
    float aspect = res.x / res.y;

    // Paper background
    float3 bg = float3(0.97, 0.96, 0.94);

    // Transform to centered coordinates
    float2 p = uv;
    p.x = (p.x - 0.5) * aspect;
    p.y = p.y - 0.5;
    p *= 7.0;

    float3 col = bg;

    // Title
    {
        float2 tp = (p - float2(-0.8, 2.2)) * 0.9;
        float d = renderLine(tp, 0, 6);
        float a = 1.0 - smoothstep(-0.02, 0.02, d);
        col = lerp(col, float3(0.12, 0.10, 0.15), a);
    }

    // Body text
    float lh = 1.0;
    float sc = 1.3;
    float sy = 1.0;

    {
        float2 lp = (p - float2(-2.6, sy)) * sc;
        float d = renderLine(lp, 1, 21);
        col = lerp(col, float3(0.18, 0.16, 0.14), 1.0 - smoothstep(-0.025, 0.025, d));
    }
    {
        float2 lp = (p - float2(-2.6, sy - lh)) * sc;
        float d = renderLine(lp, 2, 16);
        col = lerp(col, float3(0.18, 0.16, 0.14), 1.0 - smoothstep(-0.025, 0.025, d));
    }
    {
        float2 lp = (p - float2(-2.6, sy - lh*2.0)) * sc;
        float d = renderLine(lp, 3, 13);
        col = lerp(col, float3(0.18, 0.16, 0.14), 1.0 - smoothstep(-0.025, 0.025, d));
    }
    {
        float2 lp = (p - float2(-2.6, sy - lh*3.0)) * sc;
        float d = renderLine(lp, 4, 18);
        col = lerp(col, float3(0.18, 0.16, 0.14), 1.0 - smoothstep(-0.025, 0.025, d));
    }

    return float4(col, 1.0);
}
