// Flat position-color sky geometry for XR multiview.
// Each layer selects its rotation-only sky view-projection by view_index.

struct StereoUniforms {
    view_projections: array<mat4x4<f32>, 2>,
};

@group(0) @binding(0)
var<uniform> uniforms: StereoUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    input: VertexInput,
    @builtin(view_index) view_index: i32,
) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.view_projections[u32(view_index)] * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
