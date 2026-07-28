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
};

fn dimension_brightness(light_level: f32) -> f32 {
    let normalized = clamp(light_level, 0.0, 15.0) / 15.0;
    return normalized / (4.0 - 3.0 * normalized);
}

fn lerp_vec3(left: vec3<f32>, right: vec3<f32>, delta: f32) -> vec3<f32> {
    return left + (right - left) * delta;
}

// __MCLONE_TARGET_COLOR_TRANSFER_WGSL__

fn apply_color_profile(color: vec4<f32>) -> vec4<f32> {
    return mclone_apply_target_color_transform_rgba(
        color,
        uniforms.render_options.w,
    );
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

fn nearest_periodic_lift(value: f32, observer: f32, period: f32) -> f32 {
    if (period <= 0.0) {
        return value;
    }
    return observer + (value - observer) - round((value - observer) / period) * period;
}

fn observer_local_position(position: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        nearest_periodic_lift(position.x, uniforms.camera_position.x, uniforms.fog_distances.z),
        position.y,
        nearest_periodic_lift(position.z, uniforms.camera_position.z, uniforms.fog_distances.w),
    );
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world_position = observer_local_position(input.position);
    output.position = uniforms.view_projection * vec4<f32>(world_position, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    output.world_position = world_position;
    let block_light = f32((input.packed_light >> 4u) & 15u);
    let sky_light = f32((input.packed_light >> 20u) & 15u);
    let light = lightmap_color(block_light, sky_light, uniforms.render_options.y);
    output.light = select(light, vec3<f32>(1.0), uniforms.render_options.x > 0.5);
    return output;
}

// MCLONE_FOG_FUNCTION

fn apply_fog(color: vec4<f32>, world_position: vec3<f32>) -> vec4<f32> {
    let fog_factor = mclone_fog_factor(
        world_position,
        uniforms.camera_position,
        uniforms.render_options,
        uniforms.fog_color,
        uniforms.fog_distances,
    );
    return vec4<f32>(lerp_vec3(color.rgb, uniforms.fog_color.rgb, fog_factor), color.a);
}

fn shade_texel(input: VertexOutput, texel: vec4<f32>) -> vec4<f32> {
    let color = vec4<f32>(texel.rgb * input.color.rgb * input.light, texel.a * input.color.a);
    return apply_color_profile(apply_fog(color, input.world_position));
}

@fragment
fn fs_main_solid(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas_texture, atlas_sampler, input.uv);
    return shade_texel(input, texel);
}

@fragment
fn fs_main_cutout(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas_texture, atlas_sampler, input.uv);
    if (texel.a < 0.1) {
        discard;
    }
    return shade_texel(input, texel);
}
