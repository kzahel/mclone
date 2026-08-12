fn mclone_fog_factor(
    world_position: vec3<f32>,
    camera_position: vec4<f32>,
    render_options: vec4<f32>,
    fog_color: vec4<f32>,
    fog_distances: vec4<f32>,
) -> f32 {
    let options = bitcast<u32>(render_options.z);
    let mode = options & 3u;
    if (mode == 0u) {
        return 0.0;
    }

    let fog_distance = distance(world_position, camera_position.xyz);
    var atmosphere = 0.0;
    if (mode == 1u) {
        let fog_start = fog_distances.x;
        let fog_end = max(fog_distances.y, fog_start + 0.001);
        atmosphere = clamp((fog_distance - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    } else {
        let visibility = max(fog_distances.x, 0.001);
        let distance_ratio = fog_distance / visibility;
        var optical_depth = 2.995732 * distance_ratio;
        if ((options & 4u) != 0u) {
            optical_depth = 2.995732 * distance_ratio * distance_ratio;
        }
        if (mode == 3u) {
            let falloff = max(f32((options >> 4u) & 1023u), 1.0);
            let midpoint_y = 0.5 * (camera_position.y + world_position.y);
            let height_density = exp2(-max(midpoint_y - camera_position.w, 0.0) / falloff);
            optical_depth *= height_density;
        }
        atmosphere = (1.0 - exp(-optical_depth)) * clamp(fog_color.a, 0.0, 1.0);
    }
    if ((options & 8u) == 0u) {
        return atmosphere;
    }

    let coverage_end = max(fog_distances.y, 0.001);
    let guard_ratio = f32((options >> 14u) & 127u) / 127.0;
    let guard_start = coverage_end * guard_ratio;
    let coverage_guard =
        clamp((fog_distance - guard_start) / max(coverage_end - guard_start, 0.001), 0.0, 1.0);
    return 1.0 - (1.0 - atmosphere) * (1.0 - coverage_guard);
}
