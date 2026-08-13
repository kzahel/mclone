use super::*;
use mclone_input::InputPromptKind;

fn resolved_flat_input(touch_controls_visible: bool) -> ResolvedFlatInput {
    ResolvedFlatInput {
        preferred_prompt: Some(if touch_controls_visible {
            InputPromptKind::Touch
        } else {
            InputPromptKind::KeyboardMouse
        }),
        touch_controls_visible,
        accepts_keyboard_mouse: true,
        accepts_touch: true,
        accepts_gamepad: false,
        accepts_xr_controller: false,
    }
}

fn resolved_gamepad_input() -> ResolvedFlatInput {
    ResolvedFlatInput {
        preferred_prompt: Some(InputPromptKind::Gamepad),
        touch_controls_visible: false,
        accepts_keyboard_mouse: true,
        accepts_touch: true,
        accepts_gamepad: true,
        accepts_xr_controller: false,
    }
}

#[test]
fn gui_scale_matches_minecraft_style_thresholds() {
    assert_eq!(GuiScale::from_pixels(319, 239).scale, 1);
    assert_eq!(GuiScale::from_pixels(640, 480).scale, 2);
    assert_eq!(GuiScale::from_pixels(1280, 720).scale, 3);
    assert_eq!(GuiScale::from_pixels(1920, 1080).scale, 4);
}

#[test]
fn slider_value_mapping_uses_the_rendered_track_geometry() {
    let slider = Slider::new(
        WidgetId(1),
        Rect::new(20.0, 10.0, 100.0, 20.0),
        "Value",
        0.5,
    );
    let track = slider.track_rect();

    assert_eq!(slider.value_from_point(Point { x: track.x, y: 0.0 }), 0.0);
    assert_eq!(
        slider.value_from_point(Point {
            x: track.center_x(),
            y: 0.0,
        }),
        0.5,
    );
    assert_eq!(
        slider.value_from_point(Point {
            x: track.right(),
            y: 0.0,
        }),
        1.0,
    );
}

#[test]
fn draw_list_intersects_clip_stack() {
    let mut draw = GuiDrawList::new();
    draw.push_clip(Rect::new(10.0, 10.0, 20.0, 20.0));
    draw.push_clip(Rect::new(20.0, 5.0, 20.0, 12.0));
    draw.fill(Rect::new(0.0, 0.0, 100.0, 100.0), Color::WHITE);
    let [GuiDrawCommand::SolidRect { clip, .. }] = draw.commands() else {
        panic!("expected one solid rect");
    };
    assert_eq!(
        *clip,
        Some(ClipRect {
            x: 20.0,
            y: 10.0,
            width: 10.0,
            height: 7.0
        })
    );
}

#[test]
fn font_atlas_draw_emits_one_text_command() {
    let font = Font::default();
    let mut draw = GuiDrawList::new();

    font.draw_shadow_atlas(&mut draw, "Controls", 12.5, 18.5, Color::WHITE);

    let [
        GuiDrawCommand::Text {
            text,
            x,
            y,
            color,
            shadow,
            ..
        },
    ] = draw.commands()
    else {
        panic!("expected one text command");
    };
    assert_eq!(text, "Controls");
    assert_eq!((*x, *y), (12.0, 18.0));
    assert_eq!(*color, Color::WHITE);
    assert_eq!(*shadow, true);
}

#[test]
fn button_reports_hit_only_when_enabled() {
    let button = Button::new(WidgetId(1), Rect::new(10.0, 20.0, 50.0, 12.0), "Start");
    assert!(button.contains(Point { x: 12.0, y: 21.0 }));
    assert!(!button.contains(Point { x: 5.0, y: 21.0 }));
    assert!(!button.enabled(false).contains(Point { x: 12.0, y: 21.0 }));
}

#[test]
fn loading_progress_palette_uses_java_inspired_colors() {
    assert_eq!(
        LoadingProgressCellStatus::None.color(),
        Color::rgba(0, 0, 0, 255)
    );
    assert_eq!(
        LoadingProgressCellStatus::Terrain.color(),
        Color::rgba(209, 209, 209, 255)
    );
    assert_eq!(
        LoadingProgressCellStatus::Surface.color(),
        Color::rgba(114, 104, 9, 255)
    );
    assert_eq!(
        LoadingProgressCellStatus::Features.color(),
        Color::rgba(33, 198, 0, 255)
    );
    assert_eq!(
        LoadingProgressCellStatus::Light.color(),
        Color::rgba(204, 204, 204, 255)
    );
    assert_eq!(LoadingProgressCellStatus::TargetReady.color(), Color::WHITE);
}

#[test]
fn loading_progress_percent_uses_target_ready_chunks() {
    let progress = LoadingProgressOverlay::new(1, 2, 9, false, []);
    assert_eq!(progress.grid_side(), 3);
    assert_eq!(progress.percent(), 22);

    let over_complete = LoadingProgressOverlay::new(1, 12, 9, true, []);
    assert_eq!(over_complete.percent(), 100);

    let empty_target = LoadingProgressOverlay::new(0, 1, 0, false, []);
    assert_eq!(empty_target.percent(), 0);
}

#[test]
fn movement_experience_modes_have_stable_labels_and_cycles() {
    assert_eq!(GameCollisionMode::Normal.label(), "Normal");
    assert_eq!(GameCollisionMode::Normal.next(), GameCollisionMode::NoClip);
    assert_eq!(GameCollisionMode::NoClip.next(), GameCollisionMode::Normal);

    assert_eq!(GameTravelAssistMode::Off.label(), "Off");
    assert_eq!(
        GameTravelAssistMode::Off.next(),
        GameTravelAssistMode::Blink
    );
    assert_eq!(
        GameTravelAssistMode::Blink.next(),
        GameTravelAssistMode::Off
    );
    assert_eq!(GameTravelAssistMode::Warp.next(), GameTravelAssistMode::Off);
    assert!(!GameTravelAssistMode::Off.is_enabled());
    assert!(GameTravelAssistMode::Blink.is_enabled());

    assert_eq!(GameTurnMode::Snap15.label(), "Snap 15");
    assert_eq!(GameTurnMode::Snap15.next(), GameTurnMode::Snap30);
    assert_eq!(GameTurnMode::Snap30.next(), GameTurnMode::Snap45);
    assert_eq!(GameTurnMode::Snap45.next(), GameTurnMode::Smooth);
    assert_eq!(GameTurnMode::Smooth.next(), GameTurnMode::Snap15);
    assert_eq!(
        GameTurnMode::from(GameXrTurnMode::Smooth),
        GameTurnMode::Smooth
    );
    assert_eq!(
        GameXrTurnMode::from(GameTurnMode::Snap30),
        GameXrTurnMode::Snap30
    );
    assert_eq!(
        GameXrTurnMode::from(GameTurnMode::Snap45),
        GameXrTurnMode::Snap45
    );
}

#[test]
fn loading_progress_status_lookup_uses_latest_cell() {
    let progress = LoadingProgressOverlay::new(
        1,
        0,
        9,
        false,
        [
            LoadingProgressCell::new(0, 0, LoadingProgressCellStatus::Terrain),
            LoadingProgressCell::new(0, 0, LoadingProgressCellStatus::Features).playable(true),
        ],
    );

    assert_eq!(progress.status_grid.len(), 9);
    assert_eq!(
        progress.status_at(0, 0),
        LoadingProgressCellStatus::Features
    );
    assert_eq!(progress.status_at(1, 1), LoadingProgressCellStatus::None);
    assert_eq!(progress.playable_cell().unwrap().relative_x, 0);
    assert!(!progress.playable_ready);
}

#[test]
fn loading_progress_sparse_snapshot_defaults_missing_cells() {
    let progress = LoadingProgressOverlay::new(
        2,
        1,
        25,
        false,
        [
            LoadingProgressCell::new(-2, -2, LoadingProgressCellStatus::Terrain),
            LoadingProgressCell::new(2, 2, LoadingProgressCellStatus::TargetReady).playable(true),
        ],
    );

    assert_eq!(progress.status_grid.len(), 25);
    assert_eq!(
        progress.status_at(-2, -2),
        LoadingProgressCellStatus::Terrain
    );
    assert_eq!(
        progress.status_at(2, 2),
        LoadingProgressCellStatus::TargetReady
    );
    assert_eq!(progress.status_at(0, 0), LoadingProgressCellStatus::None);
    assert_eq!(progress.status_at(3, 0), LoadingProgressCellStatus::None);
    assert_eq!(progress.playable_cell().unwrap().relative_z, 2);
}

#[test]
fn loading_progress_dense_snapshot_lookup_uses_normalized_grid() {
    let radius = 3;
    let mut cells = Vec::new();
    for relative_z in -radius..=radius {
        for relative_x in -radius..=radius {
            cells.push(LoadingProgressCell::new(
                relative_x,
                relative_z,
                LoadingProgressCellStatus::Terrain,
            ));
        }
    }
    cells.push(LoadingProgressCell::new(
        1,
        -2,
        LoadingProgressCellStatus::Features,
    ));
    cells.push(LoadingProgressCell::new(
        -4,
        0,
        LoadingProgressCellStatus::Light,
    ));

    let progress = LoadingProgressOverlay::new(3, 0, 49, false, cells);

    assert_eq!(progress.status_grid.len(), 49);
    assert_eq!(
        progress.status_at(1, -2),
        LoadingProgressCellStatus::Features
    );
    assert_eq!(
        progress.status_at(-3, 3),
        LoadingProgressCellStatus::Terrain
    );
    assert_eq!(progress.status_at(-4, 0), LoadingProgressCellStatus::None);
}

#[test]
fn loading_progress_overlay_draws_grid_cells_and_playable_outline() {
    let progress = LoadingProgressOverlay::new(
        1,
        3,
        9,
        false,
        [
            LoadingProgressCell::new(-1, -1, LoadingProgressCellStatus::Terrain),
            LoadingProgressCell::new(0, 0, LoadingProgressCellStatus::Features).playable(true),
            LoadingProgressCell::new(1, 1, LoadingProgressCellStatus::TargetReady),
        ],
    );
    let mut draw = GuiDrawList::new();

    render_loading_progress_overlay(GuiScale::from_pixels(960, 540), &mut draw, &progress);

    let cell_rects = draw
        .commands()
        .iter()
        .filter_map(|command| match command {
            GuiDrawCommand::SolidRect { rect, color, .. }
                if rect.width == 2.0 && rect.height == 2.0 =>
            {
                Some((*rect, *color))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(cell_rects.len(), 9);
    assert!(
        cell_rects
            .iter()
            .any(|(_, color)| { *color == LoadingProgressCellStatus::Terrain.color() })
    );
    assert!(
        cell_rects
            .iter()
            .any(|(_, color)| { *color == LoadingProgressCellStatus::Features.color() })
    );
    assert!(cell_rects.iter().any(|(_, color)| *color == Color::WHITE));

    let playable_outline_color = Color::rgba(242, 96, 96, 255);
    assert!(draw.commands().iter().any(|command| {
        matches!(
            command,
            GuiDrawCommand::SolidRect { color, .. } if *color == playable_outline_color
        )
    }));
}

#[test]
fn loading_progress_panel_draws_compact_grid_without_fullscreen_scrim() {
    let progress = LoadingProgressOverlay::new(
        1,
        3,
        9,
        false,
        [
            LoadingProgressCell::new(-1, -1, LoadingProgressCellStatus::Terrain),
            LoadingProgressCell::new(0, 0, LoadingProgressCellStatus::TargetReady).playable(true),
            LoadingProgressCell::new(1, 1, LoadingProgressCellStatus::Features),
        ],
    );
    let mut draw = GuiDrawList::new();

    render_loading_progress_panel_at(
        GuiScale::from_pixels(960, 540),
        &mut draw,
        &progress,
        Point { x: 800.0, y: 4.0 },
        "VIEW",
    );

    assert!(!draw.commands().iter().any(|command| {
        matches!(
            command,
            GuiDrawCommand::SolidRect { rect, .. }
                if rect.x == 0.0 && rect.y == 0.0 && rect.width == 960.0 && rect.height == 540.0
        )
    }));
    assert!(draw.commands().iter().any(|command| {
        matches!(
            command,
            GuiDrawCommand::SolidRect { rect, .. } if rect.x == 800.0 && rect.y == 4.0
        )
    }));
    let cell_rects = draw
        .commands()
        .iter()
        .filter(|command| {
            matches!(
                command,
                GuiDrawCommand::SolidRect { color, .. }
                    if *color == LoadingProgressCellStatus::None.color()
                        || *color == LoadingProgressCellStatus::Terrain.color()
                        || *color == LoadingProgressCellStatus::TargetReady.color()
                        || *color == LoadingProgressCellStatus::Features.color()
            )
        })
        .count();
    assert_eq!(cell_rects, 9);
}

#[test]
fn debug_overlay_renders_title_and_lines() {
    let overlay = DebugOverlay::new("DEBUG", ["POS 1.0 64.0 -2.0", "CHUNKS 9"]);
    let mut draw = GuiDrawList::new();

    render_debug_overlay(GuiScale::from_pixels(960, 540), &mut draw, &overlay);

    let text_commands = draw
        .commands()
        .iter()
        .filter(|command| matches!(command, GuiDrawCommand::Text { .. }))
        .count();
    assert_eq!(text_commands, 3);
    assert!(draw.commands().len() <= 16, "{:?}", draw.commands());
}

#[test]
fn flat_debug_overlay_formats_common_flat_hud_lines() {
    let mut overlay = FlatDebugOverlay::new(
        [1.25, 64.0, -2.5],
        [3, -4],
        32.0,
        "WALK",
        true,
        FlatDebugView::with_center(2, [3, -4]),
    );
    overlay.runner = Some(FlatDebugRunner::new("WEB-WORKER", 1, 2));
    overlay.chunks = Some(FlatDebugChunkCounts::loaded_visible(9, 8));
    overlay.draw = Some(FlatDebugDrawCounts {
        drawn_sections: 10,
        section_count: 16,
        drawn_faces: 120,
        face_count: 200,
    });
    overlay.actors = Some(FlatDebugActorCounts {
        drawn_actors: 1,
        actor_count: 2,
        drawn_actor_indices: 180,
    });
    overlay.mesh = Some(FlatDebugMeshCounts {
        build_count: 3,
        upload_count: 4,
        render_count: 5,
    });
    overlay.pending_compile_jobs = Some(6);
    overlay.day_time = Some(1200);
    overlay.time_of_day = Some(0.25);
    overlay.selected_hotbar_slot = Some(4);
    overlay.target = Some(FlatDebugTarget::Block { x: 1, y: 2, z: 3 });
    overlay.render_options = Some(FlatDebugRenderOptions {
        section_occlusion_culling: true,
        force_fullbright: false,
        color_profile: "VANILLA",
    });
    overlay.seed = Some(12345);

    let lines = overlay.lines();
    assert_eq!(lines[0], "POS 1.2 64.0 -2.5");
    assert_eq!(lines[1], "CHUNK 3 -4 SPEED 32.0");
    assert_eq!(lines[2], "SEED 12345");
    assert_eq!(lines[3], "MODE WALK GROUND Y");
    assert_eq!(lines[4], "VIEW R2 CENTER 3 -4");
    assert_eq!(lines[5], "RUN WEB-WORKER CQ1 UQ2");
    assert!(lines.iter().any(|line| line == "CHUNKS L9 V8"));
    assert!(lines.iter().any(|line| line == "DRAW S 10/16 F 120/200"));
    assert!(lines.iter().any(|line| line == "ACTOR R 1/2 I180"));
    assert!(lines.iter().any(|line| line == "MESH B3 U4 R5"));
    assert!(lines.iter().any(|line| line == "PENDING 6"));
    assert!(lines.iter().any(|line| line == "TIME 1200 0.250"));
    assert!(lines.iter().any(|line| line == "SLOT 5"));
    assert!(lines.iter().any(|line| line == "TARGET 1 2 3"));
    assert!(
        lines
            .iter()
            .any(|line| line == "OCC ON  LIGHT  COLOR VANILLA")
    );
    assert_eq!(overlay.to_debug_overlay().title, "DEBUG");
}

#[test]
fn status_overlay_visibility_controls_rendering() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut draw = GuiDrawList::new();

    render_status_overlay(scale, &mut draw, &StatusOverlay::hidden());
    assert!(draw.commands().is_empty());

    render_status_overlay(scale, &mut draw, &StatusOverlay::new("loading", true));
    assert!(!draw.commands().is_empty());
}

#[test]
fn crosshair_renders_center_marks() {
    let mut draw = GuiDrawList::new();

    render_crosshair(GuiScale::from_pixels(960, 540), &mut draw);

    assert_eq!(draw.commands().len(), 4);
    assert_eq!(
        draw.commands()[0],
        GuiDrawCommand::SolidRect {
            rect: Rect::new(239.0, 128.0, 3.0, 15.0),
            color: Color::rgba(0, 0, 0, 115),
            clip: None,
        }
    );
    assert_eq!(
        draw.commands()[1],
        GuiDrawCommand::SolidRect {
            rect: Rect::new(233.0, 134.0, 15.0, 3.0),
            color: Color::rgba(0, 0, 0, 115),
            clip: None,
        }
    );
    assert_eq!(
        draw.commands()[2],
        GuiDrawCommand::SolidRect {
            rect: Rect::new(240.0, 129.0, 1.0, 13.0),
            color: Color::rgba(238, 244, 250, 220),
            clip: None,
        }
    );
    assert_eq!(
        draw.commands()[3],
        GuiDrawCommand::SolidRect {
            rect: Rect::new(234.0, 135.0, 13.0, 1.0),
            color: Color::rgba(238, 244, 250, 220),
            clip: None,
        }
    );
}

#[test]
fn flat_hud_renders_crosshair_hotbar_touch_and_status() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut draw = GuiDrawList::new();
    let mut hud = FlatHud::new(resolved_flat_input(true));
    hud.hotbar = FlatHotbarOverlay::selected(2);
    hud.touch = TouchOverlay {
        visible: true,
        menu_pressed: false,
        movement: TouchJoystickOverlay::default(),
        jump_pressed: false,
        sprint_pressed: false,
        sneak_pressed: false,
        descend_pressed: false,
        interaction_visible: true,
        attack_pressed: false,
        use_pressed: false,
        hotbar_visible: true,
        selected_hotbar_slot: 2,
        hotbar_pressed_slot: None,
        hotbar_icons: EMPTY_HOTBAR_ICONS,
        hotbar_item_stacks: EMPTY_HOTBAR_ITEM_STACKS,
    };
    hud.status = StatusOverlay::new("ready", true);

    assert!(hud.has_visible_commands());
    assert!(!hud.should_render_flat_hotbar());
    render_flat_hud(scale, &mut draw, &hud);

    assert!(!draw.commands().is_empty());
}

#[test]
fn flat_hud_labels_targeted_wheat_below_the_crosshair() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut draw = GuiDrawList::new();
    let mut hud = FlatHud::new(resolved_flat_input(false));
    hud.wheat_target = Some(WheatTargetHud::Mature);

    render_flat_hud(scale, &mut draw, &hud);

    assert!(draw.commands().iter().any(|command| matches!(
        command,
        GuiDrawCommand::Text { text, .. } if text == "Mature wheat - ATK to harvest"
    )));
    assert_eq!(WheatTargetHud::Sprout.label(), "Wheat sprout");
    assert_eq!(WheatTargetHud::Growing.label(), "Growing wheat");
}

#[test]
fn touch_hotbar_uses_the_same_inventory_contents_as_flat_hotbar() {
    let mut draw = GuiDrawList::new();
    let mut hud = FlatHud::new(resolved_flat_input(true));
    let mut inventory = EMPTY_HOTBAR_ITEM_STACKS;
    inventory[0] = Some(mclone_protocol::ItemStackSnapshot {
        kind: mclone_protocol::ItemKind::HuntingSpear,
        count: 1,
    });
    hud.hotbar = FlatHotbarOverlay::selected(0).with_item_stacks(inventory);
    hud.touch = TouchOverlay {
        visible: true,
        hotbar_visible: true,
        selected_hotbar_slot: 8,
        ..TouchOverlay::hidden()
    };

    assert!(!hud.should_render_flat_hotbar());
    render_flat_hud(GuiScale::from_pixels(780, 1688), &mut draw, &hud);

    assert!(draw.commands().iter().any(|command| matches!(
        command,
        GuiDrawCommand::Text { text, .. } if text == "SP"
    )));
    assert!(draw.commands().iter().any(|command| matches!(
        command,
        GuiDrawCommand::Text { text, .. } if text == "Hunting spear"
    )));
    assert!(!draw.commands().iter().any(|command| matches!(
        command,
        GuiDrawCommand::Text { text, .. } if text == "1"
    )));
}

#[test]
fn flat_hud_uses_flat_hotbar_when_touch_controls_are_hidden() {
    let mut draw = GuiDrawList::new();
    let mut hud = FlatHud::new(resolved_flat_input(false));
    hud.hotbar = FlatHotbarOverlay::selected(4);
    hud.touch = TouchOverlay {
        visible: true,
        hotbar_visible: true,
        selected_hotbar_slot: 4,
        ..TouchOverlay::hidden()
    };

    assert!(hud.should_render_flat_hotbar());
    render_flat_hud(GuiScale::from_pixels(960, 540), &mut draw, &hud);

    assert!(draw.commands().len() > 4);
}

#[test]
fn flat_hud_renders_authoritative_statistics_above_hotbar() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut draw = GuiDrawList::new();
    let mut hud = FlatHud::new(resolved_flat_input(false));
    hud.hotbar = FlatHotbarOverlay::selected(0);
    hud.player_statistics = Some(PlayerStatisticsHud {
        jumps: 37,
        successful_block_placements: 12,
    });

    render_flat_hud(scale, &mut draw, &hud);

    assert!(draw.commands().iter().any(|command| matches!(
        command,
        GuiDrawCommand::Text { text, .. } if text == "Jumps 37  Placed 12"
    )));
}

#[test]
fn flat_hud_renders_mallard_field_notes_and_completion_color() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut draw = GuiDrawList::new();
    let mut hud = FlatHud::new(resolved_flat_input(false));
    hud.hotbar = FlatHotbarOverlay::selected(0);
    hud.mallard_field_guide = Some(MallardFieldGuideHud {
        discovered: 4,
        total: 6,
        complete: false,
    });

    render_flat_hud(scale, &mut draw, &hud);

    assert!(draw.commands().iter().any(|command| matches!(
        command,
        GuiDrawCommand::Text { text, color, .. }
            if text == "Mallard field notes 4/6"
                && *color == Color::rgba(114, 206, 255, 255)
    )));
}

#[test]
fn flat_hud_renders_ten_authoritative_health_hearts() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut draw = GuiDrawList::new();
    let mut hud = FlatHud::new(resolved_flat_input(false));
    hud.hotbar = FlatHotbarOverlay::selected(0);
    hud.player_health = Some(PlayerHealthHud {
        health: 15.0,
        max_health: 20.0,
    });

    render_flat_hud(scale, &mut draw, &hud);

    let red_commands = draw
        .commands()
        .iter()
        .filter(|command| {
            matches!(
                command,
                GuiDrawCommand::SolidRect { color, .. }
                    if *color == Color::rgba(225, 47, 55, 255)
            )
        })
        .count();
    assert_eq!(red_commands, 48);
}

#[test]
fn flat_hud_hides_crosshair_when_disabled() {
    let mut draw = GuiDrawList::new();
    let mut hud = FlatHud::new(resolved_flat_input(false));
    hud.crosshair_visible = false;

    assert!(!hud.has_visible_commands());
    render_flat_hud(GuiScale::from_pixels(960, 540), &mut draw, &hud);

    assert!(draw.commands().is_empty());
}

#[test]
fn flat_hotbar_renders_icon_textures_when_present() {
    let mut draw = GuiDrawList::new();
    let mut hud = FlatHud::new(resolved_flat_input(false));
    let icon = GuiTextureUv::new(0.1, 0.2, 0.3, 0.4);
    let mut icons = EMPTY_HOTBAR_ICONS;
    icons[0] = Some(icon);
    hud.hotbar = FlatHotbarOverlay::selected_with_icons(0, icons);

    render_flat_hud(GuiScale::from_pixels(960, 540), &mut draw, &hud);

    assert!(draw.commands().iter().any(|command| matches!(
        command,
        GuiDrawCommand::TextureRect { uv, .. } if *uv == icon
    )));
}

#[test]
fn flat_hud_renders_gamepad_prompt_affordances() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut keyboard_draw = GuiDrawList::new();
    let mut keyboard_hud = FlatHud::new(resolved_flat_input(false));
    keyboard_hud.hotbar = FlatHotbarOverlay::selected(1);
    render_flat_hud(scale, &mut keyboard_draw, &keyboard_hud);

    let mut gamepad_draw = GuiDrawList::new();
    let mut gamepad_hud = FlatHud::new(resolved_gamepad_input());
    gamepad_hud.hotbar = FlatHotbarOverlay::selected(1);

    assert!(gamepad_hud.effective_gamepad_overlay().visible);
    render_flat_hud(scale, &mut gamepad_draw, &gamepad_hud);

    assert!(gamepad_draw.commands().len() > keyboard_draw.commands().len());
}

#[test]
fn gamepad_prompt_rects_stay_inside_gui_space() {
    let scale = GuiScale::from_pixels(640, 480);
    for (rect, _) in gamepad_action_prompt_rects(scale) {
        assert!(rect.x >= 0.0);
        assert!(rect.y >= 0.0);
        assert!(rect.right() <= scale.width);
        assert!(rect.bottom() <= scale.height);
    }
}

#[test]
fn gamepad_prompts_follow_layout_family_and_semantic_bindings() {
    let bindings = GamepadBindings::default();
    assert_eq!(
        gamepad_prompt_for_action(
            ControllerLayoutFamily::XboxLike,
            &bindings,
            InputBindingAction::Jump,
        ),
        Some("A")
    );
    assert_eq!(
        gamepad_prompt_for_action(
            ControllerLayoutFamily::NintendoLike,
            &bindings,
            InputBindingAction::Jump,
        ),
        Some("B")
    );
    assert_eq!(
        gamepad_prompt_for_action(
            ControllerLayoutFamily::PlayStationLike,
            &bindings,
            InputBindingAction::Attack,
        ),
        Some("R2")
    );
    assert_eq!(
        gamepad_control_label(ControllerLayoutFamily::Unknown, GamepadControl::SouthButton,),
        "S"
    );
}

#[test]
fn touch_overlay_renders_native_controls_when_visible() {
    let mut draw = GuiDrawList::new();
    let overlay = TouchOverlay {
        visible: true,
        menu_pressed: false,
        movement: TouchJoystickOverlay {
            active: true,
            base: Point { x: 80.0, y: 320.0 },
            thumb: Point { x: 96.0, y: 284.0 },
        },
        jump_pressed: true,
        sprint_pressed: false,
        sneak_pressed: true,
        descend_pressed: false,
        interaction_visible: true,
        attack_pressed: false,
        use_pressed: true,
        hotbar_visible: true,
        selected_hotbar_slot: 2,
        hotbar_pressed_slot: Some(4),
        hotbar_icons: EMPTY_HOTBAR_ICONS,
        hotbar_item_stacks: EMPTY_HOTBAR_ITEM_STACKS,
    };

    render_touch_overlay(GuiScale::from_pixels(780, 1688), &mut draw, &overlay);

    assert!(!draw.commands().is_empty());
}

#[test]
fn touch_control_hit_rects_stay_inside_gui_space() {
    let scale = GuiScale::from_pixels(780, 1688);
    let movement = touch_movement_zone_rect(scale);
    let actions = touch_action_button_rects(scale);
    let hotbar = touch_hotbar_slot_rects(scale);

    assert_eq!(touch_menu_button_rect(), Rect::new(10.0, 10.0, 40.0, 40.0));
    assert!(movement.contains(Point {
        x: movement.x + 4.0,
        y: movement.y + 4.0,
    }));
    for rect in [
        actions.jump,
        actions.descend,
        actions.attack,
        actions.use_item,
    ] {
        assert!(rect.x >= 0.0);
        assert!(rect.y >= 0.0);
        assert!(rect.right() <= scale.width);
        assert!(rect.bottom() <= scale.height);
    }
    assert_eq!(actions.attack.y, actions.jump.y);
    assert_eq!(actions.use_item.y, actions.descend.y);
    assert!(actions.attack.x < actions.jump.x);
    assert!(actions.use_item.x < actions.descend.x);
    assert_eq!(hotbar.len(), 9);
    for rect in hotbar {
        assert!(rect.x >= 0.0);
        assert!(rect.y >= 0.0);
        assert!(rect.right() <= scale.width);
        assert!(rect.bottom() <= scale.height);
    }
}

#[test]
fn shared_touch_hit_test_resolves_every_control_family() {
    let scale = GuiScale::from_pixels(780, 1688);
    let center = |rect: Rect| Point {
        x: rect.x + rect.width * 0.5,
        y: rect.y + rect.height * 0.5,
    };
    let actions = touch_action_button_rects(scale);

    assert_eq!(
        touch_control_at(scale, center(touch_menu_button_rect())),
        TouchControl::MenuButton
    );
    assert_eq!(
        touch_control_at(scale, center(actions.jump)),
        TouchControl::JumpButton
    );
    assert_eq!(
        touch_control_at(scale, center(actions.attack)),
        TouchControl::AttackButton
    );
    assert_eq!(
        touch_control_at(scale, center(touch_hotbar_slot_rects(scale)[8])),
        TouchControl::HotbarSlot(8)
    );
    assert_eq!(
        touch_control_at(scale, center(touch_movement_zone_rect(scale))),
        TouchControl::MovementStick
    );
    assert_eq!(
        touch_control_at(scale, Point { x: 390.0, y: 400.0 }),
        TouchControl::LookDrag
    );
}

#[test]
fn server_cadence_cycles_only_emit_valid_cadences() {
    let cadence = GameSimulationCadence::new(20, 20, 60);

    assert_eq!(
        cadence.next_host_rate(),
        GameSimulationCadence::new(30, 30, 60)
    );
    assert!(cadence.next_host_rate().is_valid());
    assert_eq!(
        cadence.next_gameplay_rate(),
        GameSimulationCadence::new(20, 60, 60)
    );
    assert!(cadence.next_gameplay_rate().is_valid());
    assert_eq!(
        cadence.next_physics_rate(),
        GameSimulationCadence::new(20, 20, 120)
    );
    assert!(cadence.next_physics_rate().is_valid());

    let normalized = GameSimulationCadence::new(60, 30, 30).next_host_rate();
    assert_eq!(normalized, GameSimulationCadence::new(20, 20, 60));
    assert!(normalized.is_valid());
}
