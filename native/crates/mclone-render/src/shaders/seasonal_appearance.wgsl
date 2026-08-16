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
            case 2u: { return mix(vec3<f32>(1.08, 0.78, 0.50), vec3<f32>(1.03, 0.88, 0.66), moisture); }
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

fn mclone_seasonal_hash(position: vec3<f32>) -> f32 {
    let cell = floor(position * vec3<f32>(2.0, 1.0, 2.0));
    return fract(sin(dot(cell, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
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
        let breakup = mclone_seasonal_hash(source_position);
        let snow_amount = smoothstep(breakup - 0.18, breakup + 0.18, total_snow);
        let snow_color = mix(color * 0.58, vec3<f32>(0.92, 0.95, 0.98), 0.66);
        color = mix(color, snow_color, snow_amount * 0.88);
    }

    var vegetation_visibility = 1.0;
    if (family == 2u) {
        vegetation_visibility -= weights.w * regional_strength * 0.28;
        vegetation_visibility -= total_snow * 0.86;
    } else if (family == 3u || family == 4u) {
        vegetation_visibility -= total_snow * 0.08;
    }
    return vec4<f32>(color, clamp(vegetation_visibility, 0.0, 1.0));
}
