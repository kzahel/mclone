struct ViewUniforms {
    view_projection: mat4x4<f32>,
    render_options: vec4<f32>,
    camera_position: vec4<f32>,
    fog_color: vec4<f32>,
    fog_distances: vec4<f32>,
};

struct StereoUniforms {
    views: array<ViewUniforms, 2>,
};

@group(0) @binding(0)
var<uniform> stereo_uniforms: StereoUniforms;

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
    @location(2) light: vec3<f32>,
    @location(3) world_position: vec3<f32>,
    @location(4) @interpolate(flat) view_index: i32,
};

fn dimension_brightness(light_level: f32) -> f32 {
    let normalized = clamp(light_level, 0.0, 15.0) / 15.0;
    return normalized / (4.0 - 3.0 * normalized);
}

fn lerp_vec3(left: vec3<f32>, right: vec3<f32>, delta: f32) -> vec3<f32> {
    return left + (right - left) * delta;
}

fn srgb_decode_channel(value: f32) -> f32 {
    let clamped = clamp(value, 0.0, 1.0);
    if (clamped <= 0.04045) {
        return clamped / 12.92;
    }
    return pow((clamped + 0.055) / 1.055, 2.4);
}

fn srgb_encode_channel(value: f32) -> f32 {
    let clamped = clamp(value, 0.0, 1.0);
    if (clamped <= 0.0031308) {
        return clamped * 12.92;
    }
    return 1.055 * pow(clamped, 1.0 / 2.4) - 0.055;
}

fn srgb_decode(color: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_decode_channel(color.r),
        srgb_decode_channel(color.g),
        srgb_decode_channel(color.b),
    );
}

fn srgb_encode(color: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_encode_channel(color.r),
        srgb_encode_channel(color.g),
        srgb_encode_channel(color.b),
    );
}

fn apply_color_profile(color: vec4<f32>, uniforms: ViewUniforms) -> vec4<f32> {
    let mode = uniforms.render_options.w;
    if (mode > 1.5) {
        return vec4<f32>(srgb_encode(color.rgb), color.a);
    }
    if (mode > 0.5) {
        return vec4<f32>(srgb_decode(color.rgb), color.a);
    }
    return color;
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
fn vs_main(
    input: VertexInput,
    @builtin(view_index) view_index: i32,
) -> VertexOutput {
    let uniforms = stereo_uniforms.views[u32(view_index)];
    var output: VertexOutput;
    output.position = uniforms.view_projection * vec4<f32>(input.position, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    output.world_position = input.position;
    output.view_index = view_index;
    let block_light = f32((input.packed_light >> 4u) & 15u);
    let sky_light = f32((input.packed_light >> 20u) & 15u);
    let light = lightmap_color(block_light, sky_light, uniforms.render_options.y);
    output.light = select(light, vec3<f32>(1.0), uniforms.render_options.x > 0.5);
    return output;
}

fn apply_fog(color: vec4<f32>, world_position: vec3<f32>, uniforms: ViewUniforms) -> vec4<f32> {
    if (uniforms.render_options.z <= 0.5) {
        return color;
    }
    let fog_distance = distance(world_position, uniforms.camera_position.xyz);
    let fog_start = uniforms.fog_distances.x;
    let fog_end = max(uniforms.fog_distances.y, fog_start + 0.001);
    let fog_factor = clamp((fog_distance - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    return vec4<f32>(lerp_vec3(color.rgb, uniforms.fog_color.rgb, fog_factor), color.a);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uniforms = stereo_uniforms.views[u32(input.view_index)];
    let texel = textureSample(atlas_texture, atlas_sampler, input.uv);
    if (texel.a < 0.1) {
        discard;
    }
    let color = vec4<f32>(texel.rgb * input.color.rgb * input.light, texel.a * input.color.a);
    return apply_color_profile(apply_fog(color, input.world_position, uniforms), uniforms);
}
