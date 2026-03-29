// Frame graph texture composite pass

struct Uniforms {
    float4 viewportParams; // xy = target size
    float4 sourceRect; // xy = origin, zw = size
    float4 targetRect; // xy = origin, zw = size
};

[[vk::binding(0, 0)]]
Texture2D<float4> sourceTexture;

[[vk::binding(1, 0)]]
SamplerState sourceSampler;

[[vk::binding(2, 0)]]
ConstantBuffer<Uniforms> uniforms;

struct VertexOutput {
    float4 position : SV_Position;
};

[shader("vertex")]
VertexOutput vs_main(uint vertexID : SV_VertexID) {
    VertexOutput output;
    float2 positions[3] = {
        float2(-1.0, -1.0),
        float2(3.0, -1.0),
        float2(-1.0, 3.0)
    };
    output.position = float4(positions[vertexID], 0.0, 1.0);
    return output;
}

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    float2 screenUv = input.position.xy / max(uniforms.viewportParams.xy, 1e-5.xx);
    float2 relUv = (screenUv - uniforms.targetRect.xy) / max(uniforms.targetRect.zw, 1e-5.xx);
    if (any(relUv < 0.0) || any(relUv > 1.0)) {
        discard;
    }

    float2 sampleUv = uniforms.sourceRect.xy + relUv * uniforms.sourceRect.zw;
    return sourceTexture.Sample(sourceSampler, sampleUv);
}
