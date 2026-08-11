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

struct U64 {
    low: u32,
    high: u32,
};

struct CoastIntent {
    family: f32,
    proximity: f32,
    selector: f32,
    character: f32,
    depositional_suitability: f32,
    rocky_suitability: f32,
    transition: f32,
    cold_response: f32,
};

override terrain_sample_halo_radius: u32 = 0u;

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

@group(0) @binding(2)
var<storage, read_write> normal_heights: array<f32>;

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

fn gradient_fade_derivative(value: f32) -> f32 {
    return 30.0 * value * value * (value - 1.0) * (value - 1.0);
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

fn gradient_noise_with_derivative(
    domain: U64,
    scale: i32,
    world_x: f32,
    world_z: f32,
) -> vec3<f32> {
    let scaled_x = world_x / f32(scale);
    let scaled_z = world_z / f32(scale);
    let lattice_x = i32(floor(scaled_x));
    let lattice_z = i32(floor(scaled_z));
    let fraction_x = scaled_x - f32(lattice_x);
    let fraction_z = scaled_z - f32(lattice_z);
    let blend_x = gradient_fade(fraction_x);
    let blend_z = gradient_fade(fraction_z);
    let blend_dx = gradient_fade_derivative(fraction_x);
    let blend_dz = gradient_fade_derivative(fraction_z);
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
    let top_dx = lerp_value(gradient_00.x, gradient_10.x, blend_x)
        + (corner_10 - corner_00) * blend_dx;
    let bottom_dx = lerp_value(gradient_01.x, gradient_11.x, blend_x)
        + (corner_11 - corner_01) * blend_dx;
    let scaled_value = lerp_value(top, bottom, blend_z);
    let scaled_dx = lerp_value(top_dx, bottom_dx, blend_z);
    let top_dz = lerp_value(gradient_00.y, gradient_10.y, blend_x);
    let bottom_dz = lerp_value(gradient_01.y, gradient_11.y, blend_x);
    let scaled_dz = lerp_value(top_dz, bottom_dz, blend_z)
        + (bottom - top) * blend_dz;
    let normalization = 1.4142135624;
    let unclamped = scaled_value * normalization;
    if unclamped < -1.0 || unclamped > 1.0 {
        return vec3<f32>(clamp(unclamped, -1.0, 1.0), 0.0, 0.0);
    }
    let derivative_scale = normalization / f32(scale);
    return vec3<f32>(
        unclamped,
        scaled_dx * derivative_scale,
        scaled_dz * derivative_scale,
    );
}

fn gradient_noise(
    domain: U64,
    scale: i32,
    world_x: f32,
    world_z: f32,
) -> f32 {
    return gradient_noise_with_derivative(domain, scale, world_x, world_z).x;
}

fn mountain_strength(continentalness: f32, ruggedness: f32) -> f32 {
    let inland = smooth_curve(clamp((continentalness - 0.08) / 0.42, 0.0, 1.0));
    let region = smooth_curve(clamp((ruggedness + 0.20) / 0.90, 0.0, 1.0));
    return inland * region;
}

fn landform_family_code(
    continentalness: f32,
    relief: f32,
    ruggedness: f32,
    ridges: f32,
) -> f32 {
    var inland_strength = 0.0;
    if continentalness > 0.0 {
        inland_strength =
            0.28 + smooth_curve(clamp(continentalness / 0.42, 0.0, 1.0)) * 0.72;
    }
    let relief_energy =
        smooth_curve(clamp((abs(relief) - 0.03) / 0.62, 0.0, 1.0));
    let rolling_region =
        1.0 - smooth_curve(clamp((ruggedness + 0.58) / 0.88, 0.0, 1.0));
    let rolling_strength =
        inland_strength * rolling_region * (0.28 + relief_energy * 0.72);
    let ridge_region =
        smooth_curve(clamp((ruggedness + 0.62) / 0.72, 0.0, 1.0))
        * (1.0 - smooth_curve(clamp((ruggedness - 0.08) / 0.62, 0.0, 1.0)));
    let ridge_expression =
        smooth_curve(clamp((ridges - 0.08) / 0.92, 0.0, 1.0));
    let ridge_valley_strength =
        inland_strength * ridge_region * (0.72 + ridge_expression * 0.28);
    let mountain = mountain_strength(continentalness, ruggedness);
    let basin_strength =
        inland_strength
        * smooth_curve(clamp((-relief - 0.04) / 0.56, 0.0, 1.0))
        * (1.0 - mountain * 0.55);
    let structural_strength = max(
        max(rolling_strength, ridge_valley_strength),
        max(basin_strength, mountain),
    );
    let quiet_strength =
        inland_strength
        * (1.0
            - smooth_curve(clamp((structural_strength - 0.12) / 0.55, 0.0, 1.0)));

    let quiet_family_score =
        inland_strength
        * (1.0 - smooth_curve(clamp((ruggedness + 0.50) / 0.50, 0.0, 1.0)))
        * (1.0 - relief_energy * 0.45);
    let rolling_family_score =
        inland_strength
        * smooth_curve(clamp((ruggedness + 0.80) / 0.50, 0.0, 1.0))
        * (1.0 - smooth_curve(clamp((ruggedness + 0.10) / 0.35, 0.0, 1.0)))
        * (0.65 + relief_energy * 0.35);
    let basin_family_score =
        inland_strength
        * smooth_curve(clamp((basin_strength - 0.58) / 0.32, 0.0, 1.0));

    var score = quiet_family_score;
    var family = 4.0;
    if rolling_family_score >= score {
        score = rolling_family_score;
        family = 5.0;
    }
    if ridge_valley_strength >= score {
        score = ridge_valley_strength;
        family = 6.0;
    }
    if basin_family_score * 1.10 >= score {
        score = basin_family_score * 1.10;
        family = 7.0;
    }
    if mountain * 1.15 >= score {
        family = 8.0;
    }
    return family;
}

fn coast_intent(
    continentalness: f32,
    relief: f32,
    ruggedness: f32,
    ridges: f32,
    temperature: f32,
    selector: f32,
) -> CoastIntent {
    var proximity = 0.0;
    if continentalness >= 0.0 {
        proximity =
            1.0 - smooth_curve(clamp(continentalness / 0.16, 0.0, 1.0));
    } else {
        proximity =
            1.0 - smooth_curve(clamp(-continentalness / 0.10, 0.0, 1.0));
    }
    let rugged =
        smooth_curve(clamp((ruggedness + 0.30) / 1.10, 0.0, 1.0));
    let ridge = smooth_curve(clamp((ridges - 0.12) / 0.88, 0.0, 1.0));
    let relief_energy =
        smooth_curve(clamp((abs(relief) - 0.04) / 0.72, 0.0, 1.0));
    let rocky_suitability = clamp(
        rugged * 0.58 + ridge * 0.27 + relief_energy * 0.15,
        0.0,
        1.0,
    );
    let depositional_suitability = clamp(
        1.0 - rocky_suitability * 0.78 - relief_energy * 0.22,
        0.0,
        1.0,
    );
    let character = clamp(
        selector * 0.72
            + ruggedness * 0.18
            + (ridges * 2.0 - 1.0) * 0.07
            + relief * 0.03,
        -1.0,
        1.0,
    );
    var family = 3.0;
    if continentalness < -0.10 {
        family = 0.0;
    } else if continentalness > 0.16 {
        family = 5.0;
    } else if character <= -0.28 && depositional_suitability >= 0.22 {
        family = 1.0;
    } else if character <= 0.04 {
        family = 2.0;
    } else if character <= 0.30 {
        family = 3.0;
    } else if rocky_suitability >= 0.26 {
        family = 4.0;
    }
    let nearest_boundary = min(
        abs(character + 0.28),
        min(abs(character - 0.04), abs(character - 0.30)),
    );
    let transition =
        1.0 - smooth_curve(clamp(nearest_boundary / 0.12, 0.0, 1.0));
    let cold_response =
        smooth_curve(clamp((-temperature - 0.18) / 0.42, 0.0, 1.0));
    return CoastIntent(
        family,
        proximity,
        selector,
        character,
        depositional_suitability,
        rocky_suitability,
        transition * proximity,
        cold_response,
    );
}

fn coast_realization_texture(
    relief: f32,
    ridges: f32,
    mountain_detail: f32,
) -> f32 {
    return clamp(
        mountain_detail * 0.72
            + relief * 0.18
            + (ridges * 2.0 - 1.0) * 0.10,
        -1.0,
        1.0,
    );
}

fn coast_realization_proximity(
    coast: CoastIntent,
    texture: f32,
) -> f32 {
    let jitter = 0.06 + coast.transition * 0.04;
    return clamp(coast.proximity + texture * jitter, 0.0, 1.0);
}

fn coast_realization_character(
    coast: CoastIntent,
    texture: f32,
) -> f32 {
    let jitter = 0.03 + coast.transition * 0.11;
    return clamp(coast.character + texture * jitter, -1.0, 1.0);
}

fn coast_realized_family(
    coast: CoastIntent,
    texture: f32,
) -> f32 {
    if coast.family == 0.0 || coast.family == 5.0 {
        return coast.family;
    }
    let character = coast_realization_character(coast, texture);
    if character <= -0.28 && coast.depositional_suitability >= 0.22 {
        return 1.0;
    }
    if character <= 0.04 {
        return 2.0;
    }
    if character <= 0.30 {
        return 3.0;
    }
    if coast.rocky_suitability >= 0.26 {
        return 4.0;
    }
    return 3.0;
}

fn coast_rocky_surface_strength(
    coast: CoastIntent,
    texture: f32,
) -> f32 {
    let character = coast_realization_character(coast, texture);
    let rocky_gate =
        smooth_curve(clamp((character - 0.12) / 0.34, 0.0, 1.0));
    let rocky_support = smooth_curve(
        clamp((coast.rocky_suitability - 0.12) / 0.50, 0.0, 1.0),
    );
    return rocky_gate
        * rocky_support
        * coast_realization_proximity(coast, texture);
}

fn macro_snow_cover_active(
    surface_y: f32,
    continentalness: f32,
    coast: CoastIntent,
    texture: f32,
) -> bool {
    let threshold = 0.72 - texture * 0.08;
    return continentalness > 0.0
        && surface_y <= 93.0
        && coast.cold_response >= threshold;
}

fn coast_sandy_material(
    coast: CoastIntent,
    texture: f32,
) -> f32 {
    let proximity = coast_realization_proximity(coast, texture);
    let inland_edge = smooth_curve(
        clamp((0.96 - proximity) / 0.10, 0.0, 1.0),
    );
    if coast.rocky_suitability >= 0.52 && texture >= 0.34 {
        return 7.0;
    }
    if texture < -0.50 + inland_edge * 0.46 {
        return 4.0;
    }
    return 6.0;
}

fn coast_gravel_material(
    coast: CoastIntent,
    texture: f32,
) -> f32 {
    let proximity = coast_realization_proximity(coast, texture);
    let inland_edge = smooth_curve(
        clamp((0.96 - proximity) / 0.10, 0.0, 1.0),
    );
    if coast.rocky_suitability >= 0.56
        && texture >= 0.08 - coast.transition * 0.12 {
        return 1.0;
    }
    if texture < -0.42 + inland_edge * 0.30 {
        return 4.0;
    }
    if texture < -0.12 + inland_edge * 0.22 {
        return 13.0;
    }
    return 7.0;
}

fn river_bank_material(
    base_surface_y: f32,
    distance: f32,
    half_width: f32,
    texture: f32,
    coast: CoastIntent,
) -> f32 {
    if base_surface_y > 68.0 {
        return 4.0;
    }
    let bank_run = max(distance - half_width, 0.0);
    let sand_opportunity = smooth_curve(
        clamp((0.22 - coast.character) / 0.60, 0.0, 1.0),
    ) * coast.depositional_suitability;
    let sandy_run =
        sand_opportunity * (1.20 + (texture * 0.5 + 0.5) * 1.40);
    if bank_run <= sandy_run {
        return 6.0;
    }
    if bank_run <= sandy_run + 1.40 {
        if texture >= 0.18 {
            return 7.0;
        }
        if texture >= -0.18 {
            return 13.0;
        }
    }
    return 4.0;
}

fn macro_surface_material(
    surface_y: f32,
    continentalness: f32,
    relief: f32,
    ruggedness: f32,
    ridges: f32,
    mountain_detail: f32,
    temperature: f32,
    coast: CoastIntent,
) -> f32 {
    if continentalness <= 0.0 {
        return 2.0;
    }
    let coast_texture =
        coast_realization_texture(relief, ridges, mountain_detail);
    let realized_family = coast_realized_family(coast, coast_texture);
    let realization_proximity =
        coast_realization_proximity(coast, coast_texture);
    if continentalness > 0.0 {
        if macro_snow_cover_active(
            surface_y,
            continentalness,
            coast,
            coast_texture,
        ) {
            return 8.0;
        }
        if realized_family == 1.0 && realization_proximity >= 0.86 {
            return coast_sandy_material(coast, coast_texture);
        }
        if realized_family == 2.0 && realization_proximity >= 0.86 {
            return coast_gravel_material(coast, coast_texture);
        }
        if realized_family == 4.0 && realization_proximity >= 0.12 {
            return 4.0;
        }
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
    mountain_detail_large: f32,
    mountain_detail_fine: f32,
) -> f32 {
    let land_strength = smooth_curve(clamp(continentalness / 0.45, 0.0, 1.0));
    let base = 64.0 + land_strength * 18.0;
    let inland_strength =
        select(
            0.0,
            0.28 + smooth_curve(clamp(continentalness / 0.42, 0.0, 1.0)) * 0.72,
            continentalness > 0.0,
        );
    let relief_energy =
        smooth_curve(clamp((abs(relief) - 0.03) / 0.62, 0.0, 1.0));
    let rolling_region =
        1.0 - smooth_curve(clamp((ruggedness + 0.58) / 0.88, 0.0, 1.0));
    let rolling =
        inland_strength * rolling_region * (0.28 + relief_energy * 0.72);
    let ridge_region =
        smooth_curve(clamp((ruggedness + 0.62) / 0.72, 0.0, 1.0))
        * (1.0 - smooth_curve(clamp((ruggedness - 0.08) / 0.62, 0.0, 1.0)));
    let ridge_expression =
        smooth_curve(clamp((ridges - 0.08) / 0.92, 0.0, 1.0));
    let ridge_valley =
        inland_strength * ridge_region * (0.72 + ridge_expression * 0.28);
    let mountain = mountain_strength(continentalness, ruggedness);
    let basin =
        inland_strength
        * smooth_curve(clamp((-relief - 0.04) / 0.56, 0.0, 1.0))
        * (1.0 - mountain * 0.55);
    let structural_strength = max(
        max(rolling, ridge_valley),
        max(basin, mountain),
    );
    let quiet_strength =
        inland_strength
        * (1.0
            - smooth_curve(clamp((structural_strength - 0.12) / 0.55, 0.0, 1.0)));
    let relief_amplitude =
        2.0
        + land_strength * 7.0
        + quiet_strength * 10.0
        + rolling * 20.0
        + ridge_valley * 6.0;
    let rolling_relief = relief * relief_amplitude;
    let ridge_profile =
        smooth_curve(clamp((ridges - 0.12) / 0.88, 0.0, 1.0)) - 0.32;
    let ridge_relief =
        ridge_valley * ridge_profile * (14.0 + land_strength * 20.0);
    let basin_floor = -basin * (3.0 + max(-relief, 0.0) * 7.0);
    let shoulder = smooth_curve(clamp((ridges - 0.22) / 0.78, 0.0, 1.0));
    let lift = mountain * (4.0 + shoulder * 12.0 + shoulder * shoulder * 38.0);
    let ordinary_large_texture =
        mountain_detail_large
        * (
            max(
                quiet_strength * 24.0,
                max(rolling * 32.0, ridge_valley * 36.0),
            )
            + basin * 2.5
        );
    let ordinary_fine_texture =
        mountain_detail_fine
        * (max(rolling * 2.0, ridge_valley * 3.5) + basin * 0.35);
    let mountain_detail =
        clamp(mountain_detail_large * 0.70 + mountain_detail_fine * 0.30, -1.0, 1.0);
    let mountain_texture =
        mountain_detail * mountain * (6.0 + shoulder * 14.0);
    return round_away_from_zero(
        clamp(
            base
                + rolling_relief
                + ridge_relief
                + basin_floor
                + lift
                + ordinary_large_texture
                + ordinary_fine_texture
                + mountain_texture,
            62.0,
            160.0,
        ),
    );
}

fn coast_adjusted_land_surface_height(
    provisional_surface_y: f32,
    coast: CoastIntent,
    continentalness: f32,
    relief: f32,
    ridges: f32,
    mountain_detail: f32,
) -> f32 {
    if coast.family == 0.0 || coast.family == 5.0 || coast.proximity == 0.0 {
        return provisional_surface_y;
    }
    let texture =
        coast_realization_texture(relief, ridges, mountain_detail);
    let realization_proximity =
        coast_realization_proximity(coast, texture);
    let rocky_gate =
        smooth_curve(clamp((coast.character - 0.18) / 0.22, 0.0, 1.0));
    let rocky_support = smooth_curve(
        clamp((coast.rocky_suitability - 0.18) / 0.52, 0.0, 1.0),
    );
    let rocky_shape = 0.82 + (texture * 0.5 + 0.5) * 0.36;
    let rocky_influence =
        rocky_gate * rocky_support * realization_proximity * rocky_shape;
    var gravel_rise = 0.0;
    if coast.family == 2.0 {
        gravel_rise =
            realization_proximity
                * (0.35 + coast.rocky_suitability * 1.65);
    }
    let ridge_shoulder =
        smooth_curve(clamp((ridges - 0.16) / 0.84, 0.0, 1.0));
    let rocky_lift = 4.0
        + coast.rocky_suitability * 14.0
        + ridge_shoulder * 6.0
        + max(relief, 0.0) * 4.0
        + mountain_detail * 3.0;
    let inland_strength =
        smooth_curve(clamp(continentalness / 0.45, 0.0, 1.0));
    let continental_base = 64.0 + inland_strength * 18.0;
    let incoming_positive_relief =
        max(provisional_surface_y - continental_base, 0.0);
    let rocky_complement =
        1.0
        - smooth_curve(clamp(incoming_positive_relief / 12.0, 0.0, 1.0))
            * 0.72;
    let adjustment =
        clamp(
            rocky_influence * rocky_lift * rocky_complement + gravel_rise,
            0.0,
            22.0,
        );
    return round_away_from_zero(
        clamp(provisional_surface_y + adjustment, 62.0, 160.0),
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

struct RiverGeometry {
    signed_distance: f32,
    distance: f32,
    half_width: f32,
    tangent: vec2<f32>,
    width_noise: f32,
    reach_noise: f32,
    morphology_detail: f32,
    center: vec2<f32>,
};

struct HydrologyResult {
    surface_display_water_material: vec4<f32>,
    hydrology: vec4<f32>,
    hydrology_detail: vec4<f32>,
    semantics: vec4<f32>,
};

struct ForestIntent {
    summary: vec4<f32>,
    detail: vec4<f32>,
};

fn forest_intent(
    world_x: i32,
    world_z: i32,
    biome: f32,
    temperature: f32,
    moisture: f32,
) -> ForestIntent {
    let grove = clamp(
        value_noise(GROVE_DOMAIN, GROVE_SCALE, world_x, world_z) * 0.5 + 0.5,
        0.0,
        1.0,
    );
    if biome == 7.0 {
        let clustered = grove * grove * grove;
        return ForestIntent(
            vec4<f32>(
                0.04 + clustered * 0.16,
                0.025 + clustered * 0.105,
                1.0,
                0.0,
            ),
            vec4<f32>(7.0, 1.4, grove, 1.0),
        );
    }
    if biome == 6.0 {
        return ForestIntent(
            vec4<f32>(0.58 + grove * 0.30, 0.56 + grove * 0.32, 1.0, 0.0),
            vec4<f32>(8.0, 1.8, grove, 1.0),
        );
    }
    if biome == 4.0 {
        return ForestIntent(
            vec4<f32>(0.68 + grove * 0.29, 0.66 + grove * 0.30, 2.0, 0.0),
            vec4<f32>(10.0, 2.4, grove, 1.0),
        );
    }
    if biome == 5.0 {
        let core = temperature >= 0.18 && moisture <= -0.10;
        let base_density = select(0.045, 0.14, core);
        let density_span = select(0.075, 0.14, core);
        let base_coverage = select(0.04, 0.12, core);
        let coverage_span = select(0.09, 0.16, core);
        return ForestIntent(
            vec4<f32>(
                base_coverage + grove * coverage_span,
                base_density + grove * density_span,
                3.0,
                0.0,
            ),
            vec4<f32>(7.5, 1.5, grove, 1.0),
        );
    }
    return ForestIntent(vec4<f32>(0.0), vec4<f32>(0.0));
}

fn compact_influence(distance: f32, radius: f32, feather: f32) -> f32 {
    if distance >= radius {
        return 0.0;
    }
    return 1.0 - smooth_curve(clamp((distance - radius + feather) / feather, 0.0, 1.0));
}

fn hydraulic_surface_height(world_x: i32, world_z: i32) -> f32 {
    let continentalness = clamp(
        value_noise(CONTINENT_LARGE_DOMAIN, CONTINENT_LARGE_SCALE, world_x, world_z) * 0.55
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
    let relief = clamp(
        value_noise(RELIEF_LARGE_DOMAIN, RELIEF_LARGE_SCALE, world_x, world_z) * 0.50
            + value_noise(RELIEF_DETAIL_DOMAIN, RELIEF_DETAIL_SCALE, world_x, world_z) * 0.30
            + value_noise(RELIEF_FINE_DOMAIN, RELIEF_FINE_SCALE, world_x, world_z) * 0.20,
        -1.0,
        1.0,
    );
    let land_strength = smooth_curve(clamp(continentalness / 0.45, 0.0, 1.0));
    return clamp(
        64.0 + land_strength * 18.0 + relief * (2.0 + land_strength * 7.0),
        63.0,
        104.0,
    );
}

fn river_geometry(
    world_x: i32,
    world_z: i32,
    relief_large: f32,
    relief_detail: f32,
    ruggedness_detail: f32,
) -> RiverGeometry {
    let x = f32(world_x);
    let z = f32(world_z);
    let warp_x = relief_detail * 72.0 + ruggedness_detail * 24.0;
    let warp_z = ruggedness_detail * 72.0 - relief_large * 24.0;
    let large = gradient_noise_with_derivative(
        RIVER_LARGE_DOMAIN,
        RIVER_LARGE_SCALE,
        x + warp_x,
        z + warp_z,
    );
    let detail = gradient_noise_with_derivative(
        RIVER_DETAIL_DOMAIN,
        RIVER_DETAIL_SCALE,
        x + warp_x * 0.35,
        z + warp_z * 0.35,
    );
    let center_value = clamp(large.x * 0.82 + detail.x * 0.18, -1.0, 1.0);
    let gradient = large.yz * 0.82 + detail.yz * 0.18;
    let gradient_length = max(length(gradient), 1.0 / 2048.0);
    let signed_distance = clamp(center_value / gradient_length, -512.0, 512.0);
    let normal = gradient / gradient_length;
    var tangent = vec2<f32>(-normal.y, normal.x);
    if tangent.y < 0.0 || (tangent.y == 0.0 && tangent.x < 0.0) {
        tangent = -tangent;
    }
    let center = vec2<f32>(x, z) - normal * signed_distance;
    let width_noise = value_noise(
        RIVER_WIDTH_DOMAIN,
        RIVER_WIDTH_SCALE,
        world_x,
        world_z,
    ) * 0.5 + 0.5;
    let reach_noise = value_noise(
        RIVER_REACH_DOMAIN,
        RIVER_REACH_SCALE,
        i32(round_away_from_zero(center.x)),
        i32(round_away_from_zero(center.y)),
    );
    let morphology_detail = gradient_noise(
        RIVER_MORPHOLOGY_DETAIL_DOMAIN,
        RIVER_MORPHOLOGY_DETAIL_SCALE,
        center.x,
        center.y,
    );
    let half_width = clamp(
        4.5 + width_noise * 4.0 + reach_noise * 1.20 + morphology_detail * 0.65,
        3.75,
        9.75,
    );
    return RiverGeometry(
        signed_distance,
        abs(signed_distance),
        half_width,
        tangent,
        width_noise,
        reach_noise,
        morphology_detail,
        center,
    );
}

fn surface_recipe_code(
    surface_y: f32,
    water_y: f32,
    channel_influence: f32,
    bank_influence: f32,
    wetland_pool_influence: f32,
    continentalness: f32,
    relief: f32,
    ruggedness: f32,
    ridges: f32,
    mountain_detail: f32,
    temperature: f32,
    coast: CoastIntent,
) -> f32 {
    if channel_influence > 0.0 {
        return 2.0;
    }
    if wetland_pool_influence >= 0.55 {
        return 3.0;
    }
    let coast_texture =
        coast_realization_texture(relief, ridges, mountain_detail);
    let realized_family = coast_realized_family(coast, coast_texture);
    let realization_proximity =
        coast_realization_proximity(coast, coast_texture);
    if macro_snow_cover_active(
        surface_y,
        continentalness,
        coast,
        coast_texture,
    ) {
        return 11.0;
    }
    if bank_influence > 0.0 && surface_y <= water_y + 3.0 {
        return 4.0;
    }
    if continentalness <= 0.0 {
        return 0.0;
    }
    if realized_family == 1.0
        && realization_proximity >= 0.86
        && surface_y <= 68.0 {
        return 1.0;
    }
    if realized_family == 2.0
        && realization_proximity >= 0.86
        && surface_y <= 73.0 {
        return 9.0;
    }
    if realization_proximity >= 0.12
        && coast_rocky_surface_strength(coast, coast_texture) >= 0.06
        && surface_y <= 93.0 {
        return 10.0;
    }
    let adjusted_temperature =
        clamp(temperature - clamp(max(surface_y - 72.0, 0.0) / 96.0, 0.0, 0.75), -1.0, 1.0);
    if surface_y >= 96.0 && adjusted_temperature <= -0.18 {
        return 7.0;
    }
    let altitude = smooth_curve(clamp((surface_y - 82.0) / 48.0, 0.0, 1.0));
    let crest = smooth_curve(clamp((ridges - 0.35) / 0.65, 0.0, 1.0));
    let exposure = mountain_strength(continentalness, ruggedness)
        * (altitude * 0.35 + crest * 0.65);
    let erosion = clamp((exposure - 0.48) / 0.44, 0.0, 1.0);
    if surface_y >= 84.0 && erosion >= 0.82 {
        return 8.0;
    }
    if surface_y >= 72.0 && erosion >= 0.18 {
        return 6.0;
    }
    return 5.0;
}

fn biome_recipe_code(
    surface_y: f32,
    channel_influence: f32,
    wetland_pool_influence: f32,
    bank_influence: f32,
    wetland_influence: f32,
    continentalness: f32,
    ruggedness: f32,
    ridges: f32,
    temperature: f32,
    moisture: f32,
    coast: CoastIntent,
) -> f32 {
    if channel_influence > 0.0 || wetland_pool_influence >= 0.55 {
        return 2.0;
    }
    if bank_influence > 0.0 && wetland_influence > 0.25 {
        return 7.0;
    }
    if continentalness <= 0.0 {
        return 0.0;
    }
    if (coast.family == 1.0 || coast.family == 2.0)
        && coast.proximity >= 0.86
        && surface_y <= 71.0 {
        return 1.0;
    }
    let adjusted_temperature =
        clamp(temperature - clamp(max(surface_y - 72.0, 0.0) / 96.0, 0.0, 0.75), -1.0, 1.0);
    if surface_y >= 96.0 && adjusted_temperature <= -0.18 {
        return 3.0;
    }
    if adjusted_temperature <= -0.12 && moisture >= -0.05 {
        return 4.0;
    }
    let steppe_core = temperature >= 0.18 && moisture <= -0.10;
    let warmth = clamp((temperature - 0.04) / 0.32, 0.0, 1.0);
    let dryness = clamp((0.08 - moisture) / 0.32, 0.0, 1.0);
    let steppe_shoulder = temperature >= 0.08
        && moisture <= 0.04
        && sqrt(warmth * dryness) >= 0.38;
    if steppe_core || steppe_shoulder {
        return 5.0;
    }
    let mountain = mountain_strength(continentalness, ruggedness);
    let mountain_valley = surface_y > 63.0 && mountain >= 0.35 && ridges <= 0.28;
    let mountain_shoulder = surface_y > 63.0 && mountain >= 0.15 && ridges >= 0.65;
    let altitude = smooth_curve(clamp((surface_y - 82.0) / 48.0, 0.0, 1.0));
    let crest = smooth_curve(clamp((ridges - 0.35) / 0.65, 0.0, 1.0));
    let exposure = mountain * (altitude * 0.35 + crest * 0.65);
    if surface_y >= 75.0 && !mountain_valley && !mountain_shoulder && exposure < 0.44 {
        return 6.0;
    }
    return 7.0;
}

fn landform_kind_code(
    channel_influence: f32,
    wetland_pool_influence: f32,
    wetland_influence: f32,
    continentalness: f32,
    relief: f32,
    ruggedness: f32,
    ridges: f32,
    coast: CoastIntent,
) -> f32 {
    if channel_influence > 0.0 {
        return 2.0;
    }
    if wetland_pool_influence >= 0.55 || wetland_influence > 0.25 {
        return 3.0;
    }
    if continentalness <= 0.0 {
        return 0.0;
    }
    if coast.proximity >= 0.12
        && coast.family >= 1.0
        && coast.family <= 4.0 {
        return 1.0;
    }
    return landform_family_code(
        continentalness,
        relief,
        ruggedness,
        ridges,
    );
}

fn complete_hydrology(
    world_x: i32,
    world_z: i32,
    continentalness: f32,
    relief: f32,
    ruggedness: f32,
    ridges: f32,
    mountain_detail: f32,
    temperature: f32,
    moisture: f32,
    base_surface_y: f32,
    macro_material: f32,
    geometry: RiverGeometry,
    coast: CoastIntent,
) -> HydrologyResult {
    let depth = clamp(
        round_away_from_zero(
            3.2 + geometry.half_width * 0.10 + geometry.reach_noise * 0.85
                - geometry.morphology_detail * 0.55,
        ),
        2.0,
        6.0,
    );
    var surface_y = base_surface_y;
    var channel_influence = 0.0;
    var bank_influence = 0.0;
    var wetland_influence = 0.0;
    var wetland_pool_influence = 0.0;
    var submerged_outlet_influence = 0.0;
    var half_width = geometry.half_width;
    let water_y = 63.0;

    if geometry.distance <= 64.0 {
        if continentalness <= 0.0 {
            let depth_signal = -continentalness;
            let outlet_fade =
                1.0 - smooth_curve(clamp(depth_signal / 0.18, 0.0, 1.0));
            let fan = 1.0
                + smooth_curve(clamp(depth_signal / 0.10, 0.0, 1.0))
                    * outlet_fade
                    * 0.45;
            half_width = geometry.half_width * fan;
            submerged_outlet_influence =
                compact_influence(geometry.distance, half_width - 0.5, 3.0) * outlet_fade;
            let outlet_depth = max(depth, 4.0);
            let channel_depth =
                2.0 + submerged_outlet_influence * max(outlet_depth - 2.0, 0.0);
            surface_y = min(
                base_surface_y,
                63.0 - max(round_away_from_zero(channel_depth), 2.0),
            );
        } else {
            let offset = geometry.tangent * 16.0;
            let backward = hydraulic_surface_height(
                i32(round_away_from_zero(geometry.center.x - offset.x)),
                i32(round_away_from_zero(geometry.center.y - offset.y)),
            );
            let forward = hydraulic_surface_height(
                i32(round_away_from_zero(geometry.center.x + offset.x)),
                i32(round_away_from_zero(geometry.center.y + offset.y)),
            );
            let grade = abs(forward - backward) / 32.0;
            let mountain_region = mountain_strength(continentalness, ruggedness);
            let low_grade =
                1.0 - smooth_curve(clamp((grade - 0.02) / 0.16, 0.0, 1.0));
            let low_mountain =
                1.0 - smooth_curve(clamp(mountain_region / 0.55, 0.0, 1.0));
            let inland_wetland = smooth_curve(clamp(continentalness / 0.18, 0.0, 1.0));
            let wetland_selector =
                smooth_curve(clamp((geometry.width_noise - 0.42) / 0.58, 0.0, 1.0));
            wetland_influence =
                low_grade * low_mountain * inland_wetland * wetland_selector;
            let incision_depth = clamp(base_surface_y - 64.0, 0.0, 28.0);
            let bank_asymmetry = sign(geometry.signed_distance)
                * (geometry.reach_noise * 0.35 + geometry.morphology_detail * 0.65);
            let bank_span = (12.0 + incision_depth * 1.4 + wetland_influence * 16.0)
                * clamp(1.0 + bank_asymmetry * 0.28, 0.72, 1.28);
            channel_influence =
                compact_influence(geometry.distance, geometry.half_width - 0.5, 3.0);
            bank_influence = 1.0
                - smooth_curve(
                    clamp(
                        (geometry.distance - geometry.half_width) / bank_span,
                        0.0,
                        1.0,
                    ),
                );
            let pool_texture = gradient_noise(
                WETLAND_POOL_DOMAIN,
                WETLAND_POOL_SCALE,
                f32(world_x) + geometry.tangent.x * 19.0,
                f32(world_z) + geometry.tangent.y * 19.0,
            ) * 0.5 + 0.5;
            if channel_influence == 0.0 {
                wetland_pool_influence = wetland_influence
                    * bank_influence
                    * smooth_curve(clamp((pool_texture - 0.30) / 0.45, 0.0, 1.0));
            }
            if channel_influence > 0.0 {
                let channel_depth =
                    1.0 + channel_influence * max(depth - 1.0, 0.0);
                surface_y = min(
                    base_surface_y,
                    round_away_from_zero(63.0 - channel_depth),
                );
            } else if wetland_pool_influence >= 0.55 {
                surface_y = min(base_surface_y, 62.0);
            } else if bank_influence > 0.0 {
                let bank_run = max(geometry.distance - geometry.half_width, 0.0);
                let collar_width = clamp(
                    2.0 + geometry.morphology_detail * 0.85 + bank_asymmetry * 0.45,
                    1.0,
                    3.0,
                );
                let terrace_run = clamp(
                    3.25 + geometry.reach_noise * 0.65 - bank_asymmetry * 0.55,
                    2.4,
                    4.2,
                );
                let terrace_phase =
                    max(geometry.morphology_detail * 1.25 + bank_asymmetry * 0.75, 0.0);
                let terrace_rise =
                    min(floor(max(bank_run - collar_width + terrace_phase, 0.0) / terrace_run), 5.0);
                let carved_y = min(base_surface_y, 64.0 + terrace_rise);
                let bank_target = round_away_from_zero(
                    base_surface_y * (1.0 - bank_influence) + carved_y * bank_influence,
                );
                surface_y = select(bank_target, 64.0, bank_run <= collar_width);
            }
        }
    }

    let wetland_pool = wetland_pool_influence >= 0.55 && channel_influence == 0.0;
    let watercourse_water = channel_influence > 0.0 || wetland_pool;
    let water = surface_y < 63.0 || watercourse_water;
    let display_y = select(
        surface_y,
        max(surface_y, water_y),
        water,
    );
    var visible_material = macro_material;
    if water {
        visible_material = 2.0;
    } else if macro_material != 8.0
        && bank_influence > 0.0
        && channel_influence == 0.0
        && surface_y <= water_y + 3.0 {
        let bank_texture =
            coast_realization_texture(relief, ridges, mountain_detail);
        visible_material = river_bank_material(
            base_surface_y,
            geometry.distance,
            half_width,
            bank_texture,
            coast,
        );
    }
    let surface_recipe = surface_recipe_code(
        surface_y,
        water_y,
        channel_influence,
        bank_influence,
        wetland_pool_influence,
        continentalness,
        relief,
        ruggedness,
        ridges,
        mountain_detail,
        temperature,
        coast,
    );
    let biome = biome_recipe_code(
        surface_y,
        channel_influence,
        wetland_pool_influence,
        bank_influence,
        wetland_influence,
        continentalness,
        ruggedness,
        ridges,
        temperature,
        moisture,
        coast,
    );
    let landform = landform_kind_code(
        channel_influence,
        wetland_pool_influence,
        wetland_influence,
        continentalness,
        relief,
        ruggedness,
        ridges,
        coast,
    );
    return HydrologyResult(
        vec4<f32>(
            surface_y,
            display_y,
            select(0.0, 1.0, water),
            visible_material,
        ),
        vec4<f32>(
            geometry.signed_distance,
            channel_influence,
            bank_influence,
            half_width,
        ),
        vec4<f32>(
            wetland_influence,
            wetland_pool_influence,
            submerged_outlet_influence,
            visible_material,
        ),
        vec4<f32>(0.0, biome, landform, surface_recipe),
    );
}

fn evaluate_point(world_x: i32, world_z: i32) -> TerrainPreviewSample {
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
    let mountain_detail_large = gradient_noise(
        MOUNTAIN_DETAIL_LARGE_DOMAIN,
        MOUNTAIN_DETAIL_LARGE_SCALE,
        f32(world_x) + large_warp_x,
        f32(world_z) + large_warp_z,
    );
    let mountain_detail_fine = gradient_noise(
        MOUNTAIN_DETAIL_FINE_DOMAIN,
        MOUNTAIN_DETAIL_FINE_SCALE,
        f32(world_x) + fine_warp_x,
        f32(world_z) + fine_warp_z,
    );
    let mountain_detail =
        clamp(mountain_detail_large * 0.70 + mountain_detail_fine * 0.30, -1.0, 1.0);

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

    let coast_selector = value_noise(
        COAST_DOMAIN,
        COAST_SCALE,
        world_x,
        world_z,
    );
    let coast = coast_intent(
        continentalness,
        relief,
        ruggedness,
        ridges,
        temperature,
        coast_selector,
    );
    let provisional_surface_y = land_surface_height(
        continentalness,
        relief,
        ruggedness,
        ridges,
        mountain_detail_large,
        mountain_detail_fine,
    );
    var base_surface_y = coast_adjusted_land_surface_height(
        provisional_surface_y,
        coast,
        continentalness,
        relief,
        ridges,
        mountain_detail,
    );
    var ocean_water = 0.0;
    var base_display_y = base_surface_y;
    if continentalness <= 0.0 {
        base_surface_y = ocean_floor(world_x, world_z, continentalness);
        base_display_y = 63.0;
        ocean_water = 1.0;
    }
    let macro_material = macro_surface_material(
        base_surface_y,
        continentalness,
        relief,
        ruggedness,
        ridges,
        mountain_detail,
        temperature,
        coast,
    );
    let geometry = river_geometry(
        world_x,
        world_z,
        relief_large,
        relief_detail,
        ruggedness_detail,
    );
    let hydrology = complete_hydrology(
        world_x,
        world_z,
        continentalness,
        relief,
        ruggedness,
        ridges,
        mountain_detail,
        temperature,
        moisture,
        base_surface_y,
        macro_material,
        geometry,
        coast,
    );

    var sample: TerrainPreviewSample;
    sample.terrain = vec4<f32>(
        hydrology.surface_display_water_material.x,
        hydrology.surface_display_water_material.y,
        continentalness,
        relief,
    );
    sample.climate = vec4<f32>(
        temperature,
        moisture,
        hydrology.surface_display_water_material.z,
        ruggedness,
    );
    sample.large_fields = vec4<f32>(
        base_surface_y,
        base_display_y,
        ocean_water,
        macro_material,
    );
    sample.hydrology = hydrology.hydrology;
    sample.hydrology_detail = hydrology.hydrology_detail;
    sample.semantics = hydrology.semantics;
    sample.forest_summary = vec4<f32>(0.0);
    sample.forest_detail = vec4<f32>(0.0);
    return sample;
}

fn point_forest_intent(world_x: i32, world_z: i32, sample: TerrainPreviewSample) -> ForestIntent {
    return forest_intent(
        world_x,
        world_z,
        sample.semantics.y,
        sample.climate.x,
        sample.climate.y,
    );
}

fn forest_footprint_summary(world_x: i32, world_z: i32, sample_spacing: i32) -> ForestIntent {
    let offset = sample_spacing / 4;
    let coordinates = array<vec2<i32>, 4>(
        vec2<i32>(world_x - offset, world_z - offset),
        vec2<i32>(world_x + offset, world_z - offset),
        vec2<i32>(world_x - offset, world_z + offset),
        vec2<i32>(world_x + offset, world_z + offset),
    );
    var intents: array<ForestIntent, 4>;
    var family_coverage = vec3<f32>(0.0);
    var coverage = 0.0;
    var density = 0.0;
    var grove = 0.0;
    var total_family_coverage = 0.0;
    for (var index = 0u; index < 4u; index += 1u) {
        let coordinate = coordinates[index];
        let point = evaluate_point(coordinate.x, coordinate.y);
        let intent = point_forest_intent(coordinate.x, coordinate.y, point);
        intents[index] = intent;
        coverage += intent.summary.x;
        density += intent.summary.y;
        grove += intent.detail.z;
        total_family_coverage += intent.summary.x;
        let family = u32(round(intent.summary.z));
        if family >= 1u && family <= 3u {
            family_coverage[family - 1u] += intent.summary.x;
        }
    }
    if total_family_coverage <= 0.000001 {
        return ForestIntent(vec4<f32>(0.0), vec4<f32>(0.0));
    }
    var dominant_index = 0u;
    if family_coverage.y > family_coverage.x {
        dominant_index = 1u;
    }
    if family_coverage.z > family_coverage[dominant_index] {
        dominant_index = 2u;
    }
    let mean_canopy_height = (
        intents[0].detail.x * intents[0].summary.x
        + intents[1].detail.x * intents[1].summary.x
        + intents[2].detail.x * intents[2].summary.x
        + intents[3].detail.x * intents[3].summary.x
    ) / total_family_coverage;
    var canopy_height_variance = 0.0;
    for (var index = 0u; index < 4u; index += 1u) {
        let mean_delta = intents[index].detail.x - mean_canopy_height;
        canopy_height_variance += intents[index].summary.x * (
            intents[index].detail.y * intents[index].detail.y
            + mean_delta * mean_delta
        );
    }
    return ForestIntent(
        vec4<f32>(
            coverage * 0.25,
            density * 0.25,
            f32(dominant_index + 1u),
            1.0 - family_coverage[dominant_index] / total_family_coverage,
        ),
        vec4<f32>(
            mean_canopy_height,
            sqrt(canopy_height_variance / total_family_coverage),
            grove * 0.25,
            1.0,
        ),
    );
}

@compute @workgroup_size(8, 8, 1)
fn compute_main(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let drawn_samples_per_axis = params.layer_samples_size.y;
    let halo_radius = terrain_sample_halo_radius;
    let storage_samples_per_axis = drawn_samples_per_axis + halo_radius * 2u;
    if invocation.x >= storage_samples_per_axis
        || invocation.y >= storage_samples_per_axis {
        return;
    }
    let sample_spacing = params.origin_spacing_cells.z;
    let logical_x = i32(invocation.x) - i32(halo_radius);
    let logical_z = i32(invocation.y) - i32(halo_radius);
    let world_x = params.origin_spacing_cells.x + logical_x * sample_spacing;
    let world_z = params.origin_spacing_cells.y + logical_z * sample_spacing;
    let height_index = invocation.y * storage_samples_per_axis + invocation.x;
    var sample = evaluate_point(world_x, world_z);
    normal_heights[height_index] = sample.terrain.y;
    let interior = logical_x >= 0
        && logical_z >= 0
        && logical_x < i32(drawn_samples_per_axis)
        && logical_z < i32(drawn_samples_per_axis);
    if !interior {
        return;
    }
    if params.content_stage_flags.x >= 4u {
        var forest = point_forest_intent(world_x, world_z, sample);
        if sample_spacing > 4 {
            forest = forest_footprint_summary(world_x, world_z, sample_spacing);
        }
        sample.forest_summary = forest.summary;
        sample.forest_detail = forest.detail;
    }
    let index = u32(logical_z) * drawn_samples_per_axis + u32(logical_x);
    gpu_samples[index] = sample;
}
