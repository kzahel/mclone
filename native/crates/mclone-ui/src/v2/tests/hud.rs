use super::*;

#[test]
fn game_ui_host_flat_hud_retained_cache_tracks_static_geometry() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut host = GameUiHost::new_ingame();
    let mut hud = FlatHud::new(keyboard_mouse_input());
    hud.hotbar = FlatHotbarOverlay::selected(2);

    let first = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        first.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 2,
            cache_hit_count: 0,
        }
    );
    assert!(!first.draw.commands().is_empty());

    let second = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        second.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 0,
            cache_hit_count: 2,
        }
    );
    assert_eq!(second.draw, first.draw);

    let mut icons = EMPTY_HOTBAR_ICONS;
    icons[0] = Some(GuiTextureUv::new(0.1, 0.2, 0.3, 0.4));
    hud.hotbar = FlatHotbarOverlay::selected_with_icons(2, icons);
    let icon_changed = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        icon_changed.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 1,
            cache_hit_count: 1,
        }
    );
    assert_ne!(icon_changed.draw, second.draw);

    hud.hotbar = FlatHotbarOverlay::selected_with_icons(3, icons);
    let selected_changed = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        selected_changed.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 1,
            cache_hit_count: 1,
        }
    );
    assert_ne!(selected_changed.draw, icon_changed.draw);
}

#[test]
fn game_ui_host_flat_hud_status_cache_rebuilds_independently() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut host = GameUiHost::new_ingame();
    let mut hud = FlatHud::new(keyboard_mouse_input());
    hud.hotbar = FlatHotbarOverlay::selected(2);
    hud.status = crate::StatusOverlay::new("ready", true);

    let first = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        first.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 3,
            cache_hit_count: 0,
        }
    );

    let second = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        second.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 0,
            cache_hit_count: 3,
        }
    );
    assert_eq!(second.draw, first.draw);

    hud.status = crate::StatusOverlay::new("syncing", true);
    let message_changed = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        message_changed.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 1,
            cache_hit_count: 2,
        }
    );
    assert_ne!(message_changed.draw, second.draw);

    hud.status = crate::StatusOverlay::hidden();
    let hidden = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        hidden.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 0,
            cache_hit_count: 2,
        }
    );
    assert_ne!(hidden.draw, message_changed.draw);

    hud.status = crate::StatusOverlay::new("syncing", true);
    let restored = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        restored.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 1,
            cache_hit_count: 2,
        }
    );
}

#[test]
fn game_ui_host_flat_hud_gamepad_prompt_cache_rebuilds_independently() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut host = GameUiHost::new_ingame();
    let mut hud = FlatHud::new(gamepad_input());
    hud.hotbar = FlatHotbarOverlay::selected(2);

    let first = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        first.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 3,
            cache_hit_count: 0,
        }
    );

    let second = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        second.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 0,
            cache_hit_count: 3,
        }
    );
    assert_eq!(second.draw, first.draw);

    hud.gamepad.action_hints_visible = false;
    let prompts_changed = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        prompts_changed.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 1,
            cache_hit_count: 2,
        }
    );
    assert_ne!(prompts_changed.draw, second.draw);

    hud.gamepad.hotbar_hints_visible = false;
    let prompts_hidden = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        prompts_hidden.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 0,
            cache_hit_count: 2,
        }
    );
    assert_ne!(prompts_hidden.draw, prompts_changed.draw);
}

#[test]
fn game_ui_host_flat_hud_touch_prompt_cache_rebuilds_independently() {
    let scale = GuiScale::from_pixels(780, 1688);
    let mut host = GameUiHost::new_ingame();
    let mut hud = FlatHud::new(touch_input());
    hud.hotbar = FlatHotbarOverlay::selected(2);
    hud.touch = crate::TouchOverlay {
        visible: true,
        interaction_visible: true,
        hotbar_visible: true,
        selected_hotbar_slot: 2,
        ..crate::TouchOverlay::hidden()
    };

    let first = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        first.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 2,
            cache_hit_count: 0,
        }
    );

    let second = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        second.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 0,
            cache_hit_count: 2,
        }
    );
    assert_eq!(second.draw, first.draw);

    hud.touch.jump_pressed = true;
    let touch_changed = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        touch_changed.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 1,
            cache_hit_count: 1,
        }
    );
    assert_ne!(touch_changed.draw, second.draw);
}

#[test]
fn game_ui_host_flat_hud_debug_cache_rebuilds_independently() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut host = GameUiHost::new_ingame();
    let mut hud = FlatHud::new(keyboard_mouse_input());
    hud.hotbar = FlatHotbarOverlay::selected(2);
    hud.debug = Some(crate::FlatHudDebugOverlay::new(crate::DebugOverlay::new(
        "DEBUG",
        ["POS 1.0 64.0 -2.0", "CHUNKS 9"],
    )));

    let first = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        first.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 3,
            cache_hit_count: 0,
        }
    );

    let second = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        second.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 0,
            cache_hit_count: 3,
        }
    );
    assert_eq!(second.draw, first.draw);

    hud.debug = Some(crate::FlatHudDebugOverlay::new(crate::DebugOverlay::new(
        "DEBUG",
        ["POS 1.0 64.0 -2.0", "CHUNKS 10"],
    )));
    let debug_changed = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        debug_changed.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 1,
            cache_hit_count: 2,
        }
    );
    assert_ne!(debug_changed.draw, second.draw);

    hud.debug = None;
    let debug_hidden = host.render_flat_hud_draw_list(scale, &hud);
    assert_eq!(
        debug_hidden.retained_cache,
        UiDrawCacheStats {
            rebuild_count: 0,
            cache_hit_count: 2,
        }
    );
    assert_ne!(debug_hidden.draw, debug_changed.draw);
}

#[test]
fn game_ui_host_flat_hud_draw_matches_standalone_renderer() {
    let scale = GuiScale::from_pixels(960, 540);
    let mut host = GameUiHost::new_ingame();
    let mut hud = FlatHud::new(keyboard_mouse_input());
    let mut icons = EMPTY_HOTBAR_ICONS;
    icons[0] = Some(GuiTextureUv::new(0.1, 0.2, 0.3, 0.4));
    hud.hotbar = FlatHotbarOverlay::selected_with_icons(4, icons);
    hud.status = crate::StatusOverlay::new("ready", true);
    hud.debug = Some(crate::FlatHudDebugOverlay::new(crate::DebugOverlay::new(
        "DEBUG",
        ["POS 1.0 64.0 -2.0"],
    )));

    let retained = host.render_flat_hud_draw_list(scale, &hud);
    let mut standalone = GuiDrawList::new();
    render_flat_hud(scale, &mut standalone, &hud);

    assert_eq!(retained.draw, standalone);
}
