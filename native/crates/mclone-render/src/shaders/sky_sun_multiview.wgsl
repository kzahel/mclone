struct StereoUniforms {
    view_projections: array<mat4x4<f32>, 2>,
};

@group(0) @binding(0)
var<uniform> uniforms: StereoUniforms;

@group(1) @binding(0)
var sun_texture: texture_2d<f32>;

@group(1) @binding(1)
var sun_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    input: VertexInput,
    @builtin(view_index) view_index: i32,
) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.view_projections[u32(view_index)] * vec4<f32>(input.position, 1.0);
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(sun_texture, sun_sampler, input.uv);
}
