struct ViewUniform {
    view_projection: mat4x4<f32>,
    render_options: vec4<f32>,
    camera_position: vec4<f32>,
    fog_color: vec4<f32>,
    fog_distances: vec4<f32>,
    source_anchor_scale: vec4<f32>,
    composition_anchor: vec4<f32>,
    clip_plane: vec4<f32>,
    prepared_options: vec4<f32>,
};

struct StereoViewUniform {
    views: array<ViewUniform, 2>,
};

struct ActorUniform {
    model: mat4x4<f32>,
    packed_light: u32,
    opacity: f32,
    padding: vec2<u32>,
};

struct PartPalette {
    matrices: array<mat4x4<f32>, 64>,
};

@group(0) @binding(0)
var<uniform> stereo: StereoViewUniform;

@group(1) @binding(0)
var<uniform> actor: ActorUniform;

@group(2) @binding(0)
var<uniform> palette: PartPalette;

@group(3) @binding(0)
var figure_texture: texture_2d<f32>;

@group(3) @binding(1)
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
    @location(3) light: vec3<f32>,
    @location(4) composition_position: vec3<f32>,
    @location(5) alpha_cutoff: f32,
    @location(6) local_position: vec3<f32>,
    @location(7) @interpolate(flat) part_id: u32,
    @location(8) @interpolate(flat) view_index: u32,
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
fn vs_main(input: VertexInput, @builtin(view_index) builtin_view_index: i32) -> VertexOutput {
    let view_index = u32(builtin_view_index);
    let view = stereo.views[view_index];
    let part_matrix = palette.matrices[input.part_id];
    let source_position = actor.model * part_matrix * vec4<f32>(input.position, 1.0);
    let composition_position = view.composition_anchor.xyz
        + (source_position.xyz - view.source_anchor_scale.xyz) * view.source_anchor_scale.w;
    let actor_normal = normalize((actor.model * part_matrix * vec4<f32>(input.normal, 0.0)).xyz);
    let block_light = f32((actor.packed_light >> 4u) & 15u);
    let sky_light = f32((actor.packed_light >> 20u) & 15u);
    let sampled_light = lightmap_color(block_light, sky_light, view.render_options.y);
    var output: VertexOutput;
    output.position = view.view_projection * vec4<f32>(composition_position, 1.0);
    output.normal = actor_normal;
    output.uv = input.uv;
    output.color = input.color;
    output.light = select(sampled_light, vec3<f32>(1.0), view.render_options.x > 0.5);
    output.composition_position = composition_position;
    output.alpha_cutoff = input.alpha_cutoff;
    output.local_position = input.position;
    output.part_id = input.part_id;
    output.view_index = view_index;
    return output;
}

// MCLONE_FOG_FUNCTION

fn apply_fog(color: vec4<f32>, world_position: vec3<f32>, view: ViewUniform) -> vec4<f32> {
    let fog_factor = mclone_fog_factor(
        world_position,
        view.camera_position,
        view.render_options,
        view.fog_color,
        view.fog_distances,
    );
    return vec4<f32>(lerp_vec3(color.rgb, view.fog_color.rgb, fog_factor), color.a);
}

fn clipped(input: VertexOutput, view: ViewUniform) -> bool {
    return view.prepared_options.x > 0.5
        && dot(view.clip_plane.xyz, input.composition_position) + view.clip_plane.w < 0.0;
}

fn surface(input: VertexOutput) -> vec4<f32> {
    return textureSample(figure_texture, figure_sampler, input.uv) * input.color;
}

fn lit_rgb(input: VertexOutput, texel: vec4<f32>) -> vec3<f32> {
    let key_direction = normalize(vec3<f32>(3.0, 5.0, 4.0));
    let diffuse = max(dot(normalize(input.normal), key_direction), 0.0);
    let face_light = clamp(0.55 + diffuse * 0.45, 0.0, 1.0);
    return texel.rgb * input.light * face_light;
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
    let view = stereo.views[input.view_index];
    if (clipped(input, view)) { discard; }
    let texel = surface(input);
    if (texel.a <= 0.0001 || actor.opacity <= coverage_hash(input)) { discard; }
    return apply_fog(vec4<f32>(lit_rgb(input, texel), 1.0), input.composition_position, view);
}

@fragment
fn fs_mask_threshold(input: VertexOutput) -> @location(0) vec4<f32> {
    let view = stereo.views[input.view_index];
    if (clipped(input, view)) { discard; }
    let texel = surface(input);
    if (texel.a < input.alpha_cutoff || actor.opacity <= coverage_hash(input)) { discard; }
    return apply_fog(vec4<f32>(lit_rgb(input, texel), 1.0), input.composition_position, view);
}

@fragment
fn fs_mask_dither(input: VertexOutput) -> @location(0) vec4<f32> {
    let view = stereo.views[input.view_index];
    if (clipped(input, view)) { discard; }
    let texel = surface(input);
    if (texel.a * actor.opacity <= coverage_hash(input)) { discard; }
    return apply_fog(vec4<f32>(lit_rgb(input, texel), 1.0), input.composition_position, view);
}

@fragment
fn fs_blend_depth(input: VertexOutput) -> @location(0) vec4<f32> {
    let view = stereo.views[input.view_index];
    if (clipped(input, view)) { discard; }
    let alpha = surface(input).a * actor.opacity;
    if (alpha <= 0.0001) { discard; }
    return vec4<f32>(0.0);
}

@fragment
fn fs_blend_color(input: VertexOutput) -> @location(0) vec4<f32> {
    let view = stereo.views[input.view_index];
    if (clipped(input, view)) { discard; }
    let texel = surface(input);
    let alpha = texel.a * actor.opacity;
    if (alpha <= 0.0001) { discard; }
    let color = apply_fog(
        vec4<f32>(lit_rgb(input, texel), alpha),
        input.composition_position,
        view,
    );
    return vec4<f32>(color.rgb * alpha, alpha);
}

@fragment
fn fs_additive(input: VertexOutput) -> @location(0) vec4<f32> {
    let view = stereo.views[input.view_index];
    if (clipped(input, view)) { discard; }
    let texel = surface(input);
    let alpha = texel.a * actor.opacity;
    if (alpha <= 0.0001) { discard; }
    let color = apply_fog(
        vec4<f32>(lit_rgb(input, texel), alpha),
        input.composition_position,
        view,
    );
    return vec4<f32>(color.rgb * alpha, alpha);
}
