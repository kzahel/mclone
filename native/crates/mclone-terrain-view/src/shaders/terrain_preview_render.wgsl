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
// __MCLONE_SURFACE_COLUMN_PROFILE_WGSL__

const TERRAIN_HORIZON_NORMAL_EDGE_WEST: u32 = 0x08000000u;
const TERRAIN_HORIZON_NORMAL_EDGE_EAST: u32 = 0x10000000u;
const TERRAIN_HORIZON_NORMAL_EDGE_NORTH: u32 = 0x20000000u;
const TERRAIN_HORIZON_NORMAL_EDGE_SOUTH: u32 = 0x40000000u;
const TERRAIN_HORIZON_DIAGNOSTIC_NATURAL: u32 = 0u;
const TERRAIN_HORIZON_DIAGNOSTIC_OWNERSHIP_LEVEL: u32 = 1u;
const TERRAIN_HORIZON_DIAGNOSTIC_TOPOLOGY: u32 = 2u;
const TERRAIN_HORIZON_DIAGNOSTIC_ALBEDO: u32 = 3u;
const TERRAIN_HORIZON_DIAGNOSTIC_ENVIRONMENT: u32 = 4u;
const TERRAIN_HORIZON_DIAGNOSTIC_GEOMETRY: u32 = 5u;
const TERRAIN_HORIZON_DIAGNOSTIC_OCCLUSION: u32 = 6u;
const TERRAIN_HORIZON_DIAGNOSTIC_WATER: u32 = 7u;
const TERRAIN_HORIZON_DIAGNOSTIC_TEXTURE: u32 = 8u;
override terrain_sample_halo_radius: u32 = 0u;
override terrain_render_cell_stride: u32 = 1u;

@group(0) @binding(0)
var<uniform> params: TerrainPreviewParams;

@group(0) @binding(1)
var<storage, read> gpu_samples: array<TerrainPreviewSample>;

@group(0) @binding(2)
var<storage, read> reference_samples: array<TerrainPreviewSample>;

@group(0) @binding(3)
var<storage, read> normal_heights: array<f32>;

struct TerrainPreviewMaterialTable {
    top_uvs: array<vec4<f32>, 256>,
    side_uvs: array<vec4<f32>, 256>,
    tint_flags: array<vec4<f32>, 256>,
    grass_tints: array<vec4<f32>, 256>,
};

@group(1) @binding(0)
var material_atlas: texture_2d<f32>;

@group(1) @binding(1)
var material_sampler: sampler;

@group(1) @binding(2)
var<uniform> material_table: TerrainPreviewMaterialTable;

struct TerrainExactCoverageParams {
    origin_size: vec4<i32>,
    mode_count_generation: vec4<u32>,
    transition_origin_size: vec4<i32>,
    boundary_origin_size: vec4<i32>,
    options: vec4<u32>,
};

@group(2) @binding(0)
var<uniform> exact_coverage: TerrainExactCoverageParams;

@group(2) @binding(1)
var<storage, read> exact_coverage_words: array<u32>;

@group(2) @binding(2)
var exact_transition_field: texture_2d<f32>;

@group(2) @binding(3)
var exact_transition_sampler: sampler;

@group(2) @binding(4)
var exact_boundary_profile: texture_2d<u32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world_xz: vec2<f32>,
    @location(2) light: f32,
    @location(3) @interpolate(flat) material: u32,
    @location(4) @interpolate(flat) textured: u32,
    @location(5) river: vec4<f32>,
    @location(6) semantics: vec4<f32>,
    @location(7) world_position: vec4<f32>,
    @location(8) @interpolate(flat) biome: u32,
    @location(9) surface_y: f32,
    @location(10) @interpolate(flat) view_index: u32,
    @location(11) world_uv: vec2<f32>,
    @location(12) @interpolate(flat) surface_kind: u32,
    @location(13) @interpolate(flat) near_shell: u32,
    @location(14) @interpolate(flat) surface_recipe: u32,
    @location(15) @interpolate(flat) column_top_y: f32,
};

fn exact_chunk_masked(chunk: vec2<i32>) -> bool {
    if exact_coverage.mode_count_generation.x == 0u
        || exact_coverage.origin_size.z <= 0
        || exact_coverage.origin_size.w <= 0 {
        return false;
    }
    let local = chunk - exact_coverage.origin_size.xy;
    if local.x < 0 || local.y < 0
        || local.x >= exact_coverage.origin_size.z
        || local.y >= exact_coverage.origin_size.w {
        return false;
    }
    let bit = u32(local.y) * 64u + u32(local.x);
    return (exact_coverage_words[bit / 32u] & (1u << (bit % 32u))) != 0u;
}

fn exact_chunk_painted(world_xz: vec2<f32>) -> bool {
    return exact_chunk_masked(vec2<i32>(floor(world_xz / 16.0)));
}

fn direct_exact_handoff() -> bool {
    return exact_coverage.options.x == 1u;
}

fn exact_transition_weight(world_xz: vec2<f32>) -> f32 {
    if !direct_exact_handoff()
        || exact_coverage.mode_count_generation.x == 0u
        || exact_coverage.transition_origin_size.z <= 0
        || exact_coverage.transition_origin_size.w <= 0 {
        return 0.0;
    }
    let local_blocks = world_xz
        - vec2<f32>(exact_coverage.transition_origin_size.xy);
    let field_blocks = vec2<f32>(exact_coverage.transition_origin_size.zw) * 4.0;
    if any(local_blocks < vec2<f32>(0.0))
        || any(local_blocks >= field_blocks) {
        return 0.0;
    }
    let texture_size = vec2<f32>(textureDimensions(exact_transition_field));
    return textureSampleLevel(
        exact_transition_field,
        exact_transition_sampler,
        local_blocks / 4.0 / texture_size,
        0.0,
    ).x;
}

fn exact_boundary_column(block_xz: vec2<i32>) -> u32 {
    if !direct_exact_handoff()
        || exact_coverage.mode_count_generation.x == 0u
        || exact_coverage.boundary_origin_size.z <= 0
        || exact_coverage.boundary_origin_size.w <= 0 {
        return 0u;
    }
    let local = block_xz - exact_coverage.boundary_origin_size.xy;
    if any(local < vec2<i32>(0))
        || local.x >= exact_coverage.boundary_origin_size.z
        || local.y >= exact_coverage.boundary_origin_size.w {
        return 0u;
    }
    return textureLoad(exact_boundary_profile, local, 0).x;
}

fn exact_boundary_valid(packed: u32) -> bool {
    return (packed & 0x80000000u) != 0u;
}

fn exact_boundary_water(packed: u32) -> bool {
    return (packed & 0x01000000u) != 0u;
}

fn exact_boundary_top_y(packed: u32) -> f32 {
    let encoded = packed & 0xffffu;
    let signed_height = select(i32(encoded), i32(encoded) - 65536, encoded >= 32768u);
    return f32(signed_height);
}

fn grid_corner(vertex_in_cell: u32) -> vec2<u32> {
    switch vertex_in_cell {
        case 0u: { return vec2<u32>(0u, 0u); }
        case 1u: { return vec2<u32>(0u, 1u); }
        case 2u: { return vec2<u32>(1u, 0u); }
        case 3u: { return vec2<u32>(0u, 1u); }
        case 4u: { return vec2<u32>(1u, 1u); }
        default: { return vec2<u32>(1u, 0u); }
    }
}

fn base_sample(final_sample: TerrainPreviewSample) -> TerrainPreviewSample {
    var base_sample = final_sample;
    base_sample.terrain.x = final_sample.large_fields.x;
    base_sample.terrain.y = final_sample.large_fields.y;
    base_sample.climate.z = final_sample.large_fields.z;
    base_sample.hydrology_detail.w = final_sample.large_fields.w;
    return base_sample;
}

fn staged_gpu_sample(index: u32) -> TerrainPreviewSample {
    let gpu = gpu_samples[index];
    let reference = reference_samples[index];
    if params.content_stage_flags.x >= 2u && reference.semantics.x > 0.0 {
        return reference;
    }
    return gpu;
}

fn selected_sample(index: u32, instance_index: u32) -> TerrainPreviewSample {
    let source_mode = params.seed_source_view.z;
    var sample = staged_gpu_sample(index);
    if source_mode == 1u {
        sample = reference_samples[index];
    } else if source_mode == 2u && instance_index == 0u {
        sample = reference_samples[index];
    }
    if params.content_stage_flags.x == 0u {
        return base_sample(sample);
    }
    return sample;
}

fn preview_profile() -> u32 {
    return params.content_stage_flags.w & 1u;
}

fn surface_quality() -> u32 {
    return (params.content_stage_flags.w >> 1u) & 3u;
}

fn sample_halo_radius() -> i32 {
    return i32(terrain_sample_halo_radius);
}

fn normal_height_index(sample_x: i32, sample_z: i32) -> u32 {
    let radius = sample_halo_radius();
    let drawn_max = i32(params.layer_samples_size.y) - 1;
    let stored_x = sample_x + radius;
    let stored_z = sample_z + radius;
    let storage_samples_per_axis = u32(drawn_max + 1 + radius * 2);
    return u32(stored_z) * storage_samples_per_axis + u32(stored_x);
}

fn selected_grid_height(
    sample_x: i32,
    sample_z: i32,
    instance_index: u32,
) -> f32 {
    if terrain_sample_halo_radius > 0u {
        return normal_heights[normal_height_index(sample_x, sample_z)];
    }
    let samples_per_axis = params.layer_samples_size.y;
    let index = u32(sample_z) * samples_per_axis + u32(sample_x);
    return selected_sample(index, instance_index).terrain.y;
}

// A fine clipmap level owns the exact rectangular hole cut out of its parent.
// Along that rectangle the parent surface is linear between every other fine
// sample. Weld odd fine-edge vertices to that same interpolation so the two
// independently drawn heightfields share one geometric boundary.
fn terrain_horizon_geometry_height(
    sample_x: i32,
    sample_z: i32,
    cells: i32,
    cell_stride: i32,
    instance_index: u32,
) -> f32 {
    let original_height = selected_grid_height(sample_x, sample_z, instance_index);
    if sample_halo_radius() == 0 {
        return original_height;
    }
    let flags = params.content_stage_flags.w;
    let west_or_east = (
        ((flags & TERRAIN_HORIZON_NORMAL_EDGE_WEST) != 0u && sample_x == 0)
        || ((flags & TERRAIN_HORIZON_NORMAL_EDGE_EAST) != 0u && sample_x == cells)
    );
    let north_or_south = (
        ((flags & TERRAIN_HORIZON_NORMAL_EDGE_NORTH) != 0u && sample_z == 0)
        || ((flags & TERRAIN_HORIZON_NORMAL_EDGE_SOUTH) != 0u && sample_z == cells)
    );
    let rendered_x = sample_x / cell_stride;
    let rendered_z = sample_z / cell_stride;
    if west_or_east && (rendered_z & 1) != 0 {
        return 0.5 * (
            selected_grid_height(
                sample_x,
                sample_z - cell_stride,
                instance_index,
            )
            + selected_grid_height(
                sample_x,
                sample_z + cell_stride,
                instance_index,
            )
        );
    }
    if north_or_south && (rendered_x & 1) != 0 {
        return 0.5 * (
            selected_grid_height(
                sample_x - cell_stride,
                sample_z,
                instance_index,
            )
            + selected_grid_height(
                sample_x + cell_stride,
                sample_z,
                instance_index,
            )
        );
    }
    return original_height;
}

// The spacing-one voxel shell rounds its top independently of the smooth
// parent. Re-evaluate the stitched fine-edge endpoint that lies on the parent
// boundary so the reserved cardinal face terminates on the parent's actual
// piecewise-linear profile instead of another rounded fine sample.
fn terrain_horizon_parent_boundary_y(
    sample_x: i32,
    sample_z: i32,
    cells: i32,
    instance_index: u32,
) -> f32 {
    return terrain_horizon_geometry_height(
        sample_x,
        sample_z,
        cells,
        1,
        instance_index,
    ) + 1.0;
}

fn terrain_horizon_coarse_footprint_weight(
    sample_x: i32,
    sample_z: i32,
    cells: i32,
) -> f32 {
    if sample_halo_radius() == 0 {
        return 0.0;
    }
    let flags = params.content_stage_flags.w;
    var distance = 3;
    if (flags & TERRAIN_HORIZON_NORMAL_EDGE_WEST) != 0u {
        distance = min(distance, sample_x);
    }
    if (flags & TERRAIN_HORIZON_NORMAL_EDGE_EAST) != 0u {
        distance = min(distance, cells - sample_x);
    }
    if (flags & TERRAIN_HORIZON_NORMAL_EDGE_NORTH) != 0u {
        distance = min(distance, sample_z);
    }
    if (flags & TERRAIN_HORIZON_NORMAL_EDGE_SOUTH) != 0u {
        distance = min(distance, cells - sample_z);
    }
    return clamp((2.0 - f32(distance)) * 0.5, 0.0, 1.0);
}

fn terrain_horizon_near_material_weight(
    cell_x: u32,
    cell_z: u32,
    cells: u32,
) -> f32 {
    let flags = params.content_stage_flags.w;
    var distance = 32u;
    if (flags & TERRAIN_HORIZON_NORMAL_EDGE_WEST) != 0u {
        distance = min(distance, cell_x);
    }
    if (flags & TERRAIN_HORIZON_NORMAL_EDGE_EAST) != 0u {
        distance = min(distance, cells - 1u - cell_x);
    }
    if (flags & TERRAIN_HORIZON_NORMAL_EDGE_NORTH) != 0u {
        distance = min(distance, cell_z);
    }
    if (flags & TERRAIN_HORIZON_NORMAL_EDGE_SOUTH) != 0u {
        distance = min(distance, cells - 1u - cell_z);
    }
    return clamp((f32(distance) + 0.5) / 32.0, 0.0, 1.0);
}

fn rgb8(color: u32) -> vec3<f32> {
    return vec3<f32>(
        f32((color >> 16u) & 255u),
        f32((color >> 8u) & 255u),
        f32(color & 255u),
    ) / 255.0;
}

fn vanilla_grass_color(biome: u32) -> vec3<f32> {
    if biome == 0u || biome == 7u || biome == 24u
        || (biome >= 44u && biome <= 50u) {
        return rgb8(0x8eb971u);
    }
    if biome == 2u || biome == 17u || biome == 130u
        || biome == 35u || biome == 36u || biome == 163u || biome == 164u {
        return rgb8(0xb5b755u);
    }
    if biome == 3u || biome == 20u || biome == 25u || biome == 26u
        || biome == 34u || biome == 131u || biome == 162u {
        return rgb8(0x8ab689u);
    }
    if biome == 4u || biome == 18u || biome == 132u {
        return rgb8(0x79c05au);
    }
    if biome == 5u || biome == 19u || biome == 30u || biome == 31u
        || biome == 32u || biome == 33u || biome == 133u || biome == 158u
        || biome == 160u || biome == 161u {
        return rgb8(0x86b783u);
    }
    if biome == 6u || biome == 134u {
        return rgb8(0x647139u);
    }
    if biome == 10u || biome == 11u || biome == 12u || biome == 13u
        || biome == 140u {
        return rgb8(0x80b497u);
    }
    if biome == 14u || biome == 15u {
        return rgb8(0x55c93fu);
    }
    if (biome >= 21u && biome <= 23u) || biome == 149u || biome == 151u
        || biome == 168u || biome == 169u {
        return rgb8(0x59c93cu);
    }
    if (biome >= 27u && biome <= 28u) || biome == 155u || biome == 156u {
        return rgb8(0x88bb67u);
    }
    if biome == 29u || biome == 157u {
        return rgb8(0x507a32u);
    }
    if (biome >= 37u && biome <= 39u)
        || (biome >= 165u && biome <= 167u) {
        return rgb8(0x90814du);
    }
    return rgb8(0x91bd59u);
}
fn mclone_grass_biome(recipe: u32) -> u32 {
    switch recipe {
        // Keep this mapping aligned with
        // mclone_overworld_biome_id_for_sample.
        case 0u: { return 0u; }  // ocean
        case 1u: { return 16u; } // shore / beach
        case 2u: { return 7u; }  // river
        case 3u: { return 13u; } // snowy alpine
        case 4u: { return 5u; }  // cool wet conifer / taiga
        case 5u: { return 35u; } // warm dry steppe / savanna
        case 6u: { return 4u; }  // temperate woodland / forest
        default: { return 1u; }  // temperate meadow / plains
    }
}

fn water_surface_color(ground_y: f32, light: f32) -> vec3<f32> {
    let depth = clamp((63.0 - ground_y) / 52.0, 0.0, 1.0);
    return mix(
        vec3<f32>(0.16, 0.55, 0.68),
        vec3<f32>(0.025, 0.17, 0.34),
        depth,
    ) * light;
}

fn vanilla_mountain_exposure_biome(biome: u32) -> bool {
    return biome == 3u || biome == 20u || biome == 34u
        || biome == 131u || biome == 162u
        || biome == 163u || biome == 164u;
}

fn terrain_material(sample: TerrainPreviewSample) -> u32 {
    if sample.climate.z >= 0.5 {
        return 2u;
    }
    return u32(round(select(
        sample.large_fields.w,
        sample.hydrology_detail.w,
        surface_quality() >= 1u,
    )));
}

fn terrain_color(sample: TerrainPreviewSample, light: f32) -> vec3<f32> {
    let temperature = sample.climate.x;
    let moisture = sample.climate.y;
    let material = terrain_material(sample);
    if material == 2u {
        return water_surface_color(sample.terrain.x, light);
    }
    if material == 1u {
        return vec3<f32>(0.48, 0.49, 0.47) * light;
    }
    if material == 5u {
        return vec3<f32>(0.48, 0.36, 0.23) * light;
    }
    if material == 6u {
        return vec3<f32>(0.82, 0.76, 0.53) * light;
    }
    if material == 7u {
        return vec3<f32>(0.47, 0.46, 0.43) * light;
    }
    if material == 8u || material == 40u {
        return vec3<f32>(0.92, 0.95, 0.96) * light;
    }
    if material == 13u {
        return vec3<f32>(0.45, 0.35, 0.23) * light;
    }
    if material == 14u {
        return vec3<f32>(0.35, 0.27, 0.16) * light;
    }
    if material == 15u {
        return vec3<f32>(0.48, 0.39, 0.48) * light;
    }
    if material == 38u {
        return vec3<f32>(0.76, 0.34, 0.12) * light;
    }
    if material >= 16u && material <= 32u {
        return vec3<f32>(0.62, 0.36, 0.25) * light;
    }
    if material != 4u {
        return vec3<f32>(0.52, 0.48, 0.39) * light;
    }
    if preview_profile() == 1u {
        return vanilla_grass_color(u32(round(sample.semantics.y))) * light;
    }
    return vanilla_grass_color(
        mclone_grass_biome(u32(round(sample.semantics.y))),
    ) * light;
}

fn height_color(height: f32) -> vec3<f32> {
    let normalized = clamp((height - 20.0) / 140.0, 0.0, 1.0);
    let low = vec3<f32>(0.04, 0.14, 0.27);
    let middle = vec3<f32>(0.24, 0.66, 0.38);
    let high = vec3<f32>(0.96, 0.91, 0.72);
    if normalized < 0.5 {
        return mix(low, middle, normalized * 2.0);
    }
    return mix(middle, high, (normalized - 0.5) * 2.0);
}

fn continentalness_color(value: f32) -> vec3<f32> {
    if value <= 0.0 {
        return mix(
            vec3<f32>(0.025, 0.13, 0.35),
            vec3<f32>(0.17, 0.62, 0.73),
            clamp(value + 1.0, 0.0, 1.0),
        );
    }
    return mix(
        vec3<f32>(0.77, 0.71, 0.43),
        vec3<f32>(0.18, 0.48, 0.25),
        clamp(value, 0.0, 1.0),
    );
}

fn error_color(error: f32) -> vec3<f32> {
    let normalized = clamp(error / 32.0, 0.0, 1.0);
    if normalized < 0.25 {
        return mix(vec3<f32>(0.06, 0.24, 0.14), vec3<f32>(0.53, 0.76, 0.22), normalized * 4.0);
    }
    if normalized < 0.65 {
        return mix(
            vec3<f32>(0.53, 0.76, 0.22),
            vec3<f32>(0.97, 0.58, 0.12),
            (normalized - 0.25) / 0.40,
        );
    }
    return mix(
        vec3<f32>(0.97, 0.58, 0.12),
        vec3<f32>(0.82, 0.09, 0.38),
        (normalized - 0.65) / 0.35,
    );
}

fn vanilla_biome_color(biome: u32) -> vec3<f32> {
    if biome == 0u || biome == 10u || biome == 24u || (biome >= 44u && biome <= 50u) {
        return vec3<f32>(0.04, 0.28, 0.60);
    }
    if biome == 7u || biome == 11u {
        return vec3<f32>(0.05, 0.50, 0.88);
    }
    if biome == 2u || biome == 17u || (biome >= 37u && biome <= 39u)
        || biome == 130u || (biome >= 165u && biome <= 167u) {
        return vec3<f32>(0.82, 0.70, 0.42);
    }
    if biome == 12u || biome == 13u || biome == 26u || biome == 30u
        || biome == 31u || biome == 140u || biome == 158u {
        return vec3<f32>(0.90, 0.95, 0.98);
    }
    if biome == 6u || biome == 134u {
        return vec3<f32>(0.20, 0.42, 0.24);
    }
    if (biome >= 21u && biome <= 23u) || biome == 149u || biome == 151u
        || biome == 168u || biome == 169u {
        return vec3<f32>(0.08, 0.52, 0.20);
    }
    if biome == 14u || biome == 15u {
        return vec3<f32>(0.56, 0.24, 0.50);
    }
    if biome == 35u || biome == 36u || biome == 163u || biome == 164u {
        return vec3<f32>(0.65, 0.62, 0.25);
    }
    if biome == 3u || biome == 20u || biome == 25u || biome == 34u
        || biome == 131u || biome == 162u {
        return vec3<f32>(0.48, 0.49, 0.47);
    }
    if biome == 5u || biome == 19u || (biome >= 30u && biome <= 33u)
        || biome == 133u || biome == 160u || biome == 161u {
        return vec3<f32>(0.12, 0.38, 0.27);
    }
    if biome == 4u || biome == 18u || (biome >= 27u && biome <= 29u)
        || biome == 132u || (biome >= 155u && biome <= 157u) {
        return vec3<f32>(0.16, 0.46, 0.24);
    }
    if biome == 16u {
        return vec3<f32>(0.86, 0.82, 0.61);
    }
    return vec3<f32>(0.48, 0.68, 0.30);
}

fn sample_color(
    sample: TerrainPreviewSample,
    reference: TerrainPreviewSample,
    gpu: TerrainPreviewSample,
    light: f32,
) -> vec3<f32> {
    let layer = params.layer_samples_size.x;
    if layer == 1u {
        return height_color(sample.terrain.y);
    }
    if layer == 2u {
        var reference_stage = reference;
        var gpu_stage = gpu;
        if params.content_stage_flags.x == 0u {
            reference_stage = base_sample(reference);
            gpu_stage = base_sample(gpu);
        }
        return error_color(abs(reference_stage.terrain.x - gpu_stage.terrain.x));
    }
    if layer == 3u {
        return continentalness_color(sample.terrain.z);
    }
    if layer == 4u {
        let warmth = clamp(sample.climate.x * 0.5 + 0.5, 0.0, 1.0);
        let moisture = clamp(sample.climate.y * 0.5 + 0.5, 0.0, 1.0);
        return vec3<f32>(warmth, moisture, 1.0 - warmth * 0.65);
    }
    if layer == 5u {
        let channel = clamp(sample.hydrology.y, 0.0, 1.0);
        let bank = clamp(sample.hydrology.z, 0.0, 1.0);
        return mix(
            vec3<f32>(0.055, 0.075, 0.11),
            mix(vec3<f32>(0.76, 0.53, 0.18), vec3<f32>(0.04, 0.58, 0.94), channel),
            max(bank, channel),
        );
    }
    if layer == 6u {
        let wetland = clamp(sample.hydrology_detail.x, 0.0, 1.0);
        let pool = clamp(sample.hydrology_detail.y, 0.0, 1.0);
        return mix(
            vec3<f32>(0.055, 0.075, 0.11),
            mix(vec3<f32>(0.24, 0.65, 0.33), vec3<f32>(0.09, 0.73, 0.72), pool),
            max(wetland, pool),
        );
    }
    if layer == 7u {
        let biome = u32(round(sample.semantics.y));
        if preview_profile() == 1u {
            return vanilla_biome_color(biome) * light;
        }
        let colors = array<vec3<f32>, 8>(
            vec3<f32>(0.04, 0.28, 0.60),
            vec3<f32>(0.82, 0.70, 0.42),
            vec3<f32>(0.05, 0.50, 0.88),
            vec3<f32>(0.90, 0.95, 0.98),
            vec3<f32>(0.12, 0.38, 0.27),
            vec3<f32>(0.65, 0.51, 0.22),
            vec3<f32>(0.20, 0.52, 0.25),
            vec3<f32>(0.48, 0.68, 0.30),
        );
        return colors[min(biome, 7u)] * light;
    }
    if layer == 8u {
        if preview_profile() == 1u {
            return terrain_color(sample, light);
        }
        let recipe = clamp(sample.semantics.w / 8.0, 0.0, 1.0);
        return mix(vec3<f32>(0.06, 0.22, 0.38), vec3<f32>(0.94, 0.78, 0.42), recipe) * light;
    }
    if layer == 9u {
        let planned = clamp(sample.semantics.x, 0.0, 1.0);
        return mix(vec3<f32>(0.055, 0.075, 0.11), vec3<f32>(0.95, 0.20, 0.76), planned);
    }
    if layer == 10u {
        let landform = u32(round(sample.semantics.z));
        let colors = array<vec3<f32>, 9>(
            vec3<f32>(0.08, 0.22, 0.90),
            vec3<f32>(0.94, 0.76, 0.24),
            vec3<f32>(0.02, 0.78, 1.00),
            vec3<f32>(0.05, 0.68, 0.55),
            vec3<f32>(0.48, 0.78, 0.30),
            vec3<f32>(0.78, 0.58, 0.18),
            vec3<f32>(0.72, 0.32, 0.76),
            vec3<f32>(0.16, 0.62, 0.44),
            vec3<f32>(0.82, 0.82, 0.86),
        );
        return colors[min(landform, 8u)] * light;
    }
    if layer == 11u {
        let coverage = clamp(sample.forest_summary.x, 0.0, 1.0);
        let family = u32(round(sample.forest_summary.z));
        let family_colors = array<vec3<f32>, 4>(
            vec3<f32>(0.09, 0.11, 0.14),
            vec3<f32>(0.20, 0.76, 0.30),
            vec3<f32>(0.12, 0.55, 0.46),
            vec3<f32>(0.82, 0.66, 0.19),
        );
        let grove = clamp(sample.forest_detail.z, 0.0, 1.0);
        let family_mix = clamp(sample.forest_summary.w, 0.0, 1.0);
        var forest_color = family_colors[min(family, 3u)];
        forest_color *= 0.68 + grove * 0.32;
        forest_color = mix(forest_color, vec3<f32>(0.78, 0.84, 0.60), family_mix * 0.42);
        return mix(vec3<f32>(0.035, 0.055, 0.075), forest_color, coverage);
    }
    return terrain_color(sample, light);
}

fn terrain_vertex(
    vertex_index: u32,
    instance_index: u32,
    view_index: u32,
) -> VertexOutput {
    let cells = u32(params.origin_spacing_cells.w);
    let cell_stride = max(terrain_render_cell_stride, 1u);
    let render_cells = cells / cell_stride;
    let voxel_shell = !direct_exact_handoff()
        && sample_halo_radius() > 0
        && params.origin_spacing_cells.z == 1
        && cell_stride == 1u;
    let vertices_per_cell = select(6u, 30u, voxel_shell);
    let cell_index = vertex_index / vertices_per_cell;
    let cell_x = (cell_index % render_cells) * cell_stride;
    let cell_z = (cell_index / render_cells) * cell_stride;
    let vertex_in_cell = vertex_index % vertices_per_cell;
    let face_index = select(0u, vertex_in_cell / 6u, voxel_shell);
    let corner = grid_corner(vertex_in_cell % 6u);
    let smooth_sample_x = cell_x + corner.x * cell_stride;
    let smooth_sample_z = cell_z + corner.y * cell_stride;
    let sample_x = select(smooth_sample_x, cell_x, voxel_shell);
    let sample_z = select(smooth_sample_z, cell_z, voxel_shell);
    let logical_x = i32(sample_x);
    let logical_z = i32(sample_z);
    let index = sample_z * params.layer_samples_size.y + sample_x;
    let sample = selected_sample(index, instance_index);
    let reference = reference_samples[index];
    let gpu = gpu_samples[index];
    let stitched_height = terrain_horizon_geometry_height(
        logical_x,
        logical_z,
        i32(cells),
        i32(cell_stride),
        instance_index,
    );

    let radius = sample_halo_radius();
    let cells_i = i32(cells);
    let cell_stride_i = i32(cell_stride);
    let left_x = max(logical_x - cell_stride_i, -radius);
    let right_x = min(logical_x + cell_stride_i, cells_i + radius);
    let north_z = max(logical_z - cell_stride_i, -radius);
    let south_z = min(logical_z + cell_stride_i, cells_i + radius);
    let left = terrain_horizon_geometry_height(
        left_x,
        logical_z,
        cells_i,
        cell_stride_i,
        instance_index,
    );
    let right = terrain_horizon_geometry_height(
        right_x,
        logical_z,
        cells_i,
        cell_stride_i,
        instance_index,
    );
    let north = terrain_horizon_geometry_height(
        logical_x,
        north_z,
        cells_i,
        cell_stride_i,
        instance_index,
    );
    let south = terrain_horizon_geometry_height(
        logical_x,
        south_z,
        cells_i,
        cell_stride_i,
        instance_index,
    );
    let sample_spacing = max(f32(params.origin_spacing_cells.z), 1.0);
    let narrow_slope_x = (right - left)
        / max(f32(right_x - left_x) * sample_spacing, 1.0);
    let narrow_slope_z = (south - north)
        / max(f32(south_z - north_z) * sample_spacing, 1.0);
    let coarse_footprint_weight = terrain_horizon_coarse_footprint_weight(
        logical_x,
        logical_z,
        cells_i,
    );
    var slope_x = narrow_slope_x;
    var slope_z = narrow_slope_z;
    if coarse_footprint_weight > 0.0 {
        let wide_left_x = max(logical_x - 2 * cell_stride_i, -radius);
        let wide_right_x = min(logical_x + 2 * cell_stride_i, cells_i + radius);
        let wide_north_z = max(logical_z - 2 * cell_stride_i, -radius);
        let wide_south_z = min(logical_z + 2 * cell_stride_i, cells_i + radius);
        let wide_left = terrain_horizon_geometry_height(
            wide_left_x,
            logical_z,
            cells_i,
            cell_stride_i,
            instance_index,
        );
        let wide_right = terrain_horizon_geometry_height(
            wide_right_x,
            logical_z,
            cells_i,
            cell_stride_i,
            instance_index,
        );
        let wide_north = terrain_horizon_geometry_height(
            logical_x,
            wide_north_z,
            cells_i,
            cell_stride_i,
            instance_index,
        );
        let wide_south = terrain_horizon_geometry_height(
            logical_x,
            wide_south_z,
            cells_i,
            cell_stride_i,
            instance_index,
        );
        let wide_slope_x = (wide_right - wide_left)
            / max(f32(wide_right_x - wide_left_x) * sample_spacing, 1.0);
        let wide_slope_z = (wide_south - wide_north)
            / max(f32(wide_south_z - wide_north_z) * sample_spacing, 1.0);
        slope_x = mix(narrow_slope_x, wide_slope_x, coarse_footprint_weight);
        slope_z = mix(narrow_slope_z, wide_slope_z, coarse_footprint_weight);
    }
    let normal = normalize(vec3<f32>(-slope_x * 4.0, 1.0, -slope_z * 4.0));
    let smooth_geometric_shade = clamp(
        dot(normal, normalize(vec3<f32>(-0.45, 0.82, -0.35))) * 0.48 + 0.58,
        0.34,
        1.05,
    );
    var light = smooth_geometric_shade;

    let cell_world_x = params.origin_spacing_cells.x
        + i32(cell_x) * params.origin_spacing_cells.z;
    let cell_world_z = params.origin_spacing_cells.y
        + i32(cell_z) * params.origin_spacing_cells.z;
    var vertex_world_x = f32(params.origin_spacing_cells.x
        + i32(smooth_sample_x) * params.origin_spacing_cells.z);
    var vertex_world_z = f32(params.origin_spacing_cells.y
        + i32(smooth_sample_z) * params.origin_spacing_cells.z);
    var vertex_world_y = stitched_height + 1.0;
    var world_uv = vec2<f32>(vertex_world_x, vertex_world_z);
    var surface_kind = 0u;
    var vertex_material = terrain_material(sample);
    let voxel_smooth_transition_weight = select(
        0.0,
        terrain_horizon_near_material_weight(cell_x, cell_z, cells),
        voxel_shell,
    );
    var appearance_transition_weight = voxel_smooth_transition_weight;
    if direct_exact_handoff() {
        appearance_transition_weight = exact_transition_weight(vec2<f32>(
            vertex_world_x,
            vertex_world_z,
        ));
    }


    if direct_exact_handoff()
        && params.origin_spacing_cells.z == 1
        && cell_stride == 1u
        && vertex_material != 2u
        && !exact_chunk_painted(vec2<f32>(
            f32(cell_world_x) + 0.5,
            f32(cell_world_z) + 0.5,
        )) {
        let boundary_profiles = array<u32, 4>(
            exact_boundary_column(vec2<i32>(cell_world_x - 1, cell_world_z)),
            exact_boundary_column(vec2<i32>(cell_world_x + 1, cell_world_z)),
            exact_boundary_column(vec2<i32>(cell_world_x, cell_world_z - 1)),
            exact_boundary_column(vec2<i32>(cell_world_x, cell_world_z + 1)),
        );
        let corner_touches_side = array<bool, 4>(
            corner.x == 0u,
            corner.x == 1u,
            corner.y == 0u,
            corner.y == 1u,
        );
        var connector_top_y = 0.0;
        var connector_vertex = false;
        for (var side = 0u; side < 4u; side += 1u) {
            let packed = boundary_profiles[side];
            if exact_boundary_valid(packed) && !exact_boundary_water(packed) {
                if (packed & 0x02000000u) != 0u {
                    vertex_material = (packed >> 16u) & 0xffu;
                }
                if corner_touches_side[side] {
                    let top_y = exact_boundary_top_y(packed);
                    connector_top_y = select(
                        max(connector_top_y, top_y),
                        top_y,
                        !connector_vertex,
                    );
                    connector_vertex = true;
                }
            }
        }
        if connector_vertex {
            vertex_world_y = connector_top_y;
            surface_kind = 3u;
        }
    }

    if voxel_shell {
        let top_y = round(stitched_height) + 1.0;
        vertex_world_x = f32(cell_world_x) + f32(corner.x);
        vertex_world_z = f32(cell_world_z) + f32(corner.y);
        vertex_world_y = top_y;
        world_uv = vec2<f32>(vertex_world_x, vertex_world_z);
        light = 1.0;

        if face_index != 0u {
            var neighbor_x = i32(cell_x);
            var neighbor_z = i32(cell_z);
            if face_index == 1u {
                neighbor_x -= 1;
            } else if face_index == 2u {
                neighbor_x += 1;
            } else if face_index == 3u {
                neighbor_z -= 1;
            } else {
                neighbor_z += 1;
            }
            let neighbor_y = round(terrain_horizon_geometry_height(
                neighbor_x,
                neighbor_z,
                i32(cells),
                1,
                instance_index,
            )) + 1.0;
            let discard_exact = exact_coverage.mode_count_generation.x == 1u;
            let current_exact = discard_exact && exact_chunk_painted(vec2<f32>(
                f32(cell_world_x) + 0.5,
                f32(cell_world_z) + 0.5,
            ));
            let neighbor_exact = discard_exact && exact_chunk_painted(vec2<f32>(
                f32(cell_world_x + (neighbor_x - i32(cell_x))) + 0.5,
                f32(cell_world_z + (neighbor_z - i32(cell_z))) + 0.5,
            ));
            let flags = params.content_stage_flags.w;
            let outer_edge = (
                face_index == 1u
                    && cell_x == 0u
                    && (flags & TERRAIN_HORIZON_NORMAL_EDGE_WEST) != 0u
            ) || (
                face_index == 2u
                    && cell_x + 1u == cells
                    && (flags & TERRAIN_HORIZON_NORMAL_EDGE_EAST) != 0u
            ) || (
                face_index == 3u
                    && cell_z == 0u
                    && (flags & TERRAIN_HORIZON_NORMAL_EDGE_NORTH) != 0u
            ) || (
                face_index == 4u
                    && cell_z + 1u == cells
                    && (flags & TERRAIN_HORIZON_NORMAL_EDGE_SOUTH) != 0u
            );
            var bottom_y = top_y;
            var upper_y = top_y;
            var parent_boundary_y = top_y;
            var use_parent_boundary = false;
            if !current_exact && neighbor_exact && terrain_material(sample) != 2u {
                // The exact coverage contract currently supplies readiness but
                // not a complete surface profile. Keep a bounded curtain on
                // the procedural side of the ownership plane as the explicit
                // fallback connector.
                bottom_y = top_y - 32.0;
                surface_kind = 2u;
            } else if !current_exact && outer_edge {
                // The top is block-rounded, but the adjacent spacing-two mesh
                // consumes the continuous stitched parent profile. Connect to
                // that profile at this segment endpoint. Taking endpoint
                // envelopes preserves the cardinal face winding even where
                // the two profiles cross inside one block-wide segment.
                // The camera-centered fine clipmap observes this perimeter
                // from inside. Reverse only the connector's horizontal
                // endpoint order so its front face points inward; ordinary
                // voxel risers retain their outward cardinal winding.
                let endpoint = 1 - i32(corner.x);
                var parent_sample_x = i32(cell_x);
                var parent_sample_z = i32(cell_z);
                if face_index == 1u {
                    parent_sample_x = 0;
                    parent_sample_z += 1 - endpoint;
                } else if face_index == 2u {
                    parent_sample_x = i32(cells);
                    parent_sample_z += endpoint;
                } else if face_index == 3u {
                    parent_sample_x += endpoint;
                    parent_sample_z = 0;
                } else {
                    parent_sample_x += 1 - endpoint;
                    parent_sample_z = i32(cells);
                }
                parent_boundary_y = terrain_horizon_parent_boundary_y(
                    parent_sample_x,
                    parent_sample_z,
                    i32(cells),
                    instance_index,
                );
                use_parent_boundary = true;
                surface_kind = 1u;
            } else if !current_exact && top_y > neighbor_y {
                bottom_y = neighbor_y;
                surface_kind = 1u;
            }

            let cardinal_horizontal = f32(corner.x);
            let horizontal = select(
                cardinal_horizontal,
                1.0 - cardinal_horizontal,
                use_parent_boundary,
            );
            if use_parent_boundary {
                bottom_y = min(top_y, parent_boundary_y);
                upper_y = max(top_y, parent_boundary_y);
            }
            vertex_world_y = mix(bottom_y, upper_y, f32(corner.y));
            if face_index == 1u {
                vertex_world_x = f32(cell_world_x)
                    + select(0.0, 0.001, neighbor_exact);
                vertex_world_z = f32(cell_world_z) + 1.0 - horizontal;
                light = 0.6;
            } else if face_index == 2u {
                vertex_world_x = f32(cell_world_x) + 1.0
                    - select(0.0, 0.001, neighbor_exact);
                vertex_world_z = f32(cell_world_z) + horizontal;
                light = 0.6;
            } else if face_index == 3u {
                vertex_world_x = f32(cell_world_x) + horizontal;
                vertex_world_z = f32(cell_world_z)
                    + select(0.0, 0.001, neighbor_exact);
                light = 0.8;
            } else {
                vertex_world_x = f32(cell_world_x) + 1.0 - horizontal;
                vertex_world_z = f32(cell_world_z) + 1.0
                    - select(0.0, 0.001, neighbor_exact);
                light = 0.8;
            }
            world_uv = select(
                vec2<f32>(vertex_world_z, vertex_world_y),
                vec2<f32>(vertex_world_x, vertex_world_y),
                face_index >= 3u,
            );
        }
        // Keep one voxel geometry owner while its face shade approaches the
        // smooth owner's slope shade over the same committed outer-footprint
        // band already used by material presentation.
        light = mix(smooth_geometric_shade, light, voxel_smooth_transition_weight);
    }

    let relative_x = vertex_world_x - f32(params.viewport_center_extent.x)
        - params.presentation_center_extent.x;
    let relative_z = vertex_world_z - f32(params.viewport_center_extent.y)
        - params.presentation_center_extent.y;
    let compare = params.seed_source_view.z == 2u;
    let stacked_compare = compare && params.content_stage_flags.z == 1u;
    var view_projection = params.view_projection;
    if view_index != 0u {
        view_projection = params.view_projection_right;
    }
    var clip_position = view_projection * vec4<f32>(
        vec3<f32>(
            relative_x,
            vertex_world_y,
            relative_z,
        ),
        1.0,
    );
    if (params.multiview_options.x & (1u << view_index)) == 0u {
        clip_position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    }
    if compare {
        if stacked_compare {
            let panel_center = select(0.5, -0.5, instance_index == 1u);
            clip_position.y = clip_position.y * 0.5
                + panel_center * clip_position.w;
        } else {
            let panel_center = select(-0.5, 0.5, instance_index == 1u);
            clip_position.x = clip_position.x * 0.5
                + panel_center * clip_position.w;
        }
    }

    var out: VertexOutput;
    out.position = clip_position;
    out.color = sample_color(sample, reference, gpu, light);
    out.world_xz = vec2<f32>(vertex_world_x, vertex_world_z);
    out.light = light;
    out.material = vertex_material;
    out.textured = select(
        0u,
        1u,
        params.layer_samples_size.x == 0u && preview_profile() == 0u,
    );
    out.river = vec4<f32>(
        sample.hydrology.x,
        sample.hydrology.w,
        sample.hydrology.y,
        sample.terrain.x,
    );
    out.semantics = vec4<f32>(
        sample.hydrology_detail.x,
        sample.hydrology_detail.y,
        sample.semantics.x,
        select(0.0, sample.forest_summary.x, params.content_stage_flags.x >= 4u),
    );
    out.world_position = vec4<f32>(
        vertex_world_x,
        vertex_world_y,
        vertex_world_z,
        appearance_transition_weight,
    );
    out.biome = u32(round(sample.semantics.y));
    out.surface_y = sample.terrain.x;
    out.view_index = view_index;
    out.world_uv = world_uv;
    out.surface_kind = surface_kind;
    out.near_shell = select(0u, 1u, voxel_shell);
    out.surface_recipe = u32(round(sample.semantics.w));
    out.column_top_y = select(stitched_height + 1.0, round(stitched_height) + 1.0, voxel_shell);
    return out;
}

fn apply_material_texture(
    base_color: vec3<f32>,
    material: u32,
    side_surface: bool,
    world_uv: vec2<f32>,
    world_dx: vec2<f32>,
    world_dy: vec2<f32>,
    blocks_per_pixel: f32,
    exact_weight: f32,
) -> vec3<f32> {
    let sprite = select(
        material_table.top_uvs[material],
        material_table.side_uvs[material],
        side_surface,
    );
    let sprite_size = sprite.zw - sprite.xy;
    let local_uv = fract(world_uv);
    let atlas_uv = sprite.xy + local_uv * sprite_size;
    let atlas_dx = world_dx * sprite_size;
    let atlas_dy = world_dy * sprite_size;
    let texel = textureSampleGrad(
        material_atlas,
        material_sampler,
        atlas_uv,
        atlas_dx,
        atlas_dy,
    );
    let far_texture_weight = mix(
        0.82,
        0.42,
        clamp((blocks_per_pixel - 1.0) / 7.0, 0.0, 1.0),
    );
    let resolved_exact_weight = clamp(exact_weight, 0.0, 1.0);
    let texture_weight = mix(far_texture_weight, 1.0, resolved_exact_weight);
    let texture_detail = mix(
        clamp(texel.rgb * 1.25, vec3<f32>(0.0), vec3<f32>(1.5)),
        texel.rgb,
        resolved_exact_weight,
    );
    return base_color * mix(vec3<f32>(1.0), texture_detail, texture_weight);
}

fn material_texture_weight(blocks_per_pixel: f32, exact_weight: f32) -> f32 {
    return mix(
        mix(
            0.82,
            0.42,
            clamp((blocks_per_pixel - 1.0) / 7.0, 0.0, 1.0),
        ),
        1.0,
        clamp(exact_weight, 0.0, 1.0),
    );
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

fn resolved_surface_material(input: VertexOutput) -> u32 {
    if input.near_shell == 0u || input.surface_kind == 0u {
        return input.material;
    }
    let profile = mclone_preview_column_profile(input.material, input.surface_recipe);
    let depth = u32(max(floor(input.column_top_y - input.world_position.y + 0.0001), 0.0));
    if depth == 0u {
        return profile.x;
    }
    if depth < profile.z {
        return profile.y;
    }
    return profile.w;
}

fn material_uses_grass_tint(material: u32, side_surface: bool) -> bool {
    let flags = material_table.tint_flags[material];
    return select(flags.x, flags.y, side_surface) >= 0.5;
}

fn surface_tint(input: VertexOutput, material: u32, side_surface: bool) -> vec3<f32> {
    if !material_uses_grass_tint(material, side_surface) {
        return vec3<f32>(1.0);
    }
    let biome = select(
        input.biome,
        mclone_grass_biome(input.biome),
        preview_profile() == 0u,
    );
    return material_table.grass_tints[min(biome, 255u)].rgb;
}

fn full_sky_environmental_illumination() -> vec3<f32> {
    return vec3<f32>(
        params.fog_render_options.x,
        params.fog_render_options.y,
        params.fog_render_options.w,
    );
}

@vertex
fn vertex_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    return terrain_vertex(vertex_index, instance_index, 0u);
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
    if exact_coverage.mode_count_generation.x == 1u
        && input.material != 2u
        && exact_chunk_painted(input.world_xz) {
        discard;
    }
    let world_dx = dpdx(input.world_xz);
    let world_dy = dpdy(input.world_xz);
    let blocks_per_pixel = max(length(world_dx), length(world_dy));
    let material_dx = dpdx(input.world_uv);
    let material_dy = dpdy(input.world_uv);
    let material_blocks_per_pixel = max(length(material_dx), length(material_dy));
    let river_anti_alias = max(fwidth(input.river.x), blocks_per_pixel * 0.35);
    let pool_anti_alias = max(fwidth(input.semantics.y), 0.01);
    let physical_channel_edge = max(fwidth(input.river.z), 0.001);
    let horizon_diagnostic = params.multiview_options.y;
    let albedo_diagnostic = horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_ALBEDO;
    let environmental_illumination = full_sky_environmental_illumination();
    var color = input.color;
    var albedo = input.color / max(input.light, 0.001);
    var diagnostic_river_alpha = 0.0;
    var diagnostic_pool_alpha = 0.0;
    let face_normal = normalize(cross(
        dpdx(input.world_position.xyz),
        dpdy(input.world_position.xyz),
    ));
    let steep_mountain_face = abs(face_normal.y) < 0.80
        && vanilla_mountain_exposure_biome(input.biome);
    let grass_family = input.material == 4u || input.material == 5u
        || input.material == 13u || input.material == 14u;
    // Resolve grass from the same active-pack tint table on both sides of the
    // voxel-to-smooth boundary. The vertex color remains the fallback for
    // untinted materials, whose far response deliberately retains a stable
    // low-frequency approximation as texture detail recedes.
    if input.textured != 0u && input.material < 256u
        && material_uses_grass_tint(input.material, false) {
        albedo = surface_tint(input, input.material, false);
        color = albedo * input.light;
    }
    if preview_profile() == 1u && surface_quality() >= 1u
        && steep_mountain_face && grass_family {
        color = vec3<f32>(0.48, 0.49, 0.47) * input.light;
        albedo = vec3<f32>(0.48, 0.49, 0.47);
    }
    let side_surface = input.surface_kind != 0u;
    let display_material = resolved_surface_material(input);
    if input.textured != 0u
        && (input.near_shell != 0u || direct_exact_handoff())
        && display_material < 256u {
        var far_color = color;
        var far_albedo = albedo;
        if input.material < 256u {
            far_color = apply_material_texture(
                far_color,
                input.material,
                false,
                input.world_uv,
                material_dx,
                material_dy,
                material_blocks_per_pixel,
                0.0,
            );
            if albedo_diagnostic {
                far_albedo = apply_material_texture(
                    far_albedo,
                    input.material,
                    false,
                    input.world_uv,
                    material_dx,
                    material_dy,
                    material_blocks_per_pixel,
                    0.0,
                );
            }
        }
        var near_color = far_color;
        var near_albedo = far_albedo;
        if display_material != 2u {
            near_color = surface_tint(input, display_material, side_surface)
                * input.light;
            near_color = apply_material_texture(
                near_color,
                display_material,
                side_surface,
                input.world_uv,
                material_dx,
                material_dy,
                material_blocks_per_pixel,
                1.0,
            );
            if albedo_diagnostic {
                near_albedo = apply_material_texture(
                    surface_tint(input, display_material, side_surface),
                    display_material,
                    side_surface,
                    input.world_uv,
                    material_dx,
                    material_dy,
                    material_blocks_per_pixel,
                    1.0,
                );
            }
        }
        color = mix(far_color, near_color, input.world_position.w);
        if albedo_diagnostic {
            albedo = mix(far_albedo, near_albedo, input.world_position.w);
        }
    } else if input.textured != 0u && input.material < 256u {
        color = apply_material_texture(
            color,
            input.material,
            false,
            input.world_uv,
            material_dx,
            material_dy,
            material_blocks_per_pixel,
            0.0,
        );
        if albedo_diagnostic {
            albedo = apply_material_texture(
                albedo,
                input.material,
                false,
                input.world_uv,
                material_dx,
                material_dy,
                material_blocks_per_pixel,
                0.0,
            );
        }
    }
    if input.textured != 0u
        && params.content_stage_flags.x >= 1u
        && input.material != 2u
        && input.surface_kind == 0u
        && preview_profile() == 0u {
        let visible_half_width = max(input.river.y, blocks_per_pixel * 0.70);
        let river_distance_alpha = 1.0 - smoothstep(
            visible_half_width,
            visible_half_width + river_anti_alias,
            abs(input.river.x),
        );
        let physical_channel_alpha = smoothstep(
            0.0,
            physical_channel_edge,
            input.river.z,
        );
        let continental_channel_alpha = select(
            0.0,
            physical_channel_alpha,
            input.river.w > 0.0,
        );
        let river_alpha = river_distance_alpha * continental_channel_alpha;
        diagnostic_river_alpha = river_alpha;
        var river_color = color;
        var river_albedo = albedo;
        if input.material != 2u {
            river_color = apply_material_texture(
                water_surface_color(input.surface_y, input.light),
                2u,
                false,
                input.world_xz,
                world_dx,
                world_dy,
                blocks_per_pixel,
                input.world_position.w,
            );
            if albedo_diagnostic {
                river_albedo = apply_material_texture(
                    water_surface_color(input.surface_y, 1.0),
                    2u,
                    false,
                    input.world_xz,
                    world_dx,
                    world_dy,
                    blocks_per_pixel,
                    input.world_position.w,
                );
            }
        }
        color = mix(color, river_color, river_alpha);
        if albedo_diagnostic {
            albedo = mix(albedo, river_albedo, river_alpha);
        }
        let pool_alpha = smoothstep(
            0.55 - pool_anti_alias,
            0.55 + pool_anti_alias,
            input.semantics.y,
        );
        diagnostic_pool_alpha = pool_alpha;
        let water_alpha = max(river_alpha, pool_alpha);
        color = mix(
            color,
            water_surface_color(input.river.w, input.light),
            water_alpha * 0.88,
        );
        if albedo_diagnostic {
            albedo = mix(
                albedo,
                water_surface_color(input.river.w, 1.0),
                water_alpha * 0.88,
            );
        }
        if params.content_stage_flags.x >= 4u {
            let cover = clamp(input.semantics.w, 0.0, 1.0);
            color = mix(color, color * vec3<f32>(0.57, 0.82, 0.58), cover * 0.36);
            if albedo_diagnostic {
                albedo = mix(
                    albedo,
                    albedo * vec3<f32>(0.57, 0.82, 0.58),
                    cover * 0.36,
                );
            }
        }
    }
    // Environment is independent of material, water ownership, and clipmap
    // topology. Apply the exact renderer's full-sky/zero-block-light term once
    // after all albedo and geometric-shade composition.
    color *= environmental_illumination;
    if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_OWNERSHIP_LEVEL {
        color = terrain_horizon_level_color(u32(params.origin_spacing_cells.z));
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_TOPOLOGY {
        color = vec3<f32>(0.08, 0.42, 0.95);
        if input.near_shell != 0u {
            color = vec3<f32>(0.18, 0.86, 0.22);
            if input.surface_kind == 1u {
                color = vec3<f32>(1.0, 0.55, 0.06);
            } else if input.surface_kind == 2u {
                color = vec3<f32>(0.96, 0.06, 0.72);
            }
        }
        if input.surface_kind == 3u {
            color = vec3<f32>(0.04, 0.94, 0.82);
        }
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_ALBEDO {
        color = albedo;
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_ENVIRONMENT {
        color = environmental_illumination;
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_GEOMETRY {
        color = vec3<f32>(input.light);
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_OCCLUSION {
        // Procedural terrain currently has no local AO term. White is the
        // identity multiplier and makes that absence explicit at the seam.
        color = vec3<f32>(1.0);
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_WATER {
        color = vec3<f32>(0.035);
        if input.material == 2u {
            let bed_depth = clamp((63.0 - input.surface_y) / 32.0, 0.0, 1.0);
            color = mix(
                vec3<f32>(0.06, 0.46, 0.92),
                vec3<f32>(0.08, 0.12, 0.48),
                bed_depth,
            );
        }
        color = mix(
            color,
            vec3<f32>(0.04, 0.94, 0.82),
            diagnostic_river_alpha,
        );
        color = mix(
            color,
            vec3<f32>(0.88, 0.10, 0.94),
            diagnostic_pool_alpha,
        );
    } else if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_TEXTURE {
        var texture_weight = 0.0;
        if input.textured != 0u {
            texture_weight = material_texture_weight(material_blocks_per_pixel, 0.0);
            if input.near_shell != 0u && display_material != 2u {
                texture_weight = mix(texture_weight, 1.0, input.world_position.w);
            }
            if diagnostic_river_alpha > 0.0 {
                let river_texture_weight = material_texture_weight(
                    blocks_per_pixel,
                    input.world_position.w,
                );
                texture_weight = mix(
                    texture_weight,
                    river_texture_weight,
                    diagnostic_river_alpha,
                );
            }
        }
        let footprint = clamp(log2(max(material_blocks_per_pixel, 1.0)) / 6.0, 0.0, 1.0);
        color = vec3<f32>(texture_weight, footprint, input.world_position.w);
    }
    if exact_coverage.mode_count_generation.x == 2u && exact_painted {
        let checker = (i32(floor(input.world_xz.x / 2.0))
            + i32(floor(input.world_xz.y / 2.0))) & 1;
        let coverage_color = select(
            vec3<f32>(0.96, 0.05, 0.72),
            vec3<f32>(1.0, 0.72, 0.08),
            checker == 0,
        );
        color = mix(color, coverage_color, 0.82);
    }
    if horizon_diagnostic == TERRAIN_HORIZON_DIAGNOSTIC_NATURAL {
        var fog_camera_position = params.fog_camera_position;
        if input.view_index != 0u {
            fog_camera_position = params.fog_camera_position_right;
        }
        let fog_factor = mclone_fog_factor(
            input.world_position.xyz,
            fog_camera_position,
            params.fog_render_options,
            params.fog_color,
            params.fog_distances,
        );
        color = mix(color, params.fog_color.rgb, fog_factor);
    }
    return mclone_apply_target_color_transform_rgba(
        vec4<f32>(color, 1.0),
        terrain_target_color_transform,
    );
}
