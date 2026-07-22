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
    @location(5) alpha_cutoff: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) alpha_cutoff: f32,
    @location(4) local_position: vec3<f32>,
    @location(5) @interpolate(flat) part_id: u32,
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
    output.alpha_cutoff = input.alpha_cutoff;
    output.local_position = input.position;
    output.part_id = input.part_id;
    return output;
}

fn surface(input: VertexOutput) -> vec4<f32> {
    return textureSample(figure_texture, figure_sampler, input.uv) * input.color;
}

fn lit_rgb(input: VertexOutput, texel: vec4<f32>) -> vec3<f32> {
    let normal = normalize(input.normal);
    let key_direction = normalize(vec3<f32>(3.0, 5.0, 4.0));
    let diffuse = max(dot(normal, key_direction), 0.0);
    let hemisphere = mix(0.55, 1.0, normal.y * 0.5 + 0.5);
    let light = clamp(0.34 + diffuse * 0.55 + hemisphere * 0.20, 0.0, 1.15);
    return texel.rgb * light;
}

fn coverage_hash(input: VertexOutput) -> f32 {
    let cell = floor(input.local_position * 64.0)
        + vec3<f32>(f32(input.part_id) * 0.37, f32(input.part_id) * 0.61, f32(input.part_id) * 0.83);
    var value = fract(cell * 0.1031);
    value = value + dot(value, value.yzx + 33.33);
    return fract((value.x + value.y) * value.z);
}

@fragment
fn fs_opaque(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = surface(input);
    if (texel.a <= 0.0001) {
        discard;
    }
    return vec4<f32>(lit_rgb(input, texel), 1.0);
}

@fragment
fn fs_mask_threshold(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = surface(input);
    if (texel.a < input.alpha_cutoff) {
        discard;
    }
    return vec4<f32>(lit_rgb(input, texel), 1.0);
}

@fragment
fn fs_mask_dither(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = surface(input);
    if (texel.a <= coverage_hash(input)) {
        discard;
    }
    return vec4<f32>(lit_rgb(input, texel), 1.0);
}

@fragment
fn fs_blend_depth(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = surface(input);
    if (texel.a <= 0.0001) {
        discard;
    }
    return vec4<f32>(0.0);
}

@fragment
fn fs_blend_color(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = surface(input);
    if (texel.a <= 0.0001) {
        discard;
    }
    return vec4<f32>(lit_rgb(input, texel) * texel.a, texel.a);
}

@fragment
fn fs_additive(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = surface(input);
    if (texel.a <= 0.0001) {
        discard;
    }
    return vec4<f32>(lit_rgb(input, texel) * texel.a, texel.a);
}
