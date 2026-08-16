fn mclone_seasonal_landmark_weight(phase: f32, landmark: f32) -> f32 {
    let distance = fract(phase - landmark + 0.5) - 0.5;
    return pow(max(cos(distance * 6.283185307), 0.0), 2.0);
}

fn mclone_seasonal_moisture(class_index: u32) -> f32 {
    switch class_index {
        case 0u: { return 0.05; }
        case 1u: { return 0.30; }
        case 2u: { return 0.65; }
        default: { return 0.95; }
    }
}

fn mclone_seasonal_target(family: u32, season: u32, moisture: f32) -> vec3<f32> {
    if (family == 2u) {
        switch season {
            case 0u: { return mix(vec3<f32>(0.98, 1.01, 0.96), vec3<f32>(0.88, 1.12, 0.84), moisture); }
            case 1u: { return vec3<f32>(1.0); }
            case 2u: { return mix(vec3<f32>(1.22, 0.60, 0.26), vec3<f32>(1.05, 0.86, 0.58), moisture); }
            default: { return vec3<f32>(0.72, 0.76, 0.68); }
        }
    }
    if (family == 3u) {
        switch season {
            case 0u: { return vec3<f32>(0.90, 1.10, 0.86); }
            case 1u: { return vec3<f32>(1.0); }
            case 2u: { return vec3<f32>(1.18, 0.58, 0.30); }
            default: { return vec3<f32>(0.64, 0.60, 0.52); }
        }
    }
    if (family == 4u) {
        switch season {
            case 0u: { return vec3<f32>(0.96, 1.03, 0.95); }
            case 1u: { return vec3<f32>(1.0); }
            case 2u: { return vec3<f32>(0.93, 0.95, 0.85); }
            default: { return vec3<f32>(0.70, 0.84, 0.83); }
        }
    }
    return vec3<f32>(1.0);
}

fn mclone_seasonal_nearest_delta(value: f32, center: f32, period: f32) -> f32 {
    let delta = value - center;
    if (period <= 0.0) {
        return delta;
    }
    return delta - round(delta / period) * period;
}

fn mclone_seasonal_hash(cell: vec2<f32>) -> f32 {
    var point = fract(vec3<f32>(cell.x, cell.y, cell.x) * 0.1031);
    point = point + vec3<f32>(dot(point, point.yzx + vec3<f32>(33.33)));
    return fract((point.x + point.y) * point.z);
}

fn mclone_seasonal_wrapped_cell(cell: f32, cell_count: f32) -> f32 {
    if (cell_count <= 0.0) {
        return cell;
    }
    return cell - floor(cell / cell_count) * cell_count;
}

fn mclone_seasonal_value_noise(
    position: vec2<f32>,
    topology_periods: vec2<f32>,
    cell_scale: f32,
) -> f32 {
    let cell_counts = vec2<f32>(
        select(0.0, max(round(topology_periods.x / cell_scale), 1.0), topology_periods.x > 0.0),
        select(0.0, max(round(topology_periods.y / cell_scale), 1.0), topology_periods.y > 0.0),
    );
    let safe_periods = max(topology_periods, vec2<f32>(1.0));
    let sample = vec2<f32>(
        select(position.x / cell_scale, position.x / safe_periods.x * cell_counts.x, topology_periods.x > 0.0),
        select(position.y / cell_scale, position.y / safe_periods.y * cell_counts.y, topology_periods.y > 0.0),
    );
    let cell = floor(sample);
    let fraction = fract(sample);
    let blend = fraction * fraction * (vec2<f32>(3.0) - 2.0 * fraction);
    let cell_00 = vec2<f32>(
        mclone_seasonal_wrapped_cell(cell.x, cell_counts.x),
        mclone_seasonal_wrapped_cell(cell.y, cell_counts.y),
    );
    let cell_10 = vec2<f32>(
        mclone_seasonal_wrapped_cell(cell.x + 1.0, cell_counts.x),
        cell_00.y,
    );
    let cell_01 = vec2<f32>(
        cell_00.x,
        mclone_seasonal_wrapped_cell(cell.y + 1.0, cell_counts.y),
    );
    let cell_11 = vec2<f32>(cell_10.x, cell_01.y);
    let lower = mix(mclone_seasonal_hash(cell_00), mclone_seasonal_hash(cell_10), blend.x);
    let upper = mix(mclone_seasonal_hash(cell_01), mclone_seasonal_hash(cell_11), blend.x);
    return mix(lower, upper, blend.y);
}

fn mclone_seasonal_cell_noise(
    position: vec2<f32>,
    topology_periods: vec2<f32>,
    cell_scale: f32,
) -> f32 {
    let cell_counts = vec2<f32>(
        select(0.0, max(round(topology_periods.x / cell_scale), 1.0), topology_periods.x > 0.0),
        select(0.0, max(round(topology_periods.y / cell_scale), 1.0), topology_periods.y > 0.0),
    );
    let safe_periods = max(topology_periods, vec2<f32>(1.0));
    let sample = vec2<f32>(
        select(position.x / cell_scale, position.x / safe_periods.x * cell_counts.x, topology_periods.x > 0.0),
        select(position.y / cell_scale, position.y / safe_periods.y * cell_counts.y, topology_periods.y > 0.0),
    );
    let cell = floor(sample);
    return mclone_seasonal_hash(vec2<f32>(
        mclone_seasonal_wrapped_cell(cell.x, cell_counts.x),
        mclone_seasonal_wrapped_cell(cell.y, cell_counts.y),
    ));
}

fn mclone_seasonal_snow_breakup(
    position: vec3<f32>,
    topology_periods: vec2<f32>,
) -> f32 {
    let terrain_position = position.xz + vec2<f32>(position.y * 0.31, -position.y * 0.23);
    let broad = mclone_seasonal_value_noise(terrain_position, topology_periods, 14.0);
    let edge = mclone_seasonal_cell_noise(
        terrain_position + vec2<f32>(31.0, -47.0),
        topology_periods,
        3.5,
    );
    return clamp(broad * 0.78 + edge * 0.22, 0.0, 1.0);
}

/// Returns transformed RGB and vegetation visibility in alpha. `season_local`
/// is [enabled, local phase, response strength, thermal forcing]. `snow_pulse`
/// is [canonical center x/z, radius, intensity].
fn mclone_seasonal_surface(
    base_color: vec3<f32>,
    packed_response: u32,
    source_position: vec3<f32>,
    season_local: vec4<f32>,
    snow_pulse: vec4<f32>,
    topology_periods: vec2<f32>,
) -> vec4<f32> {
    let response_key = packed_response & 255u;
    let family = response_key & 7u;
    if (season_local.x < 0.5 || family == 0u) {
        return vec4<f32>(base_color, 1.0);
    }

    let upward_exposed = (response_key & 8u) != 0u;
    let temperature = -0.75 + f32((response_key >> 4u) & 3u) * 0.5;
    let moisture = mclone_seasonal_moisture((response_key >> 6u) & 3u);
    let warmth = clamp((temperature + 1.0) * 0.5, 0.0, 1.0);
    let regional_strength = clamp(season_local.z * (1.0 - warmth * moisture * 0.58), 0.0, 1.0);
    let phase = fract(season_local.y);
    let weights = vec4<f32>(
        mclone_seasonal_landmark_weight(phase, 0.0),
        mclone_seasonal_landmark_weight(phase, 0.25),
        mclone_seasonal_landmark_weight(phase, 0.5),
        mclone_seasonal_landmark_weight(phase, 0.75),
    );
    let total_weight = max(dot(weights, vec4<f32>(1.0)), 0.000001);
    var tint_target = vec3<f32>(0.0);
    for (var season = 0u; season < 4u; season += 1u) {
        tint_target += mclone_seasonal_target(family, season, moisture)
            * weights[season] / total_weight;
    }
    var color = base_color * mix(vec3<f32>(1.0), tint_target, regional_strength);

    let current_temperature = clamp(temperature + season_local.w * 0.6, -1.0, 1.0);
    let retention = 1.0 - smoothstep(-0.45, 0.18, current_temperature);
    var snow_weight = 0.0;
    if (upward_exposed) {
        switch family {
            case 1u: { snow_weight = 1.0; }
            case 2u: { snow_weight = 0.9; }
            case 3u: { snow_weight = 0.52; }
            case 4u: { snow_weight = 0.68; }
            default: {}
        }
    }
    let seasonal_snow = clamp(
        weights.w * season_local.z * retention * (0.45 + moisture * 0.55) * snow_weight,
        0.0,
        1.0,
    );
    var recent_snow = 0.0;
    if (snow_pulse.z > 0.0 && snow_pulse.w > 0.0 && snow_weight > 0.0) {
        let offset = vec2<f32>(
            mclone_seasonal_nearest_delta(source_position.x, snow_pulse.x, topology_periods.x),
            mclone_seasonal_nearest_delta(source_position.z, snow_pulse.y, topology_periods.y),
        );
        let normalized_distance = clamp(length(offset) / snow_pulse.z, 0.0, 1.0);
        let falloff = 1.0 - smoothstep(0.62, 1.0, normalized_distance);
        recent_snow = falloff * snow_pulse.w * retention * snow_weight;
    }
    let total_snow = clamp(seasonal_snow + recent_snow, 0.0, 1.0);
    if (total_snow > 0.0) {
        let breakup = mclone_seasonal_snow_breakup(source_position, topology_periods);
        let snow_amount = smoothstep(breakup - 0.18, breakup + 0.18, total_snow);
        let snow_color = mix(color * 0.58, vec3<f32>(0.92, 0.95, 0.98), 0.66);
        color = mix(color, snow_color, snow_amount * 0.88);
    }

    var vegetation_visibility = 1.0;
    if (family == 2u) {
        let temperate_fit = smoothstep(-0.65, -0.15, temperature)
            * (1.0 - smoothstep(0.55, 0.85, temperature));
        let autumn_dormancy = weights.z
            * regional_strength
            * temperate_fit
            * (0.76 - moisture * 0.36);
        vegetation_visibility -= autumn_dormancy;
        vegetation_visibility -= weights.w * regional_strength * 0.28;
        vegetation_visibility -= total_snow * 0.86;
    } else if (family == 3u || family == 4u) {
        vegetation_visibility -= total_snow * 0.08;
    }
    return vec4<f32>(color, clamp(vegetation_visibility, 0.0, 1.0));
}
