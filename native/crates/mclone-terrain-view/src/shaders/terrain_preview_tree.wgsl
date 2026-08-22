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
    view_projection: mat4x4<f32>,
    fog_camera_position: vec4<f32>,
    fog_render_options: vec4<f32>,
    fog_color: vec4<f32>,
    fog_distances: vec4<f32>,
    view_projection_right: mat4x4<f32>,
    fog_camera_position_right: vec4<f32>,
    multiview_options: vec4<u32>,
};

struct TerrainPreviewSample {
    terrain: vec4<f32>,
    climate: vec4<f32>,
    large_fields: vec4<f32>,
    hydrology: vec4<f32>,
    hydrology_detail: vec4<f32>,
    semantics: vec4<f32>,
    forest_summary: vec4<f32>,
    forest_detail: vec4<f32>,
};

// __MCLONE_TARGET_COLOR_TRANSFER_WGSL__
const terrain_target_color_transform: f32 = __MCLONE_TARGET_COLOR_TRANSFORM__;
// MCLONE_FOG_FUNCTION

const TERRAIN_HORIZON_DIAGNOSTIC_NATURAL: u32 = 0u;
const TERRAIN_HORIZON_DIAGNOSTIC_OWNERSHIP_LEVEL: u32 = 1u;
const TERRAIN_HORIZON_DIAGNOSTIC_TOPOLOGY: u32 = 2u;
const TERRAIN_HORIZON_DIAGNOSTIC_ALBEDO: u32 = 3u;
const TERRAIN_HORIZON_DIAGNOSTIC_ENVIRONMENT: u32 = 4u;
const TERRAIN_HORIZON_DIAGNOSTIC_GEOMETRY: u32 = 5u;
const TERRAIN_HORIZON_DIAGNOSTIC_OCCLUSION: u32 = 6u;
const TERRAIN_HORIZON_DIAGNOSTIC_WATER: u32 = 7u;
const TERRAIN_HORIZON_DIAGNOSTIC_TEXTURE: u32 = 8u;
const TERRAIN_HORIZON_DIAGNOSTIC_FRONTIER_SUPPORT: u32 = 9u;

@group(0) @binding(0)
var<uniform> params: TerrainPreviewParams;

@group(0) @binding(1)
var<storage, read> terrain_samples: array<TerrainPreviewSample>;

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
    // RGB is unlit family albedo; A is the proxy's geometric height shade.
    @location(0) color: vec4<f32>,
    @location(1) world_xz: vec2<f32>,
    @location(2) world_position: vec3<f32>,
    @location(3) @interpolate(flat) view_index: u32,
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
    let bit = u32(local.y) * 65u + u32(local.x);
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
    if family == 4u {
        return vec3<f32>(0.08, 0.34, 0.12);
    }
    return vec3<f32>(0.16, 0.45, 0.20);
}

fn canopy_fan_position(vertex_in_cell: u32) -> vec2<f32> {
    let corner = vertex_in_cell / 3u;
    let vertex = vertex_in_cell % 3u;
    if vertex == 2u {
        return vec2<f32>(0.5, 0.5);
    }
    let corners = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
    );
    return corners[(corner + vertex) & 3u];
}

fn canopy_height_hash(world_x: i32, world_z: i32) -> f32 {
    var value = bitcast<u32>(world_x) * 0x9e3779b9u;
    value = value ^ (bitcast<u32>(world_z) * 0x85ebca6bu);
    value = value ^ (params.seed_source_view.x * 0xc2b2ae35u);
    value = value ^ (value >> 16u);
    value = value * 0x7feb352du;
    value = value ^ (value >> 15u);
    return f32(value & 1023u) / 1023.0;
}

fn canopy_vertex(vertex_index: u32, view_index: u32) -> VertexOutput {
    let canopy_cells_per_axis = 16u;
    let tile_cells = u32(params.origin_spacing_cells.w);
    let sample_stride = max(tile_cells / canopy_cells_per_axis, 1u);
    let canopy_cell = vertex_index / 12u;
    let canopy_x = canopy_cell % canopy_cells_per_axis;
    let canopy_z = canopy_cell / canopy_cells_per_axis;
    let local_position = canopy_fan_position(vertex_index % 12u);
    let center_sample_x = min(canopy_x * sample_stride + sample_stride / 2u, tile_cells);
    let center_sample_z = min(canopy_z * sample_stride + sample_stride / 2u, tile_cells);
    let center_index = center_sample_z * params.layer_samples_size.y + center_sample_x;
    let center_sample = terrain_samples[center_index];
    let shaped_position = local_position;
    let sample_x = min(
        canopy_x * sample_stride + u32(round(shaped_position.x * f32(sample_stride))),
        tile_cells,
    );
    let sample_z = min(
        canopy_z * sample_stride + u32(round(shaped_position.y * f32(sample_stride))),
        tile_cells,
    );
    let surface_index = sample_z * params.layer_samples_size.y + sample_x;
    let surface = terrain_samples[surface_index];
    let world_x = f32(params.origin_spacing_cells.x)
        + (f32(canopy_x * sample_stride) + shaped_position.x * f32(sample_stride))
            * f32(params.origin_spacing_cells.z);
    let world_z = f32(params.origin_spacing_cells.y)
        + (f32(canopy_z * sample_stride) + shaped_position.y * f32(sample_stride))
            * f32(params.origin_spacing_cells.z);
    let center_vertex = (vertex_index % 3u) == 2u;
    var coverage = clamp(
        center_sample.forest_summary.x
            * clamp(center_sample.forest_detail.w, 0.0, 1.0)
            * (1.0 - clamp(center_sample.climate.z, 0.0, 1.0)),
        0.0,
        1.0,
    );
    var mean_height = max(center_sample.forest_detail.x, 2.0);
    var variation = max(center_sample.forest_detail.y, 0.0);
    var family = u32(round(center_sample.forest_summary.z));
    var family_mix = clamp(center_sample.forest_summary.w, 0.0, 1.0);
    if !center_vertex {
        coverage = clamp(
            surface.forest_summary.x
                * clamp(surface.forest_detail.w, 0.0, 1.0)
                * (1.0 - clamp(surface.climate.z, 0.0, 1.0)),
            0.0,
            1.0,
        );
        mean_height = max(surface.forest_detail.x, 2.0);
        variation = max(surface.forest_detail.y, 0.0);
        family = u32(round(surface.forest_summary.z));
        family_mix = clamp(surface.forest_summary.w, 0.0, 1.0);
    }
    let height_noise = canopy_height_hash(i32(round(world_x)), i32(round(world_z)));
    let canopy_height = max(mean_height + (height_noise - 0.5) * variation, 1.5);
    let lift = canopy_height * smoothstep(0.10, 0.68, coverage);
    let crown_shape = select(0.82 + height_noise * 0.06, 1.0, center_vertex);
    let world = vec3<f32>(
        world_x,
        surface.terrain.y + 1.05 + lift * crown_shape,
        world_z,
    );
    let relative_x = world.x - f32(params.viewport_center_extent.x)
        - params.presentation_center_extent.x;
    let relative_z = world.z - f32(params.viewport_center_extent.y)
        - params.presentation_center_extent.y;
    var view_projection = params.view_projection;
    if view_index != 0u {
        view_projection = params.view_projection_right;
    }
    var clip_position = view_projection * vec4<f32>(
        vec3<f32>(relative_x, world.y, relative_z),
        1.0,
    );
    if (params.multiview_options.x & (1u << view_index)) == 0u {
        clip_position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    }
    var canopy_color = family_color(family, false);
    canopy_color = mix(canopy_color, vec3<f32>(0.38, 0.52, 0.18), family_mix * 0.36);
    canopy_color *= 0.72 + height_noise * 0.16;

    var out: VertexOutput;
    out.position = clip_position;
    out.color = vec4<f32>(canopy_color, coverage);
    out.world_xz = world.xz;
    out.world_position = world;
    out.view_index = view_index;
    return out;
}

fn terrain_horizon_level_color(sample_spacing: u32) -> vec3<f32> {
    let colors = array<vec3<f32>, 10>(
        vec3<f32>(0.95, 0.18, 0.12),
        vec3<f32>(1.00, 0.55, 0.08),
        vec3<f32>(0.92, 0.88, 0.12),
        vec3<f32>(0.30, 0.82, 0.18),
        vec3<f32>(0.08, 0.78, 0.72),
        vec3<f32>(0.08, 0.48, 0.96),
        vec3<f32>(0.30, 0.20, 0.92),
        vec3<f32>(0.68, 0.16, 0.90),
        vec3<f32>(0.95, 0.18, 0.62),
        vec3<f32>(0.75, 0.75, 0.75),
    );
    let level = min(u32(round(log2(max(f32(sample_spacing), 1.0)))), 9u);
    return colors[level];
}

fn full_sky_environmental_illumination() -> vec3<f32> {
    return vec3<f32>(
        params.fog_render_options.x,
        params.fog_render_options.y,
        params.fog_render_options.w,
    );
}

fn tree_vertex(
    input: TreeInstance,
    vertex_index: u32,
    view_index: u32,
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
        } else if family == 4u {
            let directions = array<vec2<f32>, 4>(
                vec2<f32>(0.0, -1.0),
                vec2<f32>(1.0, 0.0),
                vec2<f32>(0.0, 1.0),
                vec2<f32>(-1.0, 0.0),
            );
            let direction = directions[orientation];
            if box_index == 2u {
                center.x += direction.x * crown_radius * 0.34;
                center.z += direction.y * crown_radius * 0.34;
                center.y += crown_depth * 0.30;
                half_extent = vec3<f32>(
                    max(crown_radius * 0.74, 0.7),
                    max(crown_depth * 0.30, 0.7),
                    max(crown_radius * 0.74, 0.7),
                );
            } else {
                center.y -= crown_depth * 0.08;
                half_extent.y = max(crown_depth * 0.46, 0.8);
            }
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
    let relative_x = world.x - f32(params.viewport_center_extent.x)
        - params.presentation_center_extent.x;
    let relative_z = world.z - f32(params.viewport_center_extent.y)
        - params.presentation_center_extent.y;
    var view_projection = params.view_projection;
    if view_index != 0u {
        view_projection = params.view_projection_right;
    }
    var clip_position = view_projection * vec4<f32>(
        vec3<f32>(
            relative_x,
            world.y,
            relative_z,
        ),
        1.0,
    );
    if (params.multiview_options.x & (1u << view_index)) == 0u {
        clip_position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    }

    if params.seed_source_view.z == 2u {
        if params.content_stage_flags.z == 1u {
            let panel_center = select(0.5, -0.5, input.panel_rank.x > 0.5);
            clip_position.y = clip_position.y * 0.5
                + panel_center * clip_position.w;
        } else {
            let panel_center = select(-0.5, 0.5, input.panel_rank.x > 0.5);
            clip_position.x = clip_position.x * 0.5
                + panel_center * clip_position.w;
        }
    }

    var out: VertexOutput;
    out.position = clip_position;
    let height_shade = clamp(0.78 + corner.y * 0.08, 0.62, 0.92);
    out.color = vec4<f32>(family_color(family, trunk), height_shade);
    out.world_xz = world.xz;
    out.world_position = world;
    out.view_index = view_index;
    return out;
}

@vertex
fn vertex_main(
    input: TreeInstance,
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    return tree_vertex(input, vertex_index, 0u);
}

@vertex
fn canopy_vertex_main(
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    return canopy_vertex(vertex_index, 0u);
}

// __MCLONE_MULTIVIEW_VERTEX_ENTRY__

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
    let horizon_diagnostic = params.multiview_options.y;
    let environmental_illumination = full_sky_environmental_illumination();
    // V2 proxy trees converge into the continuous canopy by projected scale,
    // not by clipmap level. This prevents subpixel opaque crowns from drawing
    // a camera-centered stippled ring in continental views.
    let world_dx = dpdx(input.world_xz);
    let world_dy = dpdy(input.world_xz);
    let blocks_per_pixel = max(length(world_dx), length(world_dy));
    let v2_forest_representation = (params.content_stage_flags.w & 4u) != 0u;
    let proxy_opacity = select(
        1.0,
        1.0 - smoothstep(0.45, 2.25, blocks_per_pixel),
        v2_forest_representation,
    );
    if proxy_opacity < 0.01 {
        discard;
    }
    var color = input.color.rgb * input.color.a * environmental_illumination;
    if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_FRONTIER_SUPPORT {
        color = vec3<f32>(0.0);
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_OWNERSHIP_LEVEL {
        color = terrain_horizon_level_color(u32(params.origin_spacing_cells.z));
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_TOPOLOGY {
        color = vec3<f32>(0.94, 0.10, 0.72);
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_ALBEDO {
        color = input.color.rgb;
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_ENVIRONMENT {
        color = environmental_illumination;
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_GEOMETRY {
        color = vec3<f32>(input.color.a);
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_OCCLUSION {
        color = vec3<f32>(1.0);
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_WATER
        || horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_TEXTURE {
        color = vec3<f32>(0.0);
    }
    if exact_coverage.mode_count_generation.x == 2u && exact_painted {
        color = mix(color, vec3<f32>(1.0, 0.08, 0.72), 0.86);
    }
    if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_NATURAL {
        var fog_camera_position = params.fog_camera_position;
        if input.view_index != 0u {
            fog_camera_position = params.fog_camera_position_right;
        }
        let fog_factor = mclone_fog_factor(
            input.world_position,
            fog_camera_position,
            params.fog_render_options,
            params.fog_color,
            params.fog_distances,
        );
        color = mix(color, params.fog_color.rgb, fog_factor);
    }
    let diagnostic_owns_pixels = horizon_diagnostic != TERRAIN_HORIZON_DIAGNOSTIC_NATURAL
        && horizon_diagnostic != TERRAIN_HORIZON_DIAGNOSTIC_ALBEDO
        && horizon_diagnostic != TERRAIN_HORIZON_DIAGNOSTIC_GEOMETRY;
    let output_alpha = select(proxy_opacity, 1.0, diagnostic_owns_pixels);
    return mclone_apply_target_color_transform_rgba(
        vec4<f32>(color, output_alpha),
        terrain_target_color_transform,
    );
}


@fragment
fn canopy_fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if params.clipmap_inner_bounds.z > params.clipmap_inner_bounds.x
        && params.clipmap_inner_bounds.w > params.clipmap_inner_bounds.y
        && input.world_xz.x >= f32(params.clipmap_inner_bounds.x)
        && input.world_xz.x < f32(params.clipmap_inner_bounds.z)
        && input.world_xz.y >= f32(params.clipmap_inner_bounds.y)
        && input.world_xz.y < f32(params.clipmap_inner_bounds.w) {
        discard;
    }
    let exact_painted = exact_chunk_painted(input.world_xz);
    if exact_coverage.mode_count_generation.x == 1u && exact_painted {
        discard;
    }
    let horizon_diagnostic = params.multiview_options.y;
    let environmental_illumination = full_sky_environmental_illumination();
    // Canopy represents projected forest mass, not a categorical LOD style.
    // Every V2 level offers the same veil and screen derivatives raise its
    // contribution continuously as individual crowns become unresolvable.
    // Adjacent levels therefore agree at their shared projected boundary.
    let world_dx = dpdx(input.world_xz);
    let world_dy = dpdy(input.world_xz);
    let blocks_per_pixel = max(length(world_dx), length(world_dy));
    let projected_scale_weight = smoothstep(0.45, 2.25, blocks_per_pixel);
    let canopy_opacity = clamp(input.color.a * projected_scale_weight * 0.78, 0.0, 0.78);
    if canopy_opacity < 0.01 {
        discard;
    }
    var color = input.color.rgb * environmental_illumination;
    if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_FRONTIER_SUPPORT {
        color = vec3<f32>(0.0);
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_OWNERSHIP_LEVEL {
        color = terrain_horizon_level_color(u32(params.origin_spacing_cells.z));
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_TOPOLOGY {
        color = vec3<f32>(0.74, 0.18, 0.88);
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_ALBEDO {
        color = input.color.rgb;
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_ENVIRONMENT {
        color = environmental_illumination;
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_GEOMETRY {
        color = vec3<f32>(0.84);
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_OCCLUSION {
        color = vec3<f32>(1.0);
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_WATER
        || horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_TEXTURE {
        color = vec3<f32>(0.0);
    }
    if exact_coverage.mode_count_generation.x == 2u && exact_painted {
        color = mix(color, vec3<f32>(1.0, 0.08, 0.72), 0.86);
    }
    if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_NATURAL {
        var fog_camera_position = params.fog_camera_position;
        if input.view_index != 0u {
            fog_camera_position = params.fog_camera_position_right;
        }
        let fog_factor = mclone_fog_factor(
            input.world_position,
            fog_camera_position,
            params.fog_render_options,
            params.fog_color,
            params.fog_distances,
        );
        color = mix(color, params.fog_color.rgb, fog_factor);
    }
    let diagnostic_owns_pixels = horizon_diagnostic != TERRAIN_HORIZON_DIAGNOSTIC_NATURAL
        && horizon_diagnostic != TERRAIN_HORIZON_DIAGNOSTIC_ALBEDO
        && horizon_diagnostic != TERRAIN_HORIZON_DIAGNOSTIC_GEOMETRY;
    let output_alpha = select(canopy_opacity, 1.0, diagnostic_owns_pixels);
    return mclone_apply_target_color_transform_rgba(
        vec4<f32>(color, output_alpha),
        terrain_target_color_transform,
    );
}
