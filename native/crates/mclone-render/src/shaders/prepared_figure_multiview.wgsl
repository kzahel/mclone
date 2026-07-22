struct StereoViewUniform {
    view_projections: array<mat4x4<f32>, 2>,
};

struct RestPalette {
    matrices: array<mat4x4<f32>, 64>,
};

@group(0) @binding(0)
var<uniform> stereo_view: StereoViewUniform;

@group(1) @binding(0)
var<uniform> palette: RestPalette;

@group(2) @binding(0)
var figure_texture: texture_2d<f32>;

@group(2) @binding(1)
var figure_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) part_id: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput, @builtin(view_index) view_index: i32) -> VertexOutput {
    let part_matrix = palette.matrices[input.part_id];
    let world_position = part_matrix * vec4<f32>(input.position, 1.0);
    var output: VertexOutput;
    output.position = stereo_view.view_projections[u32(view_index)] * world_position;
    output.normal = normalize((part_matrix * vec4<f32>(input.normal, 0.0)).xyz);
    output.uv = input.uv;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(figure_texture, figure_sampler, input.uv);
    if (texel.a < 0.1) {
        discard;
    }
    let normal = normalize(input.normal);
    let key_direction = normalize(vec3<f32>(3.0, 5.0, 4.0));
    let diffuse = max(dot(normal, key_direction), 0.0);
    let hemisphere = mix(0.55, 1.0, normal.y * 0.5 + 0.5);
    let light = clamp(0.34 + diffuse * 0.55 + hemisphere * 0.20, 0.0, 1.15);
    return vec4<f32>(texel.rgb * input.color.rgb * light, texel.a * input.color.a);
}
