struct TerrainPreviewParams {
    origin_spacing_cells: vec4<i32>,
    seed_source_view: vec4<u32>,
    layer_samples_size: vec4<u32>,
};

struct TerrainPreviewSample {
    terrain: vec4<f32>,
    climate: vec4<f32>,
    large_fields: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> params: TerrainPreviewParams;

@group(0) @binding(1)
var<storage, read_write> gpu_samples: array<TerrainPreviewSample>;

fn mix_hash(value: u32) -> u32 {
    var mixed = value;
    mixed = (mixed ^ (mixed >> 16u)) * 0x7feb352du;
    mixed = (mixed ^ (mixed >> 15u)) * 0x846ca68bu;
    return mixed ^ (mixed >> 16u);
}

fn lattice_hash(domain: u32, x: i32, z: i32) -> u32 {
    var value = params.seed_source_view.x
        ^ ((params.seed_source_view.y << 16u) | (params.seed_source_view.y >> 16u))
        ^ domain;
    value = value ^ (bitcast<u32>(x) * 0x9e3779b9u);
    value = value ^ (bitcast<u32>(z) * 0x85ebca6bu);
    return mix_hash(value);
}

fn floor_div(value: i32, divisor: i32) -> i32 {
    let quotient = value / divisor;
    let remainder = value % divisor;
    return quotient - select(0, 1, remainder < 0);
}

fn floor_mod(value: i32, divisor: i32) -> i32 {
    let remainder = value % divisor;
    return remainder + select(0, divisor, remainder < 0);
}

fn smooth_curve(value: f32) -> f32 {
    return value * value * (3.0 - 2.0 * value);
}

fn lerp_value(from_value: f32, to_value: f32, amount: f32) -> f32 {
    return from_value + (to_value - from_value) * amount;
}

fn lattice_value(domain: u32, x: i32, z: i32) -> f32 {
    let hash = lattice_hash(domain, x, z);
    return f32(hash >> 8u) * (1.0 / 8388607.5) - 1.0;
}

fn value_noise(domain: u32, scale: i32, world_x: i32, world_z: i32) -> f32 {
    let lattice_x = floor_div(world_x, scale);
    let lattice_z = floor_div(world_z, scale);
    let fraction_x = f32(floor_mod(world_x, scale)) / f32(scale);
    let fraction_z = f32(floor_mod(world_z, scale)) / f32(scale);
    let blend_x = smooth_curve(fraction_x);
    let blend_z = smooth_curve(fraction_z);
    let top = lerp_value(
        lattice_value(domain, lattice_x, lattice_z),
        lattice_value(domain, lattice_x + 1, lattice_z),
        blend_x,
    );
    let bottom = lerp_value(
        lattice_value(domain, lattice_x, lattice_z + 1),
        lattice_value(domain, lattice_x + 1, lattice_z + 1),
        blend_x,
    );
    return lerp_value(top, bottom, blend_z);
}

fn band_weight(scale: i32, sample_spacing: i32) -> f32 {
    return clamp(f32(scale) / max(f32(sample_spacing) * 2.0, 1.0), 0.0, 1.0);
}

fn weighted_field(
    world_x: i32,
    world_z: i32,
    sample_spacing: i32,
    domain_a: u32,
    scale_a: i32,
    weight_a: f32,
    domain_b: u32,
    scale_b: i32,
    weight_b: f32,
    domain_c: u32,
    scale_c: i32,
    weight_c: f32,
) -> f32 {
    let admitted_a = weight_a * band_weight(scale_a, sample_spacing);
    let admitted_b = weight_b * band_weight(scale_b, sample_spacing);
    let admitted_c = weight_c * band_weight(scale_c, sample_spacing);
    let total = max(admitted_a + admitted_b + admitted_c, 0.0001);
    return (
        value_noise(domain_a, scale_a, world_x, world_z) * admitted_a
        + value_noise(domain_b, scale_b, world_x, world_z) * admitted_b
        + value_noise(domain_c, scale_c, world_x, world_z) * admitted_c
    ) / total;
}

fn mountain_strength(continentalness: f32, ruggedness: f32) -> f32 {
    let inland = smooth_curve(clamp((continentalness - 0.08) / 0.42, 0.0, 1.0));
    let region = smooth_curve(clamp((ruggedness + 0.20) / 0.90, 0.0, 1.0));
    return inland * region;
}

fn land_surface_height(
    continentalness: f32,
    relief: f32,
    ruggedness: f32,
    ridges: f32,
    mountain_detail: f32,
) -> f32 {
    let land_strength = smooth_curve(clamp(continentalness / 0.45, 0.0, 1.0));
    let base = 64.0 + land_strength * 18.0;
    let rolling_relief = relief * (2.0 + land_strength * 7.0);
    let mountain = mountain_strength(continentalness, ruggedness);
    let shoulder = smooth_curve(clamp((ridges - 0.22) / 0.78, 0.0, 1.0));
    let lift = mountain * (4.0 + shoulder * 12.0 + shoulder * shoulder * 38.0);
    let texture = mountain_detail * mountain * (6.0 + shoulder * 14.0);
    return clamp(round(base + rolling_relief + lift + texture), 62.0, 160.0);
}

fn ocean_floor(
    world_x: i32,
    world_z: i32,
    sample_spacing: i32,
    continentalness: f32,
) -> f32 {
    let depth_signal = -continentalness;
    let inner = smooth_curve(clamp(depth_signal / 0.08, 0.0, 1.0));
    let outer = smooth_curve(clamp(depth_signal / 0.24, 0.0, 1.0));
    let basin = smooth_curve(clamp((depth_signal - 0.18) / 0.32, 0.0, 1.0));
    let basin_selector = value_noise(0x62617331u, 1536, world_x, world_z) * 0.5 + 0.5;
    let seabed = weighted_field(
        world_x,
        world_z,
        sample_spacing,
        0x62617332u,
        384,
        0.68,
        0x62617333u,
        96,
        0.32,
        0x62617334u,
        48,
        0.0,
    );
    let depth = 2.0
        + inner * 4.0
        + outer * 6.0
        + basin * (18.0 + basin_selector * 10.0)
        + seabed * (1.5 + basin * 7.0);
    return 63.0 - clamp(round(depth), 2.0, 52.0);
}

fn evaluate(world_x: i32, world_z: i32, sample_spacing: i32) -> TerrainPreviewSample {
    let continentalness = clamp(
        weighted_field(
            world_x,
            world_z,
            sample_spacing,
            0x636f6e31u,
            2048,
            0.55,
            0x636f6e32u,
            1024,
            0.30,
            0x636f6e33u,
            512,
            0.15,
        ),
        -1.0,
        1.0,
    );
    let relief = clamp(
        weighted_field(
            world_x,
            world_z,
            sample_spacing,
            0x72656c31u,
            384,
            0.50,
            0x72656c32u,
            128,
            0.30,
            0x72656c33u,
            48,
            0.20,
        ),
        -1.0,
        1.0,
    );
    let ruggedness = clamp(
        weighted_field(
            world_x,
            world_z,
            sample_spacing,
            0x72756731u,
            1536,
            0.72,
            0x72756732u,
            512,
            0.28,
            0x72756733u,
            256,
            0.0,
        ),
        -1.0,
        1.0,
    );
    let ridge_source = weighted_field(
        world_x,
        world_z,
        sample_spacing,
        0x72696431u,
        384,
        0.78,
        0x72696432u,
        128,
        0.22,
        0x72696433u,
        64,
        0.0,
    );
    let ridge_linear = clamp(1.0 - abs(ridge_source), 0.0, 1.0);
    let ridges = ridge_linear * ridge_linear;
    let mountain_detail = weighted_field(
        world_x,
        world_z,
        sample_spacing,
        0x6d647431u,
        32,
        0.70,
        0x6d647432u,
        8,
        0.30,
        0x6d647433u,
        4,
        0.0,
    );
    let temperature = clamp(
        weighted_field(
            world_x,
            world_z,
            sample_spacing,
            0x74656d31u,
            1536,
            0.78,
            0x74656d32u,
            384,
            0.22,
            0x74656d33u,
            192,
            0.0,
        ),
        -1.0,
        1.0,
    );
    let moisture = clamp(
        weighted_field(
            world_x,
            world_z,
            sample_spacing,
            0x6d6f6931u,
            1024,
            0.74,
            0x6d6f6932u,
            256,
            0.26,
            0x6d6f6933u,
            128,
            0.0,
        ),
        -1.0,
        1.0,
    );

    var surface_y = land_surface_height(
        continentalness,
        relief,
        ruggedness,
        ridges,
        mountain_detail,
    );
    var water = 0.0;
    var display_y = surface_y;
    if continentalness <= 0.0 {
        surface_y = ocean_floor(
            world_x,
            world_z,
            sample_spacing,
            continentalness,
        );
        display_y = 63.0;
        water = 1.0;
    }

    var sample: TerrainPreviewSample;
    sample.terrain = vec4<f32>(surface_y, display_y, continentalness, relief);
    sample.climate = vec4<f32>(temperature, moisture, water, ruggedness);
    sample.large_fields = vec4<f32>(surface_y, display_y, water, 0.0);
    return sample;
}

@compute @workgroup_size(8, 8, 1)
fn compute_main(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let samples_per_axis = params.layer_samples_size.y;
    if invocation.x >= samples_per_axis || invocation.y >= samples_per_axis {
        return;
    }
    let sample_spacing = params.origin_spacing_cells.z;
    let world_x = params.origin_spacing_cells.x + i32(invocation.x) * sample_spacing;
    let world_z = params.origin_spacing_cells.y + i32(invocation.y) * sample_spacing;
    let index = invocation.y * samples_per_axis + invocation.x;
    gpu_samples[index] = evaluate(world_x, world_z, sample_spacing);
}
