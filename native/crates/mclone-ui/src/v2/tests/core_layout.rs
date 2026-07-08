use super::*;

#[test]
fn layout_hit_test_uses_topmost_enabled_widget() {
    let mut layout = UiLayout::new(Some(UiScreenId::Pause), 1);
    let rect = Rect::new(10.0, 20.0, 40.0, 20.0);
    layout.push(UiWidget::button(UiWidgetId(1), rect, "First"));
    layout.push(UiWidget::button(UiWidgetId(2), rect, "Second"));

    assert_eq!(layout.hit_test(point_in(rect)), Some(UiWidgetId(2)));
}

#[test]
fn layout_hit_test_ignores_disabled_widgets() {
    let mut layout = UiLayout::new(Some(UiScreenId::Pause), 1);
    let rect = Rect::new(10.0, 20.0, 40.0, 20.0);
    layout.push(UiWidget::button(UiWidgetId(1), rect, "Disabled").enabled(false));

    assert_eq!(layout.hit_test(point_in(rect)), None);
}

#[test]
fn pointer_up_activates_only_captured_widget() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Pause));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let resume = surface.layout().widgets()[0].rect;
    let options = surface.layout().widgets()[1].rect;

    assert!(surface.pointer_down(point_in(resume), GameUiRenderState::default()));
    let (_handled, action) = surface.pointer_up(point_in(options), GameUiRenderState::default());

    assert_eq!(action, None);
}

#[test]
fn pointer_move_updates_hover_without_action() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Pause));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let options = surface.layout().widgets()[1].rect;

    let (handled, action) = surface.pointer_move(point_in(options), GameUiRenderState::default());
    let debug = surface.debug_snapshot().expect("active surface has debug");

    assert!(handled);
    assert_eq!(action, None);
    assert_eq!(debug.hovered, Some(UI_V2_PAUSE_OPTIONS));
    assert_eq!(debug.captured, None);
}

#[test]
fn panel_revision_changes_for_visual_interaction_not_pointer_jitter() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Pause));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let options = surface.layout().widgets()[1].rect;
    let initial = surface.panel_revision().expect("active surface");

    surface.pointer_move(point_in(options), GameUiRenderState::default());
    let hovered = surface.panel_revision().expect("active surface");
    assert!(hovered.interaction > initial.interaction);

    surface.pointer_move(
        Point {
            x: options.x + 2.0,
            y: options.y + 2.0,
        },
        GameUiRenderState::default(),
    );
    assert_eq!(surface.panel_revision(), Some(hovered));

    surface.pointer_move(Point { x: 0.0, y: 0.0 }, GameUiRenderState::default());
    let unhovered = surface.panel_revision().expect("active surface");
    assert!(unhovered.interaction > hovered.interaction);
}

#[test]
fn game_ui_host_v2_panel_draw_cache_tracks_panel_revision() {
    let mut host = GameUiHost::new_ingame();
    host.set_screen(Some(GameScreen::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Display,
    }));
    host.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState {
        xr_turn_mode: Some(GameXrTurnMode::Snap15),
        server_cadence: Some(GameSimulationCadence::default()),
        ..GameUiRenderState::default()
    };

    let first = host
        .render_v2_panel_draw_list(state)
        .expect("Options is a v2 panel");
    assert_eq!(first.cache, UiDrawCacheStats::rebuild());
    assert!(!first.draw.commands().is_empty());

    let second = host
        .render_v2_panel_draw_list(state)
        .expect("Options is a v2 panel");
    assert_eq!(second.cache, UiDrawCacheStats::cache_hit());
    assert_eq!(second.revision, first.revision);
    assert_eq!(second.draw, first.draw);

    let snapshot = host.v2_debug_snapshot().expect("Options has debug data");
    let crosshair = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_OPTIONS_CROSSHAIR)
        .expect("crosshair row")
        .rect;
    host.pointer_move(point_in(crosshair));

    let hovered = host
        .render_v2_panel_draw_list(state)
        .expect("Options is a v2 panel");
    assert_eq!(hovered.cache, UiDrawCacheStats::rebuild());
    assert_ne!(hovered.revision, first.revision);

    host.pointer_move(Point {
        x: crosshair.x + 2.0,
        y: crosshair.y + 2.0,
    });
    let jitter = host
        .render_v2_panel_draw_list(state)
        .expect("Options is a v2 panel");
    assert_eq!(jitter.cache, UiDrawCacheStats::cache_hit());
    assert_eq!(jitter.revision, hovered.revision);
    assert_eq!(jitter.draw, hovered.draw);
}

#[test]
fn game_ui_host_v2_panel_draw_cache_covers_server_settings() {
    let mut host = GameUiHost::new();
    host.set_screen(Some(GameScreen::ServerSettings {
        parent: GameOptionsParent::Pause,
    }));
    host.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState {
        server_cadence: Some(GameSimulationCadence::new(20, 20, 60)),
        ..GameUiRenderState::default()
    };

    let first = host
        .render_v2_panel_draw_list(state)
        .expect("ServerSettings is a v2 panel");
    assert_eq!(first.cache, UiDrawCacheStats::rebuild());
    assert!(!first.draw.commands().is_empty());

    let second = host
        .render_v2_panel_draw_list(state)
        .expect("ServerSettings is a v2 panel");
    assert_eq!(second.cache, UiDrawCacheStats::cache_hit());
    assert_eq!(second.revision, first.revision);
}
