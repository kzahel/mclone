struct TerrainPreviewParams {
    origin_spacing_cells: vec4<i32>,
    seed_source_view: vec4<u32>,
    layer_samples_size: vec4<u32>,
    camera: vec4<f32>,
};

struct TerrainPreviewSample {
    terrain: vec4<f32>,
    climate: vec4<f32>,
    large_fields: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> params: TerrainPreviewParams;

@group(0) @binding(1)
var<storage, read> gpu_samples: array<TerrainPreviewSample>;

@group(0) @binding(2)
var<storage, read> reference_samples: array<TerrainPreviewSample>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

fn grid_corner(vertex_in_cell: u32) -> vec2<u32> {
    switch vertex_in_cell {
        case 0u: { return vec2<u32>(0u, 0u); }
        case 1u: { return vec2<u32>(1u, 0u); }
        case 2u: { return vec2<u32>(0u, 1u); }
        case 3u: { return vec2<u32>(0u, 1u); }
        case 4u: { return vec2<u32>(1u, 0u); }
        default: { return vec2<u32>(1u, 1u); }
    }
}

fn reference_base_sample(index: u32) -> TerrainPreviewSample {
    let final_sample = reference_samples[index];
    var base_sample = final_sample;
    base_sample.terrain.x = final_sample.large_fields.x;
    base_sample.terrain.y = final_sample.large_fields.y;
    base_sample.climate.z = final_sample.large_fields.z;
    return base_sample;
}

fn selected_sample(index: u32, instance_index: u32) -> TerrainPreviewSample {
    let source_mode = params.seed_source_view.z;
    if source_mode == 1u {
        return reference_samples[index];
    }
    if source_mode == 2u && instance_index == 0u {
        return reference_base_sample(index);
    }
    return gpu_samples[index];
}

fn terrain_color(sample: TerrainPreviewSample, light: f32) -> vec3<f32> {
    let surface_y = sample.terrain.x;
    let temperature = sample.climate.x;
    let moisture = sample.climate.y;
    let water = sample.climate.z >= 0.5;
    if water {
        let depth = clamp((63.0 - surface_y) / 52.0, 0.0, 1.0);
        return mix(vec3<f32>(0.16, 0.55, 0.68), vec3<f32>(0.025, 0.17, 0.34), depth) * light;
    }
    if surface_y >= 126.0 {
        return mix(vec3<f32>(0.64, 0.66, 0.62), vec3<f32>(0.94, 0.96, 0.94), clamp((surface_y - 126.0) / 20.0, 0.0, 1.0)) * light;
    }
    if surface_y >= 98.0 {
        return mix(vec3<f32>(0.34, 0.39, 0.34), vec3<f32>(0.58, 0.57, 0.52), clamp((surface_y - 98.0) / 28.0, 0.0, 1.0)) * light;
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
        return error_color(abs(reference.large_fields.x - gpu.large_fields.x));
    }
    if layer == 3u {
        return continentalness_color(sample.terrain.z);
    }
    if layer == 4u {
        let warmth = clamp(sample.climate.x * 0.5 + 0.5, 0.0, 1.0);
        let moisture = clamp(sample.climate.y * 0.5 + 0.5, 0.0, 1.0);
        return vec3<f32>(warmth, moisture, 1.0 - warmth * 0.65);
    }
    return terrain_color(sample, light);
}

@vertex
fn vertex_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let cells = u32(params.origin_spacing_cells.w);
    let samples_per_axis = params.layer_samples_size.y;
    let cell_index = vertex_index / 6u;
    let cell_x = cell_index % cells;
    let cell_z = cell_index / cells;
    let corner = grid_corner(vertex_index % 6u);
    let sample_x = cell_x + corner.x;
    let sample_z = cell_z + corner.y;
    let index = sample_z * samples_per_axis + sample_x;
    let sample = selected_sample(index, instance_index);
    let reference = reference_samples[index];
    let gpu = gpu_samples[index];

    let left_x = select(sample_x - 1u, sample_x, sample_x == 0u);
    let right_x = min(sample_x + 1u, samples_per_axis - 1u);
    let north_z = select(sample_z - 1u, sample_z, sample_z == 0u);
    let south_z = min(sample_z + 1u, samples_per_axis - 1u);
    let left = selected_sample(sample_z * samples_per_axis + left_x, instance_index);
    let right = selected_sample(sample_z * samples_per_axis + right_x, instance_index);
    let north = selected_sample(north_z * samples_per_axis + sample_x, instance_index);
    let south = selected_sample(south_z * samples_per_axis + sample_x, instance_index);
    let slope_x = (right.terrain.y - left.terrain.y) / max(f32(right_x - left_x), 1.0);
    let slope_z = (south.terrain.y - north.terrain.y) / max(f32(south_z - north_z), 1.0);
    let normal = normalize(vec3<f32>(-slope_x * 0.12, 1.0, -slope_z * 0.12));
    let light = clamp(dot(normal, normalize(vec3<f32>(-0.45, 0.82, -0.35))) * 0.48 + 0.58, 0.34, 1.05);

    let grid_x = f32(sample_x) / f32(cells) * 2.0 - 1.0;
    let grid_z = f32(sample_z) / f32(cells) * 2.0 - 1.0;
    let width = max(f32(params.layer_samples_size.z), 1.0);
    let height = max(f32(params.layer_samples_size.w), 1.0);
    let compare = params.seed_source_view.z == 2u;
    let view_width = select(width, width * 0.5, compare);
    let aspect = view_width / height;
    var clip_x = grid_x;
    var clip_y = -grid_z;
    var clip_z = 0.5;
    if params.seed_source_view.w == 1u {
        let yaw = params.camera.x;
        let pitch = params.camera.y;
        let view_right = vec2<f32>(cos(yaw), -sin(yaw));
        let view_depth = vec2<f32>(sin(yaw), cos(yaw));
        let horizontal = vec2<f32>(grid_x, grid_z);
        let camera_x = dot(horizontal, view_right);
        let camera_depth = dot(horizontal, view_depth);
        let world_height = (sample.terrain.y - 63.0) / 72.0;
        clip_x = camera_x * 0.72;
        clip_y = (world_height * cos(pitch) + camera_depth * sin(pitch)) * 0.78;
        clip_z = clamp(
            0.5 - camera_depth * cos(pitch) * 0.24 + world_height * sin(pitch) * 0.08,
            0.02,
            0.98,
        );
    }
    if aspect > 1.0 {
        clip_x = clip_x / aspect;
    } else {
        clip_y = clip_y * aspect;
    }
    if compare {
        let panel_center = select(-0.5, 0.5, instance_index == 1u);
        clip_x = clip_x * 0.46 + panel_center;
    }

    var out: VertexOutput;
    out.position = vec4<f32>(clip_x, clip_y, clip_z, 1.0);
    out.color = sample_color(sample, reference, gpu, light);
    return out;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
