// Shared scene present pass

struct Uniforms {
    uint4 flags;
};

[[vk::binding(0, 0)]]
Texture2D<float4> sceneTexture;

[[vk::binding(1, 0)]]
SamplerState sceneSampler;

[[vk::binding(2, 0)]]
ConstantBuffer<Uniforms> uniforms;

struct VertexOutput {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

float srgbToLinearComponent(float c) {
    return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4);
}

float3 srgbToLinear(float3 color) {
    return float3(
        srgbToLinearComponent(color.x),
        srgbToLinearComponent(color.y),
        srgbToLinearComponent(color.z)
    );
}

[shader("vertex")]
VertexOutput vs_main(uint vertexID : SV_VertexID) {
    VertexOutput output;
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
    output.position = float4(positions[vertexID], 0.0, 1.0);
    output.uv = uvs[vertexID];
    return output;
}

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    float4 sampled = sceneTexture.Sample(sceneSampler, input.uv);
    if (uniforms.flags.x != 0) {
        return float4(srgbToLinear(sampled.rgb), sampled.a);
    }
    return sampled;
}
