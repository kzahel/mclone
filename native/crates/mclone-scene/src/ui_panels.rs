use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct XrGameplayInteractionEdges {
    pub(super) attack: bool,
    pub(super) use_item: bool,
}

impl XrGameplayInteractionEdges {
    pub(super) const fn any(self) -> bool {
        self.attack || self.use_item
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum XrGameplayInteractionAction {
    Attack,
    Use,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum XrUiPanelAnchor {
    #[default]
    Head,
    LeftHand,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct XrMenuPanelOverlayState {
    gui_scale: GuiScale,
    progress: Option<LoadingProgressOverlay>,
    status: Option<StatusOverlay>,
}

impl XrMenuPanelOverlayState {
    fn new(
        gui_scale: GuiScale,
        progress: Option<&LoadingProgressOverlay>,
        status: &StatusOverlay,
    ) -> Option<Self> {
        let progress = progress.cloned();
        let status = (status.visible && !status.message.is_empty()).then(|| status.clone());
        if progress.is_none() && status.is_none() {
            return None;
        }
        Some(Self {
            gui_scale,
            progress,
            status,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct XrMenuPanelOverlayDraw {
    draw: GuiDrawList,
    cache_revision: Option<UiPanelRevision>,
    draw_cache: UiDrawCacheStats,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct XrMenuPanelOverlayCache {
    state: Option<XrMenuPanelOverlayState>,
    draw: GuiDrawList,
    revision_content: u64,
}

impl XrMenuPanelOverlayCache {
    fn render(
        &mut self,
        gui_scale: GuiScale,
        progress: Option<&LoadingProgressOverlay>,
        status: &StatusOverlay,
    ) -> XrMenuPanelOverlayDraw {
        let Some(state) = XrMenuPanelOverlayState::new(gui_scale, progress, status) else {
            self.state = None;
            self.draw.clear();
            return XrMenuPanelOverlayDraw {
                draw: GuiDrawList::new(),
                cache_revision: None,
                draw_cache: UiDrawCacheStats::default(),
            };
        };

        if self.state.as_ref() == Some(&state) {
            return XrMenuPanelOverlayDraw {
                draw: self.draw.clone(),
                cache_revision: Some(UiPanelRevision::new(self.revision_content, 0)),
                draw_cache: UiDrawCacheStats::cache_hit(),
            };
        }

        let mut draw = GuiDrawList::new();
        if let Some(progress) = state.progress.as_ref() {
            render_loading_progress_overlay(gui_scale, &mut draw, progress);
        }
        if let Some(status) = state.status.as_ref() {
            render_status_overlay(gui_scale, &mut draw, status);
        }
        self.revision_content = self.revision_content.wrapping_add(1).max(1);
        self.state = Some(state);
        self.draw = draw.clone();
        XrMenuPanelOverlayDraw {
            draw,
            cache_revision: Some(UiPanelRevision::new(self.revision_content, 0)),
            draw_cache: UiDrawCacheStats::rebuild(),
        }
    }
}

pub(crate) struct XrMenuPanelDraw {
    pub(super) panel_draw: GuiDrawList,
    pub(super) cache_revision: Option<UiPanelRevision>,
    pub(super) overlay_draw: GuiDrawList,
    pub(super) overlay_cache_revision: Option<UiPanelRevision>,
    pub(super) draw_cache: UiDrawCacheStats,
}

pub(crate) fn prepare_xr_menu_panel_draw(
    ui: &mut GameUiHost,
    overlay_cache: &mut XrMenuPanelOverlayCache,
    gui_scale: GuiScale,
    ui_state: GameUiRenderState,
    progress: Option<&LoadingProgressOverlay>,
    status: &StatusOverlay,
) -> XrMenuPanelDraw {
    ui.set_scale(gui_scale);
    let panel_draw = ui.render_v2_panel_draw_list(ui_state);
    let overlay_draw = overlay_cache.render(gui_scale, progress, status);
    let mut draw_cache = panel_draw
        .as_ref()
        .map_or_else(UiDrawCacheStats::default, |panel_draw| panel_draw.cache);
    draw_cache.add(overlay_draw.draw_cache);
    XrMenuPanelDraw {
        panel_draw: panel_draw
            .as_ref()
            .map_or_else(GuiDrawList::new, |panel_draw| panel_draw.draw.clone()),
        cache_revision: panel_draw.map(|panel_draw| panel_draw.revision),
        overlay_draw: overlay_draw.draw,
        overlay_cache_revision: overlay_draw.cache_revision,
        draw_cache,
    }
}

pub fn xr_menu_toggle_pressed(actions: &PlayerActionFrame) -> bool {
    actions.held.contains(&PlayerAction::OpenMenu)
}

pub fn xr_game_ui_toggle_pressed(actions: &PlayerActionFrame) -> bool {
    actions.held.contains(&PlayerAction::OpenBlockPalette)
}

pub(crate) fn xr_analog_button_down(value: f32, was_down: bool) -> bool {
    let value = if value.is_finite() { value } else { 0.0 };
    if was_down {
        value >= XR_MENU_POINTER_TRIGGER_RELEASE
    } else {
        value >= XR_MENU_POINTER_TRIGGER_PRESS
    }
}

pub fn xr_controller_interaction_ray_from_controllers(
    controllers: &[TrackedControllerState],
    transform: XrStageToWorld,
) -> Option<(Vec3, Vec3)> {
    [XrHand::Right, XrHand::Left].into_iter().find_map(|hand| {
        controllers
            .iter()
            .find(|controller| controller.hand == hand)
            .and_then(|controller| xr_controller_interaction_ray(controller, transform))
    })
}

pub(crate) fn xr_interaction_ray_with_head_fallback(
    input: &XrInputFrame,
    transform: XrStageToWorld,
    head_gaze_stage: Option<(Vec3, Vec3)>,
) -> Option<(Vec3, Vec3)> {
    xr_controller_interaction_ray_from_controllers(&input.tracked, transform).or_else(|| {
        head_gaze_stage.map(|(origin, direction)| {
            (
                transform.transform_position(origin),
                transform.transform_direction(direction).normalize_or_zero(),
            )
        })
    })
}

pub(crate) fn xr_controller_interaction_ray(
    controller: &TrackedControllerState,
    transform: XrStageToWorld,
) -> Option<(Vec3, Vec3)> {
    let ray_origin = transform.transform_position(controller.aim_position?);
    let ray_direction = transform.transform_direction(controller.aim_direction?);
    if !ray_origin.is_finite()
        || !ray_direction.is_finite()
        || ray_direction.length_squared() <= f32::EPSILON
    {
        return None;
    }
    Some((ray_origin, ray_direction.normalize()))
}

pub fn xr_gameplay_controller_ray_line_from_controllers(
    input: &XrInputFrame,
    transform: XrStageToWorld,
    hit_distance: Option<f32>,
    pick_range: f32,
) -> Option<WorldGuiLine> {
    let controller = input
        .tracked
        .iter()
        .find(|controller| controller.hand == XR_GAMEPLAY_INTERACTION_HAND)?;
    let (ray_origin, ray_direction) = xr_controller_interaction_ray(controller, transform)?;
    let pick_range = if pick_range.is_finite() && pick_range > 0.0 {
        pick_range
    } else {
        XR_MENU_CONTROLLER_RAY_LENGTH_BLOCKS
    };
    let distance = hit_distance
        .filter(|distance| distance.is_finite() && *distance >= 0.0)
        .unwrap_or(pick_range)
        .clamp(0.0, pick_range);
    Some(WorldGuiLine::new(
        ray_origin,
        ray_origin + ray_direction * distance,
        xr_gameplay_controller_ray_color(input, controller.hand),
    ))
}

pub(crate) fn xr_gameplay_controller_ray_color(input: &XrInputFrame, hand: XrHand) -> [f32; 4] {
    let specific = input.xr_specific.controller(hand);
    let trigger_active = specific.is_some_and(|controller| {
        controller.pointer_select_value.is_finite()
            && controller.pointer_select_value >= XR_MENU_POINTER_TRIGGER_PRESS
    });
    let squeeze_active = specific.is_some_and(|controller| {
        controller.squeeze_value.is_finite()
            && controller.squeeze_value >= XR_MENU_POINTER_TRIGGER_PRESS
    });
    if trigger_active || squeeze_active {
        XR_MENU_TRIGGER_RAY_COLOR
    } else {
        xr_menu_controller_ray_color(
            hand,
            specific.map_or(0.0, |state| state.pointer_select_value),
        )
    }
}

pub fn xr_menu_panel_from_render_views(render_views: [ChunkRenderView; 2]) -> WorldGuiPanel {
    let center_position = (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
    let forward = average_unit_direction(
        render_views[0].camera_forward,
        render_views[1].camera_forward,
        Vec3::NEG_Z,
    );
    let up = average_unit_direction(
        render_views[0].camera_up,
        render_views[1].camera_up,
        Vec3::Y,
    );
    let right = average_unit_direction(
        render_views[0].camera_right,
        render_views[1].camera_right,
        Vec3::X,
    );
    WorldGuiPanel::new(
        center_position + forward * XR_MENU_PANEL_DISTANCE_BLOCKS,
        right,
        up,
        XR_MENU_PANEL_WIDTH_BLOCKS,
        xr_menu_panel_height_blocks(),
    )
}

pub fn xr_diagnostic_panel_from_render_views(render_views: [ChunkRenderView; 2]) -> WorldGuiPanel {
    let center_position = (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
    let forward = average_unit_direction(
        render_views[0].camera_forward,
        render_views[1].camera_forward,
        Vec3::NEG_Z,
    );
    let up = average_unit_direction(
        render_views[0].camera_up,
        render_views[1].camera_up,
        Vec3::Y,
    );
    let right = average_unit_direction(
        render_views[0].camera_right,
        render_views[1].camera_right,
        Vec3::X,
    );
    WorldGuiPanel::new(
        center_position
            + forward * XR_DIAGNOSTIC_PANEL_DISTANCE_BLOCKS
            + right * XR_DIAGNOSTIC_PANEL_RIGHT_OFFSET_BLOCKS
            + up * XR_DIAGNOSTIC_PANEL_UP_OFFSET_BLOCKS,
        right,
        up,
        XR_DIAGNOSTIC_PANEL_WIDTH_BLOCKS,
        xr_diagnostic_panel_height_blocks(),
    )
}

pub fn xr_game_ui_panel_from_controllers(
    controllers: &[TrackedControllerState],
    transform: XrStageToWorld,
    render_views: [ChunkRenderView; 2],
) -> Option<WorldGuiPanel> {
    let controller = controllers
        .iter()
        .find(|controller| controller.hand == XR_GAME_UI_PANEL_HAND)?;
    let stage_anchor = controller.grip_position.or(controller.aim_position)?;
    let anchor = transform.transform_position(stage_anchor);
    if !anchor.is_finite() {
        return None;
    }

    let eye_center = (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
    let view_up = average_unit_direction(
        render_views[0].camera_up,
        render_views[1].camera_up,
        Vec3::Y,
    );
    let view_right = average_unit_direction(
        render_views[0].camera_right,
        render_views[1].camera_right,
        Vec3::X,
    );
    let view_forward = average_unit_direction(
        render_views[0].camera_forward,
        render_views[1].camera_forward,
        Vec3::NEG_Z,
    );
    let mut normal = eye_center - anchor;
    if !normal.is_finite() || normal.length_squared() <= f32::EPSILON {
        normal = -view_forward;
    }
    let normal = normal.normalize_or_zero();
    if normal.length_squared() <= f32::EPSILON {
        return None;
    }
    let mut right = view_right - normal * view_right.dot(normal);
    if !right.is_finite() || right.length_squared() <= f32::EPSILON {
        right = view_up.cross(normal);
    }
    let right = right.normalize_or_zero();
    if right.length_squared() <= f32::EPSILON {
        return None;
    }
    let up = normal.cross(right).normalize_or_zero();
    if up.length_squared() <= f32::EPSILON {
        return None;
    }
    let center = anchor
        + up * XR_GAME_UI_PANEL_UP_OFFSET_BLOCKS
        + normal * XR_GAME_UI_PANEL_FORWARD_OFFSET_BLOCKS;
    Some(WorldGuiPanel::new(
        center,
        right,
        up,
        XR_GAME_UI_PANEL_WIDTH_BLOCKS,
        xr_game_ui_panel_height_blocks(),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrMenuPointerHit {
    pub hand: XrHand,
    pub point: Point,
    pub trigger: f32,
    pub distance: f32,
}

pub fn xr_menu_pointer_hit_from_controllers(
    input: &XrInputFrame,
    transform: XrStageToWorld,
    panel: WorldGuiPanel,
    gui_scale: GuiScale,
) -> Option<XrMenuPointerHit> {
    [XrHand::Right, XrHand::Left].into_iter().find_map(|hand| {
        input
            .tracked
            .iter()
            .find(|controller| controller.hand == hand)
            .and_then(|controller| {
                let ray_origin = transform.transform_position(controller.aim_position?);
                let ray_direction = transform.transform_direction(controller.aim_direction?);
                xr_menu_panel_pointer_hit(panel, gui_scale, ray_origin, ray_direction).map(
                    |(point, distance)| XrMenuPointerHit {
                        hand,
                        point,
                        trigger: input
                            .xr_specific
                            .controller(hand)
                            .map_or(0.0, |specific| specific.pointer_select_value),
                        distance,
                    },
                )
            })
    })
}

pub fn xr_menu_controller_ray_lines_from_controllers(
    input: &XrInputFrame,
    transform: XrStageToWorld,
    panel: WorldGuiPanel,
) -> Vec<WorldGuiLine> {
    [XrHand::Left, XrHand::Right]
        .into_iter()
        .filter_map(|hand| {
            input
                .tracked
                .iter()
                .find(|controller| controller.hand == hand)
                .and_then(|controller| {
                    xr_menu_controller_ray_line(
                        controller,
                        input
                            .xr_specific
                            .controller(hand)
                            .map_or(0.0, |specific| specific.pointer_select_value),
                        transform,
                        panel,
                    )
                })
        })
        .collect()
}

pub(crate) fn xr_menu_controller_ray_line(
    controller: &TrackedControllerState,
    pointer_select_value: f32,
    transform: XrStageToWorld,
    panel: WorldGuiPanel,
) -> Option<WorldGuiLine> {
    let ray_origin = transform.transform_position(controller.aim_position?);
    let ray_direction = transform.transform_direction(controller.aim_direction?);
    if !ray_origin.is_finite()
        || !ray_direction.is_finite()
        || ray_direction.length_squared() <= f32::EPSILON
    {
        return None;
    }
    let direction = ray_direction.normalize();
    let distance = xr_menu_panel_ray_distance(panel, ray_origin, direction)
        .unwrap_or(XR_MENU_CONTROLLER_RAY_LENGTH_BLOCKS)
        .clamp(0.0, XR_MENU_CONTROLLER_RAY_LENGTH_BLOCKS);
    Some(WorldGuiLine::new(
        ray_origin,
        ray_origin + direction * distance,
        xr_menu_controller_ray_color(controller.hand, pointer_select_value),
    ))
}

pub fn xr_menu_controller_ray_color(hand: XrHand, pointer_select_value: f32) -> [f32; 4] {
    if pointer_select_value.is_finite() && pointer_select_value >= XR_MENU_POINTER_TRIGGER_PRESS {
        return XR_MENU_TRIGGER_RAY_COLOR;
    }
    match hand {
        XrHand::Left => XR_MENU_LEFT_RAY_COLOR,
        XrHand::Right => XR_MENU_RIGHT_RAY_COLOR,
    }
}

pub fn xr_menu_panel_ray_distance(
    panel: WorldGuiPanel,
    ray_origin: Vec3,
    ray_direction: Vec3,
) -> Option<f32> {
    if !ray_origin.is_finite()
        || !ray_direction.is_finite()
        || ray_direction.length_squared() <= f32::EPSILON
    {
        return None;
    }
    let direction = ray_direction.normalize();
    let normal = panel.right.cross(panel.up).normalize_or_zero();
    if normal.length_squared() <= f32::EPSILON {
        return None;
    }
    let denominator = direction.dot(normal);
    if denominator.abs() <= 1.0e-5 {
        return None;
    }
    let distance = (panel.center - ray_origin).dot(normal) / denominator;
    if !distance.is_finite() || distance < 0.0 {
        return None;
    }
    let hit = ray_origin + direction * distance;
    let local = hit - panel.center;
    let panel_x = local.dot(panel.right) + panel.width * 0.5;
    let panel_y = panel.height * 0.5 - local.dot(panel.up);
    if panel_x < 0.0 || panel_x > panel.width || panel_y < 0.0 || panel_y > panel.height {
        return None;
    }
    Some(distance)
}

pub fn xr_menu_panel_pointer_hit(
    panel: WorldGuiPanel,
    gui_scale: GuiScale,
    ray_origin: Vec3,
    ray_direction: Vec3,
) -> Option<(Point, f32)> {
    if !ray_origin.is_finite()
        || !ray_direction.is_finite()
        || ray_direction.length_squared() <= f32::EPSILON
    {
        return None;
    }
    let direction = ray_direction.normalize();
    let normal = panel.right.cross(panel.up).normalize_or_zero();
    if normal.length_squared() <= f32::EPSILON {
        return None;
    }
    let denominator = direction.dot(normal);
    if denominator.abs() <= 1.0e-5 {
        return None;
    }
    let distance = (panel.center - ray_origin).dot(normal) / denominator;
    if !distance.is_finite() || distance < 0.0 {
        return None;
    }
    let hit = ray_origin + direction * distance;
    let local = hit - panel.center;
    let panel_x = local.dot(panel.right) + panel.width * 0.5;
    let panel_y = panel.height * 0.5 - local.dot(panel.up);
    if panel_x < 0.0 || panel_x > panel.width || panel_y < 0.0 || panel_y > panel.height {
        return None;
    }
    Some((
        Point {
            x: panel_x / panel.width * gui_scale.width,
            y: panel_y / panel.height * gui_scale.height,
        },
        distance,
    ))
}

pub fn xr_menu_pointer_trigger_down(trigger: f32, was_down: bool) -> bool {
    xr_analog_button_down(trigger, was_down)
}

pub(crate) fn xr_menu_panel_height_blocks() -> f32 {
    XR_MENU_PANEL_WIDTH_BLOCKS * XR_MENU_PANEL_PIXELS[1] as f32 / XR_MENU_PANEL_PIXELS[0] as f32
}

pub(crate) fn xr_diagnostic_panel_height_blocks() -> f32 {
    XR_DIAGNOSTIC_PANEL_WIDTH_BLOCKS * XR_DIAGNOSTIC_PANEL_PIXELS[1] as f32
        / XR_DIAGNOSTIC_PANEL_PIXELS[0] as f32
}

pub(crate) fn xr_game_ui_panel_height_blocks() -> f32 {
    XR_GAME_UI_PANEL_WIDTH_BLOCKS * XR_MENU_PANEL_PIXELS[1] as f32 / XR_MENU_PANEL_PIXELS[0] as f32
}

impl McloneSceneHost {
    pub(crate) fn current_ui_render_state(&self) -> GameUiRenderState {
        let render_distance = self.active_world.local_startup.as_ref().map_or_else(
            || {
                self.active_world
                    .runtime
                    .as_ref()
                    .map_or(self.active_world.scene.render_distance, |runtime| {
                        runtime.render_distance()
                    })
            },
            |startup| startup.render_distance(self.active_world.scene.render_distance),
        );
        let block_palette = self.active_world.runtime.as_ref().map_or_else(
            || {
                self.active_world
                    .local_startup
                    .as_ref()
                    .as_ref()
                    .and_then(|startup| startup.mesh_catalog())
                    .map_or_else(Default::default, |catalog| {
                        debug_block_palette_overlay(
                            catalog,
                            self.active_world.interaction.selected_hotbar_slot(),
                        )
                    })
            },
            |runtime| {
                debug_block_palette_overlay(
                    &runtime.mesh_assets().catalog,
                    self.active_world.interaction.selected_hotbar_slot(),
                )
            },
        );
        GameUiRenderState {
            lobby_scenario_available: self
                .client_experience
                .profile()
                .lobby_scenario
                .is_supported(),
            world_catalog: self
                .client_experience
                .catalog()
                .ui_state_with_active_world(self.active_local_world_id()),
            asset_packs: self.client_experience.asset_packs().ui_state(),
            storage_profile: self.storage_profile_ui,
            render_distance: (render_distance as i32).clamp(1, MAX_XR_RENDER_DISTANCE as i32),
            min_render_distance: 1,
            max_render_distance: MAX_XR_RENDER_DISTANCE as i32,
            section_occlusion_culling: self.render_options.section_occlusion_culling,
            leaf_detail: game_leaf_detail(self.mesh_assets.catalog.leaf_detail()),
            grass_detail: game_grass_detail(self.render_options.grass_detail),
            force_fullbright: self.render_options.force_fullbright,
            player_collision_box_visible: self.player_collision_box_visible,
            first_person_player_visible: self.active_world.camera.first_person_player_visible(),
            crosshair_visible: None,
            frame_pipeline_overlay_visible: self.diagnostic_panel.frame_metrics_visible(),
            debug_diagnostics_visible: self.diagnostic_panel.debug_diagnostics_visible(),
            auxiliary_split_mode: None,
            local_play: None,
            player_model: self.active_world.player_model,
            movement_mode: game_movement_mode(self.active_world.camera.movement_mode()),
            collision_mode: Some(game_collision_mode(
                self.active_world.camera.collision_mode(),
            )),
            travel_assist_mode: Some(self.travel_assist_mode),
            turn_mode: Some(self.turn_policy.game_mode().into()),
            xr_turn_mode: Some(self.turn_policy.game_mode()),
            fly_speed_multiplier: self.active_world.camera.fly_speed_multiplier() as f32,
            min_fly_speed_multiplier: ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER as f32,
            max_fly_speed_multiplier: ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER as f32,
            movement_speed_multiplier: self.active_world.camera.movement_speed_multiplier() as f32,
            min_movement_speed_multiplier: ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER as f32,
            max_movement_speed_multiplier: ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER as f32,
            frame_pacing_mode: GameFramePacingMode::Vsync,
            fps_cap: self
                .display_refresh_hz
                .map(|hz| hz.round().clamp(1.0, 999.0) as u32)
                .unwrap_or(XR_UI_FPS_CAP),
            flat_presentation: None,
            server_cadence: None,
            touch_controls_mode: None,
            touch_settings: None,
            block_palette,
        }
    }

    pub(crate) fn refresh_debug_diagnostics_overlay(&mut self) {
        if !self.diagnostic_panel.debug_diagnostics_visible() {
            self.diagnostic_panel.clear_debug_overlay();
            return;
        }
        if self.diagnostic_panel.should_refresh_debug_overlay() {
            let overlay = self.debug_diagnostics_overlay();
            self.diagnostic_panel.set_debug_overlay(overlay);
        }
    }

    pub(crate) fn debug_diagnostics_overlay(&self) -> DebugOverlay {
        let snapshot = self.active_world.camera.snapshot();
        let runtime_stats = self
            .active_world
            .runtime
            .as_ref()
            .map(|runtime| runtime.stats());
        let render_distance = runtime_stats
            .map_or(self.active_world.scene.render_distance, |stats| {
                stats.render_distance
            });
        let tracking_radius = runtime_stats.map_or(0, |stats| stats.chunk_tracking_radius);
        let interest_center =
            runtime_stats.map_or(snapshot.chunk_pos, |stats| stats.interest_center);
        let host = runtime_stats
            .map(|stats| stats.host_mode.label().to_ascii_uppercase())
            .unwrap_or_else(|| "STARTUP".to_owned());
        let runner = runtime_stats
            .and_then(|stats| stats.server_runner_kind)
            .map(|kind| kind.label().to_ascii_uppercase())
            .unwrap_or_else(|| "REMOTE".to_owned());
        let actor_indices = self.active_world.render_stats.drawn_actor_index_count;
        #[allow(unused_mut)]
        let mut lines = vec![
            format!(
                "GEN {}",
                self.active_world
                    .scene
                    .world_generation_profile
                    .label()
                    .to_ascii_uppercase()
            ),
            format!(
                "POS {:.1} {:.1} {:.1}",
                snapshot.eye.x, snapshot.eye.y, snapshot.eye.z
            ),
            format!(
                "CHUNK {} {} SPEED {:.1}",
                snapshot.chunk_pos.x,
                snapshot.chunk_pos.z,
                self.active_world.camera.speed_blocks_per_second()
            ),
            format!(
                "MODE {}/{} GROUND {}",
                self.active_world.camera.movement_mode().label(),
                self.active_world.camera.collision_mode().label(),
                if self.active_world.camera.on_ground() {
                    "Y"
                } else {
                    "N"
                }
            ),
            format!(
                "VIEW R{} T{} C{} {}",
                render_distance, tracking_radius, interest_center.x, interest_center.z
            ),
            format!(
                "HOST {} {} SQ{} UQ{}",
                host,
                runner,
                runtime_stats.map_or(0, |stats| stats.server_command_queue_depth),
                runtime_stats.map_or(0, |stats| stats.server_update_queue_depth)
            ),
            format!(
                "CHUNKS L{} V{} P{}",
                runtime_stats.map_or(0, |stats| stats.loaded_chunks),
                runtime_stats.map_or(0, |stats| stats.client_visible_chunks),
                runtime_stats.map_or(0, |stats| stats.pending_jobs)
            ),
            format!(
                "DRAW S {}/{} F {}/{}",
                self.active_world.render_stats.drawn_section_count,
                self.active_world.render_stats.section_count,
                self.active_world.render_stats.drawn_face_count,
                self.active_world.render_stats.face_count
            ),
            format!(
                "ACTOR R {}/{} I{}",
                self.active_world.render_stats.drawn_actor_count,
                self.active_world.render_stats.actor_count,
                actor_indices
            ),
            format!(
                "MESH R{} U{} D{} SQ{} CQ{} X{}",
                self.active_world.render_stats.last_rebuilt_section_count,
                self.active_world.render_stats.last_uploaded_section_count,
                self.active_world.render_stats.last_deferred_section_count,
                self.active_world
                    .render_stats
                    .last_submitted_compile_section_count,
                self.active_world
                    .render_stats
                    .last_completed_compile_section_count,
                self.active_world
                    .render_stats
                    .last_stale_compile_section_count
            ),
            format!(
                "PENDING R{} C{}",
                runtime_stats.map_or(0, |stats| stats.pending_render_chunks),
                self.active_world.render_stats.last_pending_compile_jobs
            ),
            format!(
                "OPTIONS OCC {} FULL {} {}",
                if self.render_options.section_occlusion_culling {
                    "Y"
                } else {
                    "N"
                },
                if self.render_options.force_fullbright {
                    "Y"
                } else {
                    "N"
                },
                self.render_options.color_profile.label()
            ),
        ];
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(preview) = self.embedded_world_preview_snapshot() {
            let standby = self.warm_world_standby_snapshot();
            lines.insert(
                0,
                format!(
                    "PREVIEW {:?} W{} {} C({},{})..({},{}) S{:.4}",
                    preview.phase,
                    preview.source_world.get(),
                    preview
                        .source_host_mode
                        .map_or("starting", |mode| mode.label()),
                    preview.region.min_chunk().x,
                    preview.region.min_chunk().z,
                    preview.region.max_chunk().x,
                    preview.region.max_chunk().z,
                    preview.placement.uniform_scale(),
                ),
            );
            lines.insert(
                1,
                format!(
                    "B POLL {:.3} COMPILE {}/{} UP {} Q{}/{} CAD {}/{}/{}",
                    preview.preparation.last_runtime_poll_ms,
                    preview.preparation.last_submitted_compile_section_count,
                    preview.preparation.last_accepted_compile_result_count,
                    preview.preparation.last_uploaded_section_count,
                    preview.preparation.queued_upload_lifecycle_items,
                    preview.preparation.pending_compile_jobs,
                    standby
                        .as_ref()
                        .map_or(0, |standby| standby.standby_cadence.host_rate_hz),
                    standby
                        .as_ref()
                        .map_or(0, |standby| standby.standby_cadence.gameplay_rate_hz),
                    standby
                        .as_ref()
                        .map_or(0, |standby| standby.standby_cadence.physics_rate_hz),
                ),
            );
            lines.insert(
                2,
                format!(
                    "B DRAW {}/{} T{}/{}/{} CULL {:.3} DRAW {:.3} CPU{} GPU{}",
                    preview.render.last_drawn_section_count,
                    preview.render.last_drawn_index_count,
                    preview.render.last_translucent_order.active_section_count,
                    preview.render.last_translucent_order.preview_section_count,
                    preview.render.last_translucent_order.source_switch_count,
                    preview.render.last_cull_ms,
                    preview.render.last_draw_ms,
                    standby
                        .as_ref()
                        .map_or(0, |standby| standby.startup_seed_owned_bytes),
                    standby
                        .as_ref()
                        .map_or(0, |standby| standby.estimated_gpu_terrain_bytes),
                ),
            );
            if let Some(mutation) = preview.last_mutation {
                lines.insert(
                    3,
                    format!(
                        "B MUT {:?} ({},{},{}) C{}/{} U{}",
                        mutation.phase,
                        mutation.block.x,
                        mutation.block.y,
                        mutation.block.z,
                        mutation.submitted_compile_section_count,
                        mutation.accepted_compile_result_count,
                        mutation.uploaded_section_count,
                    ),
                );
            }
            if let Some(warning) = preview.boundary_warning {
                lines.insert(1, format!("PREVIEW EDGE {warning}"));
            }
            if let Some(failure) = preview.failure {
                lines.insert(1, format!("PREVIEW FAIL {failure}"));
            }
        } else if let Some(gate) = self.world_gate_snapshot() {
            lines.insert(
                0,
                format!(
                    "GATE {} W{}>W{} X{}",
                    gate.availability.label().to_ascii_uppercase(),
                    gate.active_world_id.get(),
                    gate.destination_world_id.get(),
                    gate.crossing_count,
                ),
            );
        } else if let Some(standby) = self.warm_world_standby_snapshot() {
            lines.insert(
                0,
                format!("GATE {}", standby.phase.label().to_ascii_uppercase()),
            );
            if let Some(failure) = standby.failure {
                lines.insert(1, format!("GATE FAIL {failure}"));
            }
        }
        let topology = self
            .active_world
            .runtime
            .as_ref()
            .map_or(HorizontalTopology::UNBOUNDED, |runtime| {
                runtime.client().topology()
            });
        let mut lens_lines = self.worldgen_lens.inspection_lines(
            self.active_world.scene.seed,
            self.active_world.scene.world_generation_profile,
            topology,
            Vec3::new(
                snapshot.eye.x as f32,
                snapshot.eye.y as f32,
                snapshot.eye.z as f32,
            ),
        );
        lens_lines.append(&mut lines);
        lines = lens_lines;
        DebugOverlay::new("XR DEBUG", lines)
    }

    pub(crate) fn client_experience_settings_state(&self) -> ClientExperienceSettingsState {
        ClientExperienceSettingsState::from(self.current_ui_render_state())
    }

    pub(crate) fn session_projection(&self) -> ClientSessionStatusProjection {
        client_session_status_projection(
            self.active_world
                .runtime
                .as_ref()
                .and_then(|runtime| runtime.session_status())
                .or_else(|| self.session.status()),
            self.status_overlay.clone(),
            self.startup_progress_overlay(),
        )
    }

    pub(crate) fn startup_progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.active_world
            .local_startup
            .as_ref()
            .and_then(SceneLocalStartup::progress_overlay)
            .or_else(|| {
                self.active_world
                    .external_runtime_startup_pending
                    .then(|| {
                        self.active_world
                            .runtime
                            .as_ref()
                            .and_then(|runtime| runtime.startup_progress_overlay())
                    })
                    .flatten()
            })
    }

    pub(crate) fn active_remote_addr(&self) -> Option<String> {
        match self.session.state() {
            GameSessionState::Active {
                session: ActiveSessionDescriptor::Remote { endpoint },
            } => Some(endpoint.address.clone()),
            GameSessionState::NoSession
            | GameSessionState::Starting { .. }
            | GameSessionState::Active {
                session: ActiveSessionDescriptor::LocalWorld { .. },
            }
            | GameSessionState::Failed { .. } => None,
        }
    }

    pub(crate) fn clear_inactive_session_status(&mut self) {
        if client_session_should_clear_inactive_status(self.session.state()) {
            self.status_overlay = StatusOverlay::hidden();
        }
    }

    pub(crate) fn apply_menu_toggle_input(&mut self, actions: &PlayerActionFrame) {
        let toggle_down = xr_menu_toggle_pressed(actions);
        if toggle_down && !self.menu_toggle_down {
            if self.active_world.local_startup.is_some() {
                if !self.ui.is_active() {
                    self.ui.open_pause();
                    self.menu_panel_anchor = XrUiPanelAnchor::Head;
                    self.menu_panel_recenter_pending = true;
                }
                self.menu_toggle_down = toggle_down;
                return;
            }
            if self.ui.is_active() {
                self.ui.close();
                self.ui.clear_input();
                self.menu_pointer_down = false;
                self.menu_panel_pose = None;
                self.menu_panel_recenter_pending = false;
                log::info!("XR menu closed");
            } else {
                self.ui.open_pause();
                self.menu_panel_anchor = XrUiPanelAnchor::Head;
                self.menu_panel_recenter_pending = true;
                log::info!("XR menu opened");
            }
        }
        self.menu_toggle_down = toggle_down;
    }

    pub(crate) fn apply_game_ui_toggle_input(&mut self, actions: &PlayerActionFrame) {
        let toggle_down = xr_game_ui_toggle_pressed(actions);
        if toggle_down && !self.game_ui_toggle_down {
            if self.active_world.local_startup.is_some() || self.active_world.runtime.is_none() {
                self.game_ui_toggle_down = toggle_down;
                return;
            }
            if self.ui.screen() == Some(GameScreen::BlockPalette)
                && self.menu_panel_anchor == XrUiPanelAnchor::LeftHand
            {
                self.ui.close();
                self.ui.clear_input();
                self.menu_pointer_down = false;
                self.menu_panel_pose = None;
                self.menu_panel_recenter_pending = false;
                log::info!("XR game UI closed");
            } else {
                self.ui.apply_action(GameUiAction::OpenBlockPalette);
                self.menu_panel_anchor = XrUiPanelAnchor::LeftHand;
                self.menu_pointer_down = false;
                self.menu_panel_pose = None;
                self.menu_panel_recenter_pending = true;
                log::info!("XR game UI opened");
            }
        }
        self.game_ui_toggle_down = toggle_down;
    }

    pub(crate) fn update_menu_panel_pose(&mut self, render_views: [ChunkRenderView; 2]) {
        if !self.ui.is_active() {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            self.menu_panel_pose = None;
            self.menu_panel_anchor = XrUiPanelAnchor::Head;
            self.menu_panel_recenter_pending = false;
            return;
        }
        match self.menu_panel_anchor {
            XrUiPanelAnchor::Head => {
                if self.menu_panel_pose.is_some() && !self.menu_panel_recenter_pending {
                    return;
                }
                self.menu_panel_pose = Some(xr_menu_panel_from_render_views(render_views));
                self.menu_panel_recenter_pending = false;
            }
            XrUiPanelAnchor::LeftHand => {
                let hand_panel = self.tracking_origin.and_then(|origin| {
                    let transform = XrStageToWorld::from_tracking_origin(
                        origin,
                        self.active_world.camera.snapshot(),
                    )
                    .ok()?;
                    xr_game_ui_panel_from_controllers(
                        &self.latest_xr_input.tracked,
                        transform,
                        render_views,
                    )
                });
                if let Some(panel) = hand_panel {
                    self.menu_panel_pose = Some(panel);
                    self.menu_panel_recenter_pending = false;
                } else if self.menu_panel_pose.is_none() || self.menu_panel_recenter_pending {
                    self.menu_panel_pose = Some(xr_menu_panel_from_render_views(render_views));
                    self.menu_panel_recenter_pending = false;
                }
            }
        }
    }

    pub(crate) fn apply_menu_pointer_input(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        if !self.ui.is_active() {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            return Ok(false);
        }
        let Some(panel) = self.menu_panel_pose else {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            return Ok(false);
        };
        let Some(origin) = self.tracking_origin else {
            return Ok(false);
        };
        let transform =
            XrStageToWorld::from_tracking_origin(origin, self.active_world.camera.snapshot())?;
        let gui_scale = GuiScale::from_pixels(XR_MENU_PANEL_PIXELS[0], XR_MENU_PANEL_PIXELS[1]);
        self.ui.set_scale(gui_scale);
        let hit = xr_menu_pointer_hit_from_controllers(
            &self.latest_xr_input,
            transform,
            panel,
            gui_scale,
        );
        let Some(hit) = hit else {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            return Ok(false);
        };
        let trigger_down = xr_menu_pointer_trigger_down(hit.trigger, self.menu_pointer_down);
        let action = if trigger_down && !self.menu_pointer_down {
            self.ui.pointer_down(hit.point);
            None
        } else if !trigger_down && self.menu_pointer_down {
            let (_handled, action) = self.ui.pointer_up(hit.point);
            action
        } else {
            let (_handled, action) = self.ui.pointer_move(hit.point);
            action
        };
        self.menu_pointer_down = trigger_down;
        if let Some(action) = action {
            return self.apply_xr_ui_action(action, device, queue);
        }
        Ok(false)
    }

    pub(crate) fn sync_carried_item(&mut self) -> Result<bool> {
        let Some(command) = self.active_world.interaction.ensure_has_sent_carried_item() else {
            return Ok(false);
        };
        let Some(runtime) = &mut self.active_world.runtime else {
            return Ok(false);
        };
        runtime
            .send_gameplay_command(command)
            .map(|_| true)
            .context("failed to sync XR carried item to server")
    }

    pub(crate) fn sync_player_appearance(&mut self) -> Result<bool> {
        let Some(runtime) = &mut self.active_world.runtime else {
            return Ok(false);
        };
        runtime
            .send_gameplay_command(set_player_appearance_command_for_ui_model(
                self.active_world.local_participant.player_model,
            ))
            .map(|_| true)
            .context("failed to sync XR player appearance to server")
    }

    pub(crate) fn assign_debug_hotbar_slot(
        &mut self,
        slot: u8,
        block_state: BlockStateId,
    ) -> Result<bool> {
        let Some(command) = self
            .active_world
            .interaction
            .set_debug_hotbar_slot(slot, Some(block_state))
        else {
            return Ok(false);
        };
        let Some(runtime) = &mut self.active_world.runtime else {
            return Ok(false);
        };
        runtime
            .send_gameplay_command(command)
            .map(|_| true)
            .context("failed to assign XR debug hotbar slot")
    }

    pub(crate) fn assign_debug_hotbar_actor(
        &mut self,
        slot: u8,
        actor: DebugActorTool,
    ) -> Result<bool> {
        let actor = match actor {
            DebugActorTool::Chicken => DebugActorKind::Chicken,
            DebugActorTool::Mannequin => DebugActorKind::Mannequin,
        };
        let Some(command) = self
            .active_world
            .interaction
            .set_debug_hotbar_item(slot, Some(DebugHotbarItem::SpawnActor(actor)))
        else {
            return Ok(false);
        };
        let Some(runtime) = &mut self.active_world.runtime else {
            return Ok(false);
        };
        runtime
            .send_gameplay_command(command)
            .map(|_| true)
            .context("failed to assign XR debug actor hotbar slot")
    }

    pub(crate) fn apply_xr_gameplay_interaction_edges(
        &mut self,
        mut edges: XrGameplayInteractionEdges,
    ) -> Result<()> {
        if !edges.any() {
            return Ok(());
        }
        if edges.use_item
            && let Some(origin) = self.tracking_origin
        {
            let transform =
                XrStageToWorld::from_tracking_origin(origin, self.active_world.camera.snapshot())?;
            if let Some((ray_origin, ray_direction)) = self.current_xr_interaction_ray(transform)
                && self.request_embedded_world_activation(
                    vec3d_from_glam(ray_origin),
                    vec3d_from_glam(ray_direction),
                )
            {
                edges.use_item = false;
            }
        }
        if !edges.any() {
            return Ok(());
        }
        let behavior = self.active_world.scene.world_behavior_profile;
        let denied_attack = edges.attack && !behavior.allows_player_break();
        let denied_use = edges.use_item && !behavior.allows_player_place();
        edges.attack &= behavior.allows_player_break();
        edges.use_item &= behavior.allows_player_place();
        if denied_attack || denied_use {
            log::info!(
                "XR gameplay interaction denied by active world behavior profile {:?}",
                behavior
            );
        }
        if !edges.any() {
            return Ok(());
        }
        self.sync_carried_item()?;
        let Some(target) = self.current_xr_block_interaction_target() else {
            return Ok(());
        };
        if edges.attack {
            self.send_xr_gameplay_interaction_command(
                XrGameplayInteractionAction::Attack,
                &target,
            )?;
        }
        if edges.use_item {
            self.send_xr_gameplay_interaction_command(XrGameplayInteractionAction::Use, &target)?;
        }
        Ok(())
    }

    pub(crate) fn send_xr_gameplay_interaction_command(
        &mut self,
        action: XrGameplayInteractionAction,
        target: &BlockInteractionTarget,
    ) -> Result<()> {
        let command = match action {
            XrGameplayInteractionAction::Attack => self
                .active_world
                .interaction
                .debug_instant_break_command(target.hit),
            XrGameplayInteractionAction::Use => self
                .active_world
                .interaction
                .use_item_on_command(target.hit),
        };
        let Some(command) = command else {
            return Ok(());
        };
        let Some(runtime) = &mut self.active_world.runtime else {
            return Ok(());
        };
        runtime
            .send_gameplay_command(command)
            .context("failed to send XR gameplay interaction command")?;
        log::info!(
            "XR gameplay interaction {:?} submitted at ({}, {}, {}) face={:?}",
            action,
            target.hit.block_pos.x,
            target.hit.block_pos.y,
            target.hit.block_pos.z,
            target.hit.direction
        );
        Ok(())
    }

    pub(crate) fn xr_menu_controller_ray_lines(
        &self,
        panel: WorldGuiPanel,
    ) -> Result<Vec<WorldGuiLine>> {
        let Some(origin) = self.tracking_origin else {
            return Ok(Vec::new());
        };
        let transform =
            XrStageToWorld::from_tracking_origin(origin, self.active_world.camera.snapshot())?;
        Ok(xr_menu_controller_ray_lines_from_controllers(
            &self.latest_xr_input,
            transform,
            panel,
        ))
    }

    pub(crate) fn xr_gameplay_controller_ray_line(&self) -> Result<Option<WorldGuiLine>> {
        if self.ui.is_active() {
            return Ok(None);
        }
        let Some(origin) = self.tracking_origin else {
            return Ok(None);
        };
        let transform =
            XrStageToWorld::from_tracking_origin(origin, self.active_world.camera.snapshot())?;
        let Some((ray_origin, ray_direction)) = xr_controller_interaction_ray_from_controllers(
            &self.latest_xr_input.tracked,
            transform,
        ) else {
            return Ok(None);
        };
        let hit_distance = self.active_world.runtime.as_ref().and_then(|runtime| {
            self.active_world
                .interaction
                .target_block(
                    runtime.client(),
                    vec3d_from_glam(ray_origin),
                    vec3d_from_glam(ray_direction),
                )
                .map(|target| {
                    let hit = glam_vec3_from_vec3d(target.hit.location);
                    (hit - ray_origin).dot(ray_direction.normalize_or_zero())
                })
        });
        Ok(xr_gameplay_controller_ray_line_from_controllers(
            &self.latest_xr_input,
            transform,
            hit_distance,
            self.active_world.interaction.pick_range() as f32,
        ))
    }

    pub(crate) fn current_xr_block_interaction_target(&self) -> Option<BlockInteractionTarget> {
        if self.ui.is_active() {
            return None;
        }
        let runtime = self.active_world.runtime.as_ref()?;
        let origin = self.tracking_origin?;
        let transform =
            XrStageToWorld::from_tracking_origin(origin, self.active_world.camera.snapshot())
                .ok()?;
        let (ray_origin, ray_direction) = self.current_xr_interaction_ray(transform)?;
        self.active_world.interaction.target_block(
            runtime.client(),
            vec3d_from_glam(ray_origin),
            vec3d_from_glam(ray_direction),
        )
    }

    pub(crate) fn current_xr_selection_outline(&self) -> Option<SelectionOutline> {
        self.current_xr_block_interaction_target()
            .map(|target| SelectionOutline::new(target.outline_boxes))
    }

    fn current_xr_interaction_ray(&self, transform: XrStageToWorld) -> Option<(Vec3, Vec3)> {
        xr_interaction_ray_with_head_fallback(
            &self.latest_xr_input,
            transform,
            self.latest_xr_head_gaze_stage,
        )
    }
}
