use super::*;

#[test]
fn game_ui_host_loading_progress_fullscreen_cache_tracks_overlay_state() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut host = GameUiHost::new_ingame();
    let progress = loading_progress_overlay(3, LoadingProgressCellStatus::Features);

    let first = host.render_loading_progress_draw_list(
        scale,
        &progress,
        LoadingProgressOverlayLayer::fullscreen(),
    );
    assert_eq!(first.retained_cache, UiDrawCacheStats::rebuild());
    assert!(!first.draw.commands().is_empty());

    let second = host.render_loading_progress_draw_list(
        scale,
        &progress,
        LoadingProgressOverlayLayer::fullscreen(),
    );
    assert_eq!(second.retained_cache, UiDrawCacheStats::cache_hit());
    assert_eq!(second.draw, first.draw);

    let percent_changed = loading_progress_overlay(4, LoadingProgressCellStatus::Features);
    let percent_draw = host.render_loading_progress_draw_list(
        scale,
        &percent_changed,
        LoadingProgressOverlayLayer::fullscreen(),
    );
    assert_eq!(percent_draw.retained_cache, UiDrawCacheStats::rebuild());
    assert_ne!(percent_draw.draw, second.draw);

    let cell_changed = loading_progress_overlay(4, LoadingProgressCellStatus::Light);
    let cell_draw = host.render_loading_progress_draw_list(
        scale,
        &cell_changed,
        LoadingProgressOverlayLayer::fullscreen(),
    );
    assert_eq!(cell_draw.retained_cache, UiDrawCacheStats::rebuild());
    assert_ne!(cell_draw.draw, percent_draw.draw);
}

#[test]
fn game_ui_host_loading_progress_panel_cache_tracks_placement_and_variant() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut host = GameUiHost::new_ingame();
    let progress = loading_progress_overlay(3, LoadingProgressCellStatus::Features);
    let panel = LoadingProgressOverlayLayer::panel(Point { x: 800.0, y: 4.0 }, "VIEW");

    let fullscreen = host.render_loading_progress_draw_list(
        scale,
        &progress,
        LoadingProgressOverlayLayer::fullscreen(),
    );
    assert_eq!(fullscreen.retained_cache, UiDrawCacheStats::rebuild());

    let first_panel = host.render_loading_progress_draw_list(scale, &progress, panel.clone());
    assert_eq!(first_panel.retained_cache, UiDrawCacheStats::rebuild());
    assert_ne!(first_panel.draw, fullscreen.draw);

    let second_panel = host.render_loading_progress_draw_list(scale, &progress, panel.clone());
    assert_eq!(second_panel.retained_cache, UiDrawCacheStats::cache_hit());
    assert_eq!(second_panel.draw, first_panel.draw);

    let moved_panel = host.render_loading_progress_draw_list(
        scale,
        &progress,
        LoadingProgressOverlayLayer::panel(Point { x: 760.0, y: 8.0 }, "VIEW"),
    );
    assert_eq!(moved_panel.retained_cache, UiDrawCacheStats::rebuild());
    assert_ne!(moved_panel.draw, second_panel.draw);

    let fullscreen_again = host.render_loading_progress_draw_list(
        scale,
        &progress,
        LoadingProgressOverlayLayer::fullscreen(),
    );
    assert_eq!(
        fullscreen_again.retained_cache,
        UiDrawCacheStats::cache_hit()
    );
    assert_eq!(fullscreen_again.draw, fullscreen.draw);
}

#[test]
fn game_ui_host_loading_progress_draw_matches_standalone_renderers() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut host = GameUiHost::new_ingame();
    let progress = loading_progress_overlay(3, LoadingProgressCellStatus::Features);

    let retained_fullscreen = host.render_loading_progress_draw_list(
        scale,
        &progress,
        LoadingProgressOverlayLayer::fullscreen(),
    );
    let mut standalone_fullscreen = GuiDrawList::new();
    render_loading_progress_overlay(scale, &mut standalone_fullscreen, &progress);
    assert_eq!(retained_fullscreen.draw, standalone_fullscreen);

    let origin = Point { x: 800.0, y: 4.0 };
    let retained_panel = host.render_loading_progress_draw_list(
        scale,
        &progress,
        LoadingProgressOverlayLayer::panel(origin, "VIEW"),
    );
    let mut standalone_panel = GuiDrawList::new();
    render_loading_progress_panel_at(scale, &mut standalone_panel, &progress, origin, "VIEW");
    assert_eq!(retained_panel.draw, standalone_panel);
}

#[test]
fn game_ui_host_loading_progress_cache_is_independent_from_menu_and_hud() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut host = GameUiHost::new();
    host.set_scale(scale);
    let state = GameUiRenderState::default();
    let first_panel = host
        .render_v2_panel_draw_list(state)
        .expect("Title is a v2 panel");
    assert_eq!(first_panel.cache, UiDrawCacheStats::rebuild());

    let mut hud_host = GameUiHost::new_ingame();
    let mut hud = FlatHud::new(keyboard_mouse_input());
    hud.hotbar = FlatHotbarOverlay::selected(2);
    let first_hud = hud_host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        first_hud.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 2,
            cache_hit_count: 0,
        }
    );

    let progress = loading_progress_overlay(3, LoadingProgressCellStatus::Features);
    let changed_progress = loading_progress_overlay(4, LoadingProgressCellStatus::Light);
    assert_eq!(
        host.render_loading_progress_draw_list(
            scale,
            &progress,
            LoadingProgressOverlayLayer::fullscreen(),
        )
        .retained_cache,
        UiDrawCacheStats::rebuild()
    );
    assert_eq!(
        host.render_loading_progress_draw_list(
            scale,
            &changed_progress,
            LoadingProgressOverlayLayer::fullscreen(),
        )
        .retained_cache,
        UiDrawCacheStats::rebuild()
    );

    let second_panel = host
        .render_v2_panel_draw_list(state)
        .expect("Title is a v2 panel");
    assert_eq!(second_panel.cache, UiDrawCacheStats::cache_hit());
    assert_eq!(second_panel.draw, first_panel.draw);

    let second_hud = hud_host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        second_hud.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 0,
            cache_hit_count: 2,
        }
    );
    assert_eq!(second_hud.draw, first_hud.draw);
}
