use super::*;

#[test]
fn game_ui_host_has_title_and_ingame_start_modes() {
    let title_ui = GameUiHost::new();
    assert!(title_ui.is_active());
    assert!(title_ui.covers_world());
    assert_eq!(title_ui.screen(), Some(GameScreen::Title));

    let ingame_ui = GameUiHost::new_ingame();
    assert!(!ingame_ui.is_active());
    assert!(!ingame_ui.covers_world());
    assert_eq!(ingame_ui.screen(), None);
}

#[test]
fn pause_buttons_emit_expected_actions() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Pause));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let buttons = surface.layout().widgets().to_vec();
    let expected = [
        GameUiAction::Resume,
        GameUiAction::OpenOptions(GameOptionsParent::Pause),
        GameUiAction::QuitToTitle,
    ];

    for (button, expected) in buttons.iter().zip(expected) {
        let point = point_in(button.rect);
        assert!(surface.pointer_down(point, GameUiRenderState::default()));
        let (_handled, action) = surface.pointer_up(point, GameUiRenderState::default());
        assert_eq!(action, Some(expected));
    }
}

#[test]
fn pause_render_uses_committed_layout() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Pause));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let layout_revision = surface.layout().revision;

    let draw = surface.render_draw_list(GameUiRenderState::default());

    assert_eq!(surface.layout().revision, layout_revision);
    assert!(!draw.commands().is_empty());
}

#[test]
fn options_layout_includes_conditional_rows_from_frame_state() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Options {
        parent: GameOptionsParent::Pause,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState {
        crosshair_visible: None,
        xr_turn_mode: Some(GameXrTurnMode::Snap15),
        touch_controls_mode: Some(TouchControlsMode::Auto),
        touch_settings: Some(GameTouchSettings::new(2.0, 1.0, 5.0)),
        server_cadence: Some(GameSimulationCadence::default()),
        ..GameUiRenderState::default()
    });

    let layout = surface.layout();

    assert!(layout.widget(UI_V2_OPTIONS_CROSSHAIR).is_none());
    assert!(layout.widget(UI_V2_OPTIONS_XR_TURN_MODE).is_some());
    assert!(layout.widget(UI_V2_OPTIONS_TOUCH_CONTROLS).is_some());
    assert!(layout.widget(UI_V2_OPTIONS_TOUCH_LOOK).is_some());
    assert!(layout.widget(UI_V2_OPTIONS_SERVER_SETTINGS).is_some());
}

#[test]
fn game_ui_host_pointer_input_uses_committed_render_state() {
    let mut host = GameUiHost::new_ingame();
    host.set_screen(Some(GameScreen::Options {
        parent: GameOptionsParent::Pause,
    }));
    host.set_scale(GuiScale::from_pixels(960, 540));

    let committed_state = GameUiRenderState::default();
    host.commit_render_state(committed_state);
    let committed_snapshot = host.v2_debug_snapshot().expect("Options is a v2 screen");
    let first_person = committed_snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_OPTIONS_FIRST_PERSON_PLAYER)
        .expect("First Person Body row exists")
        .rect;
    let point = Point {
        x: first_person.x + 16.0,
        y: first_person.bottom() - 2.0,
    };

    let mut divergent_state = committed_state;
    divergent_state.touch_controls_mode = Some(TouchControlsMode::Auto);
    let mut divergent_surface = UiSurface::new();
    divergent_surface.set_screen(Some(UiScreenId::Options {
        parent: GameOptionsParent::Pause,
    }));
    divergent_surface.set_scale(GuiScale::from_pixels(960, 540));
    let (_handled, _action) = divergent_surface.pointer_move(point, divergent_state);
    let divergent_snapshot = divergent_surface
        .debug_snapshot()
        .expect("divergent Options surface is active");

    assert_eq!(hovered_label(&divergent_snapshot), Some("Crosshair"));
    assert!(host.pointer_down(point));
    let (_handled, action) = host.pointer_up(point);

    assert_eq!(action, Some(GameUiAction::ToggleFirstPersonPlayer));
}

#[test]
fn options_buttons_emit_expected_actions_from_committed_rects() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Options {
        parent: GameOptionsParent::Pause,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState {
        xr_turn_mode: Some(GameXrTurnMode::Snap15),
        server_cadence: Some(GameSimulationCadence::default()),
        ..GameUiRenderState::default()
    });
    let first_person = surface
        .layout()
        .widget(UI_V2_OPTIONS_FIRST_PERSON_PLAYER)
        .expect("first person row")
        .rect;
    let crosshair = surface
        .layout()
        .widget(UI_V2_OPTIONS_CROSSHAIR)
        .expect("crosshair row")
        .rect;
    let server_settings = surface
        .layout()
        .widget(UI_V2_OPTIONS_SERVER_SETTINGS)
        .expect("server settings row")
        .rect;
    let xr_turn = surface
        .layout()
        .widget(UI_V2_OPTIONS_XR_TURN_MODE)
        .expect("XR turn row")
        .rect;
    let frame_metrics = surface
        .layout()
        .widget(UI_V2_OPTIONS_FRAME_PIPELINE_OVERLAY)
        .expect("frame metrics row")
        .rect;
    let controls = surface
        .layout()
        .widget(UI_V2_OPTIONS_CONTROLS)
        .expect("controls row")
        .rect;

    for (rect, expected) in [
        (first_person, GameUiAction::ToggleFirstPersonPlayer),
        (crosshair, GameUiAction::ToggleCrosshair),
        (
            server_settings,
            GameUiAction::OpenServerSettings(GameOptionsParent::Pause),
        ),
        (xr_turn, GameUiAction::SetXrTurnMode(GameXrTurnMode::Snap30)),
        (frame_metrics, GameUiAction::ToggleFramePipelineOverlay),
        (
            controls,
            GameUiAction::OpenHelp(GameHelpParent::OptionsPause),
        ),
    ] {
        let point = point_in(rect);
        assert!(surface.pointer_down(point, surface.render_state));
        let (_handled, action) = surface.pointer_up(point, surface.render_state);
        assert_eq!(action, Some(expected));
    }
}

#[test]
fn server_settings_buttons_emit_expected_actions_from_committed_rects() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::ServerSettings {
        parent: GameOptionsParent::Pause,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState {
        server_cadence: Some(GameSimulationCadence::new(20, 20, 60)),
        ..GameUiRenderState::default()
    });

    let host_rate = surface
        .layout()
        .widget(UI_V2_SERVER_SETTINGS_HOST_RATE)
        .expect("host rate row")
        .rect;
    let gameplay_rate = surface
        .layout()
        .widget(UI_V2_SERVER_SETTINGS_GAMEPLAY_RATE)
        .expect("gameplay rate row")
        .rect;
    let physics_rate = surface
        .layout()
        .widget(UI_V2_SERVER_SETTINGS_PHYSICS_RATE)
        .expect("physics rate row")
        .rect;
    let back = surface
        .layout()
        .widget(UI_V2_SERVER_SETTINGS_BACK)
        .expect("back button")
        .rect;

    for (rect, expected) in [
        (
            host_rate,
            GameUiAction::SetServerSimulationCadence(GameSimulationCadence::new(30, 30, 60)),
        ),
        (
            gameplay_rate,
            GameUiAction::SetServerSimulationCadence(GameSimulationCadence::new(20, 60, 60)),
        ),
        (
            physics_rate,
            GameUiAction::SetServerSimulationCadence(GameSimulationCadence::new(20, 20, 120)),
        ),
        (back, GameUiAction::OpenOptions(GameOptionsParent::Pause)),
    ] {
        let point = point_in(rect);
        assert!(surface.pointer_down(point, surface.render_state));
        let (_handled, action) = surface.pointer_up(point, surface.render_state);
        assert_eq!(action, Some(expected));
    }
}

#[test]
fn server_settings_without_cadence_keeps_only_back_action() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::ServerSettings {
        parent: GameOptionsParent::Title,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState::default());

    assert!(
        surface
            .layout()
            .widget(UI_V2_SERVER_SETTINGS_HOST_RATE)
            .is_none()
    );
    assert!(
        surface
            .layout()
            .widget(UI_V2_SERVER_SETTINGS_GAMEPLAY_RATE)
            .is_none()
    );
    assert!(
        surface
            .layout()
            .widget(UI_V2_SERVER_SETTINGS_PHYSICS_RATE)
            .is_none()
    );

    let back = surface
        .layout()
        .widget(UI_V2_SERVER_SETTINGS_BACK)
        .expect("back button")
        .rect;
    assert!(surface.pointer_down(point_in(back), surface.render_state));
    let (_handled, action) = surface.pointer_up(point_in(back), surface.render_state);
    assert_eq!(
        action,
        Some(GameUiAction::OpenOptions(GameOptionsParent::Title))
    );
}

#[test]
fn options_disabled_far_lod_range_is_not_hit() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Options {
        parent: GameOptionsParent::Pause,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState {
        far_lod_enabled: false,
        ..GameUiRenderState::default()
    });
    let far_lod_range = surface
        .layout()
        .widget(UI_V2_OPTIONS_FAR_LOD_RANGE)
        .expect("far lod range row")
        .rect;

    assert!(surface.pointer_down(point_in(far_lod_range), surface.render_state));
    let (_handled, action) = surface.pointer_up(point_in(far_lod_range), surface.render_state);

    assert_eq!(action, None);
}

#[test]
fn options_sliders_use_committed_rects_for_click_and_drag_actions() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Options {
        parent: GameOptionsParent::Pause,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState {
        render_distance: 8,
        min_render_distance: 2,
        max_render_distance: 16,
        movement_speed_multiplier: 1.0,
        min_movement_speed_multiplier: 0.125,
        max_movement_speed_multiplier: 8.0,
        ..GameUiRenderState::default()
    });
    let radius = surface
        .layout()
        .widget(UI_V2_OPTIONS_RADIUS)
        .expect("render distance row")
        .rect;
    let movement_speed = surface
        .layout()
        .widget(UI_V2_OPTIONS_MOVEMENT_SPEED)
        .expect("movement speed row")
        .rect;

    let max_radius = Point {
        x: radius.right() - 0.1,
        y: radius.y + radius.height * 0.5,
    };
    assert!(surface.pointer_down(max_radius, surface.render_state));
    let (_handled, action) = surface.pointer_up(max_radius, surface.render_state);
    assert_eq!(action, Some(GameUiAction::SetRenderDistance(16)));

    let max_speed = Point {
        x: movement_speed.right() - 0.1,
        y: movement_speed.y + movement_speed.height * 0.5,
    };
    assert!(surface.pointer_down(point_in(movement_speed), surface.render_state));
    let (_handled, action) = surface.pointer_move(max_speed, surface.render_state);
    assert_eq!(action, Some(GameUiAction::SetMovementSpeed(8.0)));
}

#[test]
fn help_layout_retains_shortcut_rows_and_back_button() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Help {
        parent: GameHelpParent::OptionsPause,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));

    let layout = surface.layout();

    assert!(layout.help_rows().len() > 8);
    assert!(layout.help_rows().iter().any(|row| matches!(
        row.kind,
        UiHelpRowKind::Group(ShortcutHelpGroup::KeyboardMouse)
    )));
    assert!(layout.help_rows().iter().any(|row| matches!(
        row.kind,
        UiHelpRowKind::Group(ShortcutHelpGroup::RuntimeDebug)
    )));
    assert!(layout.widget(UI_V2_HELP_BACK).is_some());
}

#[test]
fn help_back_and_keys_close_to_parent() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Help {
        parent: GameHelpParent::OptionsPause,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let back = surface
        .layout()
        .widget(UI_V2_HELP_BACK)
        .expect("help back button")
        .rect;

    assert!(surface.pointer_down(point_in(back), GameUiRenderState::default()));
    let (_handled, action) = surface.pointer_up(point_in(back), GameUiRenderState::default());
    assert_eq!(
        action,
        Some(GameUiAction::CloseHelp(GameHelpParent::OptionsPause))
    );
    assert_eq!(
        surface.key_pressed(GuiKey::Escape),
        (
            true,
            Some(GameUiAction::CloseHelp(GameHelpParent::OptionsPause))
        )
    );
    assert_eq!(
        surface.key_pressed(GuiKey::F1),
        (
            true,
            Some(GameUiAction::CloseHelp(GameHelpParent::OptionsPause))
        )
    );
}

#[test]
fn help_render_uses_committed_rows_and_atlas_text_commands() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Help {
        parent: GameHelpParent::Game,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let row_count = surface.layout().help_rows().len();

    let v2_draw = surface.render_draw_list(GameUiRenderState::default());

    assert_eq!(surface.layout().help_rows().len(), row_count);
    assert!(!v2_draw.commands().is_empty());
    assert!(
        v2_draw
            .commands()
            .iter()
            .any(|command| matches!(command, GuiDrawCommand::Text { .. }))
    );
}
