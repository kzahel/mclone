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

const TERRAIN_HORIZON_NORMAL_EDGE_WEST: u32 = 0x08000000u;
const TERRAIN_HORIZON_NORMAL_EDGE_EAST: u32 = 0x10000000u;
const TERRAIN_HORIZON_NORMAL_EDGE_NORTH: u32 = 0x20000000u;
const TERRAIN_HORIZON_NORMAL_EDGE_SOUTH: u32 = 0x40000000u;
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

struct TerrainPreviewMaterialUvs {
    values: array<vec4<f32>, 256>,
};

@group(1) @binding(0)
var material_atlas: texture_2d<f32>;

@group(1) @binding(1)
var material_sampler: sampler;

@group(1) @binding(2)
var<uniform> material_uvs: TerrainPreviewMaterialUvs;

struct TerrainExactCoverageParams {
    origin_size: vec4<i32>,
    mode_count_generation: vec4<u32>,
};

@group(2) @binding(0)
var<uniform> exact_coverage: TerrainExactCoverageParams;

@group(2) @binding(1)
var<storage, read> exact_coverage_words: array<u32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world_xz: vec2<f32>,
    @location(2) light: f32,
    @location(3) @interpolate(flat) material: u32,
    @location(4) @interpolate(flat) textured: u32,
    @location(5) river: vec4<f32>,
    @location(6) semantics: vec4<f32>,
    @location(7) world_position: vec3<f32>,
    @location(8) @interpolate(flat) biome: u32,
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

fn exact_chunk_painted_interior(world_xz: vec2<f32>) -> bool {
    let chunk = vec2<i32>(floor(world_xz / 16.0));
    if !exact_chunk_masked(chunk) {
        return false;
    }
    let local = fract(world_xz / 16.0) * 16.0;
    let collar = 1.5;
    if local.x < collar && !exact_chunk_masked(chunk + vec2<i32>(-1, 0)) {
        return false;
    }
    if local.x > 16.0 - collar && !exact_chunk_masked(chunk + vec2<i32>(1, 0)) {
        return false;
    }
    if local.y < collar && !exact_chunk_masked(chunk + vec2<i32>(0, -1)) {
        return false;
    }
    if local.y > 16.0 - collar && !exact_chunk_masked(chunk + vec2<i32>(0, 1)) {
        return false;
    }
    return true;
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

fn vanilla_mountain_exposure_biome(biome: u32) -> bool {
    return biome == 3u || biome == 20u || biome == 34u
        || biome == 131u || biome == 162u
        || biome == 163u || biome == 164u;
}

fn terrain_color(sample: TerrainPreviewSample, light: f32) -> vec3<f32> {
    let surface_y = sample.terrain.x;
    let temperature = sample.climate.x;
    let moisture = sample.climate.y;
    let material = u32(round(select(
        sample.large_fields.w,
        sample.hydrology_detail.w,
        surface_quality() >= 1u,
    )));
    if material == 2u {
        let depth = clamp((63.0 - surface_y) / 52.0, 0.0, 1.0);
        return mix(vec3<f32>(0.16, 0.55, 0.68), vec3<f32>(0.025, 0.17, 0.34), depth) * light;
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
    let dry = vec3<f32>(0.63, 0.54, 0.29);
    let wet = vec3<f32>(0.17, 0.48, 0.25);
    let cold = vec3<f32>(0.30, 0.49, 0.38);
    let moisture_mix = clamp(moisture * 0.5 + 0.5, 0.0, 1.0);
    let warmth = clamp(temperature * 0.5 + 0.5, 0.0, 1.0);
    return mix(cold, mix(dry, wet, moisture_mix), warmth) * light;
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

@vertex
fn vertex_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let cells = u32(params.origin_spacing_cells.w);
    let cell_stride = max(terrain_render_cell_stride, 1u);
    let render_cells = cells / cell_stride;
    let cell_index = vertex_index / 6u;
    let cell_x = (cell_index % render_cells) * cell_stride;
    let cell_z = (cell_index / render_cells) * cell_stride;
    let corner = grid_corner(vertex_index % 6u);
    let sample_x = cell_x + corner.x * cell_stride;
    let sample_z = cell_z + corner.y * cell_stride;
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
    let light = clamp(dot(normal, normalize(vec3<f32>(-0.45, 0.82, -0.35))) * 0.48 + 0.58, 0.34, 1.05);

    let world_x = params.origin_spacing_cells.x
        + i32(sample_x) * params.origin_spacing_cells.z;
    let world_z = params.origin_spacing_cells.y
        + i32(sample_z) * params.origin_spacing_cells.z;
    let relative_x = f32(world_x - params.viewport_center_extent.x)
        - params.presentation_center_extent.x;
    let relative_z = f32(world_z - params.viewport_center_extent.y)
        - params.presentation_center_extent.y;
    let compare = params.seed_source_view.z == 2u;
    let stacked_compare = compare && params.content_stage_flags.z == 1u;
    var clip_position = params.view_projection * vec4<f32>(
        vec3<f32>(
            relative_x,
            stitched_height + 1.0,
            relative_z,
        ),
        1.0,
    );
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
    out.world_xz = vec2<f32>(f32(world_x), f32(world_z));
    out.light = light;
    out.material = u32(round(select(
        sample.large_fields.w,
        sample.hydrology_detail.w,
        surface_quality() >= 1u && params.content_stage_flags.x > 0u,
    )));
    out.textured = select(
        0u,
        1u,
        params.layer_samples_size.x == 0u && preview_profile() == 0u,
    );
    out.river = vec4<f32>(
        sample.hydrology.x,
        sample.hydrology.w,
        sample.hydrology.y,
        sample.terrain.z,
    );
    out.semantics = vec4<f32>(
        sample.hydrology_detail.x,
        sample.hydrology_detail.y,
        sample.semantics.x,
        select(0.0, sample.forest_summary.x, params.content_stage_flags.x >= 4u),
    );
    out.world_position = vec3<f32>(
        f32(world_x),
        stitched_height + 1.0,
        f32(world_z),
    );
    out.biome = u32(round(sample.semantics.y));
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
    if exact_coverage.mode_count_generation.x == 1u
        && exact_chunk_painted_interior(input.world_xz) {
        discard;
    }
    let world_dx = dpdx(input.world_xz);
    let world_dy = dpdy(input.world_xz);
    let blocks_per_pixel = max(length(world_dx), length(world_dy));
    let river_anti_alias = max(fwidth(input.river.x), blocks_per_pixel * 0.35);
    var color = input.color;
    let face_normal = normalize(cross(
        dpdx(input.world_position),
        dpdy(input.world_position),
    ));
    let steep_mountain_face = abs(face_normal.y) < 0.80
        && vanilla_mountain_exposure_biome(input.biome);
    let grass_family = input.material == 4u || input.material == 5u
        || input.material == 13u || input.material == 14u;
    if preview_profile() == 1u && surface_quality() >= 1u
        && steep_mountain_face && grass_family {
        color = vec3<f32>(0.48, 0.49, 0.47) * input.light;
    }
    if input.textured != 0u && input.material < 256u {
        let sprite = material_uvs.values[input.material];
        let sprite_size = sprite.zw - sprite.xy;
        let local_uv = fract(input.world_xz);
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
        let texture_weight = mix(
            0.82,
            0.42,
            clamp((blocks_per_pixel - 1.0) / 7.0, 0.0, 1.0),
        );
        let texture_detail = clamp(texel.rgb * 1.25, vec3<f32>(0.0), vec3<f32>(1.5));
        color *= mix(vec3<f32>(1.0), texture_detail, texture_weight);
    }
    if input.textured != 0u
        && params.content_stage_flags.x >= 1u
        && preview_profile() == 0u {
        let visible_half_width = max(input.river.y, blocks_per_pixel * 0.70);
        let river_alpha = 1.0 - smoothstep(
            visible_half_width,
            visible_half_width + river_anti_alias,
            abs(input.river.x),
        );
        let river_color = mix(
            vec3<f32>(0.11, 0.48, 0.69),
            vec3<f32>(0.035, 0.22, 0.42),
            clamp((63.0 - input.position.z) * 0.15, 0.0, 1.0),
        );
        color = mix(color, river_color * input.light, river_alpha * 0.88);
        if params.content_stage_flags.x >= 4u {
            let cover = clamp(input.semantics.w, 0.0, 1.0);
            color = mix(color, color * vec3<f32>(0.57, 0.82, 0.58), cover * 0.36);
        }
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
    let fog_factor = mclone_fog_factor(
        input.world_position,
        params.fog_camera_position,
        params.fog_render_options,
        params.fog_color,
        params.fog_distances,
    );
    color = mix(color, params.fog_color.rgb, fog_factor);
    return mclone_apply_target_color_transform_rgba(
        vec4<f32>(color, 1.0),
        terrain_target_color_transform,
    );
}
