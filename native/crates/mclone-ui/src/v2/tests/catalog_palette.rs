use super::*;

#[test]
fn title_flow_screens_route_through_v2_surface() {
    let mut host = GameUiHost::new();
    host.set_scale(GuiScale::from_pixels(960, 540));
    let state = world_catalog_render_state();

    let title = host
        .render_v2_panel_draw_list(state)
        .expect("Title is a v2 panel");
    assert_eq!(title.cache, UiDrawCacheStats::rebuild());
    let snapshot = host.v2_debug_snapshot().expect("Title has debug data");
    assert_eq!(snapshot.screen, Some(UiScreenId::Title));
    let lobby = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_TITLE_ENTER_LOBBY)
        .expect("Enter Lobby button");
    let singleplayer = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_TITLE_START)
        .expect("Singleplayer button");
    assert_eq!(lobby.label, "Enter Lobby");
    assert!(lobby.enabled);
    assert!(lobby.rect.y < singleplayer.rect.y);
    assert!(
        snapshot
            .widgets
            .iter()
            .any(|widget| widget.label == "Singleplayer")
    );
    assert!(
        snapshot
            .widgets
            .iter()
            .any(|widget| widget.label == "Join Remote")
    );

    let new_world = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_TITLE_START)
        .expect("New World button")
        .rect;
    assert!(host.pointer_down(point_in(new_world)));
    let (_handled, action) = host.pointer_up(point_in(new_world));
    assert_eq!(action, Some(GameUiAction::OpenWorldList));
    host.apply_action(action.unwrap());

    host.render_v2_panel_draw_list(state)
        .expect("WorldList is a v2 panel");
    let snapshot = host.v2_debug_snapshot().expect("WorldList has debug data");
    assert_eq!(snapshot.screen, Some(UiScreenId::WorldList));
    assert!(
        snapshot
            .widgets
            .iter()
            .any(|widget| widget.label == "Alpha Base")
    );

    let create_new = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_WORLD_LIST_CREATE)
        .expect("Create button")
        .rect;
    assert!(host.pointer_down(point_in(create_new)));
    let (_handled, action) = host.pointer_up(point_in(create_new));
    assert_eq!(action, Some(GameUiAction::OpenWorldCreate));
    host.apply_action(action.unwrap());

    host.set_new_world_seed(12345);
    host.render_v2_panel_draw_list(state)
        .expect("WorldCreate is a v2 panel");
    let snapshot = host
        .v2_debug_snapshot()
        .expect("WorldCreate has debug data");
    assert_eq!(snapshot.screen, Some(UiScreenId::WorldCreate));
    let profile = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_WORLD_CREATE_PROFILE)
        .expect("World profile button");
    assert_eq!(profile.label, "World: Vanilla 1.17 Overworld");
    assert!(host.pointer_down(point_in(profile.rect)));
    let (_handled, profile_action) = host.pointer_up(point_in(profile.rect));
    assert_eq!(
        profile_action,
        Some(GameUiAction::CycleWorldGenerationProfile)
    );
    let starter = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_WORLD_CREATE_STARTER)
        .expect("World start button");
    assert_eq!(starter.label, "Start: Wild Start");
    assert!(host.pointer_down(point_in(starter.rect)));
    assert_eq!(
        host.pointer_up(point_in(starter.rect)).1,
        Some(GameUiAction::CycleWorldStarterContent)
    );
    let showcase = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_WORLD_CREATE_SHOWCASE)
        .expect("Homestead showcase button");
    assert_eq!(showcase.label, "Use Homestead Showcase");
    assert!(host.pointer_down(point_in(showcase.rect)));
    assert_eq!(
        host.pointer_up(point_in(showcase.rect)).1,
        Some(GameUiAction::ApplyHomesteadShowcasePreset)
    );
    let create = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_WORLD_CREATE_CREATE)
        .expect("Create World button")
        .rect;
    assert!(host.pointer_down(point_in(create)));
    let (_handled, action) = host.pointer_up(point_in(create));
    assert_eq!(action, Some(GameUiAction::CreateCatalogWorld));
}

#[test]
fn lobby_capability_is_visible_and_non_actionable_when_unavailable() {
    let mut host = GameUiHost::new();
    host.set_scale(GuiScale::from_pixels(960, 540));
    let mut state = world_catalog_render_state();
    state.lobby_scenario_available = false;
    host.render_v2_panel_draw_list(state).unwrap();
    let lobby = host
        .v2_debug_snapshot()
        .unwrap()
        .widgets
        .into_iter()
        .find(|widget| widget.id == UI_V2_TITLE_ENTER_LOBBY)
        .unwrap();
    assert_eq!(lobby.label, "Enter Lobby (Unavailable)");
    assert!(!lobby.enabled);
    assert!(host.pointer_down(point_in(lobby.rect)));
    assert_eq!(host.pointer_up(point_in(lobby.rect)).1, None);
}

#[test]
fn world_list_rows_emit_selection_open_and_delete_actions() {
    let mut surface = UiSurface::new();
    let mut state = world_catalog_render_state();
    let second = WorldCatalogUiWorldId(12);
    surface.set_screen(Some(UiScreenId::WorldList));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(state);

    let rows = surface
        .layout()
        .widgets()
        .iter()
        .filter(|widget| matches!(widget.kind, UiWidgetKind::WorldRow { .. }))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, world_list_row_id(0));
    assert_eq!(rows[0].label, "Alpha Base");
    assert_eq!(rows[1].id, world_list_row_id(1));

    assert!(surface.pointer_down(point_in(rows[1].rect), state));
    let (_handled, action) = surface.pointer_up(point_in(rows[1].rect), state);
    assert_eq!(action, Some(GameUiAction::SelectWorld(second)));

    state.world_catalog.selected = Some(second);
    surface.set_render_state(state);
    let snapshot = surface.debug_snapshot().expect("WorldList debug snapshot");
    let open = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_WORLD_LIST_OPEN)
        .expect("Open button");
    assert!(open.enabled);
    assert!(surface.pointer_down(point_in(open.rect), state));
    let (_handled, action) = surface.pointer_up(point_in(open.rect), state);
    assert_eq!(action, Some(GameUiAction::OpenWorld(second)));

    let delete = surface
        .debug_snapshot()
        .expect("WorldList debug snapshot")
        .widgets
        .into_iter()
        .find(|widget| widget.id == UI_V2_WORLD_LIST_DELETE)
        .expect("Delete button");
    assert!(delete.enabled);
    assert!(surface.pointer_down(point_in(delete.rect), state));
    let (_handled, action) = surface.pointer_up(point_in(delete.rect), state);
    assert_eq!(action, Some(GameUiAction::ConfirmDeleteWorld(second)));
}

#[test]
fn active_world_delete_is_disabled_until_delete_confirm_can_delete() {
    let mut state = world_catalog_render_state();
    let active = WorldCatalogUiWorldId(11);
    state.world_catalog.selected = Some(active);
    state.world_catalog.active = Some(active);
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::WorldList));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(state);

    let delete = surface
        .debug_snapshot()
        .expect("WorldList debug snapshot")
        .widgets
        .into_iter()
        .find(|widget| widget.id == UI_V2_WORLD_LIST_DELETE)
        .expect("Delete button");
    assert!(!delete.enabled);

    surface.set_screen(Some(UiScreenId::WorldDeleteConfirm { id: active }));
    let confirm = surface
        .debug_snapshot()
        .expect("DeleteConfirm debug snapshot")
        .widgets
        .into_iter()
        .find(|widget| widget.id == UI_V2_WORLD_DELETE_CONFIRM)
        .expect("Delete confirm button");
    assert!(!confirm.enabled);

    state.world_catalog.active = None;
    surface.set_render_state(state);
    let confirm = surface
        .debug_snapshot()
        .expect("DeleteConfirm debug snapshot")
        .widgets
        .into_iter()
        .find(|widget| widget.id == UI_V2_WORLD_DELETE_CONFIRM)
        .expect("Delete confirm button");
    assert!(confirm.enabled);
    assert!(surface.pointer_down(point_in(confirm.rect), state));
    let (_handled, action) = surface.pointer_up(point_in(confirm.rect), state);
    assert_eq!(action, Some(GameUiAction::DeleteWorld(active)));
}

#[test]
fn join_remote_screen_routes_through_v2_surface() {
    let mut host = GameUiHost::new();
    host.set_screen(Some(GameScreen::JoinRemote));
    host.set_join_remote_addr("10.0.0.5:25565");
    host.set_scale(GuiScale::from_pixels(960, 540));

    let draw = host
        .render_v2_panel_draw_list(GameUiRenderState::default())
        .expect("JoinRemote is a v2 panel");
    assert!(!draw.draw.commands().is_empty());
    let snapshot = host.v2_debug_snapshot().expect("JoinRemote has debug data");
    assert_eq!(snapshot.screen, Some(UiScreenId::JoinRemote));
    let connect = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == UI_V2_JOIN_REMOTE_CONNECT)
        .expect("Connect button")
        .rect;
    assert!(host.pointer_down(point_in(connect)));
    let (_handled, action) = host.pointer_up(point_in(connect));
    assert_eq!(action, Some(GameUiAction::JoinRemote));
}

#[test]
fn block_palette_layout_uses_committed_slots_for_actions() {
    let scale = GuiScale::from_pixels(960, 540);
    let state = block_palette_state(4);
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::BlockPalette));
    surface.set_scale(scale);
    surface.set_render_state(state);

    let widgets = surface.layout().widgets().to_vec();
    assert_eq!(widgets.len(), 3);
    assert_eq!(widgets[0].id, block_palette_slot_id(0));
    assert_eq!(widgets[0].label, "Bricks");
    assert_eq!(
        widgets[0].rect,
        block_palette_slot_rect(scale, state.block_palette, 0).expect("first palette slot rect")
    );

    let point = point_in(widgets[0].rect);
    assert!(surface.pointer_down(point, state));
    let (_handled, action) = surface.pointer_up(point, state);
    assert_eq!(
        action,
        Some(GameUiAction::AssignHotbarBlock {
            slot: 4,
            block_state: 91,
        })
    );

    let actor_point = point_in(widgets[2].rect);
    assert!(surface.pointer_down(actor_point, state));
    let (_handled, action) = surface.pointer_up(actor_point, state);
    assert_eq!(
        action,
        Some(GameUiAction::AssignHotbarActor {
            slot: 4,
            actor: DebugActorTool::Chicken,
        })
    );
}

#[test]
fn block_palette_grid_cache_survives_hover_and_pointer_jitter() {
    let state = block_palette_state(2);
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::BlockPalette));
    surface.set_scale(GuiScale::from_pixels(960, 540));

    let first = surface.render_draw_list(state);
    assert_eq!(
        surface.block_palette_grid_cache,
        UiDrawCacheStats::rebuild()
    );
    assert!(!first.commands().is_empty());

    let second = surface.render_draw_list(state);
    assert_eq!(
        surface.block_palette_grid_cache,
        UiDrawCacheStats::cache_hit()
    );
    assert_eq!(second, first);

    let slots = surface.layout().widgets().to_vec();
    let slot_zero = slots[0].rect;
    surface.pointer_move(point_in(slot_zero), state);
    let hovered_revision = surface.panel_revision().expect("active surface");
    let hovered = surface.render_draw_list(state);
    assert_eq!(
        surface.block_palette_grid_cache,
        UiDrawCacheStats::cache_hit()
    );
    assert_ne!(hovered, second);

    surface.pointer_move(
        Point {
            x: slot_zero.x + 2.0,
            y: slot_zero.y + 2.0,
        },
        state,
    );
    assert_eq!(surface.panel_revision(), Some(hovered_revision));
    let jitter = surface.render_draw_list(state);
    assert_eq!(
        surface.block_palette_grid_cache,
        UiDrawCacheStats::cache_hit()
    );
    assert_eq!(jitter, hovered);

    let mut changed = state;
    changed.block_palette.entries[1] = Some(BlockPaletteEntry::new(
        41,
        Some(GuiTextureUv::new(0.2, 0.3, 0.4, 0.5)),
        "Oak Log",
    ));
    surface.render_draw_list(changed);
    assert_eq!(
        surface.block_palette_grid_cache,
        UiDrawCacheStats::rebuild()
    );
}

#[test]
fn block_palette_controller_focus_matches_pointer_highlight_and_tooltip() {
    let state = block_palette_state(2);
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::BlockPalette));
    surface.set_scale(GuiScale::from_pixels(960, 540));

    let normal = surface.render_draw_list(state);
    assert_eq!(
        surface.navigate(GuiNavigation::NextPage, state),
        (true, None)
    );
    assert_eq!(
        surface
            .debug_snapshot()
            .and_then(|snapshot| snapshot.focused),
        Some(block_palette_slot_id(0)),
    );
    let focused = surface.render_draw_list(state);
    assert_ne!(focused, normal);
    assert!(
        focused.commands().len() > normal.commands().len() + 4,
        "focus should add both the four-edge slot outline and tooltip commands",
    );

    let slot = surface.layout().widgets()[0].rect;
    surface.pointer_move(point_in(slot), state);
    let hovered = surface.render_draw_list(state);
    assert_eq!(focused, hovered);
}

#[test]
fn game_ui_host_block_palette_uses_v2_panel_cache() {
    let state = block_palette_state(4);
    let mut host = GameUiHost::new_ingame();
    host.set_screen(Some(GameScreen::BlockPalette));
    host.set_scale(GuiScale::from_pixels(960, 540));

    let first = host
        .render_v2_panel_draw_list(state)
        .expect("BlockPalette is a v2 panel");
    assert_eq!(first.cache, UiDrawCacheStats::rebuild());
    assert!(!first.draw.commands().is_empty());
    assert!(first.draw.commands().iter().any(|command| matches!(
        command,
        GuiDrawCommand::TextureRect { uv, .. }
            if *uv == GuiTextureUv::new(0.1, 0.2, 0.3, 0.4)
    )));

    let second = host
        .render_v2_panel_draw_list(state)
        .expect("BlockPalette is a v2 panel");
    assert_eq!(second.cache, UiDrawCacheStats::cache_hit());
    assert_eq!(second.revision, first.revision);
    assert_eq!(second.draw, first.draw);

    let snapshot = host
        .v2_debug_snapshot()
        .expect("BlockPalette has debug data");
    let slot = snapshot
        .widgets
        .iter()
        .find(|widget| widget.id == block_palette_slot_id(0))
        .expect("first palette slot")
        .rect;
    host.pointer_move(point_in(slot));
    let hovered = host
        .render_v2_panel_draw_list(state)
        .expect("BlockPalette is a v2 panel");
    assert_eq!(hovered.cache, UiDrawCacheStats::rebuild());
    assert_ne!(hovered.revision, first.revision);

    host.pointer_move(Point {
        x: slot.x + 2.0,
        y: slot.y + 2.0,
    });
    let jitter = host
        .render_v2_panel_draw_list(state)
        .expect("BlockPalette is a v2 panel");
    assert_eq!(jitter.cache, UiDrawCacheStats::cache_hit());
    assert_eq!(jitter.revision, hovered.revision);
    assert_eq!(jitter.draw, hovered.draw);
}
