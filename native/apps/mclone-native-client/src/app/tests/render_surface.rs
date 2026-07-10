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
