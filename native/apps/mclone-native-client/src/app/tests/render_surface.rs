use super::*;

#[test]
fn desktop_render_scale_presets_cycle_from_default() {
    assert_eq!(next_desktop_render_scale(1.0), 0.5);
    assert_eq!(next_desktop_render_scale(0.5), 0.75);
    assert_eq!(next_desktop_render_scale(0.75), 1.5);
    assert_eq!(next_desktop_render_scale(1.5), 1.0);
    assert_eq!(next_desktop_render_scale(1.25), 1.0);
}

#[test]
fn desktop_profile_does_not_apply_an_automatic_world_cap() {
    assert_eq!(
        automatic_world_render_scale(WindowPlatformProfile::Desktop, [3840, 2160]),
        None
    );
}

#[test]
fn steamos_profile_keeps_handheld_output_native() {
    assert_eq!(
        automatic_world_render_scale(WindowPlatformProfile::SteamOs, [1280, 800]),
        Some(1.0)
    );
}

#[test]
fn steamos_profile_caps_four_k_world_render_at_1080p() {
    let scale = automatic_world_render_scale(WindowPlatformProfile::SteamOs, [3840, 2160])
        .expect("SteamOS scale");

    assert!((scale - 0.5).abs() < f32::EPSILON);
    assert_eq!(scaled_frame_size([3840, 2160], scale), [1920, 1080]);
}

#[test]
fn steamos_profile_preserves_external_display_aspect_within_budget() {
    for output in [[2560, 1600], [3440, 1440]] {
        let scale = automatic_world_render_scale(WindowPlatformProfile::SteamOs, output)
            .expect("SteamOS scale");
        let world = scaled_frame_size(output, scale);

        assert!(world[1] <= STEAMOS_WORLD_RENDER_MAX_HEIGHT);
        assert!(
            u64::from(world[0]) * u64::from(world[1])
                <= STEAMOS_WORLD_RENDER_MAX_PIXELS + u64::from(world[0] + world[1])
        );
        let output_aspect = output[0] as f64 / output[1] as f64;
        let world_aspect = world[0] as f64 / world[1] as f64;
        assert!((output_aspect - world_aspect).abs() < 0.002);
    }
}

#[test]
fn gui_point_from_physical_cursor_uses_surface_gui_scale() {
    let point = gui_point_from_physical_cursor((640.0, 450.0), [1280, 900], [1280, 900]);

    assert!((point.x - 640.0 / 3.0).abs() < f32::EPSILON);
    assert_eq!(point.y, 150.0);
}

#[test]
fn gui_point_from_physical_cursor_maps_window_to_surface_size() {
    let point = gui_point_from_physical_cursor((640.0, 360.0), [1280, 720], [2560, 1440]);
    let scale = GuiScale::from_pixels(2560, 1440);

    assert_eq!(
        point,
        Point {
            x: scale.width * 0.5,
            y: scale.height * 0.5,
        }
    );
}
