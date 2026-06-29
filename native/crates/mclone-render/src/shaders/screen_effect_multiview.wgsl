@group(0) @binding(0)
var effect_texture: texture_2d<f32>;

@group(0) @binding(1)
var effect_sampler: sampler;

struct ScreenEffectView {
    uv_offset: vec2<f32>,
    padding: vec2<f32>,
    color: vec4<f32>,
};

struct ScreenEffectUniforms {
    views: array<ScreenEffectView, 2>,
};

@group(1) @binding(0)
var<uniform> uniforms: ScreenEffectUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) base_uv: vec2<f32>,
    @location(2) _color: vec4<f32>,
    @builtin(view_index) view_index: i32,
) -> VertexOutput {
    let effect = uniforms.views[u32(view_index)];
    var output: VertexOutput;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.uv = base_uv + effect.uv_offset;
    output.color = effect.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(effect_texture, effect_sampler, input.uv);
    return vec4<f32>(texel.rgb * input.color.rgb, texel.a * input.color.a);
}
