struct Uniforms {
    view_projection: mat4x4<f32>,
    render_options: vec4<f32>,
    camera_position: vec4<f32>,
    fog_color: vec4<f32>,
    fog_distances: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var actor_texture: texture_2d<f32>;

@group(1) @binding(1)
var actor_sampler: sampler;

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
    @location(2) light: vec3<f32>,
    @location(3) world_position: vec3<f32>,
};

fn dimension_brightness(light_level: f32) -> f32 {
    let normalized = clamp(light_level, 0.0, 15.0) / 15.0;
    return normalized / (4.0 - 3.0 * normalized);
}

fn lerp_vec3(left: vec3<f32>, right: vec3<f32>, delta: f32) -> vec3<f32> {
    return left + (right - left) * delta;
}

fn lightmap_color(block_light_level: f32, sky_light_level: f32, sky_darken_value: f32) -> vec3<f32> {
    let sky_darken = clamp(sky_darken_value, 0.0, 1.0);
    let sky = dimension_brightness(sky_light_level) * (sky_darken * 0.95 + 0.05);
    let block = dimension_brightness(block_light_level) * 1.5;
    var color = vec3<f32>(
        block,
        block * ((block * 0.6 + 0.4) * 0.6 + 0.4),
        block * (block * block * 0.6 + 0.4),
    );
    let sky_tint = lerp_vec3(vec3<f32>(sky_darken, sky_darken, 1.0), vec3<f32>(1.0), 0.35);
    color += sky_tint * sky;
    color = lerp_vec3(color, vec3<f32>(0.75), 0.04);
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    color = lerp_vec3(color, vec3<f32>(0.75), 0.04);
    return clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.view_projection * vec4<f32>(input.position, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    output.world_position = input.position;
    let block_light = f32((input.packed_light >> 4u) & 15u);
    let sky_light = f32((input.packed_light >> 20u) & 15u);
    let light = lightmap_color(block_light, sky_light, uniforms.render_options.y);
    output.light = select(light, vec3<f32>(1.0), uniforms.render_options.x > 0.5);
    return output;
}

fn apply_fog(color: vec4<f32>, world_position: vec3<f32>) -> vec4<f32> {
    if (uniforms.render_options.z <= 0.5) {
        return color;
    }
    let fog_distance = distance(world_position, uniforms.camera_position.xyz);
    let fog_start = uniforms.fog_distances.x;
    let fog_end = max(uniforms.fog_distances.y, fog_start + 0.001);
    let fog_factor = clamp((fog_distance - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    return vec4<f32>(lerp_vec3(color.rgb, uniforms.fog_color.rgb, fog_factor), color.a);
}

fn coverage_hash(uv: vec2<f32>) -> f32 {
    let cell = floor(uv * 128.0);
    var value = fract(vec3<f32>(cell.x, cell.y, cell.x + cell.y) * 0.1031);
    value = value + dot(value, value.yzx + 33.33);
    return fract((value.x + value.y) * value.z);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(actor_texture, actor_sampler, input.uv);
    if (texel.a < 0.1 || input.color.a <= coverage_hash(input.uv)) {
        discard;
    }
    let color = vec4<f32>(texel.rgb * input.color.rgb * input.light, 1.0);
    return apply_fog(color, input.world_position);
}
