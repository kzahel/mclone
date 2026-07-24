struct TerrainPreviewParams {
    origin_spacing_cells: vec4<i32>,
    seed_source_view: vec4<u32>,
    layer_samples_size: vec4<u32>,
    camera_eye_target: vec4<f32>,
    camera_up_fov: vec4<f32>,
    camera_projection: vec4<f32>,
    viewport_center_extent: vec4<i32>,
};

struct TerrainPreviewSample {
    terrain: vec4<f32>,
    climate: vec4<f32>,
    large_fields: vec4<f32>,
    hydrology: vec4<f32>,
    hydrology_detail: vec4<f32>,
    semantics: vec4<f32>,
};

struct U64 {
    low: u32,
    high: u32,
};

// Rust replaces this marker with domains and scales from the production spec.
// __MCLONE_PRODUCTION_FIELD_CONSTANTS__

const HASH_X_MULTIPLIER: U64 = U64(0x7f4a7c15u, 0x9e3779b9u);
const HASH_Z_MULTIPLIER: U64 = U64(0x1ce4e5b9u, 0xbf58476du);
const SPLITMIX_SECOND_MULTIPLIER: U64 = U64(0x133111ebu, 0x94d049bbu);

const GRADIENTS: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.9238795325, 0.3826834324),
    vec2<f32>(0.7071067812, 0.7071067812),
    vec2<f32>(0.3826834324, 0.9238795325),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(-0.3826834324, 0.9238795325),
    vec2<f32>(-0.7071067812, 0.7071067812),
    vec2<f32>(-0.9238795325, 0.3826834324),
    vec2<f32>(-1.0, 0.0),
    vec2<f32>(-0.9238795325, -0.3826834324),
    vec2<f32>(-0.7071067812, -0.7071067812),
    vec2<f32>(-0.3826834324, -0.9238795325),
    vec2<f32>(0.0, -1.0),
    vec2<f32>(0.3826834324, -0.9238795325),
    vec2<f32>(0.7071067812, -0.7071067812),
    vec2<f32>(0.9238795325, -0.3826834324),
);

@group(0) @binding(0)
var<uniform> params: TerrainPreviewParams;

@group(0) @binding(1)
var<storage, read_write> gpu_samples: array<TerrainPreviewSample>;

fn add_u64(left: U64, right: U64) -> U64 {
    let low = left.low + right.low;
    let carry = select(0u, 1u, low < left.low);
    return U64(low, left.high + right.high + carry);
}

fn xor_u64(left: U64, right: U64) -> U64 {
    return U64(left.low ^ right.low, left.high ^ right.high);
}

fn shift_right_u64(value: U64, amount: u32) -> U64 {
    return U64(
        (value.low >> amount) | (value.high << (32u - amount)),
        value.high >> amount,
    );
}

fn multiply_u32_wide(left: u32, right: u32) -> U64 {
    let left_low = left & 0xffffu;
    let left_high = left >> 16u;
    let right_low = right & 0xffffu;
    let right_high = right >> 16u;
    let low_product = left_low * right_low;
    let middle = (low_product >> 16u)
        + ((left_low * right_high) & 0xffffu)
        + ((left_high * right_low) & 0xffffu);
    let low = (low_product & 0xffffu) | (middle << 16u);
    let high = left_high * right_high
        + ((left_low * right_high) >> 16u)
        + ((left_high * right_low) >> 16u)
        + (middle >> 16u);
    return U64(low, high);
}

fn multiply_u64(left: U64, right: U64) -> U64 {
    let low_product = multiply_u32_wide(left.low, right.low);
    return U64(
        low_product.low,
        low_product.high + left.low * right.high + left.high * right.low,
    );
}

fn sign_extended_i32(value: i32) -> U64 {
    return U64(bitcast<u32>(value), select(0u, 0xffffffffu, value < 0));
}

fn splitmix64(value: U64) -> U64 {
    var mixed = add_u64(value, HASH_X_MULTIPLIER);
    mixed = multiply_u64(
        xor_u64(mixed, shift_right_u64(mixed, 30u)),
        HASH_Z_MULTIPLIER,
    );
    mixed = multiply_u64(
        xor_u64(mixed, shift_right_u64(mixed, 27u)),
        SPLITMIX_SECOND_MULTIPLIER,
    );
    return xor_u64(mixed, shift_right_u64(mixed, 31u));
}

fn lattice_hash(domain: U64, x: i32, z: i32) -> U64 {
    let seed = U64(
        params.seed_source_view.x,
        params.seed_source_view.y,
    );
    var value = xor_u64(seed, domain);
    value = xor_u64(
        value,
        multiply_u64(sign_extended_i32(x), HASH_X_MULTIPLIER),
    );
    value = xor_u64(
        value,
        multiply_u64(sign_extended_i32(z), HASH_Z_MULTIPLIER),
    );
    return splitmix64(value);
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

fn gradient_fade(value: f32) -> f32 {
    return value * value * value * (value * (value * 6.0 - 15.0) + 10.0);
}

fn lerp_value(from_value: f32, to_value: f32, amount: f32) -> f32 {
    return from_value + (to_value - from_value) * amount;
}

fn round_away_from_zero(value: f32) -> f32 {
    return select(ceil(value - 0.5), floor(value + 0.5), value >= 0.0);
}

fn lattice_value(domain: U64, x: i32, z: i32) -> f32 {
    let hash = lattice_hash(domain, x, z);
    let unit = f32(hash.high) * 2.3283064365386963e-10
        + f32(hash.low >> 8u) * 1.3877787807814457e-17;
    return unit * 2.0 - 1.0;
}

fn value_noise(
    domain: U64,
    scale: i32,
    world_x: i32,
    world_z: i32,
) -> f32 {
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

fn corner_gradient(domain: U64, lattice_x: i32, lattice_z: i32) -> vec2<f32> {
    let hash = lattice_hash(domain, lattice_x, lattice_z);
    return GRADIENTS[hash.low & 15u];
}

fn gradient_noise(
    domain: U64,
    scale: i32,
    world_x: f32,
    world_z: f32,
) -> f32 {
    let scaled_x = world_x / f32(scale);
    let scaled_z = world_z / f32(scale);
    let lattice_x = i32(floor(scaled_x));
    let lattice_z = i32(floor(scaled_z));
    let fraction_x = scaled_x - f32(lattice_x);
    let fraction_z = scaled_z - f32(lattice_z);
    let blend_x = gradient_fade(fraction_x);
    let blend_z = gradient_fade(fraction_z);
    let gradient_00 = corner_gradient(domain, lattice_x, lattice_z);
    let gradient_10 = corner_gradient(domain, lattice_x + 1, lattice_z);
    let gradient_01 = corner_gradient(domain, lattice_x, lattice_z + 1);
    let gradient_11 = corner_gradient(domain, lattice_x + 1, lattice_z + 1);
    let corner_00 = dot(gradient_00, vec2<f32>(fraction_x, fraction_z));
    let corner_10 = dot(gradient_10, vec2<f32>(fraction_x - 1.0, fraction_z));
    let corner_01 = dot(gradient_01, vec2<f32>(fraction_x, fraction_z - 1.0));
    let corner_11 = dot(
        gradient_11,
        vec2<f32>(fraction_x - 1.0, fraction_z - 1.0),
    );
    let top = lerp_value(corner_00, corner_10, blend_x);
    let bottom = lerp_value(corner_01, corner_11, blend_x);
    return clamp(lerp_value(top, bottom, blend_z) * 1.4142135624, -1.0, 1.0);
}

fn mountain_strength(continentalness: f32, ruggedness: f32) -> f32 {
    let inland = smooth_curve(clamp((continentalness - 0.08) / 0.42, 0.0, 1.0));
    let region = smooth_curve(clamp((ruggedness + 0.20) / 0.90, 0.0, 1.0));
    return inland * region;
}

fn macro_surface_material(
    surface_y: f32,
    continentalness: f32,
    relief: f32,
    ruggedness: f32,
    ridges: f32,
    mountain_detail: f32,
    temperature: f32,
) -> f32 {
    if continentalness <= 0.0 {
        return 2.0;
    }
    if surface_y <= 66.0 {
        return 6.0;
    }
    let altitude_cooling = clamp(max(surface_y - 72.0, 0.0) / 96.0, 0.0, 0.75);
    let adjusted_temperature = clamp(temperature - altitude_cooling, -1.0, 1.0);
    if surface_y >= 96.0 && adjusted_temperature <= -0.18 {
        return 8.0;
    }
    let altitude = smooth_curve(clamp((surface_y - 82.0) / 48.0, 0.0, 1.0));
    let crest = smooth_curve(clamp((ridges - 0.35) / 0.65, 0.0, 1.0));
    let exposure = mountain_strength(continentalness, ruggedness)
        * (altitude * 0.35 + crest * 0.65);
    let strength = clamp((exposure - 0.48) / 0.44, 0.0, 1.0);
    if surface_y >= 84.0 && strength >= 0.82 {
        return 1.0;
    }
    if surface_y >= 72.0 && strength >= 0.18 {
        let texture = clamp(
            mountain_detail * 0.68
                + relief * 0.17
                + (ridges * 2.0 - 1.0) * 0.15,
            -1.0,
            1.0,
        );
        if strength >= 0.62 && texture >= 0.36 - strength * 0.28 {
            return 1.0;
        }
        if texture >= 0.02 - strength * 0.22 {
            return 7.0;
        }
        if texture >= -0.48 - strength * 0.10 {
            return 13.0;
        }
    }
    return 4.0;
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
    return round_away_from_zero(
        clamp(base + rolling_relief + lift + texture, 62.0, 160.0),
    );
}

fn ocean_floor(
    world_x: i32,
    world_z: i32,
    continentalness: f32,
) -> f32 {
    let depth_signal = -continentalness;
    let inner = smooth_curve(clamp(depth_signal / 0.08, 0.0, 1.0));
    let outer = smooth_curve(clamp(depth_signal / 0.24, 0.0, 1.0));
    let basin = smooth_curve(clamp((depth_signal - 0.18) / 0.32, 0.0, 1.0));
    let basin_selector = gradient_noise(
        OCEAN_BASIN_DOMAIN,
        OCEAN_BASIN_SCALE,
        f32(world_x),
        f32(world_z),
    ) * 0.5 + 0.5;
    let seabed = clamp(
        gradient_noise(
            SEABED_LARGE_DOMAIN,
            SEABED_LARGE_SCALE,
            f32(world_x),
            f32(world_z),
        ) * 0.68
        + gradient_noise(
            SEABED_DETAIL_DOMAIN,
            SEABED_DETAIL_SCALE,
            f32(world_x),
            f32(world_z),
        ) * 0.32,
        -1.0,
        1.0,
    );
    let depth = 2.0
        + inner * 4.0
        + outer * 6.0
        + basin * (18.0 + clamp(basin_selector, 0.0, 1.0) * 10.0)
        + seabed * (1.5 + basin * 7.0);
    let water_depth = clamp(round_away_from_zero(depth), 2.0, 52.0);
    return 63.0 - water_depth;
}

fn evaluate(world_x: i32, world_z: i32) -> TerrainPreviewSample {
    let continentalness = clamp(
        value_noise(
            CONTINENT_LARGE_DOMAIN,
            CONTINENT_LARGE_SCALE,
            world_x,
            world_z,
        ) * 0.55
        + value_noise(
            CONTINENT_MEDIUM_DOMAIN,
            CONTINENT_MEDIUM_SCALE,
            world_x,
            world_z,
        ) * 0.30
        + value_noise(
            CONTINENT_DETAIL_DOMAIN,
            CONTINENT_DETAIL_SCALE,
            world_x,
            world_z,
        ) * 0.15,
        -1.0,
        1.0,
    );
    let relief_large = value_noise(
        RELIEF_LARGE_DOMAIN,
        RELIEF_LARGE_SCALE,
        world_x,
        world_z,
    );
    let relief_detail = value_noise(
        RELIEF_DETAIL_DOMAIN,
        RELIEF_DETAIL_SCALE,
        world_x,
        world_z,
    );
    let relief_fine = value_noise(
        RELIEF_FINE_DOMAIN,
        RELIEF_FINE_SCALE,
        world_x,
        world_z,
    );
    let relief = clamp(
        relief_large * 0.50 + relief_detail * 0.30 + relief_fine * 0.20,
        -1.0,
        1.0,
    );
    let ruggedness_large = value_noise(
        RUGGEDNESS_LARGE_DOMAIN,
        RUGGEDNESS_LARGE_SCALE,
        world_x,
        world_z,
    );
    let ruggedness_detail = value_noise(
        RUGGEDNESS_DETAIL_DOMAIN,
        RUGGEDNESS_DETAIL_SCALE,
        world_x,
        world_z,
    );
    let ruggedness = clamp(
        ruggedness_large * 0.72 + ruggedness_detail * 0.28,
        -1.0,
        1.0,
    );
    let ridge_source = value_noise(
        RIDGE_LARGE_DOMAIN,
        RIDGE_LARGE_SCALE,
        world_x,
        world_z,
    ) * 0.78
        + value_noise(
            RIDGE_DETAIL_DOMAIN,
            RIDGE_DETAIL_SCALE,
            world_x,
            world_z,
        ) * 0.22;
    let ridge_linear = clamp(1.0 - abs(ridge_source), 0.0, 1.0);
    let ridges = ridge_linear * ridge_linear;

    let large_warp_x = relief_detail * 18.0 + relief_fine * 4.0;
    let large_warp_z = ruggedness_detail * 18.0 - relief_fine * 4.0;
    let fine_warp_x = -ruggedness_detail * 7.0 + relief_detail * 3.0;
    let fine_warp_z = relief_fine * 7.0 + relief_detail * 3.0;
    let mountain_detail = clamp(
        gradient_noise(
            MOUNTAIN_DETAIL_LARGE_DOMAIN,
            MOUNTAIN_DETAIL_LARGE_SCALE,
            f32(world_x) + large_warp_x,
            f32(world_z) + large_warp_z,
        ) * 0.70
        + gradient_noise(
            MOUNTAIN_DETAIL_FINE_DOMAIN,
            MOUNTAIN_DETAIL_FINE_SCALE,
            f32(world_x) + fine_warp_x,
            f32(world_z) + fine_warp_z,
        ) * 0.30,
        -1.0,
        1.0,
    );

    let temperature_detail = gradient_noise(
        TEMPERATURE_DETAIL_DOMAIN,
        TEMPERATURE_DETAIL_SCALE,
        f32(world_x),
        f32(world_z),
    );
    let moisture_detail = gradient_noise(
        MOISTURE_DETAIL_DOMAIN,
        MOISTURE_DETAIL_SCALE,
        f32(world_x),
        f32(world_z),
    );
    let temperature_large_x = i32(round_away_from_zero(
        f32(world_x) + temperature_detail * 176.0,
    ));
    let temperature_large_z = i32(round_away_from_zero(
        f32(world_z) + moisture_detail * 176.0,
    ));
    let moisture_large_x = i32(round_away_from_zero(
        f32(world_x) - moisture_detail * 144.0,
    ));
    let moisture_large_z = i32(round_away_from_zero(
        f32(world_z) + temperature_detail * 144.0,
    ));
    let temperature = clamp(
        value_noise(
            TEMPERATURE_LARGE_DOMAIN,
            TEMPERATURE_LARGE_SCALE,
            temperature_large_x,
            temperature_large_z,
        ) * 0.78
        + temperature_detail * 0.22,
        -1.0,
        1.0,
    );
    let moisture = clamp(
        value_noise(
            MOISTURE_LARGE_DOMAIN,
            MOISTURE_LARGE_SCALE,
            moisture_large_x,
            moisture_large_z,
        ) * 0.74
        + moisture_detail * 0.26,
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
        surface_y = ocean_floor(world_x, world_z, continentalness);
        display_y = 63.0;
        water = 1.0;
    }

    var sample: TerrainPreviewSample;
    sample.terrain = vec4<f32>(surface_y, display_y, continentalness, relief);
    sample.climate = vec4<f32>(temperature, moisture, water, ruggedness);
    sample.large_fields = vec4<f32>(
        surface_y,
        display_y,
        water,
        macro_surface_material(
            surface_y,
            continentalness,
            relief,
            ruggedness,
            ridges,
            mountain_detail,
            temperature,
        ),
    );
    sample.hydrology = vec4<f32>(512.0, 0.0, 0.0, 6.0);
    sample.hydrology_detail = vec4<f32>(
        0.0,
        0.0,
        0.0,
        sample.large_fields.w,
    );
    sample.semantics = vec4<f32>(0.0, 7.0, 0.08, 5.0);
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
    gpu_samples[index] = evaluate(world_x, world_z);
}
