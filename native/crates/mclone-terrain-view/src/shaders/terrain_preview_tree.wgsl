struct TerrainPreviewParams {
    origin_spacing_cells: vec4<i32>,
    seed_source_view: vec4<u32>,
    layer_samples_size: vec4<u32>,
    camera_eye_target: vec4<f32>,
    camera_up_fov: vec4<f32>,
    camera_projection: vec4<f32>,
    viewport_center_extent: vec4<i32>,
    presentation_center_extent: vec4<f32>,
    content_stage_flags: vec4<u32>,
    clipmap_inner_bounds: vec4<i32>,
};

// __MCLONE_TARGET_COLOR_TRANSFER_WGSL__
const terrain_target_color_transform: f32 = __MCLONE_TARGET_COLOR_TRANSFORM__;

@group(0) @binding(0)
var<uniform> params: TerrainPreviewParams;

struct TerrainExactCoverageParams {
    origin_size: vec4<i32>,
    mode_count_generation: vec4<u32>,
};

@group(1) @binding(0)
var<uniform> exact_coverage: TerrainExactCoverageParams;

@group(1) @binding(1)
var<storage, read> exact_coverage_words: array<u32>;

struct TreeInstance {
    @location(0) base_height: vec4<f32>,
    @location(1) crown_family: vec4<f32>,
    @location(2) panel_rank: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world_xz: vec2<f32>,
};

fn exact_chunk_painted(world_xz: vec2<f32>) -> bool {
    if exact_coverage.mode_count_generation.x == 0u
        || exact_coverage.origin_size.z <= 0
        || exact_coverage.origin_size.w <= 0 {
        return false;
    }
    let chunk = vec2<i32>(floor(world_xz / 16.0));
    let local = chunk - exact_coverage.origin_size.xy;
    if local.x < 0 || local.y < 0
        || local.x >= exact_coverage.origin_size.z
        || local.y >= exact_coverage.origin_size.w {
        return false;
    }
    let bit = u32(local.y) * 64u + u32(local.x);
    return (exact_coverage_words[bit / 32u] & (1u << (bit % 32u))) != 0u;
}

fn cube_corner(vertex: u32) -> vec3<f32> {
    let corners = array<vec3<f32>, 36>(
        vec3<f32>(-1.0, -1.0, 1.0), vec3<f32>(1.0, -1.0, 1.0), vec3<f32>(-1.0, 1.0, 1.0),
        vec3<f32>(-1.0, 1.0, 1.0), vec3<f32>(1.0, -1.0, 1.0), vec3<f32>(1.0, 1.0, 1.0),
        vec3<f32>(1.0, -1.0, -1.0), vec3<f32>(-1.0, -1.0, -1.0), vec3<f32>(1.0, 1.0, -1.0),
        vec3<f32>(1.0, 1.0, -1.0), vec3<f32>(-1.0, -1.0, -1.0), vec3<f32>(-1.0, 1.0, -1.0),
        vec3<f32>(-1.0, -1.0, -1.0), vec3<f32>(-1.0, -1.0, 1.0), vec3<f32>(-1.0, 1.0, -1.0),
        vec3<f32>(-1.0, 1.0, -1.0), vec3<f32>(-1.0, -1.0, 1.0), vec3<f32>(-1.0, 1.0, 1.0),
        vec3<f32>(1.0, -1.0, 1.0), vec3<f32>(1.0, -1.0, -1.0), vec3<f32>(1.0, 1.0, 1.0),
        vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(1.0, -1.0, -1.0), vec3<f32>(1.0, 1.0, -1.0),
        vec3<f32>(-1.0, 1.0, 1.0), vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(-1.0, 1.0, -1.0),
        vec3<f32>(-1.0, 1.0, -1.0), vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(1.0, 1.0, -1.0),
        vec3<f32>(-1.0, -1.0, -1.0), vec3<f32>(1.0, -1.0, -1.0), vec3<f32>(-1.0, -1.0, 1.0),
        vec3<f32>(-1.0, -1.0, 1.0), vec3<f32>(1.0, -1.0, -1.0), vec3<f32>(1.0, -1.0, 1.0),
    );
    return corners[vertex];
}

fn family_color(family: u32, trunk: bool) -> vec3<f32> {
    if trunk {
        return select(vec3<f32>(0.34, 0.20, 0.09), vec3<f32>(0.43, 0.26, 0.11), family == 3u);
    }
    if family == 2u {
        return vec3<f32>(0.10, 0.30, 0.18);
    }
    if family == 3u {
        return vec3<f32>(0.37, 0.50, 0.16);
    }
    return vec3<f32>(0.16, 0.45, 0.20);
}

@vertex
fn vertex_main(
    input: TreeInstance,
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    let family = u32(round(input.crown_family.z));
    let orientation = u32(round(input.crown_family.w)) & 3u;
    let box_index = vertex_index / 36u;
    let corner = cube_corner(vertex_index % 36u);
    let trunk_height = input.base_height.w;
    let crown_radius = input.crown_family.x;
    let crown_depth = input.crown_family.y;
    var center = vec3<f32>(
        input.base_height.x,
        input.base_height.y + trunk_height * 0.5,
        input.base_height.z,
    );
    var half_extent = vec3<f32>(0.34, trunk_height * 0.5, 0.34);
    var trunk = true;

    if box_index > 0u {
        trunk = false;
        center.y = input.base_height.y + trunk_height - crown_depth * 0.32;
        half_extent = vec3<f32>(crown_radius, max(crown_depth * 0.38, 0.5), crown_radius);
        if family == 2u {
            if box_index == 2u {
                center.y += crown_depth * 0.32;
                half_extent = vec3<f32>(
                    max(crown_radius * 0.58, 0.6),
                    max(crown_depth * 0.24, 0.5),
                    max(crown_radius * 0.58, 0.6),
                );
            }
        } else if family == 3u {
            let directions = array<vec2<f32>, 4>(
                vec2<f32>(0.0, -1.0),
                vec2<f32>(1.0, 0.0),
                vec2<f32>(0.0, 1.0),
                vec2<f32>(-1.0, 0.0),
            );
            let direction = directions[orientation];
            let side = vec2<f32>(-direction.y, direction.x);
            let offset = select(direction * 1.8, side * 1.4, box_index == 2u);
            center.x += offset.x;
            center.z += offset.y;
            center.y += select(0.0, -0.8, box_index == 2u);
            half_extent = vec3<f32>(crown_radius, 0.62, crown_radius);
        } else if box_index == 2u {
            center.y += crown_depth * 0.32;
            half_extent = vec3<f32>(
                max(crown_radius * 0.72, 0.7),
                max(crown_depth * 0.26, 0.5),
                max(crown_radius * 0.72, 0.7),
            );
        }
    }

    let world = center + corner * half_extent;
    let viewport_width = max(params.presentation_center_extent.z, 0.0001);
    let viewport_height = max(params.presentation_center_extent.w, 0.0001);
    let relative_x = world.x - f32(params.viewport_center_extent.x)
        - params.presentation_center_extent.x;
    let relative_z = world.z - f32(params.viewport_center_extent.y)
        - params.presentation_center_extent.y;
    let grid_x = relative_x / (viewport_width * 0.5);
    let grid_z = relative_z / (viewport_height * 0.5);
    var clip_x = grid_x;
    var clip_y = -grid_z;
    var clip_z = 0.58;
    if params.seed_source_view.w == 1u {
        let eye = vec3<f32>(
            params.camera_eye_target.x,
            params.camera_eye_target.y + params.camera_eye_target.w,
            params.camera_eye_target.z,
        );
        let camera_target = vec3<f32>(0.0, params.camera_eye_target.w, 0.0);
        let forward = normalize(camera_target - eye);
        let right = normalize(cross(forward, params.camera_up_fov.xyz));
        let camera_up = normalize(cross(right, forward));
        let position = vec3<f32>(
            relative_x,
            world.y,
            relative_z,
        );
        let from_eye = position - eye;
        let depth = max(dot(from_eye, forward), params.camera_projection.x);
        var half_height = params.camera_projection.w;
        if params.content_stage_flags.y == 1u {
            half_height = depth * tan(params.camera_up_fov.w * 0.5);
        }
        clip_x = dot(from_eye, right) / max(half_height * params.camera_projection.z, 0.001);
        clip_y = dot(from_eye, camera_up) / max(half_height, 0.001);
        clip_z = 1.0 - clamp(
            (depth - params.camera_projection.x)
                / (params.camera_projection.y - params.camera_projection.x),
            0.0,
            1.0,
        );
    }

    if params.seed_source_view.z == 2u {
        if params.content_stage_flags.z == 1u {
            let panel_center = select(0.5, -0.5, input.panel_rank.x > 0.5);
            clip_y = clip_y * 0.5 + panel_center;
        } else {
            let panel_center = select(-0.5, 0.5, input.panel_rank.x > 0.5);
            clip_x = clip_x * 0.5 + panel_center;
        }
    }

    var out: VertexOutput;
    out.position = vec4<f32>(clip_x, clip_y, clip_z, 1.0);
    let height_shade = clamp(0.78 + corner.y * 0.08, 0.62, 0.92);
    out.color = family_color(family, trunk) * height_shade;
    out.world_xz = world.xz;
    return out;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if params.clipmap_inner_bounds.z > params.clipmap_inner_bounds.x
        && params.clipmap_inner_bounds.w > params.clipmap_inner_bounds.y
        && input.world_xz.x >= f32(params.clipmap_inner_bounds.x)
        && input.world_xz.x < f32(params.clipmap_inner_bounds.z)
        && input.world_xz.y >= f32(params.clipmap_inner_bounds.y)
        && input.world_xz.y < f32(params.clipmap_inner_bounds.w) {
        discard;
    }
    let exact_painted = exact_chunk_painted(input.world_xz);
    var color = input.color;
    if exact_coverage.mode_count_generation.x == 2u && exact_painted {
        color = mix(color, vec3<f32>(1.0, 0.08, 0.72), 0.86);
    }
    return mclone_apply_target_color_transform_rgba(
        vec4<f32>(color, 1.0),
        terrain_target_color_transform,
    );
}
