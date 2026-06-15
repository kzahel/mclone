struct Uniforms {
    view_projection: mat4x4<f32>,
    render_options: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(1) @binding(1)
var atlas_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) packed_light: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) light: f32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.view_projection * vec4<f32>(input.position, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    let block_light = f32((input.packed_light >> 4u) & 15u);
    let sky_light = f32((input.packed_light >> 20u) & 15u);
    let light = max(max(block_light, sky_light) / 15.0, 0.08);
    output.light = select(light, 1.0, uniforms.render_options.x > 0.5);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas_texture, atlas_sampler, input.uv);
    if (texel.a < 0.1) {
        discard;
    }
    return vec4<f32>(texel.rgb * input.color.rgb * input.light, texel.a * input.color.a);
}
