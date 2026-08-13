use super::*;
use crate::{
    GameAuxiliarySplitMode, GameFlatPresentationState, GameFogMode, GameFogSettings,
    GameGrassDetail, GameLeafDetail, GameLocalPlayControllerFamily, GameLocalPlayGuestInput,
    GameLocalPlayLayout, GameLocalPlayState, GameTerrainPresentation, GameWorldRenderScaleMode,
    GameXrRenderMode, GameXrRenderModeSet, GameXrRenderPathState, GameXrRenderTransitionState,
};

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
fn death_screen_is_non_dismissible_and_keeps_world_visible() {
    let screen = GameScreen::Death {
        cause: GameDeathCause::Lava,
    };
    let mut host = GameUiHost::new_ingame();
    host.set_screen(Some(screen));

    let (handled, action) = host.key_pressed(GuiKey::Escape);
    host.apply_action(GameUiAction::Resume);
    host.open_pause();

    assert!(handled);
    assert_eq!(action, None);
    assert_eq!(host.screen(), Some(screen));
    assert!(!host.covers_world());
}

#[test]
fn death_screen_renders_typed_cause_and_only_terminal_actions() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Death {
        cause: GameDeathCause::Lava,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let buttons = surface.layout().widgets().to_vec();

    assert_eq!(buttons.len(), 2);
    for (button, expected) in buttons
        .iter()
        .zip([GameUiAction::Respawn, GameUiAction::QuitToTitle])
    {
        let point = point_in(button.rect);
        assert!(surface.pointer_down(point, GameUiRenderState::default()));
        let (_, action) = surface.pointer_up(point, GameUiRenderState::default());
        assert_eq!(action, Some(expected));
    }

    let draw = surface.render_draw_list(GameUiRenderState::default());
    assert!(draw.commands().iter().any(|command| matches!(
        command,
        GuiDrawCommand::Text { text, .. } if text == "YOU DIED!"
    )));
    assert!(draw.commands().iter().any(|command| matches!(
        command,
        GuiDrawCommand::Text { text, .. } if text == "You tried to swim in lava"
    )));
}

#[test]
fn options_hub_lists_categories_and_navigation_only() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Options {
        parent: GameOptionsParent::Pause,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState::default());

    let layout = surface.layout();

    for category in GameOptionsCategory::ALL {
        assert!(
            layout
                .widget(options_category_widget_id(category))
                .is_some(),
            "hub is missing a category button",
        );
    }
    assert!(layout.widget(UI_V2_OPTIONS_SERVER_SETTINGS).is_some());
    assert!(layout.widget(UI_V2_OPTIONS_ASSET_PACKS).is_some());
    assert!(layout.widget(UI_V2_OPTIONS_CONTROLS).is_some());
    assert!(layout.widget(UI_V2_OPTIONS_BACK).is_some());
    // Individual settings rows moved to the per-category sub-panels.
    assert!(layout.widget(UI_V2_OPTIONS_COLLISION_MODE).is_none());
    assert!(layout.widget(UI_V2_OPTIONS_RADIUS).is_none());
    assert!(layout.widget(UI_V2_OPTIONS_OCCLUSION).is_none());
}

#[test]
fn options_hub_category_buttons_open_their_panels() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::Options {
        parent: GameOptionsParent::Pause,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState::default());

    for category in GameOptionsCategory::ALL {
        let rect = surface
            .layout()
            .widget(options_category_widget_id(category))
            .expect("category button")
            .rect;
        let point = point_in(rect);
        assert!(surface.pointer_down(point, surface.render_state));
        let (_handled, action) = surface.pointer_up(point, surface.render_state);
        assert_eq!(
            action,
            Some(GameUiAction::OpenOptionsCategory(
                GameOptionsParent::Pause,
                category,
            )),
        );
    }
}

#[test]
fn options_movement_category_enables_conditional_rows_when_available() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Movement,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState {
        collision_mode: Some(GameCollisionMode::Normal),
        travel_assist_mode: Some(GameTravelAssistMode::Off),
        turn_mode: Some(GameTurnMode::Snap15),
        touch_controls_mode: Some(TouchControlsMode::Auto),
        touch_settings: Some(GameTouchSettings::new(2.0, 1.0, 5.0)),
        ..GameUiRenderState::default()
    });

    let layout = surface.layout();
    for id in [
        UI_V2_OPTIONS_COLLISION_MODE,
        UI_V2_OPTIONS_TRAVEL_ASSIST,
        UI_V2_OPTIONS_TURN_MODE,
        UI_V2_OPTIONS_TOUCH_CONTROLS,
        UI_V2_OPTIONS_TOUCH_LOOK,
    ] {
        let widget = layout.widget(id).expect("conditional row present");
        assert!(
            widget.enabled,
            "conditional row should be enabled when available"
        );
    }
}

#[test]
fn options_categories_show_unavailable_rows_disabled() {
    // Default render state leaves the platform-conditional rows absent; they
    // should still render (disabled) so every setting is discoverable.
    let mut movement = UiSurface::new();
    movement.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Movement,
    }));
    movement.set_scale(GuiScale::from_pixels(960, 540));
    movement.set_render_state(GameUiRenderState::default());
    let layout = movement.layout();
    for id in [
        UI_V2_OPTIONS_COLLISION_MODE,
        UI_V2_OPTIONS_TRAVEL_ASSIST,
        UI_V2_OPTIONS_TURN_MODE,
        UI_V2_OPTIONS_TOUCH_CONTROLS,
        UI_V2_OPTIONS_TOUCH_LOOK,
    ] {
        let widget = layout
            .widget(id)
            .expect("row present even when unavailable");
        assert!(!widget.enabled, "unavailable row should be disabled");
    }

    let mut display = UiSurface::new();
    display.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Display,
    }));
    display.set_scale(GuiScale::from_pixels(960, 540));
    display.set_render_state(GameUiRenderState {
        crosshair_visible: None,
        ..GameUiRenderState::default()
    });
    let crosshair = display
        .layout()
        .widget(UI_V2_OPTIONS_CROSSHAIR)
        .expect("crosshair row present even when unavailable");
    assert!(
        !crosshair.enabled,
        "unavailable crosshair should be disabled"
    );

    let mut debug = UiSurface::new();
    debug.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Debug,
    }));
    debug.set_scale(GuiScale::from_pixels(960, 540));
    debug.set_render_state(GameUiRenderState::default());
    assert!(
        !debug
            .layout()
            .widget(UI_V2_OPTIONS_AUXILIARY_SPLIT)
            .expect("auxiliary view row present even when unavailable")
            .enabled
    );
}

#[test]
fn debug_options_cycles_the_flat_auxiliary_view_mode() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Debug,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState {
        auxiliary_split_mode: Some(GameAuxiliarySplitMode::Off),
        ..GameUiRenderState::default()
    };
    surface.set_render_state(state);
    let row = surface
        .layout()
        .widget(UI_V2_OPTIONS_AUXILIARY_SPLIT)
        .expect("flat auxiliary view row")
        .clone();
    assert!(row.enabled);
    assert_eq!(row.value.as_deref(), Some("Off"));
    assert!(surface.pointer_down(point_in(row.rect), state));
    assert_eq!(
        surface.pointer_up(point_in(row.rect), state).1,
        Some(GameUiAction::SetAuxiliarySplitMode(
            GameAuxiliarySplitMode::Horizontal
        ))
    );
}

#[test]
fn local_play_options_make_the_initial_assignments_and_profile_scope_explicit() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::LocalPlay,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState {
        local_play: Some(GameLocalPlayState::default()),
        ..GameUiRenderState::default()
    };
    surface.set_render_state(state);

    let layout = surface
        .layout()
        .widget(UI_V2_OPTIONS_LOCAL_PLAY_LAYOUT)
        .expect("local play layout row")
        .clone();
    assert!(layout.enabled);
    assert_eq!(layout.value.as_deref(), Some("Left / Right"));
    assert!(surface.pointer_down(point_in(layout.rect), state));
    assert_eq!(
        surface.pointer_up(point_in(layout.rect), state).1,
        Some(GameUiAction::SetLocalPlayLayout(
            GameLocalPlayLayout::TopBottom
        ))
    );

    let player_one = surface
        .layout()
        .widget(UI_V2_OPTIONS_LOCAL_PLAY_PLAYER_ONE)
        .expect("player one assignment row");
    assert!(!player_one.enabled);
    assert_eq!(player_one.value.as_deref(), Some("Keyboard + Mouse"));

    let guest = surface
        .layout()
        .widget(UI_V2_OPTIONS_LOCAL_PLAY_GUEST)
        .expect("guest assignment row")
        .clone();
    assert!(guest.enabled);
    assert_eq!(guest.value.as_deref(), Some("Off"));
    assert!(surface.pointer_down(point_in(guest.rect), state));
    assert_eq!(
        surface.pointer_up(point_in(guest.rect), state).1,
        Some(GameUiAction::ToggleLocalPlayGuest)
    );

    let profile = surface
        .layout()
        .widget(UI_V2_OPTIONS_LOCAL_PLAY_STATUS)
        .expect("guest profile scope row");
    assert!(!profile.enabled);
    assert_eq!(profile.value.as_deref(), Some("Session Only"));
    let access = surface
        .layout()
        .widget(UI_V2_OPTIONS_LOCAL_PLAY_ACCESS)
        .expect("guest access scope row");
    assert!(!access.enabled);
    assert_eq!(access.value.as_deref(), Some("View Only"));
}

#[test]
fn local_play_options_show_controller_assignment_and_unavailable_state() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::LocalPlay,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState {
        local_play: Some(GameLocalPlayState {
            layout: GameLocalPlayLayout::TopBottom,
            guest_input: GameLocalPlayGuestInput::Assigned {
                family: GameLocalPlayControllerFamily::Xbox,
                device_number: 2,
            },
        }),
        ..GameUiRenderState::default()
    };
    surface.set_render_state(state);
    assert_eq!(
        surface
            .layout()
            .widget(UI_V2_OPTIONS_LOCAL_PLAY_LAYOUT)
            .expect("layout row")
            .value
            .as_deref(),
        Some("Top / Bottom")
    );
    assert_eq!(
        surface
            .layout()
            .widget(UI_V2_OPTIONS_LOCAL_PLAY_GUEST)
            .expect("guest row")
            .value
            .as_deref(),
        Some("Xbox gamepad 2")
    );

    surface.set_render_state(GameUiRenderState::default());
    for id in [
        UI_V2_OPTIONS_LOCAL_PLAY_LAYOUT,
        UI_V2_OPTIONS_LOCAL_PLAY_PLAYER_ONE,
        UI_V2_OPTIONS_LOCAL_PLAY_GUEST,
        UI_V2_OPTIONS_LOCAL_PLAY_STATUS,
        UI_V2_OPTIONS_LOCAL_PLAY_ACCESS,
    ] {
        assert!(
            !surface
                .layout()
                .widget(id)
                .expect("unavailable local play row")
                .enabled
        );
    }
}

#[test]
fn graphics_options_show_flat_resolution_and_cycle_world_scale_mode() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Graphics,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState {
        flat_presentation: Some(GameFlatPresentationState::new(
            [3840, 2160],
            [1920, 1080],
            0.5,
            GameWorldRenderScaleMode::Automatic,
        )),
        ..GameUiRenderState::default()
    };
    surface.set_render_state(state);

    let output = surface
        .layout()
        .widget(UI_V2_OPTIONS_OUTPUT_RESOLUTION)
        .expect("output resolution row")
        .clone();
    let world = surface
        .layout()
        .widget(UI_V2_OPTIONS_WORLD_RESOLUTION)
        .expect("world resolution row")
        .clone();
    let scale = surface
        .layout()
        .widget(UI_V2_OPTIONS_WORLD_RENDER_SCALE)
        .expect("world render scale row")
        .clone();

    assert_eq!(output.value.as_deref(), Some("3840 x 2160"));
    assert!(!output.enabled);
    assert_eq!(world.value.as_deref(), Some("1920 x 1080"));
    assert!(!world.enabled);
    assert_eq!(scale.value.as_deref(), Some("Auto (50%)"));
    assert!(scale.enabled);
    assert!(surface.pointer_down(point_in(scale.rect), state));
    assert_eq!(
        surface.pointer_up(point_in(scale.rect), state).1,
        Some(GameUiAction::SetWorldRenderScaleMode(
            GameWorldRenderScaleMode::Half,
        ))
    );
}

#[test]
fn graphics_options_omit_xr_render_path_on_flat_clients() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Graphics,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState::default());

    assert!(
        surface
            .layout()
            .widget(UI_V2_OPTIONS_XR_RENDER_PATH)
            .is_none()
    );
}

#[test]
fn graphics_options_show_host_confirmed_xr_render_path_state() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Graphics,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState {
        xr_render_path: Some(GameXrRenderPathState::new(
            GameXrRenderModeSet::ALL,
            GameXrRenderMode::ArrayMultiview,
            Some(GameXrRenderMode::ArrayMultiview),
            GameXrRenderMode::ArrayPerEye,
            GameXrRenderTransitionState::Pending,
        )),
        ..GameUiRenderState::default()
    };
    surface.set_render_state(state);

    let row = surface
        .layout()
        .widget(UI_V2_OPTIONS_XR_RENDER_PATH)
        .expect("XR render-path row")
        .clone();
    assert_eq!(
        row.value.as_deref(),
        Some("Array per-eye -> Array multiview (pending)")
    );
    assert!(surface.pointer_down(point_in(row.rect), state));
    assert_eq!(
        surface.pointer_up(point_in(row.rect), state).1,
        Some(GameUiAction::SetXrRenderMode(GameXrRenderMode::DualPerEye))
    );
}

#[test]
fn graphics_options_show_and_cycle_leaf_detail() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Graphics,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState {
        leaf_detail: GameLeafDetail::Bushy,
        ..GameUiRenderState::default()
    };
    surface.set_render_state(state);

    let leaf_detail = surface
        .layout()
        .widget(UI_V2_OPTIONS_LEAF_DETAIL)
        .expect("leaf detail row")
        .clone();
    assert_eq!(leaf_detail.value.as_deref(), Some("Bushy"));
    assert!(leaf_detail.enabled);
    assert!(surface.pointer_down(point_in(leaf_detail.rect), state));
    assert_eq!(
        surface.pointer_up(point_in(leaf_detail.rect), state).1,
        Some(GameUiAction::SetLeafDetail(GameLeafDetail::Blocky))
    );
}

#[test]
fn graphics_options_show_and_cycle_grass_detail() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Graphics,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState {
        grass_detail: GameGrassDetail::Lush,
        ..GameUiRenderState::default()
    };
    surface.set_render_state(state);

    let grass_detail = surface
        .layout()
        .widget(UI_V2_OPTIONS_GRASS_DETAIL)
        .expect("grass detail row")
        .clone();
    assert_eq!(grass_detail.value.as_deref(), Some("Lush"));
    assert!(grass_detail.enabled);
    assert!(surface.pointer_down(point_in(grass_detail.rect), state));
    assert_eq!(
        surface.pointer_up(point_in(grass_detail.rect), state).1,
        Some(GameUiAction::SetGrassDetail(GameGrassDetail::Ultra))
    );
}

#[test]
fn graphics_options_show_and_cycle_terrain_horizon() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Graphics,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState {
        terrain_presentation: GameTerrainPresentation::Composed,
        ..GameUiRenderState::default()
    };
    surface.set_render_state(state);

    let terrain_horizon = surface
        .layout()
        .widget(UI_V2_OPTIONS_TERRAIN_PRESENTATION)
        .expect("terrain horizon row")
        .clone();
    assert_eq!(terrain_horizon.label, "Terrain Horizon");
    assert_eq!(terrain_horizon.value.as_deref(), Some("Composed"));
    assert!(terrain_horizon.enabled);
    assert!(surface.pointer_down(point_in(terrain_horizon.rect), state));
    assert_eq!(
        surface.pointer_up(point_in(terrain_horizon.rect), state).1,
        Some(GameUiAction::SetTerrainPresentation(
            GameTerrainPresentation::ExactOnly,
        ))
    );

    let unavailable = GameUiRenderState {
        terrain_presentation_available: false,
        ..state
    };
    surface.set_render_state(unavailable);
    assert_eq!(
        surface
            .layout()
            .widget(UI_V2_OPTIONS_TERRAIN_PRESENTATION)
            .and_then(|widget| widget.value.as_deref()),
        Some("Composed (Unavailable)")
    );
}

#[test]
fn graphics_options_open_the_fog_evaluation_submenu() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Graphics,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState::default();
    surface.set_render_state(state);

    let fog = surface
        .layout()
        .widget(UI_V2_OPTIONS_FOG_SUBMENU)
        .expect("fog submenu row")
        .clone();
    assert!(surface.pointer_down(point_in(fog.rect), state));
    assert_eq!(
        surface.pointer_up(point_in(fog.rect), state).1,
        Some(GameUiAction::OpenOptionsCategory(
            GameOptionsParent::Pause,
            GameOptionsCategory::Fog,
        ))
    );
}

#[test]
fn fog_options_expose_recommended_defaults_and_mode_specific_controls() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Fog,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    let state = GameUiRenderState::default();
    surface.set_render_state(state);

    assert_eq!(
        surface
            .layout()
            .widget(UI_V2_FOG_MODE)
            .and_then(|widget| widget.value.as_deref()),
        Some("Natural")
    );
    assert_eq!(surface.layout().widgets().len(), 16);
    assert_eq!(
        surface
            .layout()
            .widget(UI_V2_FOG_COLOR_MODE)
            .and_then(|widget| widget.value.as_deref()),
        Some("Sky Adaptive")
    );
    assert!(
        !surface
            .layout()
            .widget(UI_V2_FOG_COLOR_RED)
            .expect("custom red")
            .enabled
    );
    assert!(
        !surface
            .layout()
            .widget(UI_V2_FOG_CLASSIC_START)
            .expect("classic start")
            .enabled
    );
    assert!(
        !surface
            .layout()
            .widget(UI_V2_FOG_GROUND_BASE)
            .expect("ground base")
            .enabled
    );
    assert!(
        surface
            .layout()
            .widget(UI_V2_FOG_MAX_OPACITY)
            .expect("maximum opacity")
            .enabled
    );

    let mode = surface
        .layout()
        .widget(UI_V2_FOG_MODE)
        .expect("fog mode")
        .clone();
    assert!(surface.pointer_down(point_in(mode.rect), state));
    assert_eq!(
        surface.pointer_up(point_in(mode.rect), state).1,
        Some(GameUiAction::SetFogSettings(GameFogSettings {
            mode: GameFogMode::GroundHaze,
            ..GameFogSettings::default()
        }))
    );

    assert_eq!(
        surface.key_pressed(GuiKey::Escape).1,
        Some(GameUiAction::OpenOptionsCategory(
            GameOptionsParent::Pause,
            GameOptionsCategory::Graphics,
        ))
    );
}

#[test]
fn compact_options_scroll_and_controller_focus_reveals_hidden_rows() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Graphics,
    }));
    surface.set_scale(GuiScale::from_pixels(320, 140));
    let state = GameUiRenderState {
        flat_presentation: Some(GameFlatPresentationState::new(
            [1280, 800],
            [1280, 800],
            1.0,
            GameWorldRenderScaleMode::Automatic,
        )),
        ..GameUiRenderState::default()
    };
    surface.set_render_state(state);

    let initial = surface.debug_snapshot().expect("graphics snapshot");
    assert!(initial.scroll_max > 0.0);
    assert_eq!(initial.scroll_offset, 0.0);
    assert!(surface.scroll_by(12.0, state));
    assert_eq!(
        surface
            .debug_snapshot()
            .expect("scrolled graphics snapshot")
            .scroll_offset,
        12.0,
    );

    surface.scroll_offset = 0.0;
    surface.layout_dirty = true;
    for _ in 0..10 {
        surface.navigate(GuiNavigation::NextPage, state);
        if surface
            .debug_snapshot()
            .is_some_and(|snapshot| snapshot.focused == Some(UI_V2_OPTIONS_FPS_CAP))
        {
            break;
        }
    }
    let focused = surface.debug_snapshot().expect("focused graphics snapshot");
    let clip = focused.scroll_clip.expect("graphics scroll clip");
    let fps = focused
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_OPTIONS_FPS_CAP)
        .expect("FPS cap row");
    assert_eq!(focused.focused, Some(UI_V2_OPTIONS_FPS_CAP));
    assert!(focused.scroll_offset > 0.0);
    assert!(fps.rect.y >= clip.y - f32::EPSILON);
    assert!(fps.rect.bottom() <= clip.bottom() + f32::EPSILON);
}

#[test]
fn storage_profile_title_exposes_only_supported_destructive_actions() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Title,
        category: GameOptionsCategory::StorageProfile,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState {
        world_catalog: world_catalog_state(),
        storage_profile: StorageProfileUiState::available(
            [0x42; 16],
            "Player",
            StorageProfileBackend::NativePreferences,
        ),
        ..GameUiRenderState::default()
    });

    let layout = surface.layout();
    for id in [
        UI_V2_STORAGE_PROFILE_NAME,
        UI_V2_STORAGE_PROFILE_ID,
        UI_V2_STORAGE_BACKEND,
        UI_V2_STORAGE_WORLD_COUNT,
        UI_V2_STORAGE_CLEAR_CACHE,
    ] {
        assert!(!layout.widget(id).expect("storage row").enabled);
    }
    for id in [
        UI_V2_STORAGE_RESET_IDENTITY,
        UI_V2_STORAGE_DELETE_ALL_WORLDS,
        UI_V2_STORAGE_FACTORY_RESET,
    ] {
        assert!(layout.widget(id).expect("storage action").enabled);
    }
}

#[test]
fn storage_profile_pause_disables_destructive_actions() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::StorageProfile,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState {
        world_catalog: world_catalog_state(),
        storage_profile: StorageProfileUiState::available(
            [0x42; 16],
            "Player",
            StorageProfileBackend::NativePreferences,
        ),
        ..GameUiRenderState::default()
    });

    for id in [
        UI_V2_STORAGE_CLEAR_CACHE,
        UI_V2_STORAGE_RESET_IDENTITY,
        UI_V2_STORAGE_DELETE_ALL_WORLDS,
        UI_V2_STORAGE_FACTORY_RESET,
    ] {
        assert!(!surface.layout().widget(id).expect("storage action").enabled);
    }
}

#[test]
fn storage_confirmation_requires_explicit_confirm_or_cancel() {
    let state = GameUiRenderState {
        world_catalog: world_catalog_state(),
        storage_profile: StorageProfileUiState::available(
            [0x42; 16],
            "Player",
            StorageProfileBackend::NativePreferences,
        ),
        ..GameUiRenderState::default()
    };
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::StorageConfirm {
        parent: GameOptionsParent::Title,
        action: GameStorageAction::FactoryReset,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(state);

    let confirm = surface
        .layout()
        .widget(UI_V2_STORAGE_CONFIRM)
        .expect("factory reset confirm")
        .rect;
    assert!(surface.pointer_down(point_in(confirm), state));
    assert_eq!(
        surface.pointer_up(point_in(confirm), state).1,
        Some(GameUiAction::ExecuteStorageAction(
            GameOptionsParent::Title,
            GameStorageAction::FactoryReset,
        ))
    );
    assert_eq!(
        surface.key_pressed(GuiKey::Escape),
        (
            true,
            Some(GameUiAction::CancelStorageAction(GameOptionsParent::Title)),
        )
    );
}

#[test]
fn game_ui_host_pointer_input_uses_committed_render_state() {
    let mut host = GameUiHost::new_ingame();
    host.set_screen(Some(GameScreen::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Display,
    }));
    host.set_scale(GuiScale::from_pixels(960, 540));

    let committed_state = GameUiRenderState::default();
    host.commit_render_state(committed_state);
    let committed_snapshot = host
        .v2_debug_snapshot()
        .expect("Display category is a v2 screen");
    let first_person = committed_snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_OPTIONS_FIRST_PERSON_PLAYER)
        .expect("First Person Body row exists")
        .rect;
    let point = point_in(first_person);

    assert!(host.pointer_down(point));
    let (_handled, action) = host.pointer_up(point);

    assert_eq!(action, Some(GameUiAction::ToggleFirstPersonPlayer));
}

/// Set up a surface on a given options screen with `state` committed and assert
/// that clicking each `(widget_id, expected_action)` pair fires that action.
fn assert_option_rows_emit(
    screen: UiScreenId,
    state: GameUiRenderState,
    rows: &[(UiWidgetId, GameUiAction)],
) {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(screen));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(state);
    for &(id, expected) in rows {
        let rect = surface
            .layout()
            .widget(id)
            .expect("expected settings row present")
            .rect;
        let point = point_in(rect);
        assert!(surface.pointer_down(point, surface.render_state));
        let (_handled, action) = surface.pointer_up(point, surface.render_state);
        assert_eq!(action, Some(expected));
    }
}

#[test]
fn options_hub_navigation_buttons_emit_expected_actions() {
    assert_option_rows_emit(
        UiScreenId::Options {
            parent: GameOptionsParent::Pause,
        },
        GameUiRenderState {
            server_cadence: Some(GameSimulationCadence::default()),
            ..GameUiRenderState::default()
        },
        &[
            (
                UI_V2_OPTIONS_ASSET_PACKS,
                GameUiAction::OpenAssetPacks(GameOptionsParent::Pause),
            ),
            (
                UI_V2_OPTIONS_SERVER_SETTINGS,
                GameUiAction::OpenServerSettings(GameOptionsParent::Pause),
            ),
            (
                UI_V2_OPTIONS_CONTROLS,
                GameUiAction::OpenHelp(GameHelpParent::OptionsPause),
            ),
            (UI_V2_OPTIONS_BACK, GameUiAction::BackToPause),
        ],
    );
}

#[test]
fn options_movement_category_rows_emit_expected_actions() {
    assert_option_rows_emit(
        UiScreenId::OptionsCategory {
            parent: GameOptionsParent::Pause,
            category: GameOptionsCategory::Movement,
        },
        GameUiRenderState {
            collision_mode: Some(GameCollisionMode::Normal),
            travel_assist_mode: Some(GameTravelAssistMode::Off),
            turn_mode: Some(GameTurnMode::Snap15),
            ..GameUiRenderState::default()
        },
        &[
            (
                UI_V2_OPTIONS_COLLISION_MODE,
                GameUiAction::SetCollisionMode(GameCollisionMode::NoClip),
            ),
            (
                UI_V2_OPTIONS_TRAVEL_ASSIST,
                GameUiAction::SetTravelAssistMode(GameTravelAssistMode::Blink),
            ),
            (
                UI_V2_OPTIONS_TURN_MODE,
                GameUiAction::SetTurnMode(GameTurnMode::Snap30),
            ),
        ],
    );
}

#[test]
fn options_debug_and_display_category_rows_emit_expected_actions() {
    assert_option_rows_emit(
        UiScreenId::OptionsCategory {
            parent: GameOptionsParent::Pause,
            category: GameOptionsCategory::Debug,
        },
        GameUiRenderState::default(),
        &[
            (
                UI_V2_OPTIONS_FRAME_PIPELINE_OVERLAY,
                GameUiAction::ToggleFramePipelineOverlay,
            ),
            (
                UI_V2_OPTIONS_DEBUG_DIAGNOSTICS,
                GameUiAction::ToggleDebugDiagnostics,
            ),
        ],
    );
    assert_option_rows_emit(
        UiScreenId::OptionsCategory {
            parent: GameOptionsParent::Pause,
            category: GameOptionsCategory::Display,
        },
        GameUiRenderState::default(),
        &[
            (
                UI_V2_OPTIONS_FIRST_PERSON_PLAYER,
                GameUiAction::ToggleFirstPersonPlayer,
            ),
            (UI_V2_OPTIONS_CROSSHAIR, GameUiAction::ToggleCrosshair),
        ],
    );
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
fn options_render_distance_slider_uses_committed_rect() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Graphics,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState {
        render_distance: 8,
        min_render_distance: 2,
        max_render_distance: 16,
        ..GameUiRenderState::default()
    });
    let radius = surface
        .layout()
        .widget(UI_V2_OPTIONS_RADIUS)
        .expect("render distance row")
        .rect;

    let max_radius = Point {
        x: radius.right() - 0.1,
        y: radius.y + radius.height * 0.5,
    };
    assert!(surface.pointer_down(max_radius, surface.render_state));
    let (_handled, action) = surface.pointer_up(max_radius, surface.render_state);
    assert_eq!(action, Some(GameUiAction::SetRenderDistance(16)));
}

#[test]
fn options_movement_speed_slider_uses_committed_rect_for_drag() {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Movement,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState {
        movement_speed_multiplier: 1.0,
        min_movement_speed_multiplier: 0.125,
        max_movement_speed_multiplier: 8.0,
        ..GameUiRenderState::default()
    });
    let movement_speed = surface
        .layout()
        .widget(UI_V2_OPTIONS_MOVEMENT_SPEED)
        .expect("movement speed row")
        .rect;

    let max_speed = Point {
        x: movement_speed.right() - 0.1,
        y: movement_speed.y + movement_speed.height * 0.5,
    };
    assert!(surface.pointer_down(point_in(movement_speed), surface.render_state));
    let (_handled, action) = surface.pointer_move(max_speed, surface.render_state);
    assert_eq!(action, Some(GameUiAction::SetMovementSpeed(8.0)));
}

fn assert_slider_capture_lifecycle(
    screen: UiScreenId,
    state: GameUiRenderState,
    id: UiWidgetId,
    expected_max: GameUiAction,
) {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(screen));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(state);
    let rect = surface.layout().widget(id).expect("slider row").rect;
    let press = point_in(rect);
    let past_max = Point {
        x: rect.right() + 20.0,
        y: rect.bottom() + 20.0,
    };

    assert!(surface.pointer_down(press, state));
    assert_eq!(surface.captured, Some(id));
    assert_eq!(surface.pointer_move(past_max, state).1, Some(expected_max));
    assert_eq!(surface.pointer_up(past_max, state).1, Some(expected_max));
    assert_eq!(surface.captured, None);

    // Moving after release must not continue to drive the former widget.
    assert_eq!(surface.pointer_move(press, state).1, None);
}

#[test]
fn every_options_slider_retains_capture_through_release() {
    let graphics = UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Graphics,
    };
    let graphics_state = GameUiRenderState {
        render_distance: 8,
        min_render_distance: 2,
        max_render_distance: 16,
        ..GameUiRenderState::default()
    };
    assert_slider_capture_lifecycle(
        graphics,
        graphics_state,
        UI_V2_OPTIONS_RADIUS,
        GameUiAction::SetRenderDistance(16),
    );

    let movement = UiScreenId::OptionsCategory {
        parent: GameOptionsParent::Pause,
        category: GameOptionsCategory::Movement,
    };
    let movement_state = GameUiRenderState {
        fly_speed_multiplier: 1.0,
        min_fly_speed_multiplier: 0.125,
        max_fly_speed_multiplier: 8.0,
        movement_speed_multiplier: 1.0,
        min_movement_speed_multiplier: 0.125,
        max_movement_speed_multiplier: 8.0,
        touch_settings: Some(GameTouchSettings::new(2.0, 1.0, 5.0)),
        ..GameUiRenderState::default()
    };
    assert_slider_capture_lifecycle(
        movement,
        movement_state,
        UI_V2_OPTIONS_FLY_SPEED,
        GameUiAction::SetFlySpeed(8.0),
    );
    assert_slider_capture_lifecycle(
        movement,
        movement_state,
        UI_V2_OPTIONS_MOVEMENT_SPEED,
        GameUiAction::SetMovementSpeed(8.0),
    );
    assert_slider_capture_lifecycle(
        movement,
        movement_state,
        UI_V2_OPTIONS_TOUCH_LOOK,
        GameUiAction::SetTouchLookSensitivity(5.0),
    );
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
