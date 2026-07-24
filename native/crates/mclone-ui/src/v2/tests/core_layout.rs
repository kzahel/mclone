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
fn controller_navigation_focuses_and_activates_stable_pause_order() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Pause));
    surface.set_scale(GuiScale::from_pixels(960, 540));

    assert_eq!(
        surface.navigate(GuiNavigation::Down, GameUiRenderState::default()),
        (true, None)
    );
    assert_eq!(
        surface.debug_snapshot().and_then(|debug| debug.focused),
        Some(UI_V2_PAUSE_RESUME)
    );
    surface.navigate(GuiNavigation::Down, GameUiRenderState::default());
    assert_eq!(
        surface.debug_snapshot().and_then(|debug| debug.focused),
        Some(UI_V2_PAUSE_OPTIONS)
    );
    assert_eq!(
        surface.navigate(GuiNavigation::Confirm, GameUiRenderState::default()),
        (
            true,
            Some(GameUiAction::OpenOptions(GameOptionsParent::Pause))
        )
    );
}

#[test]
fn controller_focusability_skips_disabled_widgets() {
    let mut layout = UiLayout::new(Some(UiScreenId::Pause), 1);
    layout.push(
        UiWidget::button(UiWidgetId(1), Rect::new(0.0, 0.0, 20.0, 10.0), "Disabled")
            .action(GameUiAction::Resume)
            .enabled(false),
    );
    layout.push(
        UiWidget::button(UiWidgetId(2), Rect::new(0.0, 20.0, 20.0, 10.0), "First")
            .action(GameUiAction::Resume),
    );
    layout.push(
        UiWidget::button(UiWidgetId(3), Rect::new(0.0, 40.0, 20.0, 10.0), "Second")
            .action(GameUiAction::QuitToTitle),
    );
    assert!(!widget_is_focusable(&layout.widgets()[0]));
    assert!(widget_is_focusable(&layout.widgets()[1]));
}

#[test]
fn controller_focus_matches_pointer_highlight_for_every_widget_kind() {
    let rect = Rect::new(10.0, 20.0, 160.0, 20.0);
    let widgets = [
        UiWidget::button(UiWidgetId(1), rect, "Button").action(GameUiAction::Resume),
        UiWidget::checkbox(UiWidgetId(2), rect, "Checkbox", true)
            .action(GameUiAction::ToggleSectionOcclusion),
        UiWidget::cycle(UiWidgetId(3), rect, "Cycle", "Value")
            .action(GameUiAction::CycleFramePacing),
        UiWidget::world_row(UiWidgetId(4), rect, "World", "Seed 123", false, false, true)
            .action(GameUiAction::Resume),
        UiWidget::world_row(
            UiWidgetId(8),
            rect,
            "Selected World",
            "Seed 456",
            true,
            false,
            true,
        )
        .action(GameUiAction::Resume),
        UiWidget {
            id: UiWidgetId(5),
            kind: UiWidgetKind::AssetPackRow {
                checked: true,
                locked: false,
                status: AssetPackUiRowStatus::Enabled,
            },
            rect,
            label: "Asset Pack".to_owned(),
            enabled: true,
            value: Some("First party".to_owned()),
            action: Some(UiWidgetAction::Static(GameUiAction::Resume)),
        },
        UiWidget::palette_slot(UiWidgetId(6), rect, "Palette", None).action(GameUiAction::Resume),
        UiWidget::slider(UiWidgetId(7), rect, "Slider", 0.5)
            .slider_action(UiSliderAction::RenderDistance),
    ];
    let surface = UiSurface::new();

    for widget in widgets {
        let mut normal = GuiDrawList::new();
        surface.render_widget(&mut normal, &widget, Interaction::default());
        let mut hovered = GuiDrawList::new();
        surface.render_widget(
            &mut hovered,
            &widget,
            Interaction {
                pointer: Some(point_in(widget.rect)),
                ..Interaction::default()
            },
        );
        let mut focused = GuiDrawList::new();
        surface.render_widget(
            &mut focused,
            &widget,
            Interaction {
                focused: Some(widget.id.legacy_widget_id()),
                ..Interaction::default()
            },
        );

        assert_eq!(
            focused, hovered,
            "controller focus diverged from pointer highlight for {:?}",
            widget.kind,
        );
        assert_ne!(
            focused, normal,
            "focused {:?} has no visible highlight",
            widget.kind,
        );
    }
}

#[test]
fn controller_navigation_wraps_stable_pause_order() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Pause));

    surface.navigate(GuiNavigation::Up, GameUiRenderState::default());
    assert_eq!(
        surface.debug_snapshot().and_then(|debug| debug.focused),
        Some(UI_V2_PAUSE_QUIT_TO_TITLE)
    );
    surface.navigate(GuiNavigation::Down, GameUiRenderState::default());
    assert_eq!(
        surface.debug_snapshot().and_then(|debug| debug.focused),
        Some(UI_V2_PAUSE_RESUME)
    );
}

#[test]
fn pointer_activity_clears_controller_focus() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Pause));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.navigate(GuiNavigation::Down, GameUiRenderState::default());
    assert!(
        surface
            .debug_snapshot()
            .is_some_and(|debug| debug.focused.is_some())
    );

    let options = surface.layout().widgets()[1].rect;
    surface.pointer_move(point_in(options), GameUiRenderState::default());
    let debug = surface.debug_snapshot().expect("pause debug snapshot");
    assert_eq!(debug.focused, None);
    assert_eq!(debug.hovered, Some(UI_V2_PAUSE_OPTIONS));
}

#[test]
fn controller_left_right_adjusts_focused_slider() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Graphics,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState::default();
    surface.set_render_state(state);
    surface.ensure_layout();
    surface.set_focus(Some(UI_V2_OPTIONS_RADIUS));

    let (handled, action) = surface.navigate(GuiNavigation::Right, state);
    assert!(handled);
    assert!(matches!(action, Some(GameUiAction::SetRenderDistance(_))));
}

#[test]
fn controller_back_uses_the_same_screen_policy_as_escape() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Options {
        parent: GameOptionsParent::Pause,
    }));

    assert_eq!(
        surface.navigate(GuiNavigation::Back, GameUiRenderState::default()),
        (true, Some(GameUiAction::BackToPause))
    );
}

#[test]
fn every_non_text_menu_surface_has_a_controller_target() {
    let screens = [
        UiScreenId::Title,
        UiScreenId::PreparingLobby,
        UiScreenId::WorldList,
        UiScreenId::WorldCreate,
        UiScreenId::WorldDeleteConfirm {
            id: WorldCatalogUiWorldId(1),
        },
        UiScreenId::NewWorld,
        UiScreenId::JoinRemote,
        UiScreenId::Pause,
        UiScreenId::Death {
            cause: GameDeathCause::Lava,
        },
        UiScreenId::Options {
            parent: GameOptionsParent::Pause,
        },
        UiScreenId::OptionsCategory {
            parent: GameOptionsParent::Pause,
            category: GameOptionsCategory::Graphics,
        },
        UiScreenId::OptionsCategory {
            parent: GameOptionsParent::Pause,
            category: GameOptionsCategory::Movement,
        },
        UiScreenId::OptionsCategory {
            parent: GameOptionsParent::Pause,
            category: GameOptionsCategory::Display,
        },
        UiScreenId::OptionsCategory {
            parent: GameOptionsParent::Pause,
            category: GameOptionsCategory::Debug,
        },
        UiScreenId::OptionsCategory {
            parent: GameOptionsParent::Pause,
            category: GameOptionsCategory::StorageProfile,
        },
        UiScreenId::ServerSettings {
            parent: GameOptionsParent::Pause,
        },
        UiScreenId::AssetPacks {
            parent: GameOptionsParent::Pause,
        },
        UiScreenId::StorageConfirm {
            parent: GameOptionsParent::Pause,
            action: GameStorageAction::ResetPlayerIdentity,
        },
        UiScreenId::Help {
            parent: GameHelpParent::Pause,
        },
    ];
    let mut surface = UiSurface::new();
    surface.set_scale(GuiScale::from_pixels(960, 540));
    for screen in screens {
        surface.set_screen(Some(screen));
        surface.navigate(GuiNavigation::Down, GameUiRenderState::default());
        assert!(
            surface
                .debug_snapshot()
                .is_some_and(|snapshot| snapshot.focused.is_some()),
            "{screen:?} has no controller-focusable target",
        );
    }
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
