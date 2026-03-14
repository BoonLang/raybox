// Minimal animated gradient baseline scene

struct Uniforms {
    float2 resolution;
    float time;
    float _padding;
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
    float2 positions[3] = {
        float2(-1.0, -1.0),
        float2(3.0, -1.0),
        float2(-1.0, 3.0)
    };
    float2 pos = positions[vertexID];
    output.position = float4(pos, 0.0, 1.0);
    output.uv = pos * 0.5 + 0.5;
    return output;
}

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    float t = uniforms.time * 0.1;
    float3 grad = lerp(
        float3(0.05, 0.05, 0.1),
        float3(0.1, 0.05, 0.15),
        input.uv.y + sin(t) * 0.1
    );
    return float4(grad, 1.0);
}
