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

struct GrassFrameUniforms {
    time_direction_shelter: vec4<f32>,
    shape: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> grass_frame: GrassFrameUniforms;

struct GrassInteractionUniforms {
    origin_cell_size_enabled: vec4<f32>,
    topology_periods_strength: vec4<f32>,
};

@group(3) @binding(0)
var grass_interaction_field: texture_2d<f32>;

@group(3) @binding(1)
var<uniform> grass_interaction: GrassInteractionUniforms;

struct GrassPatchInput {
    @location(0) root: vec3<i32>,
    @location(1) packed_tint: u32,
    @location(2) packed_light: u32,
    @location(3) seed: u32,
    @location(4) flags: u32,
    @location(5) reserved: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) light: vec3<f32>,
    @location(2) world_position: vec3<f32>,
    @location(3) @interpolate(flat) view_index: i32,
};

fn mix_hash(value: u32) -> u32 {
    var mixed = value;
    mixed = mixed ^ (mixed >> 16u);
    mixed = mixed * 0x7feb352du;
    mixed = mixed ^ (mixed >> 15u);
    mixed = mixed * 0x846ca68bu;
    return mixed ^ (mixed >> 16u);
}

fn hash_unit(value: u32) -> f32 {
    return f32(mix_hash(value) & 0x00ffffffu) / 16777215.0;
}

fn dimension_brightness(light_level: f32) -> f32 {
    let normalized = clamp(light_level, 0.0, 15.0) / 15.0;
    return normalized / (4.0 - 3.0 * normalized);
}

fn lerp_vec3(left: vec3<f32>, right: vec3<f32>, delta: f32) -> vec3<f32> {
    return left + (right - left) * delta;
}

fn lightmap_color(
    block_light_level: f32,
    sky_light_level: f32,
    uniforms: ViewUniforms,
) -> vec3<f32> {
    let sky_darken = clamp(uniforms.render_options.y, 0.0, 1.0);
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

fn apply_color_profile(color: vec3<f32>, uniforms: ViewUniforms) -> vec3<f32> {
    let mode = uniforms.render_options.w;
    if (mode > 1.5) {
        return vec3<f32>(
            srgb_encode_channel(color.r),
            srgb_encode_channel(color.g),
            srgb_encode_channel(color.b),
        );
    }
    if (mode > 0.5) {
        return vec3<f32>(
            srgb_decode_channel(color.r),
            srgb_decode_channel(color.g),
            srgb_decode_channel(color.b),
        );
    }
    return color;
}

fn nearest_periodic_lift(value: f32, observer: f32, period: f32) -> f32 {
    if (period <= 0.0) {
        return value;
    }
    return observer + (value - observer) - round((value - observer) / period) * period;
}

fn observer_local_position(position: vec3<f32>, uniforms: ViewUniforms) -> vec3<f32> {
    return vec3<f32>(
        nearest_periodic_lift(position.x, uniforms.camera_position.x, uniforms.fog_distances.z),
        position.y,
        nearest_periodic_lift(position.z, uniforms.camera_position.z, uniforms.fog_distances.w),
    );
}

fn grass_interaction_bend(world_sample: vec2<f32>) -> vec2<f32> {
    if (grass_interaction.origin_cell_size_enabled.w < 0.5) {
        return vec2<f32>(0.0);
    }
    let origin = grass_interaction.origin_cell_size_enabled.xy;
    let cell_size = grass_interaction.origin_cell_size_enabled.z;
    let center = origin + vec2<f32>(64.0 * cell_size);
    let lifted = vec2<f32>(
        nearest_periodic_lift(
            world_sample.x,
            center.x,
            grass_interaction.topology_periods_strength.x,
        ),
        nearest_periodic_lift(
            world_sample.y,
            center.y,
            grass_interaction.topology_periods_strength.y,
        ),
    );
    let cell = vec2<i32>(floor((lifted - origin) / cell_size));
    let dimensions = vec2<i32>(textureDimensions(grass_interaction_field));
    if (any(cell < vec2<i32>(0)) || any(cell >= dimensions)) {
        return vec2<f32>(0.0);
    }
    let sample = textureLoad(grass_interaction_field, cell, 0);
    let raw_direction = sample.rg * 2.0 - vec2<f32>(1.0);
    let direction_length = length(raw_direction);
    let direction = select(
        vec2<f32>(0.0),
        raw_direction / max(direction_length, 0.0001),
        direction_length > 0.0001,
    );
    return direction * sample.b * grass_interaction.topology_periods_strength.z;
}

fn unpack_tint(packed: u32) -> vec3<f32> {
    return vec3<f32>(
        f32(packed & 255u),
        f32((packed >> 8u) & 255u),
        f32((packed >> 16u) & 255u),
    ) / 255.0;
}

fn blade_vertex(input: GrassPatchInput, vertex_index: u32) -> vec3<f32> {
    let blade = vertex_index / 12u;
    let blade_vertex = vertex_index % 12u;
    let segment = blade_vertex / 6u;
    let corner = blade_vertex % 6u;
    let blade_hash = mix_hash(input.seed ^ (blade * 0x9e3779b9u));
    let angle = hash_unit(blade_hash) * 6.283185307;
    let radial = sqrt(hash_unit(blade_hash ^ 0x68bc21ebu)) * 0.42;
    let radial_angle = hash_unit(blade_hash ^ 0x02e5be93u) * 6.283185307;
    let center = vec2<f32>(cos(radial_angle), sin(radial_angle)) * radial;
    let axis = vec2<f32>(cos(angle), sin(angle));
    let height = 0.38 + hash_unit(blade_hash ^ 0xa511e9b3u) * 0.34;
    let width = 0.035 + hash_unit(blade_hash ^ 0x63d83595u) * 0.035;
    let root = vec3<f32>(input.root) + vec3<f32>(0.5, 0.0, 0.5);
    let segment_start = f32(segment) * 0.52;
    let segment_end = select(0.52, 1.0, segment == 1u);
    let top_corner = corner == 2u || corner == 4u || corner == 5u;
    let height_factor = select(segment_start, segment_end, top_corner);
    let side = select(-1.0, 1.0, corner == 1u || corner == 2u || corner == 4u);
    let width_factor = mix(1.0, 0.16, height_factor);
    let lateral = width * width_factor * side;
    let rise = mix(0.006, height, height_factor);

    let wind_direction = normalize(grass_frame.time_direction_shelter.yz);
    let wind_perpendicular = vec2<f32>(-wind_direction.y, wind_direction.x);
    let time = grass_frame.time_direction_shelter.x;
    let world_sample = root.xz + center;
    let broad_phase = dot(world_sample, vec2<f32>(0.89, -0.69) * grass_frame.shape.y)
        + time * grass_frame.shape.z;
    let clump_phase = dot(world_sample, vec2<f32>(0.087, 0.064))
        - time * 0.29;
    let gust = 0.58
        + sin(broad_phase) * 0.27
        + sin(clump_phase + sin(broad_phase * 0.47)) * 0.15;
    let flutter = sin(
        time * 2.7
            + dot(world_sample, vec2<f32>(0.41, -0.33))
            + hash_unit(blade_hash ^ 0xc2b2ae35u) * 6.283185307,
    );
    let sky_light = f32((input.packed_light >> 20u) & 15u);
    let shelter = mix(
        grass_frame.time_direction_shelter.w,
        1.0,
        smoothstep(4.0, 13.0, sky_light),
    );
    let resistance = mix(0.66, 1.0, hash_unit(blade_hash ^ 0x27d4eb2fu));
    let influence = height_factor * height_factor;
    let lean_angle = hash_unit(blade_hash ^ 0x165667b1u) * 6.283185307;
    let resting_lean = vec2<f32>(cos(lean_angle), sin(lean_angle))
        * (0.018 + 0.025 * hash_unit(blade_hash ^ 0xd3a2646cu));
    let wind_bend = wind_direction
        * grass_frame.shape.x
        * gust
        * shelter
        * resistance;
    let flutter_bend = wind_perpendicular
        * grass_frame.shape.w
        * flutter
        * shelter
        * height_factor;
    let bend = (resting_lean + wind_bend + flutter_bend) * influence
        + grass_interaction_bend(world_sample) * influence;
    return root
        + vec3<f32>(
            center.x + axis.x * lateral + bend.x,
            rise,
            center.y + axis.y * lateral + bend.y,
        );
}

@vertex
fn vs_main(
    input: GrassPatchInput,
    @builtin(vertex_index) vertex_index: u32,
    @builtin(view_index) view_index: i32,
) -> VertexOutput {
    let uniforms = stereo_uniforms.views[u32(view_index)];
    var output: VertexOutput;
    let world_position = observer_local_position(blade_vertex(input, vertex_index), uniforms);
    let variation = 0.78 + hash_unit(input.seed ^ 0xd1b54a35u) * 0.22;
    let tip = mix(0.82, 1.0, f32((vertex_index % 12u) / 6u) * 0.52
        + select(0.0, 0.48, vertex_index % 6u >= 2u));
    output.position = uniforms.view_projection * vec4<f32>(world_position, 1.0);
    output.color = unpack_tint(input.packed_tint) * variation * tip;
    output.world_position = world_position;
    output.view_index = view_index;
    let block_light = f32((input.packed_light >> 4u) & 15u);
    let sky_light = f32((input.packed_light >> 20u) & 15u);
    let light = lightmap_color(block_light, sky_light, uniforms);
    output.light = select(light, vec3<f32>(1.0), uniforms.render_options.x > 0.5);
    return output;
}

fn apply_fog(
    color: vec3<f32>,
    world_position: vec3<f32>,
    uniforms: ViewUniforms,
) -> vec3<f32> {
    if (uniforms.render_options.z <= 0.5) {
        return color;
    }
    let fog_distance = distance(world_position, uniforms.camera_position.xyz);
    let fog_start = uniforms.fog_distances.x;
    let fog_end = max(uniforms.fog_distances.y, fog_start + 0.001);
    let fog_factor = clamp((fog_distance - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    return lerp_vec3(color, uniforms.fog_color.rgb, fog_factor);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uniforms = stereo_uniforms.views[u32(input.view_index)];
    let color = apply_color_profile(
        apply_fog(input.color * input.light, input.world_position, uniforms),
        uniforms,
    );
    return vec4<f32>(color, 1.0);
}
