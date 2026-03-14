// Shared textured overlay shader

struct VertexInput {
    float2 position : POSITION;
    float2 texCoords : TEXCOORD0;
};

struct VertexOutput {
    float4 position : SV_Position;
    float2 texCoords : TEXCOORD0;
};

[[vk::binding(0, 0)]]
Texture2D<float4> overlayTexture;

[[vk::binding(1, 0)]]
SamplerState overlaySampler;

[shader("vertex")]
VertexOutput vs_main(VertexInput input) {
    VertexOutput output;
    output.position = float4(input.position, 0.0, 1.0);
    output.texCoords = input.texCoords;
    return output;
}

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    return overlayTexture.Sample(overlaySampler, input.texCoords);
}
