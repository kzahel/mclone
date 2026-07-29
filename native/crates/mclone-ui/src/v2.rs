use crate::{
    AssetPackUiRow, AssetPackUiRowStatus, AssetPacksUiState, BLOCK_PALETTE_ENTRY_CAPACITY,
    BLOCK_PALETTE_PADDING, BlockPaletteEntry, BlockPaletteOverlay, Button, Checkbox, Color,
    CycleButton, DebugPaletteItem, FlatHud, Font, GameDeathCause, GameFlatPresentationState,
    GameHelpParent, GameOptionsCategory, GameOptionsParent, GameScenarioId, GameScreen,
    GameStorageAction, GameTurnMode, GameUiAction, GameUiRenderState, GuiDrawList, GuiKey,
    GuiScale, GuiTextureUv, HOTBAR_SLOT_COUNT_USIZE, Interaction, LoadingProgressOverlay, Point,
    Rect, Slider, WidgetId, WorldCatalogUiEntry, WorldCatalogUiState, WorldCatalogUiWorldId,
    block_palette_panel_rect, block_palette_slot_rect, centered_panel, fly_speed_from_slider_value,
    fly_speed_label, fly_speed_slider_value, fog_classic_start_from_slider_value,
    fog_classic_start_label, fog_classic_start_slider_value, fog_color_component_from_slider_value,
    fog_color_component_label, fog_color_component_slider_value, fog_ground_base_from_slider_value,
    fog_ground_base_label, fog_ground_base_slider_value, fog_ground_falloff_from_slider_value,
    fog_ground_falloff_label, fog_ground_falloff_slider_value, fog_guard_start_from_slider_value,
    fog_guard_start_label, fog_guard_start_slider_value, fog_max_opacity_from_slider_value,
    fog_max_opacity_label, fog_max_opacity_slider_value, fog_visibility_from_slider_value,
    fog_visibility_label, fog_visibility_slider_value, movement_speed_from_slider_value,
    movement_speed_label, movement_speed_slider_value, next_touch_controls_mode,
    render_block_palette_tooltip, render_distance_from_slider_value, render_distance_label,
    render_distance_slider_value, render_flat_hud_debug_layer,
    render_flat_hud_frame_pipeline_layer, render_flat_hud_hotbar_layer,
    render_flat_hud_prompt_layer, render_flat_hud_retained_layer, render_flat_hud_status_layer,
    render_flat_hud_transient_layers, render_loading_progress_overlay,
    render_loading_progress_panel_at, render_palette_slot_contents, render_touch_panel,
    touch_controls_mode_label, touch_look_from_slider_value, touch_look_label,
    touch_look_slider_value,
};
use mclone_input::{
    FLAT_HOTBAR_SLOT_COUNT, ShortcutHelpGroup, ShortcutHelpRow,
    default_keyboard_mouse_shortcut_rows, flat_runtime_shortcut_rows,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiScreenId {
    Title,
    PreparingLobby,
    WorldList,
    WorldCreate,
    WorldDeleteConfirm {
        id: WorldCatalogUiWorldId,
    },
    NewWorld,
    JoinRemote,
    Pause,
    Death {
        cause: GameDeathCause,
    },
    BlockPalette,
    Options {
        parent: GameOptionsParent,
    },
    OptionsCategory {
        parent: GameOptionsParent,
        category: GameOptionsCategory,
    },
    ServerSettings {
        parent: GameOptionsParent,
    },
    AssetPacks {
        parent: GameOptionsParent,
    },
    StorageConfirm {
        parent: GameOptionsParent,
        action: GameStorageAction,
    },
    Help {
        parent: GameHelpParent,
    },
}

impl UiScreenId {
    pub fn from_game_screen(screen: Option<GameScreen>) -> Option<Self> {
        match screen {
            Some(GameScreen::Title) => Some(Self::Title),
            Some(GameScreen::PreparingLobby) => Some(Self::PreparingLobby),
            Some(GameScreen::WorldList) => Some(Self::WorldList),
            Some(GameScreen::WorldCreate) => Some(Self::WorldCreate),
            Some(GameScreen::WorldDeleteConfirm { id }) => Some(Self::WorldDeleteConfirm { id }),
            Some(GameScreen::NewWorld) => Some(Self::NewWorld),
            Some(GameScreen::JoinRemote) => Some(Self::JoinRemote),
            Some(GameScreen::Pause) => Some(Self::Pause),
            Some(GameScreen::Death { cause }) => Some(Self::Death { cause }),
            Some(GameScreen::BlockPalette) => Some(Self::BlockPalette),
            Some(GameScreen::Options { parent }) => Some(Self::Options { parent }),
            Some(GameScreen::OptionsCategory { parent, category }) => {
                Some(Self::OptionsCategory { parent, category })
            }
            Some(GameScreen::ServerSettings { parent }) => Some(Self::ServerSettings { parent }),
            Some(GameScreen::AssetPacks { parent }) => Some(Self::AssetPacks { parent }),
            Some(GameScreen::StorageConfirm { parent, action }) => {
                Some(Self::StorageConfirm { parent, action })
            }
            Some(GameScreen::Help { parent }) => Some(Self::Help { parent }),
            None => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiFrameState {
    pub screen: UiScreenId,
    pub scale: GuiScale,
    pub render_state: GameUiRenderState,
    pub revision: u64,
}

impl UiFrameState {
    pub const fn new(
        screen: UiScreenId,
        scale: GuiScale,
        render_state: GameUiRenderState,
        revision: u64,
    ) -> Self {
        Self {
            screen,
            scale,
            render_state,
            revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct UiPanelRevision {
    pub content: u64,
    pub interaction: u64,
}

impl UiPanelRevision {
    pub const fn new(content: u64, interaction: u64) -> Self {
        Self {
            content,
            interaction,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct UiWidgetId(pub u64);

impl UiWidgetId {
    const fn legacy_widget_id(self) -> WidgetId {
        WidgetId(10_000 + self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiWidgetKind {
    Button,
    Checkbox {
        checked: bool,
    },
    Cycle,
    WorldRow {
        selected: bool,
        locked: bool,
        compatible: bool,
    },
    AssetPackRow {
        checked: bool,
        locked: bool,
        status: AssetPackUiRowStatus,
    },
    PaletteSlot {
        icon: Option<GuiTextureUv>,
    },
    Slider {
        value: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiWidget {
    pub id: UiWidgetId,
    pub kind: UiWidgetKind,
    pub rect: Rect,
    pub label: String,
    pub enabled: bool,
    value: Option<String>,
    action: Option<UiWidgetAction>,
}

impl UiWidget {
    pub fn button(id: UiWidgetId, rect: Rect, label: impl Into<String>) -> Self {
        Self {
            id,
            kind: UiWidgetKind::Button,
            rect,
            label: label.into(),
            enabled: true,
            action: None,
            value: None,
        }
    }

    pub fn checkbox(id: UiWidgetId, rect: Rect, label: impl Into<String>, checked: bool) -> Self {
        Self {
            id,
            kind: UiWidgetKind::Checkbox { checked },
            rect,
            label: label.into(),
            enabled: true,
            value: None,
            action: None,
        }
    }

    pub fn cycle(
        id: UiWidgetId,
        rect: Rect,
        label: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            id,
            kind: UiWidgetKind::Cycle,
            rect,
            label: label.into(),
            enabled: true,
            value: Some(value.into()),
            action: None,
        }
    }

    pub fn slider(id: UiWidgetId, rect: Rect, label: impl Into<String>, value: f32) -> Self {
        Self {
            id,
            kind: UiWidgetKind::Slider {
                value: value.clamp(0.0, 1.0),
            },
            rect,
            label: label.into(),
            enabled: true,
            value: None,
            action: None,
        }
    }

    fn palette_slot(
        id: UiWidgetId,
        rect: Rect,
        label: impl Into<String>,
        icon: Option<GuiTextureUv>,
    ) -> Self {
        Self {
            id,
            kind: UiWidgetKind::PaletteSlot { icon },
            rect,
            label: label.into(),
            enabled: true,
            action: None,
            value: None,
        }
    }

    fn world_row(
        id: UiWidgetId,
        rect: Rect,
        label: impl Into<String>,
        value: impl Into<String>,
        selected: bool,
        locked: bool,
        compatible: bool,
    ) -> Self {
        Self {
            id,
            kind: UiWidgetKind::WorldRow {
                selected,
                locked,
                compatible,
            },
            rect,
            label: label.into(),
            enabled: true,
            action: None,
            value: Some(value.into()),
        }
    }

    fn asset_pack_row(id: UiWidgetId, rect: Rect, row: AssetPackUiRow) -> Self {
        let mut value = format!("{} / {}", row.origin.label(), row.status.label());
        if !row.detail.is_empty() {
            value.push_str(" / ");
            value.push_str(row.detail.as_str());
        }
        Self {
            id,
            kind: UiWidgetKind::AssetPackRow {
                checked: row.enabled,
                locked: !row.disableable,
                status: row.status,
            },
            rect,
            label: format!("{}  [{}]", row.display_name.as_str(), row.pack_id.as_str()),
            enabled: row.available && row.disableable,
            value: Some(value),
            action: Some(UiWidgetAction::Static(GameUiAction::ToggleAssetPack(
                row.ui_id,
            ))),
        }
    }

    pub fn action(mut self, action: GameUiAction) -> Self {
        self.action = Some(UiWidgetAction::Static(action));
        self
    }

    fn slider_action(mut self, action: UiSliderAction) -> Self {
        self.action = Some(UiWidgetAction::Slider(action));
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn contains(&self, point: Point) -> bool {
        self.enabled && self.rect.contains(point)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum UiWidgetAction {
    Static(GameUiAction),
    Slider(UiSliderAction),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiSliderAction {
    RenderDistance,
    FogVisibility,
    FogClassicStart,
    FogGuardStart,
    FogGroundBase,
    FogGroundFalloff,
    FogMaxOpacity,
    FogColorRed,
    FogColorGreen,
    FogColorBlue,
    FlySpeed,
    MovementSpeed,
    TouchLook,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct UiScrollRegion {
    clip: Rect,
    first_widget: usize,
    widget_count: usize,
    content_height: f32,
    offset: f32,
}

impl UiScrollRegion {
    fn contains_widget(self, index: usize) -> bool {
        (self.first_widget..self.first_widget + self.widget_count).contains(&index)
    }

    fn max_offset(self) -> f32 {
        (self.content_height - self.clip.height).max(0.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiLayout {
    pub screen: Option<UiScreenId>,
    pub revision: u64,
    widgets: Vec<UiWidget>,
    help_rows: Vec<UiHelpRow>,
    scroll_region: Option<UiScrollRegion>,
}

impl UiLayout {
    pub fn new(screen: Option<UiScreenId>, revision: u64) -> Self {
        Self {
            screen,
            revision,
            widgets: Vec::new(),
            help_rows: Vec::new(),
            scroll_region: None,
        }
    }

    pub fn push(&mut self, widget: UiWidget) {
        self.widgets.push(widget);
    }

    pub fn widgets(&self) -> &[UiWidget] {
        &self.widgets
    }

    pub fn widget(&self, id: UiWidgetId) -> Option<&UiWidget> {
        self.widgets.iter().find(|widget| widget.id == id)
    }

    pub fn hit_test(&self, point: Point) -> Option<UiWidgetId> {
        self.widgets
            .iter()
            .enumerate()
            .rev()
            .find(|(index, widget)| {
                self.scroll_region.is_none_or(|region| {
                    !region.contains_widget(*index) || region.clip.contains(point)
                }) && widget.contains(point)
            })
            .map(|(_, widget)| widget.id)
    }

    fn set_scroll_region(&mut self, region: UiScrollRegion) {
        self.scroll_region = Some(region);
    }

    fn set_help_rows(&mut self, rows: Vec<UiHelpRow>) {
        self.help_rows = rows;
    }

    fn help_rows(&self) -> &[UiHelpRow] {
        &self.help_rows
    }
}

#[derive(Clone, Debug, PartialEq)]
struct UiHelpRow {
    x: f32,
    y: f32,
    control_width: f32,
    kind: UiHelpRowKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum UiHelpRowKind {
    Group(ShortcutHelpGroup),
    Shortcut(ShortcutHelpRow),
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiDebugWidget {
    pub id: UiWidgetId,
    pub kind: UiWidgetKind,
    pub rect: Rect,
    pub label: String,
    pub value: Option<String>,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiDebugSnapshot {
    pub screen: Option<UiScreenId>,
    pub scale: GuiScale,
    pub frame_revision: u64,
    pub layout_revision: u64,
    pub pointer: Option<Point>,
    pub hovered: Option<UiWidgetId>,
    pub captured: Option<UiWidgetId>,
    pub focused: Option<UiWidgetId>,
    pub scroll_offset: f32,
    pub scroll_max: f32,
    pub scroll_clip: Option<Rect>,
    pub widgets: Vec<UiDebugWidget>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuiNavigation {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    PreviousPage,
    NextPage,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BlockPaletteGridState {
    scale: GuiScale,
    selected_hotbar_slot: u8,
    entries: [Option<BlockPaletteEntry>; BLOCK_PALETTE_ENTRY_CAPACITY],
}

impl BlockPaletteGridState {
    fn from_overlay(scale: GuiScale, overlay: BlockPaletteOverlay) -> Option<Self> {
        overlay.visible.then_some(Self {
            scale,
            selected_hotbar_slot: overlay.selected_hotbar_slot,
            entries: overlay.entries,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CachedBlockPaletteGridLayer {
    state: BlockPaletteGridState,
    draw: GuiDrawList,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiSurface {
    screen: Option<UiScreenId>,
    scale: GuiScale,
    frame_revision: u64,
    interaction_revision: u64,
    layout_revision: u64,
    layout_dirty: bool,
    layout: UiLayout,
    render_state: GameUiRenderState,
    new_world_seed: i64,
    join_remote_addr: String,
    pointer: Option<Point>,
    hovered: Option<UiWidgetId>,
    captured: Option<UiWidgetId>,
    focused: Option<UiWidgetId>,
    scroll_offset: f32,
    font: Font,
    debug_overlay: bool,
    block_palette_grid: Option<CachedBlockPaletteGridLayer>,
    block_palette_grid_cache: UiDrawCacheStats,
}

impl Default for UiSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl UiSurface {
    pub fn new() -> Self {
        let scale = GuiScale::from_pixels(1280, 900);
        Self {
            screen: None,
            scale,
            frame_revision: 0,
            interaction_revision: 0,
            layout_revision: 0,
            layout_dirty: true,
            layout: UiLayout::new(None, 0),
            render_state: GameUiRenderState::default(),
            new_world_seed: 0,
            join_remote_addr: crate::DEFAULT_JOIN_REMOTE_ADDR.to_owned(),
            pointer: None,
            hovered: None,
            captured: None,
            focused: None,
            scroll_offset: 0.0,
            font: Font::default(),
            debug_overlay: false,
            block_palette_grid: None,
            block_palette_grid_cache: UiDrawCacheStats::default(),
        }
    }

    pub fn screen(&self) -> Option<UiScreenId> {
        self.screen
    }

    pub fn is_active(&self) -> bool {
        self.screen.is_some()
    }

    pub fn set_screen(&mut self, screen: Option<UiScreenId>) {
        if self.screen == screen {
            return;
        }
        self.screen = screen;
        self.frame_revision = self.frame_revision.wrapping_add(1);
        self.layout_dirty = true;
        self.scroll_offset = 0.0;
        self.set_interaction_state(None, None, None);
        self.set_focus(None);
    }

    pub fn set_scale(&mut self, scale: GuiScale) {
        if self.scale == scale {
            return;
        }
        self.scale = scale;
        self.frame_revision = self.frame_revision.wrapping_add(1);
        self.layout_dirty = true;
        let pointer = self.pointer.map(|point| Point {
            x: point.x.clamp(0.0, scale.width),
            y: point.y.clamp(0.0, scale.height),
        });
        self.set_interaction_state(pointer, self.hovered, self.captured);
    }

    pub fn set_render_state(&mut self, render_state: GameUiRenderState) {
        if self.render_state == render_state {
            return;
        }
        self.render_state = render_state;
        self.frame_revision = self.frame_revision.wrapping_add(1);
        self.layout_dirty = true;
    }

    pub fn set_new_world_seed(&mut self, seed: i64) {
        if self.new_world_seed == seed {
            return;
        }
        self.new_world_seed = seed;
        self.frame_revision = self.frame_revision.wrapping_add(1);
    }

    pub fn set_join_remote_addr(&mut self, addr: impl Into<String>) {
        let addr = addr.into();
        if self.join_remote_addr == addr {
            return;
        }
        self.join_remote_addr = addr;
        self.frame_revision = self.frame_revision.wrapping_add(1);
    }

    pub fn set_debug_overlay(&mut self, enabled: bool) {
        if self.debug_overlay == enabled {
            return;
        }
        self.debug_overlay = enabled;
        self.frame_revision = self.frame_revision.wrapping_add(1);
    }

    pub fn clear_input(&mut self) {
        self.set_interaction_state(None, None, None);
        self.set_focus(None);
    }

    pub fn frame_state(&self) -> Option<UiFrameState> {
        self.screen.map(|screen| {
            UiFrameState::new(screen, self.scale, self.render_state, self.frame_revision)
        })
    }

    pub fn panel_revision(&self) -> Option<UiPanelRevision> {
        self.screen
            .map(|_| UiPanelRevision::new(self.frame_revision, self.interaction_revision))
    }

    pub fn layout(&mut self) -> &UiLayout {
        self.ensure_layout();
        &self.layout
    }

    pub fn debug_snapshot(&mut self) -> Option<UiDebugSnapshot> {
        self.screen?;
        self.ensure_layout();
        Some(UiDebugSnapshot {
            screen: self.screen,
            scale: self.scale,
            frame_revision: self.frame_revision,
            layout_revision: self.layout_revision,
            pointer: self.pointer,
            hovered: self.hovered,
            captured: self.captured,
            focused: self.focused,
            scroll_offset: self
                .layout
                .scroll_region
                .map_or(0.0, |region| region.offset),
            scroll_max: self
                .layout
                .scroll_region
                .map_or(0.0, UiScrollRegion::max_offset),
            scroll_clip: self.layout.scroll_region.map(|region| region.clip),
            widgets: self
                .layout
                .widgets()
                .iter()
                .map(|widget| UiDebugWidget {
                    id: widget.id,
                    kind: widget.kind,
                    rect: widget.rect,
                    label: widget.label.clone(),
                    value: widget.value.clone(),
                    enabled: widget.enabled,
                })
                .collect(),
        })
    }

    pub fn pointer_move(
        &mut self,
        point: Point,
        render_state: GameUiRenderState,
    ) -> (bool, Option<GameUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.set_render_state(render_state);
        self.ensure_layout();
        let hovered = self.layout.hit_test(point);
        self.set_interaction_state(Some(point), hovered, self.captured);
        self.set_focus(None);
        let action = self
            .captured
            .and_then(|id| self.layout.widget(id))
            .filter(|widget| matches!(widget.action, Some(UiWidgetAction::Slider(_))))
            .and_then(|widget| self.action_for_widget(widget, point));
        (true, action)
    }

    pub fn pointer_down(&mut self, point: Point, render_state: GameUiRenderState) -> bool {
        if !self.is_active() {
            return false;
        }
        self.set_render_state(render_state);
        self.ensure_layout();
        let hovered = self.layout.hit_test(point);
        self.set_interaction_state(Some(point), hovered, hovered);
        self.set_focus(None);
        true
    }

    pub fn pointer_up(
        &mut self,
        point: Point,
        render_state: GameUiRenderState,
    ) -> (bool, Option<GameUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.set_render_state(render_state);
        self.ensure_layout();
        let hovered = self.layout.hit_test(point);
        let captured = self.captured;
        self.set_interaction_state(Some(point), hovered, None);
        self.set_focus(None);
        let action = captured
            .and_then(|captured| self.layout.widget(captured))
            .and_then(|widget| match widget.action {
                // A slider owns the pointer from press through release, even
                // when the pointer leaves its visible bounds. The final value
                // is clamped by the widget's own track geometry.
                Some(UiWidgetAction::Slider(_)) => self.action_for_widget(widget, point),
                Some(UiWidgetAction::Static(_)) if self.hovered == Some(widget.id) => {
                    self.action_for_widget(widget, point)
                }
                _ => None,
            });
        (true, action)
    }

    pub fn navigate(
        &mut self,
        navigation: GuiNavigation,
        render_state: GameUiRenderState,
    ) -> (bool, Option<GameUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.set_render_state(render_state);
        self.ensure_layout();
        self.set_interaction_state(None, None, None);
        match navigation {
            GuiNavigation::Back => self.key_pressed(GuiKey::Escape),
            GuiNavigation::Confirm => (true, self.activate_focused()),
            GuiNavigation::PreviousPage => {
                self.move_focus_linear(-1);
                self.ensure_focused_visible();
                (true, None)
            }
            GuiNavigation::NextPage => {
                self.move_focus_linear(1);
                self.ensure_focused_visible();
                (true, None)
            }
            GuiNavigation::Left | GuiNavigation::Right if self.focused_widget_is_slider() => {
                let direction = if navigation == GuiNavigation::Left {
                    -1.0
                } else {
                    1.0
                };
                (true, self.adjust_focused_slider(direction))
            }
            GuiNavigation::Up
            | GuiNavigation::Down
            | GuiNavigation::Left
            | GuiNavigation::Right => {
                self.move_focus_spatial(navigation);
                self.ensure_focused_visible();
                (true, None)
            }
        }
    }

    pub fn scroll_by(&mut self, amount: f32, render_state: GameUiRenderState) -> bool {
        if !self.is_active() {
            return false;
        }
        self.set_render_state(render_state);
        self.ensure_layout();
        let Some(region) = self.layout.scroll_region else {
            return true;
        };
        let next = (self.scroll_offset + amount).clamp(0.0, region.max_offset());
        if (next - self.scroll_offset).abs() > f32::EPSILON {
            self.scroll_offset = next;
            self.frame_revision = self.frame_revision.wrapping_add(1);
            self.layout_dirty = true;
            self.ensure_layout();
        }
        true
    }

    pub fn key_pressed(&mut self, key: GuiKey) -> (bool, Option<GameUiAction>) {
        match (self.screen, key) {
            (Some(UiScreenId::Title), GuiKey::Escape) => (true, None),
            (Some(UiScreenId::Title), GuiKey::F1) => {
                (true, Some(GameUiAction::OpenHelp(GameHelpParent::Title)))
            }
            (Some(UiScreenId::PreparingLobby), GuiKey::Escape) => {
                (true, Some(GameUiAction::BackToTitle))
            }
            (Some(UiScreenId::PreparingLobby), _) => (true, None),
            (Some(UiScreenId::WorldList), GuiKey::Escape) => {
                (true, Some(GameUiAction::BackToTitle))
            }
            (Some(UiScreenId::WorldList), GuiKey::F1) => (
                true,
                Some(GameUiAction::OpenHelp(GameHelpParent::WorldList)),
            ),
            (Some(UiScreenId::WorldCreate), GuiKey::Escape) => {
                (true, Some(GameUiAction::OpenWorldList))
            }
            (Some(UiScreenId::WorldCreate), GuiKey::F1) => (
                true,
                Some(GameUiAction::OpenHelp(GameHelpParent::WorldCreate)),
            ),
            (Some(UiScreenId::WorldDeleteConfirm { .. }), GuiKey::Escape) => {
                (true, Some(GameUiAction::CancelDeleteWorld))
            }
            (Some(UiScreenId::WorldDeleteConfirm { .. }), GuiKey::F1) => (
                true,
                Some(GameUiAction::OpenHelp(GameHelpParent::WorldDeleteConfirm)),
            ),
            (Some(UiScreenId::NewWorld), GuiKey::Escape) => (true, Some(GameUiAction::BackToTitle)),
            (Some(UiScreenId::NewWorld), GuiKey::F1) => {
                (true, Some(GameUiAction::OpenHelp(GameHelpParent::NewWorld)))
            }
            (Some(UiScreenId::JoinRemote), GuiKey::Escape) => {
                (true, Some(GameUiAction::BackToTitle))
            }
            (Some(UiScreenId::JoinRemote), GuiKey::F1) => (
                true,
                Some(GameUiAction::OpenHelp(GameHelpParent::JoinRemote)),
            ),
            (Some(UiScreenId::Pause), GuiKey::Escape) => (true, Some(GameUiAction::Resume)),
            (Some(UiScreenId::Pause), GuiKey::F1) => {
                (true, Some(GameUiAction::OpenHelp(GameHelpParent::Pause)))
            }
            (Some(UiScreenId::Death { .. }), _) => (true, None),
            (Some(UiScreenId::BlockPalette), GuiKey::Escape) => (true, Some(GameUiAction::Resume)),
            (Some(UiScreenId::BlockPalette), GuiKey::F1) => {
                (true, Some(GameUiAction::OpenHelp(GameHelpParent::Pause)))
            }
            (Some(UiScreenId::Options { parent }), GuiKey::Escape) => match parent {
                GameOptionsParent::Title => (true, Some(GameUiAction::BackToTitle)),
                GameOptionsParent::Pause => (true, Some(GameUiAction::BackToPause)),
            },
            (Some(UiScreenId::Options { parent }), GuiKey::F1) => (
                true,
                Some(GameUiAction::OpenHelp(help_parent_for_options(parent))),
            ),
            (
                Some(UiScreenId::OptionsCategory {
                    parent,
                    category: GameOptionsCategory::Fog,
                }),
                GuiKey::Escape,
            ) => (
                true,
                Some(GameUiAction::OpenOptionsCategory(
                    parent,
                    GameOptionsCategory::Graphics,
                )),
            ),
            (Some(UiScreenId::OptionsCategory { parent, .. }), GuiKey::Escape) => {
                (true, Some(GameUiAction::OpenOptions(parent)))
            }
            (Some(UiScreenId::OptionsCategory { parent, .. }), GuiKey::F1) => (
                true,
                Some(GameUiAction::OpenHelp(help_parent_for_options(parent))),
            ),
            (Some(UiScreenId::ServerSettings { parent }), GuiKey::Escape) => {
                (true, Some(GameUiAction::OpenOptions(parent)))
            }
            (Some(UiScreenId::ServerSettings { parent }), GuiKey::F1) => (
                true,
                Some(GameUiAction::OpenHelp(help_parent_for_options(parent))),
            ),
            (Some(UiScreenId::AssetPacks { .. }), GuiKey::Escape)
                if self.render_state.asset_packs.apply_state.is_preparing() =>
            {
                (true, None)
            }
            (Some(UiScreenId::AssetPacks { .. }), GuiKey::Escape) => {
                (true, Some(GameUiAction::CancelAssetPacks))
            }
            (Some(UiScreenId::AssetPacks { parent }), GuiKey::F1) => (
                true,
                Some(GameUiAction::OpenHelp(help_parent_for_options(parent))),
            ),
            (Some(UiScreenId::StorageConfirm { parent, .. }), GuiKey::Escape) => {
                (true, Some(GameUiAction::CancelStorageAction(parent)))
            }
            (Some(UiScreenId::StorageConfirm { .. }), GuiKey::F1) => (true, None),
            (Some(UiScreenId::Help { parent }), GuiKey::Escape | GuiKey::F1) => {
                (true, Some(GameUiAction::CloseHelp(parent)))
            }
            (None, _) => (false, None),
        }
    }

    pub fn render_draw_list(&mut self, render_state: GameUiRenderState) -> GuiDrawList {
        self.set_render_state(render_state);
        self.ensure_layout();
        let mut draw = GuiDrawList::new();
        match self.screen {
            Some(UiScreenId::Title) => self.render_title(&mut draw),
            Some(UiScreenId::PreparingLobby) => self.render_preparing_lobby(&mut draw),
            Some(UiScreenId::WorldList) => self.render_world_list(&mut draw),
            Some(UiScreenId::WorldCreate) => self.render_world_create(&mut draw),
            Some(UiScreenId::WorldDeleteConfirm { id }) => {
                self.render_world_delete_confirm(&mut draw, id)
            }
            Some(UiScreenId::NewWorld) => self.render_new_world(&mut draw),
            Some(UiScreenId::JoinRemote) => self.render_join_remote(&mut draw),
            Some(UiScreenId::Pause) => self.render_pause(&mut draw),
            Some(UiScreenId::Death { cause }) => self.render_death(&mut draw, cause),
            Some(UiScreenId::BlockPalette) => self.render_block_palette(&mut draw),
            Some(UiScreenId::Options { parent }) => self.render_options(&mut draw, parent),
            Some(UiScreenId::OptionsCategory { parent, category }) => {
                self.render_options_category(&mut draw, parent, category)
            }
            Some(UiScreenId::ServerSettings { parent }) => {
                self.render_server_settings(&mut draw, parent)
            }
            Some(UiScreenId::AssetPacks { parent }) => self.render_asset_packs(&mut draw, parent),
            Some(UiScreenId::StorageConfirm { parent, action }) => {
                self.render_storage_confirm(&mut draw, parent, action)
            }
            Some(UiScreenId::Help { parent }) => self.render_help(&mut draw, parent),
            None => {}
        }
        if self.debug_overlay {
            self.render_debug_overlay(&mut draw);
        }
        draw
    }

    fn ensure_layout(&mut self) {
        if !self.layout_dirty {
            return;
        }
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.layout = match self.screen {
            Some(UiScreenId::Title) => title_layout(
                self.scale,
                self.layout_revision,
                self.render_state.lobby_scenario_available,
            ),
            Some(UiScreenId::PreparingLobby) => {
                preparing_lobby_layout(self.scale, self.layout_revision)
            }
            Some(UiScreenId::WorldList) => world_list_layout(
                self.scale,
                self.layout_revision,
                self.render_state.world_catalog,
            ),
            Some(UiScreenId::WorldCreate) => world_create_layout(
                self.scale,
                self.layout_revision,
                self.render_state.world_catalog,
            ),
            Some(UiScreenId::WorldDeleteConfirm { id }) => world_delete_confirm_layout(
                self.scale,
                self.layout_revision,
                id,
                self.render_state.world_catalog,
            ),
            Some(UiScreenId::NewWorld) => new_world_layout(self.scale, self.layout_revision),
            Some(UiScreenId::JoinRemote) => join_remote_layout(self.scale, self.layout_revision),
            Some(UiScreenId::Pause) => pause_layout(self.scale, self.layout_revision),
            Some(UiScreenId::Death { cause }) => {
                death_layout(self.scale, self.layout_revision, cause)
            }
            Some(UiScreenId::BlockPalette) => block_palette_layout(
                self.scale,
                self.layout_revision,
                self.render_state.block_palette,
            ),
            Some(UiScreenId::Options { parent }) => {
                options_layout(self.scale, self.layout_revision, parent, self.render_state)
            }
            Some(UiScreenId::OptionsCategory { parent, category }) => options_category_layout(
                self.scale,
                self.layout_revision,
                parent,
                category,
                self.render_state,
                self.scroll_offset,
            ),
            Some(UiScreenId::ServerSettings { parent }) => {
                server_settings_layout(self.scale, self.layout_revision, parent, self.render_state)
            }
            Some(UiScreenId::AssetPacks { parent }) => asset_packs_layout(
                self.scale,
                self.layout_revision,
                parent,
                self.render_state.asset_packs,
            ),
            Some(UiScreenId::StorageConfirm { parent, action }) => storage_confirm_layout(
                self.scale,
                self.layout_revision,
                parent,
                action,
                self.render_state,
            ),
            Some(UiScreenId::Help { parent }) => {
                help_layout(self.scale, self.layout_revision, parent, self.scroll_offset)
            }
            None => UiLayout::new(None, self.layout_revision),
        };
        if let Some(region) = self.layout.scroll_region {
            let clamped = self.scroll_offset.clamp(0.0, region.max_offset());
            if (clamped - self.scroll_offset).abs() > f32::EPSILON {
                self.scroll_offset = clamped;
                self.layout_dirty = true;
                return self.ensure_layout();
            }
        } else {
            self.scroll_offset = 0.0;
        }
        self.layout_dirty = false;
        self.hovered = self.pointer.and_then(|point| self.layout.hit_test(point));
        if self
            .captured
            .is_some_and(|captured| self.layout.widget(captured).is_none())
        {
            self.set_interaction_state(self.pointer, self.hovered, None);
        }
        if self.focused.is_some_and(|focused| {
            self.layout
                .widget(focused)
                .is_none_or(|widget| !widget_is_focusable(widget))
        }) {
            self.set_focus(None);
        }
    }

    fn interaction(&self) -> Interaction {
        let pointer = self
            .hovered
            .and_then(|hovered| self.layout.widget(hovered))
            .map(|widget| Point {
                x: widget.rect.center_x(),
                y: widget.rect.y + widget.rect.height * 0.5,
            });
        Interaction {
            pointer,
            pressed: self.captured.map(UiWidgetId::legacy_widget_id),
            focused: self.focused.map(UiWidgetId::legacy_widget_id),
        }
    }

    fn set_interaction_state(
        &mut self,
        pointer: Option<Point>,
        hovered: Option<UiWidgetId>,
        captured: Option<UiWidgetId>,
    ) {
        let pointer_changed = self.pointer != pointer;
        let visual_changed = self.hovered != hovered
            || self.captured != captured
            || (self.debug_overlay && pointer_changed);
        self.pointer = pointer;
        self.hovered = hovered;
        self.captured = captured;
        if visual_changed {
            self.interaction_revision = self.interaction_revision.wrapping_add(1);
        }
    }

    fn set_focus(&mut self, focused: Option<UiWidgetId>) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        self.interaction_revision = self.interaction_revision.wrapping_add(1);
    }

    fn focusable_widgets(&self) -> Vec<(UiWidgetId, Rect)> {
        self.layout
            .widgets()
            .iter()
            .filter(|widget| widget_is_focusable(widget))
            .map(|widget| (widget.id, widget.rect))
            .collect()
    }

    fn move_focus_linear(&mut self, step: isize) {
        let widgets = self.focusable_widgets();
        if widgets.is_empty() {
            self.set_focus(None);
            return;
        }
        let current = self
            .focused
            .and_then(|focused| widgets.iter().position(|(id, _)| *id == focused));
        let next = match current {
            Some(index) => (index as isize + step).rem_euclid(widgets.len() as isize) as usize,
            None if step < 0 => widgets.len() - 1,
            None => 0,
        };
        self.set_focus(Some(widgets[next].0));
    }

    fn move_focus_spatial(&mut self, navigation: GuiNavigation) {
        let widgets = self.focusable_widgets();
        if widgets.is_empty() {
            self.set_focus(None);
            return;
        }
        let Some((_, current_rect)) = self
            .focused
            .and_then(|focused| widgets.iter().find(|(id, _)| *id == focused).copied())
        else {
            let index = if matches!(navigation, GuiNavigation::Up | GuiNavigation::Left) {
                widgets.len() - 1
            } else {
                0
            };
            self.set_focus(Some(widgets[index].0));
            return;
        };
        let current = Point {
            x: current_rect.center_x(),
            y: current_rect.y + current_rect.height * 0.5,
        };
        let mut best: Option<(f32, UiWidgetId)> = None;
        for (id, rect) in &widgets {
            if Some(*id) == self.focused {
                continue;
            }
            let dx = rect.center_x() - current.x;
            let dy = rect.y + rect.height * 0.5 - current.y;
            let (primary, secondary) = match navigation {
                GuiNavigation::Up => (-dy, dx.abs()),
                GuiNavigation::Down => (dy, dx.abs()),
                GuiNavigation::Left => (-dx, dy.abs()),
                GuiNavigation::Right => (dx, dy.abs()),
                _ => unreachable!("only spatial navigation reaches this helper"),
            };
            if primary <= 0.5 {
                continue;
            }
            let score = primary + secondary * 3.0;
            if best.is_none_or(|(best_score, _)| score < best_score) {
                best = Some((score, *id));
            }
        }
        if let Some((_, id)) = best {
            self.set_focus(Some(id));
        } else {
            self.move_focus_linear(match navigation {
                GuiNavigation::Up | GuiNavigation::Left => -1,
                GuiNavigation::Down | GuiNavigation::Right => 1,
                _ => unreachable!("only spatial navigation reaches this helper"),
            });
        }
    }

    fn ensure_focused_visible(&mut self) {
        self.ensure_layout();
        let Some(focused) = self.focused else {
            return;
        };
        let Some(region) = self.layout.scroll_region else {
            return;
        };
        let Some((index, widget)) = self
            .layout
            .widgets()
            .iter()
            .enumerate()
            .find(|(_, widget)| widget.id == focused)
        else {
            return;
        };
        if !region.contains_widget(index) {
            return;
        }
        let next = if widget.rect.y < region.clip.y {
            self.scroll_offset - (region.clip.y - widget.rect.y)
        } else if widget.rect.bottom() > region.clip.bottom() {
            self.scroll_offset + (widget.rect.bottom() - region.clip.bottom())
        } else {
            return;
        };
        self.scroll_offset = next.clamp(0.0, region.max_offset());
        self.frame_revision = self.frame_revision.wrapping_add(1);
        self.layout_dirty = true;
        self.ensure_layout();
    }

    fn focused_widget_is_slider(&self) -> bool {
        self.focused
            .and_then(|focused| self.layout.widget(focused))
            .is_some_and(|widget| matches!(widget.kind, UiWidgetKind::Slider { .. }))
    }

    fn activate_focused(&mut self) -> Option<GameUiAction> {
        if self.focused.is_none() {
            self.move_focus_linear(1);
        }
        let widget = self
            .focused
            .and_then(|focused| self.layout.widget(focused))?
            .clone();
        if matches!(widget.kind, UiWidgetKind::Slider { .. }) {
            return None;
        }
        self.action_for_widget(
            &widget,
            Point {
                x: widget.rect.center_x(),
                y: widget.rect.y + widget.rect.height * 0.5,
            },
        )
    }

    fn adjust_focused_slider(&self, direction: f32) -> Option<GameUiAction> {
        let widget = self
            .focused
            .and_then(|focused| self.layout.widget(focused))?;
        let UiWidgetKind::Slider { value } = widget.kind else {
            return None;
        };
        let slider = Slider::new(widget.id.legacy_widget_id(), widget.rect, "", value);
        self.action_for_widget(widget, slider.point_for_value(value + direction * 0.05))
    }

    fn render_title(&self, draw: &mut GuiDrawList) {
        render_title_background(draw, self.scale);
        self.font.draw_centered_atlas(
            draw,
            "MCLONE",
            self.scale.width * 0.5,
            34.0,
            Color::rgba(245, 252, 234, 255),
        );
        self.font.draw_centered_atlas(
            draw,
            "NATIVE RUST CLIENT",
            self.scale.width * 0.5,
            48.0,
            Color::rgba(185, 212, 198, 255),
        );
        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
        self.font.draw_shadow_atlas(
            draw,
            "MINECRAFT 1.17.1 TARGET",
            4.0,
            self.scale.height - 12.0,
            Color::rgba(160, 176, 170, 255),
        );
    }

    fn render_preparing_lobby(&self, draw: &mut GuiDrawList) {
        render_title_background(draw, self.scale);
        self.font.draw_centered_atlas(
            draw,
            "PREPARING LOBBY",
            self.scale.width * 0.5,
            self.scale.height * 0.5 - 18.0,
            Color::rgba(245, 252, 234, 255),
        );
        self.font.draw_centered_atlas(
            draw,
            "Starting the authored lobby...",
            self.scale.width * 0.5,
            self.scale.height * 0.5,
            Color::rgba(185, 212, 198, 255),
        );
        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
    }

    fn render_world_list(&self, draw: &mut GuiDrawList) {
        render_title_background(draw, self.scale);
        let catalog = self.render_state.world_catalog;
        let panel = world_list_panel_rect(self.scale);
        draw.fill_gradient(
            panel,
            Color::rgba(31, 43, 45, 245),
            Color::rgba(13, 18, 20, 245),
        );
        draw.outline(panel, Color::rgba(130, 166, 154, 255));
        self.font.draw_centered_atlas(
            draw,
            "SELECT WORLD",
            panel.center_x(),
            panel.y + 10.0,
            Color::rgba(245, 252, 234, 255),
        );

        let subtitle = if catalog.loading {
            "LOADING WORLDS"
        } else if !catalog.persistent {
            "PERSISTENT WORLDS UNAVAILABLE"
        } else if catalog.entry_count() == 0 {
            "NO WORLDS FOUND"
        } else {
            "LOCAL WORLDS"
        };
        self.font.draw_centered_atlas(
            draw,
            subtitle,
            panel.center_x(),
            panel.y + 24.0,
            Color::rgba(185, 212, 198, 255),
        );

        if catalog.status.visible {
            let color = if catalog.status.ok {
                Color::rgba(190, 224, 196, 255)
            } else {
                Color::rgba(255, 178, 178, 255)
            };
            self.font.draw_centered_atlas(
                draw,
                catalog.status.message.as_str(),
                panel.center_x(),
                panel.bottom() - 39.0,
                color,
            );
        }

        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
    }

    fn render_world_create(&self, draw: &mut GuiDrawList) {
        render_title_background(draw, self.scale);
        let catalog = self.render_state.world_catalog;
        let panel = world_create_panel_rect(self.scale);
        draw.fill_gradient(
            panel,
            Color::rgba(31, 43, 45, 245),
            Color::rgba(13, 18, 20, 245),
        );
        draw.outline(panel, Color::rgba(130, 166, 154, 255));
        self.font.draw_centered_atlas(
            draw,
            "CREATE WORLD",
            panel.center_x(),
            panel.y + 12.0,
            Color::rgba(245, 252, 234, 255),
        );
        let display_name = if catalog.create_display_name.is_empty() {
            "New World"
        } else {
            catalog.create_display_name.as_str()
        };
        self.font.draw_centered_atlas(
            draw,
            display_name,
            panel.center_x(),
            panel.y + 38.0,
            Color::rgba(222, 238, 222, 255),
        );
        self.font.draw_centered_atlas(
            draw,
            &format!("Seed: {}", self.new_world_seed),
            panel.center_x(),
            panel.y + 54.0,
            Color::rgba(185, 212, 198, 255),
        );
        self.font.draw_centered_atlas(
            draw,
            &format!("World type: {}", catalog.create_generation_profile.as_str()),
            panel.center_x(),
            panel.y + 70.0,
            Color::rgba(185, 212, 198, 255),
        );
        if !catalog.create_supported {
            self.font.draw_centered_atlas(
                draw,
                "CREATE IS UNAVAILABLE",
                panel.center_x(),
                panel.y + 84.0,
                Color::rgba(255, 178, 178, 255),
            );
        }

        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
    }

    fn render_world_delete_confirm(&self, draw: &mut GuiDrawList, id: WorldCatalogUiWorldId) {
        render_title_background(draw, self.scale);
        let catalog = self.render_state.world_catalog;
        let panel = world_delete_confirm_panel_rect(self.scale);
        draw.fill_gradient(
            panel,
            Color::rgba(45, 33, 35, 245),
            Color::rgba(18, 13, 14, 245),
        );
        draw.outline(panel, Color::rgba(190, 124, 124, 255));
        self.font.draw_centered_atlas(
            draw,
            "DELETE WORLD",
            panel.center_x(),
            panel.y + 12.0,
            Color::rgba(255, 224, 224, 255),
        );

        let name = catalog
            .entry(id)
            .map(|entry| entry.display_name.as_str())
            .unwrap_or("Unknown World");
        self.font.draw_centered_atlas(
            draw,
            name,
            panel.center_x(),
            panel.y + 40.0,
            Color::rgba(245, 252, 234, 255),
        );
        let warning = if catalog.active == Some(id) {
            "QUIT TO TITLE BEFORE DELETING THE ACTIVE WORLD"
        } else if !catalog.can_delete_world(id) {
            "THIS WORLD CANNOT BE DELETED"
        } else {
            "THIS CANNOT BE UNDONE"
        };
        self.font.draw_centered_atlas(
            draw,
            warning,
            panel.center_x(),
            panel.y + 58.0,
            Color::rgba(255, 190, 190, 255),
        );

        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
    }

    fn render_storage_confirm(
        &self,
        draw: &mut GuiDrawList,
        _parent: GameOptionsParent,
        action: GameStorageAction,
    ) {
        render_title_background(draw, self.scale);
        let panel = storage_confirm_panel_rect(self.scale);
        draw.fill_gradient(
            panel,
            Color::rgba(45, 33, 35, 245),
            Color::rgba(18, 13, 14, 245),
        );
        draw.outline(panel, Color::rgba(190, 124, 124, 255));
        self.font.draw_centered_atlas(
            draw,
            action.title(),
            panel.center_x(),
            panel.y + 12.0,
            Color::rgba(255, 224, 224, 255),
        );
        let warnings = match action {
            GameStorageAction::ResetPlayerIdentity => (
                "CREATES NEW UUID; WORLDS AND PREFERENCES STAY",
                "OLD LOCAL AND REMOTE PLAYER RECORDS REMAIN",
            ),
            GameStorageAction::DeleteAllLocalWorlds => (
                "DELETES CATALOG WORLDS; PROFILE AND PREFERENCES STAY",
                "MANAGED CONTENT AND REMOTE DATA STAY",
            ),
            GameStorageAction::FactoryReset => (
                "DELETES PROFILE, PREFERENCES, WORLDS & MANAGED CONTENT",
                "REMOTE SERVER PLAYER RECORDS STAY",
            ),
        };
        for (index, warning) in [warnings.0, warnings.1].into_iter().enumerate() {
            self.font.draw_centered_atlas(
                draw,
                warning,
                panel.center_x(),
                panel.y + 42.0 + index as f32 * 12.0,
                Color::rgba(255, 178, 178, 255),
            );
        }
        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
    }

    fn render_new_world(&self, draw: &mut GuiDrawList) {
        render_title_background(draw, self.scale);
        self.font.draw_centered_atlas(
            draw,
            "NEW WORLD",
            self.scale.width * 0.5,
            self.scale.height * 0.28,
            Color::rgba(245, 252, 234, 255),
        );
        self.font.draw_centered_atlas(
            draw,
            &format!("Seed: {}", self.new_world_seed),
            self.scale.width * 0.5,
            self.scale.height * 0.28 + 22.0,
            Color::rgba(185, 212, 198, 255),
        );
        self.font.draw_centered_atlas(
            draw,
            &format!(
                "World type: {}",
                self.render_state
                    .world_catalog
                    .create_generation_profile
                    .as_str()
            ),
            self.scale.width * 0.5,
            self.scale.height * 0.28 + 38.0,
            Color::rgba(185, 212, 198, 255),
        );
        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
    }

    fn render_join_remote(&self, draw: &mut GuiDrawList) {
        render_title_background(draw, self.scale);
        self.font.draw_centered_atlas(
            draw,
            "JOIN REMOTE",
            self.scale.width * 0.5,
            self.scale.height * 0.28,
            Color::rgba(245, 252, 234, 255),
        );
        self.font.draw_centered_atlas(
            draw,
            &format!("Server: {}", self.join_remote_addr),
            self.scale.width * 0.5,
            self.scale.height * 0.28 + 22.0,
            Color::rgba(185, 212, 198, 255),
        );
        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
    }

    fn render_pause(&self, draw: &mut GuiDrawList) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 135),
        );
        self.font.draw_centered_atlas(
            draw,
            "PAUSED",
            self.scale.width * 0.5,
            self.scale.height * 0.25,
            Color::rgba(245, 252, 234, 255),
        );
        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
    }

    fn render_death(&self, draw: &mut GuiDrawList, cause: GameDeathCause) {
        draw.fill_gradient(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(96, 0, 0, 160),
            Color::rgba(30, 0, 0, 190),
        );
        self.font.draw_centered_atlas(
            draw,
            "YOU DIED!",
            self.scale.width * 0.5,
            self.scale.height * 0.25,
            Color::rgba(255, 255, 255, 255),
        );
        self.font.draw_centered_atlas(
            draw,
            cause.message(),
            self.scale.width * 0.5,
            self.scale.height * 0.25 + 24.0,
            Color::rgba(235, 235, 235, 255),
        );
        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
    }

    fn render_options(&self, draw: &mut GuiDrawList, parent: GameOptionsParent) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 150),
        );
        let panel = options_panel_rect(self.scale, self.render_state);
        draw.fill_gradient(
            panel,
            Color::rgba(33, 45, 47, 245),
            Color::rgba(15, 20, 22, 245),
        );
        draw.outline(panel, Color::rgba(130, 166, 154, 255));
        self.font.draw_centered_atlas(
            draw,
            "OPTIONS",
            panel.center_x(),
            panel.y + 12.0,
            Color::rgba(245, 252, 234, 255),
        );
        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
        let _ = parent;
    }

    fn render_options_category(
        &self,
        draw: &mut GuiDrawList,
        parent: GameOptionsParent,
        category: GameOptionsCategory,
    ) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 150),
        );
        let panel = options_category_panel_rect(self.scale, category);
        draw.fill_gradient(
            panel,
            Color::rgba(33, 45, 47, 245),
            Color::rgba(15, 20, 22, 245),
        );
        draw.outline(panel, Color::rgba(130, 166, 154, 255));
        self.font.draw_centered_atlas(
            draw,
            category.title(),
            panel.center_x(),
            panel.y + 12.0,
            Color::rgba(245, 252, 234, 255),
        );
        let interaction = self.interaction();
        if let Some(region) = self.layout.scroll_region {
            draw.push_clip(region.clip);
            for (index, widget) in self.layout.widgets().iter().enumerate() {
                if region.contains_widget(index) {
                    self.render_widget(draw, widget, interaction);
                }
            }
            draw.pop_clip();
            for (index, widget) in self.layout.widgets().iter().enumerate() {
                if !region.contains_widget(index) {
                    self.render_widget(draw, widget, interaction);
                }
            }
            self.render_scrollbar(draw, region);
        } else {
            for widget in self.layout.widgets() {
                self.render_widget(draw, widget, interaction);
            }
        }
        if category == GameOptionsCategory::StorageProfile {
            let status = if self.render_state.storage_profile.status.visible {
                self.render_state.storage_profile.status
            } else {
                self.render_state.world_catalog.status
            };
            if status.visible || self.render_state.world_catalog.loading {
                let (message, ok) = if status.visible {
                    (status.message.as_str(), status.ok)
                } else {
                    ("Updating local world storage...", true)
                };
                self.font.draw_centered_atlas(
                    draw,
                    message,
                    panel.center_x(),
                    (panel.bottom() + 2.0).min(self.scale.height - 10.0),
                    if ok {
                        Color::rgba(190, 224, 196, 255)
                    } else {
                        Color::rgba(255, 178, 178, 255)
                    },
                );
            }
        }
        let _ = parent;
    }

    fn render_scrollbar(&self, draw: &mut GuiDrawList, region: UiScrollRegion) {
        let max_offset = region.max_offset();
        if max_offset <= 0.0 {
            return;
        }
        let track = Rect::new(
            region.clip.right() - 3.0,
            region.clip.y,
            3.0,
            region.clip.height,
        );
        let thumb_height = (region.clip.height * region.clip.height / region.content_height)
            .clamp(10.0, track.height);
        let travel = (track.height - thumb_height).max(0.0);
        let thumb_y = track.y + travel * (region.offset / max_offset);
        draw.fill(track, Color::rgba(8, 12, 13, 190));
        draw.fill(
            Rect::new(track.x, thumb_y, track.width, thumb_height),
            Color::rgba(174, 220, 154, 255),
        );
    }

    fn render_server_settings(&self, draw: &mut GuiDrawList, parent: GameOptionsParent) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 150),
        );
        let panel = server_settings_panel_rect(self.scale, self.render_state);
        draw.fill_gradient(
            panel,
            Color::rgba(33, 45, 47, 245),
            Color::rgba(15, 20, 22, 245),
        );
        draw.outline(panel, Color::rgba(130, 166, 154, 255));
        self.font.draw_centered_atlas(
            draw,
            "SERVER SETTINGS",
            panel.center_x(),
            panel.y + 12.0,
            Color::rgba(245, 252, 234, 255),
        );
        if self.render_state.server_cadence.is_none() {
            self.font.draw_centered_atlas(
                draw,
                "LOCAL SERVER ONLY",
                panel.center_x(),
                panel.y + 42.0,
                Color::rgba(185, 212, 198, 255),
            );
        }
        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
        let _ = parent;
    }

    fn render_asset_packs(&self, draw: &mut GuiDrawList, parent: GameOptionsParent) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 150),
        );
        let panel = asset_packs_panel_rect(self.scale);
        draw.fill_gradient(
            panel,
            Color::rgba(33, 45, 47, 248),
            Color::rgba(15, 20, 22, 248),
        );
        draw.outline(panel, Color::rgba(130, 166, 154, 255));
        self.font.draw_centered_atlas(
            draw,
            "VISUAL PROFILES",
            panel.center_x(),
            panel.y + 10.0,
            Color::rgba(245, 252, 234, 255),
        );

        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }

        let _ = parent;
    }

    fn render_help(&self, draw: &mut GuiDrawList, parent: GameHelpParent) {
        if help_parent_covers_world(parent) {
            draw.fill_gradient(
                Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
                Color::rgba(24, 44, 51, 255),
                Color::rgba(7, 10, 12, 255),
            );
            draw.fill(
                Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
                Color::rgba(0, 0, 0, 55),
            );
        } else {
            draw.fill(
                Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
                Color::rgba(0, 0, 0, 150),
            );
        }

        let panel = help_panel_rect(self.scale);
        draw.fill_gradient(
            panel,
            Color::rgba(33, 45, 47, 245),
            Color::rgba(15, 20, 22, 245),
        );
        draw.outline(panel, Color::rgba(130, 166, 154, 255));
        self.font.draw_centered_atlas(
            draw,
            "CONTROLS",
            panel.center_x(),
            panel.y + 8.0,
            Color::rgba(245, 252, 234, 255),
        );
        self.font.draw_centered_atlas(
            draw,
            "F1 / ESC BACK",
            panel.center_x(),
            panel.y + 20.0,
            Color::rgba(185, 212, 198, 255),
        );
        if let Some(region) = self.layout.scroll_region {
            draw.push_clip(region.clip);
            for row in self.layout.help_rows() {
                self.render_help_row(draw, row);
            }
            draw.pop_clip();
            self.render_scrollbar(draw, region);
        } else {
            for row in self.layout.help_rows() {
                self.render_help_row(draw, row);
            }
        }
        let interaction = self.interaction();
        for widget in self.layout.widgets() {
            self.render_widget(draw, widget, interaction);
        }
    }

    fn render_help_row(&self, draw: &mut GuiDrawList, row: &UiHelpRow) {
        match &row.kind {
            UiHelpRowKind::Group(group) => {
                self.font.draw_shadow_atlas(
                    draw,
                    group.label(),
                    row.x,
                    row.y,
                    Color::rgba(220, 238, 220, 255),
                );
            }
            UiHelpRowKind::Shortcut(shortcut) => {
                self.font.draw_shadow_atlas(
                    draw,
                    &shortcut.control,
                    row.x,
                    row.y,
                    Color::rgba(185, 212, 198, 255),
                );
                self.font.draw_shadow_atlas(
                    draw,
                    &shortcut.action,
                    row.x + row.control_width,
                    row.y,
                    Color::rgba(214, 226, 218, 255),
                );
            }
        }
    }

    fn render_block_palette(&mut self, draw: &mut GuiDrawList) {
        let overlay = self.render_state.block_palette;
        if !overlay.visible {
            self.block_palette_grid_cache = UiDrawCacheStats::default();
            return;
        }

        let grid = self.render_block_palette_grid_layer(overlay);
        draw.append(&grid);
        self.render_block_palette_interaction_layer(draw);
    }

    fn render_block_palette_grid_layer(&mut self, overlay: BlockPaletteOverlay) -> GuiDrawList {
        let Some(state) = BlockPaletteGridState::from_overlay(self.scale, overlay) else {
            self.block_palette_grid_cache = UiDrawCacheStats::default();
            return GuiDrawList::new();
        };
        if let Some(cached) = &self.block_palette_grid {
            if cached.state == state {
                self.block_palette_grid_cache = UiDrawCacheStats::cache_hit();
                return cached.draw.clone();
            }
        }

        let mut draw = GuiDrawList::new();
        let panel = block_palette_panel_rect(self.scale, overlay);
        draw.fill(panel, Color::rgba(5, 8, 9, 188));
        draw.outline(panel, Color::rgba(132, 158, 148, 210));
        self.font.draw_shadow_atlas(
            &mut draw,
            &format!("SLOT {}", u16::from(overlay.selected_hotbar_slot) + 1),
            panel.x + BLOCK_PALETTE_PADDING,
            panel.y + 5.0,
            Color::rgba(218, 234, 226, 255),
        );

        for widget in self.layout.widgets() {
            let UiWidgetKind::PaletteSlot { icon } = widget.kind else {
                continue;
            };
            render_touch_panel(&mut draw, widget.rect, false);
            render_palette_slot_contents(&mut draw, &self.font, widget.rect, icon);
        }

        self.block_palette_grid = Some(CachedBlockPaletteGridLayer {
            state,
            draw: draw.clone(),
        });
        self.block_palette_grid_cache = UiDrawCacheStats::rebuild();
        draw
    }

    fn render_block_palette_interaction_layer(&self, draw: &mut GuiDrawList) {
        let highlighted = self.hovered.or(self.focused);
        for widget in self.layout.widgets() {
            let UiWidgetKind::PaletteSlot { icon } = widget.kind else {
                continue;
            };
            if self.captured == Some(widget.id) {
                render_touch_panel(draw, widget.rect, true);
                render_palette_slot_contents(draw, &self.font, widget.rect, icon);
            }
            if highlighted == Some(widget.id) {
                draw.outline(widget.rect.inset(-1.0), Color::rgba(245, 250, 255, 205));
            }
        }

        let Some(highlighted) = highlighted.and_then(|id| self.layout.widget(id)) else {
            return;
        };
        if matches!(highlighted.kind, UiWidgetKind::PaletteSlot { .. }) {
            let tooltip_anchor = Point {
                x: highlighted.rect.right(),
                y: highlighted.rect.y,
            };
            render_block_palette_tooltip(
                draw,
                &self.font,
                self.scale,
                tooltip_anchor,
                highlighted.label.as_str(),
            );
        }
    }

    fn render_widget(&self, draw: &mut GuiDrawList, widget: &UiWidget, interaction: Interaction) {
        match &widget.kind {
            UiWidgetKind::Button => Button::new(
                widget.id.legacy_widget_id(),
                widget.rect,
                widget.label.as_str(),
            )
            .enabled(widget.enabled)
            .render_atlas_text(draw, &self.font, interaction),
            UiWidgetKind::Checkbox { checked } => {
                let mut checkbox = Checkbox::new(
                    widget.id.legacy_widget_id(),
                    widget.rect,
                    widget.label.as_str(),
                    *checked,
                );
                checkbox.enabled = widget.enabled;
                checkbox.render_atlas_text(draw, &self.font, interaction);
            }
            UiWidgetKind::Cycle => {
                let mut cycle = CycleButton::new(
                    widget.id.legacy_widget_id(),
                    widget.rect,
                    widget.label.as_str(),
                    widget.value.as_deref().unwrap_or(""),
                );
                cycle.enabled = widget.enabled;
                cycle.render_atlas_text(draw, &self.font, interaction);
            }
            UiWidgetKind::WorldRow {
                selected,
                locked,
                compatible,
            } => {
                let highlighted = widget.enabled
                    && interaction.is_highlighted(widget.id.legacy_widget_id(), widget.rect);
                let fill = if *selected && highlighted {
                    Color::rgba(84, 112, 106, 245)
                } else if *selected {
                    Color::rgba(64, 90, 84, 230)
                } else if highlighted {
                    Color::rgba(42, 58, 58, 225)
                } else {
                    Color::rgba(19, 27, 28, 210)
                };
                let border = if *selected {
                    Color::rgba(196, 224, 180, 255)
                } else {
                    Color::rgba(72, 92, 88, 210)
                };
                let text = if *locked || !*compatible {
                    Color::rgba(155, 164, 158, 255)
                } else {
                    Color::rgba(235, 242, 232, 255)
                };
                draw.fill(widget.rect, fill);
                draw.outline(widget.rect, border);
                draw.push_clip(widget.rect.inset(3.0));
                let prefix = if *selected { "> " } else { "  " };
                self.font.draw_shadow_atlas(
                    draw,
                    &format!("{prefix}{}", widget.label),
                    widget.rect.x + 5.0,
                    widget.rect.y + 6.0,
                    text,
                );
                if let Some(value) = widget.value.as_deref() {
                    let value_color = if *locked || !*compatible {
                        Color::rgba(130, 136, 132, 255)
                    } else {
                        Color::rgba(178, 204, 190, 255)
                    };
                    let value_x = (widget.rect.right() - self.font.width(value) - 8.0)
                        .max(widget.rect.x + 80.0);
                    self.font.draw_shadow_atlas(
                        draw,
                        value,
                        value_x,
                        widget.rect.y + 6.0,
                        value_color,
                    );
                }
                draw.pop_clip();
            }
            UiWidgetKind::AssetPackRow {
                checked,
                locked,
                status,
            } => {
                let highlighted = widget.enabled
                    && interaction.is_highlighted(widget.id.legacy_widget_id(), widget.rect);
                let fill = if highlighted {
                    Color::rgba(45, 62, 60, 235)
                } else {
                    Color::rgba(19, 27, 28, 220)
                };
                let border = match status {
                    AssetPackUiRowStatus::Active => Color::rgba(166, 214, 146, 255),
                    AssetPackUiRowStatus::Preparing => Color::rgba(222, 205, 126, 255),
                    AssetPackUiRowStatus::Failed => Color::rgba(220, 126, 126, 255),
                    _ => Color::rgba(72, 92, 88, 220),
                };
                draw.fill(widget.rect, fill);
                draw.outline(widget.rect, border);
                let compact = widget.rect.height < 30.0;
                let box_rect = Rect::new(
                    widget.rect.x + 6.0,
                    widget.rect.y + (widget.rect.height - 10.0) * 0.5,
                    10.0,
                    10.0,
                );
                draw.fill(box_rect, Color::rgba(8, 12, 13, 255));
                draw.outline(box_rect, border);
                if *checked {
                    draw.fill(box_rect.inset(2.0), Color::rgba(174, 220, 154, 255));
                }
                let text_color = if *status == AssetPackUiRowStatus::Unavailable {
                    Color::rgba(150, 158, 153, 255)
                } else {
                    Color::rgba(235, 242, 232, 255)
                };
                draw.push_clip(widget.rect.inset(3.0));
                self.font.draw_shadow_atlas(
                    draw,
                    &widget.label,
                    widget.rect.x + 22.0,
                    widget.rect.y + if compact { 2.0 } else { 5.0 },
                    text_color,
                );
                if let Some(value) = widget.value.as_deref() {
                    self.font.draw_shadow_atlas(
                        draw,
                        value,
                        widget.rect.x + 22.0,
                        widget.rect.y + if compact { 13.0 } else { 18.0 },
                        Color::rgba(166, 190, 179, 255),
                    );
                }
                if *locked {
                    self.font.draw_shadow_atlas(
                        draw,
                        "LOCKED",
                        widget.rect.right() - 48.0,
                        widget.rect.y + if compact { 2.0 } else { 5.0 },
                        Color::rgba(222, 205, 126, 255),
                    );
                }
                draw.pop_clip();
            }
            UiWidgetKind::PaletteSlot { icon } => {
                render_touch_panel(
                    draw,
                    widget.rect,
                    interaction.pressed == Some(widget.id.legacy_widget_id()),
                );
                render_palette_slot_contents(draw, &self.font, widget.rect, *icon);
                if widget.enabled
                    && interaction.is_highlighted(widget.id.legacy_widget_id(), widget.rect)
                {
                    draw.outline(widget.rect.inset(-1.0), Color::rgba(245, 250, 255, 205));
                }
            }
            UiWidgetKind::Slider { value } => Slider::new(
                widget.id.legacy_widget_id(),
                widget.rect,
                widget.label.as_str(),
                *value,
            )
            .enabled(widget.enabled)
            .render_atlas_text(draw, &self.font, interaction),
        }
    }

    fn render_debug_overlay(&self, draw: &mut GuiDrawList) {
        if let Some(pointer) = self.pointer {
            let x = pointer.x.floor();
            let y = pointer.y.floor();
            draw.fill(
                Rect::new(x - 3.0, y, 7.0, 1.0),
                Color::rgba(255, 64, 64, 255),
            );
            draw.fill(
                Rect::new(x, y - 3.0, 1.0, 7.0),
                Color::rgba(255, 64, 64, 255),
            );
        }
        if let Some(hovered) = self.hovered.and_then(|id| self.layout.widget(id)) {
            draw.outline(hovered.rect, Color::rgba(255, 220, 60, 255));
        }
        if let Some(captured) = self.captured.and_then(|id| self.layout.widget(id)) {
            draw.outline(captured.rect.inset(1.0), Color::rgba(80, 180, 255, 255));
        }
    }

    fn action_for_widget(&self, widget: &UiWidget, point: Point) -> Option<GameUiAction> {
        if widget.id == UI_V2_NEW_WORLD_CREATE {
            return Some(GameUiAction::CreateWorld(self.new_world_seed));
        }
        match widget.action? {
            UiWidgetAction::Static(action) => Some(action),
            UiWidgetAction::Slider(action) => {
                let value = Slider::new(widget.id.legacy_widget_id(), widget.rect, "", 0.0)
                    .value_from_point(point);
                Some(match action {
                    UiSliderAction::RenderDistance => GameUiAction::SetRenderDistance(
                        render_distance_from_slider_value(value, self.render_state),
                    ),
                    UiSliderAction::FogVisibility => GameUiAction::SetFogSettings(
                        fog_visibility_from_slider_value(value, self.render_state.fog),
                    ),
                    UiSliderAction::FogClassicStart => GameUiAction::SetFogSettings(
                        fog_classic_start_from_slider_value(value, self.render_state.fog),
                    ),
                    UiSliderAction::FogGuardStart => GameUiAction::SetFogSettings(
                        fog_guard_start_from_slider_value(value, self.render_state.fog),
                    ),
                    UiSliderAction::FogGroundBase => GameUiAction::SetFogSettings(
                        fog_ground_base_from_slider_value(value, self.render_state.fog),
                    ),
                    UiSliderAction::FogGroundFalloff => GameUiAction::SetFogSettings(
                        fog_ground_falloff_from_slider_value(value, self.render_state.fog),
                    ),
                    UiSliderAction::FogMaxOpacity => GameUiAction::SetFogSettings(
                        fog_max_opacity_from_slider_value(value, self.render_state.fog),
                    ),
                    UiSliderAction::FogColorRed => GameUiAction::SetFogSettings(
                        fog_color_component_from_slider_value(value, self.render_state.fog, 0),
                    ),
                    UiSliderAction::FogColorGreen => GameUiAction::SetFogSettings(
                        fog_color_component_from_slider_value(value, self.render_state.fog, 1),
                    ),
                    UiSliderAction::FogColorBlue => GameUiAction::SetFogSettings(
                        fog_color_component_from_slider_value(value, self.render_state.fog, 2),
                    ),
                    UiSliderAction::FlySpeed => GameUiAction::SetFlySpeed(
                        fly_speed_from_slider_value(value, self.render_state),
                    ),
                    UiSliderAction::MovementSpeed => GameUiAction::SetMovementSpeed(
                        movement_speed_from_slider_value(value, self.render_state),
                    ),
                    UiSliderAction::TouchLook => {
                        let settings = self.render_state.touch_settings?;
                        GameUiAction::SetTouchLookSensitivity(touch_look_from_slider_value(
                            value, settings,
                        ))
                    }
                })
            }
        }
    }
}

fn widget_is_focusable(widget: &UiWidget) -> bool {
    widget.enabled && widget.action.is_some()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiDrawCacheStats {
    pub rebuild_count: u64,
    pub cache_hit_count: u64,
}

impl UiDrawCacheStats {
    pub const fn rebuild() -> Self {
        Self {
            rebuild_count: 1,
            cache_hit_count: 0,
        }
    }

    pub const fn cache_hit() -> Self {
        Self {
            rebuild_count: 0,
            cache_hit_count: 1,
        }
    }

    pub fn add(&mut self, other: Self) {
        self.rebuild_count += other.rebuild_count;
        self.cache_hit_count += other.cache_hit_count;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiPanelDrawList {
    pub draw: GuiDrawList,
    pub revision: UiPanelRevision,
    pub cache: UiDrawCacheStats,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlatHudDrawList {
    pub draw: GuiDrawList,
    pub retained_cache: UiDrawCacheStats,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadingProgressDrawList {
    pub draw: GuiDrawList,
    pub retained_cache: UiDrawCacheStats,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LoadingProgressOverlayLayer {
    Fullscreen,
    Panel { origin: Point, label: String },
}

impl LoadingProgressOverlayLayer {
    pub const fn fullscreen() -> Self {
        Self::Fullscreen
    }

    pub fn panel(origin: Point, label: impl Into<String>) -> Self {
        Self::Panel {
            origin,
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CachedV2DrawList {
    revision: UiPanelRevision,
    draw: GuiDrawList,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FlatHudRetainedState {
    scale: GuiScale,
    crosshair_visible: bool,
    hotbar_visible: bool,
}

impl FlatHudRetainedState {
    fn from_hud(scale: GuiScale, hud: &FlatHud) -> Self {
        let crosshair_visible = hud.world_hud_visible && hud.crosshair_visible;
        let hotbar_visible = hud.world_hud_visible && hud.should_render_flat_hotbar();
        Self {
            scale,
            crosshair_visible,
            hotbar_visible,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FlatHudHotbarState {
    scale: GuiScale,
    selected_hotbar_slot: u8,
    icons: [Option<GuiTextureUv>; HOTBAR_SLOT_COUNT_USIZE],
}

impl FlatHudHotbarState {
    fn from_hud(scale: GuiScale, hud: &FlatHud) -> Option<Self> {
        if !(hud.world_hud_visible && hud.should_render_flat_hotbar()) {
            return None;
        }
        Some(Self {
            scale,
            selected_hotbar_slot: hud
                .hotbar
                .selected_slot
                .min(FLAT_HOTBAR_SLOT_COUNT.saturating_sub(1)),
            icons: hud.hotbar.icons,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FlatHudStatusState {
    scale: GuiScale,
    status: crate::StatusOverlay,
}

impl FlatHudStatusState {
    fn from_hud(scale: GuiScale, hud: &FlatHud) -> Option<Self> {
        (hud.status.visible && !hud.status.message.is_empty()).then(|| Self {
            scale,
            status: hud.status.clone(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FlatHudPromptState {
    scale: GuiScale,
    touch: crate::TouchOverlay,
    gamepad: crate::GamepadHudOverlay,
    flat_hotbar_visible: bool,
}

impl FlatHudPromptState {
    fn from_hud(scale: GuiScale, hud: &FlatHud) -> Option<Self> {
        let touch = hud.effective_touch_overlay();
        let mut gamepad = hud.effective_gamepad_overlay();
        let gamepad_draws =
            gamepad.visible && (gamepad.hotbar_hints_visible || gamepad.action_hints_visible);
        if !gamepad_draws {
            gamepad = crate::GamepadHudOverlay::hidden();
        }
        if !touch.visible && !gamepad_draws {
            return None;
        }
        Some(Self {
            scale,
            touch,
            gamepad,
            flat_hotbar_visible: hud.should_render_flat_hotbar(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FlatHudDebugState {
    scale: GuiScale,
    debug: crate::FlatHudDebugOverlay,
}

impl FlatHudDebugState {
    fn from_hud(scale: GuiScale, hud: &FlatHud) -> Option<Self> {
        hud.debug
            .as_ref()
            .filter(|debug| debug.visible())
            .cloned()
            .map(|debug| Self { scale, debug })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FlatHudFramePipelineState {
    scale: GuiScale,
    overlay: crate::FramePipelineHudOverlay,
}

impl FlatHudFramePipelineState {
    fn from_hud(scale: GuiScale, hud: &FlatHud) -> Option<Self> {
        hud.frame_pipeline
            .as_ref()
            .filter(|overlay| overlay.visible())
            .cloned()
            .map(|overlay| Self { scale, overlay })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CachedFlatHudRetainedLayer {
    state: FlatHudRetainedState,
    draw: GuiDrawList,
}

#[derive(Clone, Debug, PartialEq)]
struct CachedFlatHudHotbarLayer {
    state: FlatHudHotbarState,
    draw: GuiDrawList,
}

#[derive(Clone, Debug, PartialEq)]
struct CachedFlatHudStatusLayer {
    state: FlatHudStatusState,
    draw: GuiDrawList,
}

#[derive(Clone, Debug, PartialEq)]
struct CachedFlatHudPromptLayer {
    state: FlatHudPromptState,
    draw: GuiDrawList,
}

#[derive(Clone, Debug, PartialEq)]
struct CachedFlatHudDebugLayer {
    state: FlatHudDebugState,
    draw: GuiDrawList,
}

#[derive(Clone, Debug, PartialEq)]
struct CachedFlatHudFramePipelineLayer {
    state: FlatHudFramePipelineState,
    draw: GuiDrawList,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FlatHudSurface {
    retained: Option<CachedFlatHudRetainedLayer>,
    hotbar: Option<CachedFlatHudHotbarLayer>,
    status: Option<CachedFlatHudStatusLayer>,
    prompt: Option<CachedFlatHudPromptLayer>,
    debug: Option<CachedFlatHudDebugLayer>,
    frame_pipeline: Option<CachedFlatHudFramePipelineLayer>,
}

impl FlatHudSurface {
    fn render_retained_layer(&mut self, scale: GuiScale, hud: &FlatHud) -> FlatHudDrawList {
        let state = FlatHudRetainedState::from_hud(scale, hud);
        if let Some(cached) = &self.retained {
            if cached.state == state {
                return FlatHudDrawList {
                    draw: cached.draw.clone(),
                    retained_cache: UiDrawCacheStats::cache_hit(),
                };
            }
        }

        let mut draw = GuiDrawList::new();
        render_flat_hud_retained_layer(scale, &mut draw, hud);
        self.retained = Some(CachedFlatHudRetainedLayer {
            state,
            draw: draw.clone(),
        });
        FlatHudDrawList {
            draw,
            retained_cache: UiDrawCacheStats::rebuild(),
        }
    }

    fn render_hotbar_layer(&mut self, scale: GuiScale, hud: &FlatHud) -> FlatHudDrawList {
        let Some(state) = FlatHudHotbarState::from_hud(scale, hud) else {
            return FlatHudDrawList {
                draw: GuiDrawList::new(),
                retained_cache: UiDrawCacheStats::default(),
            };
        };
        if let Some(cached) = &self.hotbar {
            if cached.state == state {
                return FlatHudDrawList {
                    draw: cached.draw.clone(),
                    retained_cache: UiDrawCacheStats::cache_hit(),
                };
            }
        }

        let mut draw = GuiDrawList::new();
        render_flat_hud_hotbar_layer(scale, &mut draw, hud);
        self.hotbar = Some(CachedFlatHudHotbarLayer {
            state,
            draw: draw.clone(),
        });
        FlatHudDrawList {
            draw,
            retained_cache: UiDrawCacheStats::rebuild(),
        }
    }

    fn render_status_layer(&mut self, scale: GuiScale, hud: &FlatHud) -> FlatHudDrawList {
        let Some(state) = FlatHudStatusState::from_hud(scale, hud) else {
            self.status = None;
            return FlatHudDrawList {
                draw: GuiDrawList::new(),
                retained_cache: UiDrawCacheStats::default(),
            };
        };
        if let Some(cached) = &self.status {
            if cached.state == state {
                return FlatHudDrawList {
                    draw: cached.draw.clone(),
                    retained_cache: UiDrawCacheStats::cache_hit(),
                };
            }
        }

        let mut draw = GuiDrawList::new();
        render_flat_hud_status_layer(scale, &mut draw, hud);
        self.status = Some(CachedFlatHudStatusLayer {
            state,
            draw: draw.clone(),
        });
        FlatHudDrawList {
            draw,
            retained_cache: UiDrawCacheStats::rebuild(),
        }
    }

    fn render_prompt_layer(&mut self, scale: GuiScale, hud: &FlatHud) -> FlatHudDrawList {
        let Some(state) = FlatHudPromptState::from_hud(scale, hud) else {
            self.prompt = None;
            return FlatHudDrawList {
                draw: GuiDrawList::new(),
                retained_cache: UiDrawCacheStats::default(),
            };
        };
        if let Some(cached) = &self.prompt {
            if cached.state == state {
                return FlatHudDrawList {
                    draw: cached.draw.clone(),
                    retained_cache: UiDrawCacheStats::cache_hit(),
                };
            }
        }

        let mut draw = GuiDrawList::new();
        render_flat_hud_prompt_layer(scale, &mut draw, hud);
        self.prompt = Some(CachedFlatHudPromptLayer {
            state,
            draw: draw.clone(),
        });
        FlatHudDrawList {
            draw,
            retained_cache: UiDrawCacheStats::rebuild(),
        }
    }

    fn render_debug_layer(&mut self, scale: GuiScale, hud: &FlatHud) -> FlatHudDrawList {
        let Some(state) = FlatHudDebugState::from_hud(scale, hud) else {
            self.debug = None;
            return FlatHudDrawList {
                draw: GuiDrawList::new(),
                retained_cache: UiDrawCacheStats::default(),
            };
        };
        if let Some(cached) = &self.debug {
            if cached.state == state {
                return FlatHudDrawList {
                    draw: cached.draw.clone(),
                    retained_cache: UiDrawCacheStats::cache_hit(),
                };
            }
        }

        let mut draw = GuiDrawList::new();
        render_flat_hud_debug_layer(scale, &mut draw, hud);
        self.debug = Some(CachedFlatHudDebugLayer {
            state,
            draw: draw.clone(),
        });
        FlatHudDrawList {
            draw,
            retained_cache: UiDrawCacheStats::rebuild(),
        }
    }

    fn render_frame_pipeline_layer(&mut self, scale: GuiScale, hud: &FlatHud) -> FlatHudDrawList {
        let Some(state) = FlatHudFramePipelineState::from_hud(scale, hud) else {
            self.frame_pipeline = None;
            return FlatHudDrawList {
                draw: GuiDrawList::new(),
                retained_cache: UiDrawCacheStats::default(),
            };
        };
        if let Some(cached) = &self.frame_pipeline {
            if cached.state == state {
                return FlatHudDrawList {
                    draw: cached.draw.clone(),
                    retained_cache: UiDrawCacheStats::cache_hit(),
                };
            }
        }

        let mut draw = GuiDrawList::new();
        render_flat_hud_frame_pipeline_layer(scale, &mut draw, hud);
        self.frame_pipeline = Some(CachedFlatHudFramePipelineLayer {
            state,
            draw: draw.clone(),
        });
        FlatHudDrawList {
            draw,
            retained_cache: UiDrawCacheStats::rebuild(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct LoadingProgressLayerState {
    scale: GuiScale,
    progress: LoadingProgressOverlay,
    layer: LoadingProgressOverlayLayer,
}

#[derive(Clone, Debug, PartialEq)]
struct CachedLoadingProgressLayer {
    state: LoadingProgressLayerState,
    draw: GuiDrawList,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct LoadingProgressSurface {
    fullscreen: Option<CachedLoadingProgressLayer>,
    panel: Option<CachedLoadingProgressLayer>,
}

impl LoadingProgressSurface {
    fn render_layer(
        &mut self,
        scale: GuiScale,
        progress: &LoadingProgressOverlay,
        layer: LoadingProgressOverlayLayer,
    ) -> LoadingProgressDrawList {
        let state = LoadingProgressLayerState {
            scale,
            progress: progress.clone(),
            layer,
        };
        let cached = match &state.layer {
            LoadingProgressOverlayLayer::Fullscreen => &mut self.fullscreen,
            LoadingProgressOverlayLayer::Panel { .. } => &mut self.panel,
        };
        if let Some(cached_layer) = cached {
            if cached_layer.state == state {
                return LoadingProgressDrawList {
                    draw: cached_layer.draw.clone(),
                    retained_cache: UiDrawCacheStats::cache_hit(),
                };
            }
        }

        let mut draw = GuiDrawList::new();
        match &state.layer {
            LoadingProgressOverlayLayer::Fullscreen => {
                render_loading_progress_overlay(scale, &mut draw, progress);
            }
            LoadingProgressOverlayLayer::Panel { origin, label } => {
                render_loading_progress_panel_at(scale, &mut draw, progress, *origin, label);
            }
        }
        *cached = Some(CachedLoadingProgressLayer {
            state,
            draw: draw.clone(),
        });
        LoadingProgressDrawList {
            draw,
            retained_cache: UiDrawCacheStats::rebuild(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameUiHost {
    screen: Option<GameScreen>,
    new_world_seed: i64,
    join_remote_addr: String,
    scale: GuiScale,
    surface: UiSurface,
    committed_render_state: GameUiRenderState,
    cached_v2_draw: Option<CachedV2DrawList>,
    hud_surface: FlatHudSurface,
    loading_progress_surface: LoadingProgressSurface,
}

impl Default for GameUiHost {
    fn default() -> Self {
        Self::new()
    }
}

impl GameUiHost {
    pub fn new() -> Self {
        Self::with_screen(Some(GameScreen::Title))
    }

    pub fn new_ingame() -> Self {
        Self::with_screen(None)
    }

    fn with_screen(screen: Option<GameScreen>) -> Self {
        let mut host = Self {
            screen,
            new_world_seed: 0,
            join_remote_addr: crate::DEFAULT_JOIN_REMOTE_ADDR.to_owned(),
            scale: GuiScale::from_pixels(1280, 900),
            surface: UiSurface::new(),
            committed_render_state: GameUiRenderState::default(),
            cached_v2_draw: None,
            hud_surface: FlatHudSurface::default(),
            loading_progress_surface: LoadingProgressSurface::default(),
        };
        host.surface.set_new_world_seed(host.new_world_seed);
        host.surface
            .set_join_remote_addr(host.join_remote_addr.clone());
        host.sync_surface_screen();
        host.surface.set_scale(host.scale);
        host
    }

    pub fn screen(&self) -> Option<GameScreen> {
        self.screen
    }

    pub fn new_world_seed(&self) -> i64 {
        self.new_world_seed
    }

    pub fn set_new_world_seed(&mut self, seed: i64) {
        self.new_world_seed = seed;
        self.surface.set_new_world_seed(seed);
    }

    pub fn join_remote_addr(&self) -> &str {
        &self.join_remote_addr
    }

    pub fn set_join_remote_addr(&mut self, addr: impl Into<String>) {
        let addr = addr.into();
        self.join_remote_addr = addr.clone();
        self.surface.set_join_remote_addr(addr);
    }

    pub fn scale(&self) -> GuiScale {
        self.scale
    }

    pub fn set_scale(&mut self, scale: GuiScale) {
        self.scale = scale;
        self.surface.set_scale(scale);
    }

    pub fn is_active(&self) -> bool {
        self.screen.is_some()
    }

    pub fn covers_world(&self) -> bool {
        matches!(
            self.screen,
            Some(
                GameScreen::Title
                    | GameScreen::WorldList
                    | GameScreen::WorldCreate
                    | GameScreen::WorldDeleteConfirm { .. }
                    | GameScreen::StorageConfirm { .. }
                    | GameScreen::NewWorld
                    | GameScreen::JoinRemote
            )
        ) || matches!(
            self.screen,
            Some(GameScreen::Help { parent }) if help_parent_covers_world(parent)
        ) || matches!(
            self.screen,
            Some(GameScreen::AssetPacks {
                parent: GameOptionsParent::Title
            })
        )
    }

    pub fn open_pause(&mut self) {
        if matches!(self.screen, Some(GameScreen::Death { .. })) {
            return;
        }
        self.screen = Some(GameScreen::Pause);
        self.sync_surface_screen();
    }

    pub fn close(&mut self) {
        self.screen = None;
        self.sync_surface_screen();
    }

    pub fn set_screen(&mut self, screen: Option<GameScreen>) {
        self.screen = screen;
        self.sync_surface_screen();
    }

    pub fn clear_input(&mut self) {
        self.surface.clear_input();
    }

    pub fn key_pressed(&mut self, key: GuiKey) -> (bool, Option<GameUiAction>) {
        if self.sync_surface_screen() {
            self.surface.key_pressed(key)
        } else {
            (false, None)
        }
    }

    pub fn navigate(&mut self, navigation: GuiNavigation) -> (bool, Option<GameUiAction>) {
        let state = self.committed_render_state;
        if self.sync_surface_screen() {
            self.surface.navigate(navigation, state)
        } else {
            (false, None)
        }
    }

    pub fn scroll_by(&mut self, amount: f32) -> bool {
        let state = self.committed_render_state;
        self.sync_surface_screen() && self.surface.scroll_by(amount, state)
    }

    pub fn pointer_move(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        let state = self.committed_render_state;
        if self.sync_surface_screen() {
            self.surface.pointer_move(point, state)
        } else {
            (false, None)
        }
    }

    pub fn pointer_down(&mut self, point: Point) -> bool {
        let state = self.committed_render_state;
        self.sync_surface_screen() && self.surface.pointer_down(point, state)
    }

    pub fn pointer_up(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        let state = self.committed_render_state;
        if self.sync_surface_screen() {
            self.surface.pointer_up(point, state)
        } else {
            (false, None)
        }
    }

    pub fn apply_action(&mut self, action: GameUiAction) {
        if matches!(self.screen, Some(GameScreen::Death { .. }))
            && !matches!(
                action,
                GameUiAction::Respawn | GameUiAction::QuitToTitle | GameUiAction::Quit
            )
        {
            return;
        }
        match action {
            GameUiAction::StartWorld
            | GameUiAction::Resume
            | GameUiAction::OpenWorld(_)
            | GameUiAction::CreateCatalogWorld
            | GameUiAction::AssignHotbarBlock { .. }
            | GameUiAction::AssignHotbarActor { .. } => self.screen = None,
            GameUiAction::EnterScenario(GameScenarioId::LobbyPreview) => {
                self.screen = Some(GameScreen::PreparingLobby)
            }
            GameUiAction::OpenWorldList | GameUiAction::CancelDeleteWorld => {
                self.screen = Some(GameScreen::WorldList)
            }
            GameUiAction::OpenWorldCreate => self.screen = Some(GameScreen::WorldCreate),
            GameUiAction::ConfirmDeleteWorld(id) => {
                self.screen = Some(GameScreen::WorldDeleteConfirm { id })
            }
            GameUiAction::DeleteWorld(_) => self.screen = Some(GameScreen::WorldList),
            GameUiAction::SelectWorld(_) => {}
            GameUiAction::OpenNewWorld => self.screen = Some(GameScreen::NewWorld),
            GameUiAction::OpenBlockPalette => self.screen = Some(GameScreen::BlockPalette),
            GameUiAction::OpenHelp(parent) => self.screen = Some(GameScreen::Help { parent }),
            GameUiAction::CloseHelp(parent) => self.screen = parent.screen(),
            GameUiAction::OpenJoinRemote => self.screen = Some(GameScreen::JoinRemote),
            GameUiAction::OpenOptions(parent) => {
                self.screen = Some(GameScreen::Options { parent });
            }
            GameUiAction::OpenOptionsCategory(parent, category) => {
                self.screen = Some(GameScreen::OptionsCategory { parent, category });
            }
            GameUiAction::OpenServerSettings(parent) => {
                self.screen = Some(GameScreen::ServerSettings { parent });
            }
            GameUiAction::OpenAssetPacks(parent) => {
                self.screen = Some(GameScreen::AssetPacks { parent });
            }
            GameUiAction::ConfirmStorageAction(parent, action) => {
                self.screen = Some(GameScreen::StorageConfirm { parent, action });
            }
            GameUiAction::ExecuteStorageAction(parent, _)
            | GameUiAction::CancelStorageAction(parent) => {
                self.screen = Some(GameScreen::OptionsCategory {
                    parent,
                    category: GameOptionsCategory::StorageProfile,
                });
            }
            GameUiAction::CancelAssetPacks => {
                if let Some(GameScreen::AssetPacks { parent }) = self.screen {
                    self.screen = Some(GameScreen::Options { parent });
                }
            }
            GameUiAction::BackToTitle | GameUiAction::QuitToTitle => {
                self.screen = Some(GameScreen::Title);
            }
            GameUiAction::BackToPause => self.screen = Some(GameScreen::Pause),
            GameUiAction::CreateWorld(_) | GameUiAction::JoinRemote => self.screen = None,
            GameUiAction::Respawn
            | GameUiAction::RerollSeed
            | GameUiAction::CycleWorldGenerationProfile => {}
            GameUiAction::ToggleSectionOcclusion
            | GameUiAction::SetLeafDetail(_)
            | GameUiAction::SetGrassDetail(_)
            | GameUiAction::SetTerrainPresentation(_)
            | GameUiAction::SetFogSettings(_)
            | GameUiAction::ToggleAssetPack(_)
            | GameUiAction::CycleTexturePresentation
            | GameUiAction::ApplyAssetPacks
            | GameUiAction::ClearRebuildableCache
            | GameUiAction::ToggleFullbright
            | GameUiAction::TogglePlayerCollisionBox
            | GameUiAction::ToggleFirstPersonPlayer
            | GameUiAction::ToggleCrosshair
            | GameUiAction::ToggleFramePipelineOverlay
            | GameUiAction::ToggleDebugDiagnostics
            | GameUiAction::SetAuxiliarySplitMode(_)
            | GameUiAction::SetLocalPlayLayout(_)
            | GameUiAction::ToggleLocalPlayGuest
            | GameUiAction::SetPlayerModel(_)
            | GameUiAction::SetMovementMode(_)
            | GameUiAction::SetCollisionMode(_)
            | GameUiAction::SetTravelAssistMode(_)
            | GameUiAction::SetTurnMode(_)
            | GameUiAction::SetXrTurnMode(_)
            | GameUiAction::CycleFramePacing
            | GameUiAction::CycleFpsCap
            | GameUiAction::SetWorldRenderScaleMode(_)
            | GameUiAction::SetXrRenderMode(_)
            | GameUiAction::SetRenderDistance(_)
            | GameUiAction::SetFlySpeed(_)
            | GameUiAction::SetMovementSpeed(_)
            | GameUiAction::SetTouchLookSensitivity(_)
            | GameUiAction::SetTouchControlsMode(_)
            | GameUiAction::SetServerSimulationCadence(_)
            | GameUiAction::Quit => {}
        }
        self.sync_surface_screen();
    }

    pub fn commit_render_state(&mut self, state: GameUiRenderState) {
        self.committed_render_state = state;
        if self.sync_surface_screen() {
            self.surface.set_render_state(state);
        }
    }

    pub fn committed_render_state(&self) -> GameUiRenderState {
        self.committed_render_state
    }

    pub fn v2_panel_revision(&mut self) -> Option<UiPanelRevision> {
        self.sync_surface_screen();
        self.surface.panel_revision()
    }

    pub fn render_draw_list(&mut self, state: GameUiRenderState) -> GuiDrawList {
        self.commit_render_state(state);
        if self.surface.is_active() {
            self.surface.render_draw_list(self.committed_render_state)
        } else {
            GuiDrawList::new()
        }
    }

    pub fn render_v2_panel_draw_list(
        &mut self,
        state: GameUiRenderState,
    ) -> Option<UiPanelDrawList> {
        self.commit_render_state(state);
        if !self.surface.is_active() {
            self.cached_v2_draw = None;
            return None;
        }
        let revision = self.surface.panel_revision()?;
        if let Some(cached) = &self.cached_v2_draw {
            if cached.revision == revision {
                return Some(UiPanelDrawList {
                    draw: cached.draw.clone(),
                    revision,
                    cache: UiDrawCacheStats::cache_hit(),
                });
            }
        }
        let draw = self.surface.render_draw_list(self.committed_render_state);
        let revision = self.surface.panel_revision()?;
        self.cached_v2_draw = Some(CachedV2DrawList {
            revision,
            draw: draw.clone(),
        });
        Some(UiPanelDrawList {
            draw,
            revision,
            cache: UiDrawCacheStats::rebuild(),
        })
    }

    pub fn render_flat_hud_draw_list(&mut self, scale: GuiScale, hud: &FlatHud) -> FlatHudDrawList {
        let retained = self.hud_surface.render_retained_layer(scale, hud);
        let hotbar = self.hud_surface.render_hotbar_layer(scale, hud);
        let status = self.hud_surface.render_status_layer(scale, hud);
        let prompt = self.hud_surface.render_prompt_layer(scale, hud);
        let debug = self.hud_surface.render_debug_layer(scale, hud);
        let frame_pipeline = self.hud_surface.render_frame_pipeline_layer(scale, hud);
        let mut draw = retained.draw.clone();
        draw.append(&hotbar.draw);
        draw.append(&status.draw);
        draw.append(&prompt.draw);
        draw.append(&debug.draw);
        draw.append(&frame_pipeline.draw);
        render_flat_hud_transient_layers(scale, &mut draw, hud);
        let mut retained_cache = retained.retained_cache;
        retained_cache.add(hotbar.retained_cache);
        retained_cache.add(status.retained_cache);
        retained_cache.add(prompt.retained_cache);
        retained_cache.add(debug.retained_cache);
        retained_cache.add(frame_pipeline.retained_cache);
        FlatHudDrawList {
            draw,
            retained_cache,
        }
    }

    pub fn append_flat_hud_draw(
        &mut self,
        scale: GuiScale,
        draw: &mut GuiDrawList,
        hud: &FlatHud,
    ) -> UiDrawCacheStats {
        let hud_draw = self.render_flat_hud_draw_list(scale, hud);
        draw.append(&hud_draw.draw);
        hud_draw.retained_cache
    }

    pub fn render_loading_progress_draw_list(
        &mut self,
        scale: GuiScale,
        progress: &LoadingProgressOverlay,
        layer: LoadingProgressOverlayLayer,
    ) -> LoadingProgressDrawList {
        self.loading_progress_surface
            .render_layer(scale, progress, layer)
    }

    pub fn append_loading_progress_draw(
        &mut self,
        scale: GuiScale,
        draw: &mut GuiDrawList,
        progress: &LoadingProgressOverlay,
        layer: LoadingProgressOverlayLayer,
    ) -> UiDrawCacheStats {
        let loading_draw = self.render_loading_progress_draw_list(scale, progress, layer);
        draw.append(&loading_draw.draw);
        loading_draw.retained_cache
    }

    pub fn set_v2_debug_overlay(&mut self, enabled: bool) {
        self.surface.set_debug_overlay(enabled);
    }

    pub fn v2_is_active(&self) -> bool {
        UiScreenId::from_game_screen(self.screen).is_some()
    }

    pub fn v2_debug_snapshot(&mut self) -> Option<UiDebugSnapshot> {
        self.sync_surface_screen();
        self.surface.debug_snapshot()
    }

    fn sync_surface_screen(&mut self) -> bool {
        let screen = UiScreenId::from_game_screen(self.screen);
        self.surface.set_screen(screen);
        screen.is_some()
    }
}

const UI_V2_TITLE_START: UiWidgetId = UiWidgetId(401);
const UI_V2_TITLE_JOIN_REMOTE: UiWidgetId = UiWidgetId(402);
const UI_V2_TITLE_OPTIONS: UiWidgetId = UiWidgetId(403);
const UI_V2_TITLE_QUIT: UiWidgetId = UiWidgetId(404);
const UI_V2_TITLE_ENTER_LOBBY: UiWidgetId = UiWidgetId(405);
const UI_V2_PREPARING_LOBBY_BACK: UiWidgetId = UiWidgetId(406);
const UI_V2_WORLD_LIST_OPEN: UiWidgetId = UiWidgetId(801);
const UI_V2_WORLD_LIST_CREATE: UiWidgetId = UiWidgetId(802);
const UI_V2_WORLD_LIST_DELETE: UiWidgetId = UiWidgetId(803);
const UI_V2_WORLD_LIST_BACK: UiWidgetId = UiWidgetId(804);
const UI_V2_WORLD_LIST_ROW_BASE: u64 = 820;
const UI_V2_WORLD_CREATE_REROLL: UiWidgetId = UiWidgetId(901);
const UI_V2_WORLD_CREATE_CREATE: UiWidgetId = UiWidgetId(902);
const UI_V2_WORLD_CREATE_BACK: UiWidgetId = UiWidgetId(903);
const UI_V2_WORLD_CREATE_PROFILE: UiWidgetId = UiWidgetId(904);
const UI_V2_WORLD_DELETE_CONFIRM: UiWidgetId = UiWidgetId(951);
const UI_V2_WORLD_DELETE_CANCEL: UiWidgetId = UiWidgetId(952);
const UI_V2_NEW_WORLD_REROLL: UiWidgetId = UiWidgetId(501);
const UI_V2_NEW_WORLD_CREATE: UiWidgetId = UiWidgetId(502);
const UI_V2_NEW_WORLD_BACK: UiWidgetId = UiWidgetId(503);
const UI_V2_JOIN_REMOTE_CONNECT: UiWidgetId = UiWidgetId(601);
const UI_V2_JOIN_REMOTE_BACK: UiWidgetId = UiWidgetId(602);
const UI_V2_PAUSE_RESUME: UiWidgetId = UiWidgetId(1);
const UI_V2_PAUSE_OPTIONS: UiWidgetId = UiWidgetId(2);
const UI_V2_PAUSE_QUIT_TO_TITLE: UiWidgetId = UiWidgetId(3);
const UI_V2_DEATH_RESPAWN: UiWidgetId = UiWidgetId(4);
const UI_V2_DEATH_QUIT_TO_TITLE: UiWidgetId = UiWidgetId(5);
const UI_V2_OPTIONS_OCCLUSION: UiWidgetId = UiWidgetId(101);
const UI_V2_OPTIONS_FULLBRIGHT: UiWidgetId = UiWidgetId(102);
const UI_V2_OPTIONS_PLAYER_BOX: UiWidgetId = UiWidgetId(105);
const UI_V2_OPTIONS_FIRST_PERSON_PLAYER: UiWidgetId = UiWidgetId(106);
const UI_V2_OPTIONS_CROSSHAIR: UiWidgetId = UiWidgetId(107);
const UI_V2_OPTIONS_PLAYER_MODEL: UiWidgetId = UiWidgetId(108);
const UI_V2_OPTIONS_MOVEMENT_MODE: UiWidgetId = UiWidgetId(109);
const UI_V2_OPTIONS_FRAME_PACING: UiWidgetId = UiWidgetId(110);
const UI_V2_OPTIONS_FPS_CAP: UiWidgetId = UiWidgetId(111);
const UI_V2_OPTIONS_RADIUS: UiWidgetId = UiWidgetId(112);
const UI_V2_OPTIONS_FLY_SPEED: UiWidgetId = UiWidgetId(113);
const UI_V2_OPTIONS_MOVEMENT_SPEED: UiWidgetId = UiWidgetId(114);
const UI_V2_OPTIONS_TOUCH_CONTROLS: UiWidgetId = UiWidgetId(115);
const UI_V2_OPTIONS_TOUCH_LOOK: UiWidgetId = UiWidgetId(116);
const UI_V2_OPTIONS_CONTROLS: UiWidgetId = UiWidgetId(117);
const UI_V2_OPTIONS_SERVER_SETTINGS: UiWidgetId = UiWidgetId(118);
const UI_V2_OPTIONS_BACK: UiWidgetId = UiWidgetId(119);
const UI_V2_OPTIONS_TURN_MODE: UiWidgetId = UiWidgetId(120);
const UI_V2_OPTIONS_FRAME_PIPELINE_OVERLAY: UiWidgetId = UiWidgetId(121);
const UI_V2_OPTIONS_COLLISION_MODE: UiWidgetId = UiWidgetId(122);
const UI_V2_OPTIONS_TRAVEL_ASSIST: UiWidgetId = UiWidgetId(123);
const UI_V2_OPTIONS_DEBUG_DIAGNOSTICS: UiWidgetId = UiWidgetId(124);
const UI_V2_OPTIONS_AUXILIARY_SPLIT: UiWidgetId = UiWidgetId(143);
const UI_V2_OPTIONS_LOCAL_PLAY_LAYOUT: UiWidgetId = UiWidgetId(148);
const UI_V2_OPTIONS_LOCAL_PLAY_PLAYER_ONE: UiWidgetId = UiWidgetId(149);
const UI_V2_OPTIONS_LOCAL_PLAY_GUEST: UiWidgetId = UiWidgetId(150);
const UI_V2_OPTIONS_LOCAL_PLAY_STATUS: UiWidgetId = UiWidgetId(151);
const UI_V2_OPTIONS_LOCAL_PLAY_ACCESS: UiWidgetId = UiWidgetId(153);
const UI_V2_OPTIONS_CAT_GRAPHICS: UiWidgetId = UiWidgetId(125);
const UI_V2_OPTIONS_CAT_MOVEMENT: UiWidgetId = UiWidgetId(126);
const UI_V2_OPTIONS_CAT_DISPLAY: UiWidgetId = UiWidgetId(127);
const UI_V2_OPTIONS_CAT_LOCAL_PLAY: UiWidgetId = UiWidgetId(152);
const UI_V2_OPTIONS_CAT_DEBUG: UiWidgetId = UiWidgetId(128);
const UI_V2_OPTIONS_ASSET_PACKS: UiWidgetId = UiWidgetId(129);
const UI_V2_OPTIONS_CAT_STORAGE: UiWidgetId = UiWidgetId(132);
const UI_V2_OPTIONS_OUTPUT_RESOLUTION: UiWidgetId = UiWidgetId(144);
const UI_V2_OPTIONS_WORLD_RESOLUTION: UiWidgetId = UiWidgetId(145);
const UI_V2_OPTIONS_WORLD_RENDER_SCALE: UiWidgetId = UiWidgetId(146);
const UI_V2_OPTIONS_LEAF_DETAIL: UiWidgetId = UiWidgetId(147);
const UI_V2_OPTIONS_GRASS_DETAIL: UiWidgetId = UiWidgetId(154);
const UI_V2_OPTIONS_TERRAIN_PRESENTATION: UiWidgetId = UiWidgetId(155);
const UI_V2_OPTIONS_FOG_SUBMENU: UiWidgetId = UiWidgetId(156);
const UI_V2_FOG_MODE: UiWidgetId = UiWidgetId(157);
const UI_V2_FOG_VISIBILITY: UiWidgetId = UiWidgetId(158);
const UI_V2_FOG_CLASSIC_START: UiWidgetId = UiWidgetId(159);
const UI_V2_FOG_COVERAGE_GUARD: UiWidgetId = UiWidgetId(160);
const UI_V2_FOG_GUARD_START: UiWidgetId = UiWidgetId(161);
const UI_V2_FOG_GROUND_BASE: UiWidgetId = UiWidgetId(162);
const UI_V2_FOG_GROUND_FALLOFF: UiWidgetId = UiWidgetId(163);
const UI_V2_FOG_MAX_OPACITY: UiWidgetId = UiWidgetId(164);
const UI_V2_FOG_EXPONENTIAL_SQUARED: UiWidgetId = UiWidgetId(165);
const UI_V2_FOG_FAR_CULL: UiWidgetId = UiWidgetId(166);
const UI_V2_FOG_WEATHER: UiWidgetId = UiWidgetId(167);
const UI_V2_FOG_COLOR_MODE: UiWidgetId = UiWidgetId(168);
const UI_V2_FOG_COLOR_RED: UiWidgetId = UiWidgetId(169);
const UI_V2_FOG_COLOR_GREEN: UiWidgetId = UiWidgetId(170);
const UI_V2_FOG_COLOR_BLUE: UiWidgetId = UiWidgetId(171);
const UI_V2_OPTIONS_XR_RENDER_PATH: UiWidgetId = UiWidgetId(172);
const UI_V2_STORAGE_PROFILE_NAME: UiWidgetId = UiWidgetId(133);
const UI_V2_STORAGE_PROFILE_ID: UiWidgetId = UiWidgetId(134);
const UI_V2_STORAGE_BACKEND: UiWidgetId = UiWidgetId(135);
const UI_V2_STORAGE_WORLD_COUNT: UiWidgetId = UiWidgetId(136);
const UI_V2_STORAGE_CLEAR_CACHE: UiWidgetId = UiWidgetId(137);
const UI_V2_STORAGE_RESET_IDENTITY: UiWidgetId = UiWidgetId(138);
const UI_V2_STORAGE_DELETE_ALL_WORLDS: UiWidgetId = UiWidgetId(139);
const UI_V2_STORAGE_FACTORY_RESET: UiWidgetId = UiWidgetId(140);
const UI_V2_STORAGE_CONFIRM: UiWidgetId = UiWidgetId(141);
const UI_V2_STORAGE_CANCEL: UiWidgetId = UiWidgetId(142);
const UI_V2_ASSET_PACK_ROW_BASE: u64 = 1300;
const UI_V2_ASSET_PACK_CANCEL: UiWidgetId = UiWidgetId(1310);
const UI_V2_ASSET_PACK_APPLY: UiWidgetId = UiWidgetId(1311);
const UI_V2_SERVER_SETTINGS_HOST_RATE: UiWidgetId = UiWidgetId(701);
const UI_V2_SERVER_SETTINGS_GAMEPLAY_RATE: UiWidgetId = UiWidgetId(702);
const UI_V2_SERVER_SETTINGS_PHYSICS_RATE: UiWidgetId = UiWidgetId(703);
const UI_V2_SERVER_SETTINGS_BACK: UiWidgetId = UiWidgetId(704);
const UI_V2_HELP_BACK: UiWidgetId = UiWidgetId(201);
const UI_V2_BLOCK_PALETTE_BASE: u64 = 3000;

fn title_layout(scale: GuiScale, revision: u64, lobby_scenario_available: bool) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::Title), revision);
    let y = scale.height * 0.5 - 46.0;
    layout.push(
        UiWidget::button(
            UI_V2_TITLE_ENTER_LOBBY,
            menu_button_rect(scale, y),
            if lobby_scenario_available {
                "Enter Lobby"
            } else {
                "Enter Lobby (Unavailable)"
            },
        )
        .enabled(lobby_scenario_available)
        .action(GameUiAction::EnterScenario(GameScenarioId::LobbyPreview)),
    );
    layout.push(
        UiWidget::button(
            UI_V2_TITLE_START,
            menu_button_rect(scale, y + 24.0),
            "Singleplayer",
        )
        .action(GameUiAction::OpenWorldList),
    );
    layout.push(
        UiWidget::button(
            UI_V2_TITLE_JOIN_REMOTE,
            menu_button_rect(scale, y + 48.0),
            "Join Remote",
        )
        .action(GameUiAction::OpenJoinRemote),
    );
    layout.push(
        UiWidget::button(
            UI_V2_TITLE_OPTIONS,
            menu_button_rect(scale, y + 72.0),
            "Options",
        )
        .action(GameUiAction::OpenOptions(GameOptionsParent::Title)),
    );
    layout.push(
        UiWidget::button(UI_V2_TITLE_QUIT, menu_button_rect(scale, y + 96.0), "Quit")
            .action(GameUiAction::Quit),
    );
    layout
}

fn preparing_lobby_layout(scale: GuiScale, revision: u64) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::PreparingLobby), revision);
    layout.push(
        UiWidget::button(
            UI_V2_PREPARING_LOBBY_BACK,
            menu_button_rect(scale, scale.height * 0.5 + 32.0),
            "Back",
        )
        .action(GameUiAction::BackToTitle),
    );
    layout
}

fn world_list_row_id(index: usize) -> UiWidgetId {
    UiWidgetId(UI_V2_WORLD_LIST_ROW_BASE + index as u64)
}

fn world_list_layout(scale: GuiScale, revision: u64, catalog: WorldCatalogUiState) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::WorldList), revision);
    let panel = world_list_panel_rect(scale);
    let row_rects = world_list_row_rects(panel);
    for (index, entry) in catalog.entries.iter().flatten().copied().enumerate() {
        let Some(rect) = row_rects.get(index).copied() else {
            break;
        };
        layout.push(
            UiWidget::world_row(
                world_list_row_id(index),
                rect,
                world_list_row_label(entry),
                world_list_row_value(entry, catalog.active),
                catalog.selected == Some(entry.id),
                entry.locked,
                entry.compatible,
            )
            .action(GameUiAction::SelectWorld(entry.id)),
        );
    }

    let selected = catalog.selected_entry().map(|entry| entry.id);
    let open_enabled = selected.is_some_and(|id| catalog.can_open_world(id));
    let delete_enabled = selected.is_some_and(|id| catalog.can_delete_world(id));
    let open_action = selected.map_or(GameUiAction::OpenWorld(WorldCatalogUiWorldId(0)), |id| {
        GameUiAction::OpenWorld(id)
    });
    let delete_action = selected.map_or(
        GameUiAction::ConfirmDeleteWorld(WorldCatalogUiWorldId(0)),
        |id| GameUiAction::ConfirmDeleteWorld(id),
    );
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_LIST_OPEN,
            world_list_footer_button_rect(panel, 0),
            "Open",
        )
        .enabled(open_enabled)
        .action(open_action),
    );
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_LIST_CREATE,
            world_list_footer_button_rect(panel, 1),
            "Create",
        )
        .enabled(catalog.create_supported)
        .action(GameUiAction::OpenWorldCreate),
    );
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_LIST_DELETE,
            world_list_footer_button_rect(panel, 2),
            "Delete",
        )
        .enabled(delete_enabled)
        .action(delete_action),
    );
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_LIST_BACK,
            world_list_footer_button_rect(panel, 3),
            "Back",
        )
        .action(GameUiAction::BackToTitle),
    );
    layout
}

fn world_create_layout(scale: GuiScale, revision: u64, catalog: WorldCatalogUiState) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::WorldCreate), revision);
    let panel = world_create_panel_rect(scale);
    let y = panel.y + 91.0;
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_CREATE_REROLL,
            menu_button_rect_at(panel.center_x(), y),
            "Reroll Seed",
        )
        .action(GameUiAction::RerollSeed),
    );
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_CREATE_PROFILE,
            menu_button_rect_at(panel.center_x(), y + 24.0),
            format!("World: {}", catalog.create_generation_profile.as_str()),
        )
        .action(GameUiAction::CycleWorldGenerationProfile),
    );
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_CREATE_CREATE,
            menu_button_rect_at(panel.center_x(), y + 48.0),
            "Create World",
        )
        .enabled(catalog.create_supported)
        .action(GameUiAction::CreateCatalogWorld),
    );
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_CREATE_BACK,
            menu_button_rect_at(panel.center_x(), y + 72.0),
            "Back",
        )
        .action(GameUiAction::OpenWorldList),
    );
    layout
}

fn world_delete_confirm_layout(
    scale: GuiScale,
    revision: u64,
    id: WorldCatalogUiWorldId,
    catalog: WorldCatalogUiState,
) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::WorldDeleteConfirm { id }), revision);
    let panel = world_delete_confirm_panel_rect(scale);
    let y = panel.y + 89.0;
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_DELETE_CONFIRM,
            menu_button_rect_at(panel.center_x(), y),
            "Delete",
        )
        .enabled(catalog.can_delete_world(id))
        .action(GameUiAction::DeleteWorld(id)),
    );
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_DELETE_CANCEL,
            menu_button_rect_at(panel.center_x(), y + 24.0),
            "Cancel",
        )
        .action(GameUiAction::CancelDeleteWorld),
    );
    layout
}

fn storage_confirm_layout(
    scale: GuiScale,
    revision: u64,
    parent: GameOptionsParent,
    action: GameStorageAction,
    state: GameUiRenderState,
) -> UiLayout {
    let mut layout = UiLayout::new(
        Some(UiScreenId::StorageConfirm { parent, action }),
        revision,
    );
    let panel = storage_confirm_panel_rect(scale);
    let world_count = state.world_catalog.entry_count();
    let enabled = match action {
        GameStorageAction::ResetPlayerIdentity => state.storage_profile.profile_actions_available,
        GameStorageAction::DeleteAllLocalWorlds => {
            state.world_catalog.delete_supported && world_count > 0
        }
        GameStorageAction::FactoryReset => {
            state.storage_profile.factory_reset_available && state.world_catalog.delete_supported
        }
    } && parent == GameOptionsParent::Title
        && state.world_catalog.active.is_none()
        && !state.world_catalog.loading;
    let y = panel.y + 89.0;
    layout.push(
        UiWidget::button(
            UI_V2_STORAGE_CONFIRM,
            menu_button_rect_at(panel.center_x(), y),
            action.confirm_label(),
        )
        .enabled(enabled)
        .action(GameUiAction::ExecuteStorageAction(parent, action)),
    );
    layout.push(
        UiWidget::button(
            UI_V2_STORAGE_CANCEL,
            menu_button_rect_at(panel.center_x(), y + 24.0),
            "Cancel",
        )
        .action(GameUiAction::CancelStorageAction(parent)),
    );
    layout
}

fn new_world_layout(scale: GuiScale, revision: u64) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::NewWorld), revision);
    let y = scale.height * 0.5 - 4.0;
    layout.push(
        UiWidget::button(UI_V2_NEW_WORLD_REROLL, menu_button_rect(scale, y), "Reroll")
            .action(GameUiAction::RerollSeed),
    );
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_CREATE_PROFILE,
            menu_button_rect(scale, y + 24.0),
            "Cycle World Type",
        )
        .action(GameUiAction::CycleWorldGenerationProfile),
    );
    layout.push(
        UiWidget::button(
            UI_V2_NEW_WORLD_CREATE,
            menu_button_rect(scale, y + 48.0),
            "Create World",
        )
        .action(GameUiAction::CreateWorld(0)),
    );
    layout.push(
        UiWidget::button(
            UI_V2_NEW_WORLD_BACK,
            menu_button_rect(scale, y + 72.0),
            "Back",
        )
        .action(GameUiAction::BackToTitle),
    );
    layout
}

fn join_remote_layout(scale: GuiScale, revision: u64) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::JoinRemote), revision);
    let y = scale.height * 0.5 + 20.0;
    layout.push(
        UiWidget::button(
            UI_V2_JOIN_REMOTE_CONNECT,
            menu_button_rect(scale, y),
            "Connect",
        )
        .action(GameUiAction::JoinRemote),
    );
    layout.push(
        UiWidget::button(
            UI_V2_JOIN_REMOTE_BACK,
            menu_button_rect(scale, y + 24.0),
            "Back",
        )
        .action(GameUiAction::BackToTitle),
    );
    layout
}

fn pause_layout(scale: GuiScale, revision: u64) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::Pause), revision);
    let y = scale.height * 0.5 - 22.0;
    layout.push(
        UiWidget::button(
            UI_V2_PAUSE_RESUME,
            menu_button_rect(scale, y),
            "Back To Game",
        )
        .action(GameUiAction::Resume),
    );
    layout.push(
        UiWidget::button(
            UI_V2_PAUSE_OPTIONS,
            menu_button_rect(scale, y + 24.0),
            "Options",
        )
        .action(GameUiAction::OpenOptions(GameOptionsParent::Pause)),
    );
    layout.push(
        UiWidget::button(
            UI_V2_PAUSE_QUIT_TO_TITLE,
            menu_button_rect(scale, y + 48.0),
            "Quit To Title",
        )
        .action(GameUiAction::QuitToTitle),
    );
    layout
}

fn death_layout(scale: GuiScale, revision: u64, cause: GameDeathCause) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::Death { cause }), revision);
    let y = scale.height * 0.5 + 20.0;
    layout.push(
        UiWidget::button(UI_V2_DEATH_RESPAWN, menu_button_rect(scale, y), "Respawn")
            .action(GameUiAction::Respawn),
    );
    layout.push(
        UiWidget::button(
            UI_V2_DEATH_QUIT_TO_TITLE,
            menu_button_rect(scale, y + 24.0),
            "Quit To Title",
        )
        .action(GameUiAction::QuitToTitle),
    );
    layout
}

fn menu_button_rect(scale: GuiScale, y: f32) -> Rect {
    menu_button_rect_at(scale.width * 0.5, y)
}

fn menu_button_rect_at(center_x: f32, y: f32) -> Rect {
    Rect::new(center_x - 90.0, y, 180.0, 20.0)
}

fn world_list_panel_rect(scale: GuiScale) -> Rect {
    centered_panel(scale, 420.0, 286.0)
}

fn world_create_panel_rect(scale: GuiScale) -> Rect {
    centered_panel(scale, 320.0, 202.0)
}

fn world_delete_confirm_panel_rect(scale: GuiScale) -> Rect {
    centered_panel(scale, 340.0, 158.0)
}

fn storage_confirm_panel_rect(scale: GuiScale) -> Rect {
    centered_panel(scale, 430.0, 158.0)
}

fn world_list_row_rects(panel: Rect) -> [Rect; 8] {
    let row_x = panel.x + 14.0;
    let row_width = panel.width - 28.0;
    let first_y = panel.y + 44.0;
    std::array::from_fn(|index| Rect::new(row_x, first_y + index as f32 * 23.0, row_width, 20.0))
}

fn world_list_footer_button_rect(panel: Rect, index: usize) -> Rect {
    let button_width = 84.0;
    let gap = 8.0;
    let total = button_width * 4.0 + gap * 3.0;
    let x = panel.center_x() - total * 0.5 + index as f32 * (button_width + gap);
    Rect::new(x, panel.bottom() - 28.0, button_width, 20.0)
}

fn world_list_row_label(entry: WorldCatalogUiEntry) -> String {
    let name = entry.display_name.as_str();
    if entry.locked {
        format!("{name} [Locked]")
    } else if !entry.compatible {
        format!("{name} [Incompatible]")
    } else {
        name.to_owned()
    }
}

fn world_list_row_value(
    entry: WorldCatalogUiEntry,
    active: Option<WorldCatalogUiWorldId>,
) -> String {
    let generation_profile = entry.generation_profile.as_str();
    if active == Some(entry.id) {
        if generation_profile.is_empty() {
            "Active".to_owned()
        } else {
            format!("{generation_profile} · Active")
        }
    } else if generation_profile.is_empty() {
        format!("Seed {}", entry.seed)
    } else {
        format!("{generation_profile} · Seed {}", entry.seed)
    }
}

fn render_title_background(draw: &mut GuiDrawList, scale: GuiScale) {
    draw.fill_gradient(
        Rect::new(0.0, 0.0, scale.width, scale.height),
        Color::rgba(24, 44, 51, 255),
        Color::rgba(7, 10, 12, 255),
    );
    draw.fill(
        Rect::new(0.0, 0.0, scale.width, scale.height),
        Color::rgba(0, 0, 0, 55),
    );
}

fn block_palette_slot_id(index: usize) -> UiWidgetId {
    UiWidgetId(UI_V2_BLOCK_PALETTE_BASE + index as u64)
}

fn block_palette_layout(scale: GuiScale, revision: u64, overlay: BlockPaletteOverlay) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::BlockPalette), revision);
    if !overlay.visible {
        return layout;
    }

    for (index, entry) in overlay.entries.into_iter().enumerate() {
        let Some(entry) = entry else {
            continue;
        };
        let Some(rect) = block_palette_slot_rect(scale, overlay, index) else {
            continue;
        };
        let action = match entry.item {
            DebugPaletteItem::Block(block_state) => GameUiAction::AssignHotbarBlock {
                slot: overlay.selected_hotbar_slot,
                block_state,
            },
            DebugPaletteItem::SpawnActor(actor) => GameUiAction::AssignHotbarActor {
                slot: overlay.selected_hotbar_slot,
                actor,
            },
        };
        layout.push(
            UiWidget::palette_slot(block_palette_slot_id(index), rect, entry.label, entry.icon)
                .action(action),
        );
    }
    layout
}

fn help_layout(
    scale: GuiScale,
    revision: u64,
    parent: GameHelpParent,
    scroll_offset: f32,
) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::Help { parent }), revision);
    let panel = help_panel_rect(scale);
    let font = Font::default();
    let mut rows = help_text_rows(panel, font.line_height());
    for row in &mut rows {
        row.y -= scroll_offset;
    }
    let content_height = rows
        .iter()
        .map(|row| row.y + font.line_height() + scroll_offset - (panel.y + 34.0))
        .fold(0.0, f32::max);
    layout.set_help_rows(rows);
    layout.set_scroll_region(UiScrollRegion {
        clip: Rect::new(
            panel.x + 8.0,
            panel.y + 32.0,
            panel.width - 16.0,
            (panel.height - 62.0).max(1.0),
        ),
        first_widget: 0,
        widget_count: 0,
        content_height,
        offset: scroll_offset,
    });
    layout.push(
        UiWidget::button(UI_V2_HELP_BACK, help_back_rect(scale), "Back")
            .action(GameUiAction::CloseHelp(parent)),
    );
    layout
}

fn help_panel_rect(scale: GuiScale) -> Rect {
    centered_panel(scale, 432.0, 264.0)
}

fn help_back_rect(scale: GuiScale) -> Rect {
    let panel = help_panel_rect(scale);
    Rect::new(panel.center_x() - 45.0, panel.bottom() - 25.0, 90.0, 20.0)
}

fn help_text_rows(panel: Rect, line_height: f32) -> Vec<UiHelpRow> {
    let rows = controls_help_rows();
    let split = (rows.len() + 1) / 2;
    let column_gap = 10.0;
    let column_width = (panel.width - 24.0 - column_gap) * 0.5;
    let left_x = panel.x + 12.0;
    let right_x = left_x + column_width + column_gap;
    let y = panel.y + 34.0;
    let mut output = Vec::with_capacity(rows.len());
    push_help_column_rows(
        &mut output,
        left_x,
        y,
        column_width,
        line_height,
        &rows[..split],
    );
    push_help_column_rows(
        &mut output,
        right_x,
        y,
        column_width,
        line_height,
        &rows[split..],
    );
    output
}

fn push_help_column_rows(
    output: &mut Vec<UiHelpRow>,
    x: f32,
    y: f32,
    width: f32,
    line_height: f32,
    rows: &[UiHelpRowKind],
) {
    let control_width = 72.0_f32.min(width * 0.45);
    let mut row_y = y;
    for row in rows {
        output.push(UiHelpRow {
            x,
            y: row_y,
            control_width,
            kind: row.clone(),
        });
        row_y += match row {
            UiHelpRowKind::Group(_) => line_height + 2.0,
            UiHelpRowKind::Shortcut(_) => line_height,
        };
    }
}

fn controls_help_rows() -> Vec<UiHelpRowKind> {
    let mut rows = Vec::new();
    rows.push(UiHelpRowKind::Group(ShortcutHelpGroup::KeyboardMouse));
    rows.extend(
        default_keyboard_mouse_shortcut_rows()
            .into_iter()
            .map(UiHelpRowKind::Shortcut),
    );
    rows.push(UiHelpRowKind::Group(ShortcutHelpGroup::RuntimeDebug));
    rows.extend(
        flat_runtime_shortcut_rows()
            .into_iter()
            .map(UiHelpRowKind::Shortcut),
    );
    rows
}

const fn help_parent_covers_world(parent: GameHelpParent) -> bool {
    matches!(
        parent,
        GameHelpParent::Title
            | GameHelpParent::WorldList
            | GameHelpParent::WorldCreate
            | GameHelpParent::WorldDeleteConfirm
            | GameHelpParent::NewWorld
            | GameHelpParent::JoinRemote
    )
}

/// Widget id for the hub button that opens a category sub-panel.
const fn options_category_widget_id(category: GameOptionsCategory) -> UiWidgetId {
    match category {
        GameOptionsCategory::Graphics => UI_V2_OPTIONS_CAT_GRAPHICS,
        GameOptionsCategory::Fog => UI_V2_OPTIONS_FOG_SUBMENU,
        GameOptionsCategory::Movement => UI_V2_OPTIONS_CAT_MOVEMENT,
        GameOptionsCategory::Display => UI_V2_OPTIONS_CAT_DISPLAY,
        GameOptionsCategory::LocalPlay => UI_V2_OPTIONS_CAT_LOCAL_PLAY,
        GameOptionsCategory::Debug => UI_V2_OPTIONS_CAT_DEBUG,
        GameOptionsCategory::StorageProfile => UI_V2_OPTIONS_CAT_STORAGE,
    }
}

/// Maximum number of setting rows a category renders.
///
/// Most platform-shaped settings stay visible but disabled. The XR render path
/// is intentionally omitted on flat clients, so Graphics may use one fewer row.
const fn options_category_row_count(category: GameOptionsCategory) -> usize {
    match category {
        GameOptionsCategory::Graphics => 13,
        GameOptionsCategory::Fog => 15,
        GameOptionsCategory::Movement => 8,
        GameOptionsCategory::Display => 3,
        GameOptionsCategory::LocalPlay => 5,
        GameOptionsCategory::Debug => 5,
        GameOptionsCategory::StorageProfile => 8,
    }
}

/// The Options hub: a short list of category buttons plus the shared
/// Server Settings / Controls Help / Back navigation. Individual settings live
/// on the per-category sub-panels reached from here.
fn options_layout(
    scale: GuiScale,
    revision: u64,
    parent: GameOptionsParent,
    state: GameUiRenderState,
) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::Options { parent }), revision);
    let panel = options_panel_rect(scale, state);
    let width = (panel.width - 36.0).max(150.0);
    let x = panel.x + 18.0;
    let mut y = panel.y + 30.0;

    for category in GameOptionsCategory::ALL {
        layout.push(
            UiWidget::button(
                options_category_widget_id(category),
                Rect::new(x, y, width, 20.0),
                category.label(),
            )
            .action(GameUiAction::OpenOptionsCategory(parent, category)),
        );
        y += 24.0;
    }

    layout.push(
        UiWidget::button(
            UI_V2_OPTIONS_ASSET_PACKS,
            Rect::new(x, y, width, 20.0),
            "Visual Profiles",
        )
        .action(GameUiAction::OpenAssetPacks(parent)),
    );
    y += 24.0;

    // Server Settings stays a first-class sub-panel; shown disabled when the
    // local server is authoritative so its availability is visible per platform.
    layout.push(
        UiWidget::button(
            UI_V2_OPTIONS_SERVER_SETTINGS,
            Rect::new(x, y, width, 20.0),
            "Server Settings",
        )
        .enabled(state.server_cadence.is_some())
        .action(GameUiAction::OpenServerSettings(parent)),
    );
    y += 24.0;
    layout.push(
        UiWidget::button(
            UI_V2_OPTIONS_CONTROLS,
            Rect::new(x, y, width, 20.0),
            "Controls Help",
        )
        .action(GameUiAction::OpenHelp(help_parent_for_options(parent))),
    );
    y += 28.0;
    layout.push(
        UiWidget::button(
            UI_V2_OPTIONS_BACK,
            Rect::new(x, y, width, 20.0),
            match parent {
                GameOptionsParent::Title => "Back",
                GameOptionsParent::Pause => "Done",
            },
        )
        .action(match parent {
            GameOptionsParent::Title => GameUiAction::BackToTitle,
            GameOptionsParent::Pause => GameUiAction::BackToPause,
        }),
    );

    layout
}

/// Build the setting rows for a category as `(row_height, widget)` pairs. Rows
/// whose backing platform state is absent are emitted disabled (with an "N/A"
/// value and no action) so every setting is discoverable while developing.
fn options_category_rows(
    parent: GameOptionsParent,
    category: GameOptionsCategory,
    state: GameUiRenderState,
) -> Vec<(f32, UiWidget)> {
    let ph = Rect::new(0.0, 0.0, 0.0, 0.0);
    match category {
        GameOptionsCategory::Graphics => {
            let mut rows = vec![
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_OUTPUT_RESOLUTION,
                        ph,
                        "Output Resolution",
                        state
                            .flat_presentation
                            .map(GameFlatPresentationState::output_size_label)
                            .unwrap_or_else(|| "N/A".to_owned()),
                    )
                    .enabled(false),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_WORLD_RESOLUTION,
                        ph,
                        "World Resolution",
                        state
                            .flat_presentation
                            .map(GameFlatPresentationState::world_size_label)
                            .unwrap_or_else(|| "N/A".to_owned()),
                    )
                    .enabled(false),
                ),
                (
                    20.0,
                    state.flat_presentation.map_or_else(
                        || {
                            UiWidget::cycle(
                                UI_V2_OPTIONS_WORLD_RENDER_SCALE,
                                ph,
                                "World Scale",
                                "N/A",
                            )
                            .enabled(false)
                        },
                        |presentation| {
                            UiWidget::cycle(
                                UI_V2_OPTIONS_WORLD_RENDER_SCALE,
                                ph,
                                "World Scale",
                                presentation
                                    .world_render_scale_mode
                                    .label(presentation.world_render_scale),
                            )
                            .action(
                                GameUiAction::SetWorldRenderScaleMode(
                                    presentation.world_render_scale_mode.next(),
                                ),
                            )
                        },
                    ),
                ),
                (
                    18.0,
                    UiWidget::checkbox(
                        UI_V2_OPTIONS_OCCLUSION,
                        ph,
                        "Section Occlusion",
                        state.section_occlusion_culling,
                    )
                    .action(GameUiAction::ToggleSectionOcclusion),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_LEAF_DETAIL,
                        ph,
                        "Leaf Detail",
                        state.leaf_detail.label(),
                    )
                    .action(GameUiAction::SetLeafDetail(state.leaf_detail.next())),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_GRASS_DETAIL,
                        ph,
                        "Grass Detail",
                        state.grass_detail.label(),
                    )
                    .action(GameUiAction::SetGrassDetail(state.grass_detail.next())),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_TERRAIN_PRESENTATION,
                        ph,
                        "Distant Terrain",
                        if state.terrain_presentation
                            == crate::GameTerrainPresentation::Experimental
                            && !state.terrain_presentation_available
                        {
                            "Experimental (Unavailable)"
                        } else {
                            state.terrain_presentation.label()
                        },
                    )
                    .action(GameUiAction::SetTerrainPresentation(
                        state.terrain_presentation.next(),
                    )),
                ),
                (
                    20.0,
                    UiWidget::button(UI_V2_OPTIONS_FOG_SUBMENU, ph, "Fog...").action(
                        GameUiAction::OpenOptionsCategory(parent, GameOptionsCategory::Fog),
                    ),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_OPTIONS_RADIUS,
                        ph,
                        render_distance_label(state),
                        render_distance_slider_value(state),
                    )
                    .slider_action(UiSliderAction::RenderDistance),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_FRAME_PACING,
                        ph,
                        "Frame Pacing",
                        state.frame_pacing_mode.label(),
                    )
                    .action(GameUiAction::CycleFramePacing),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_FPS_CAP,
                        ph,
                        "FPS Cap",
                        state.fps_cap.to_string(),
                    )
                    .action(GameUiAction::CycleFpsCap),
                ),
            ];
            if let Some(xr_render_path) = state.xr_render_path {
                rows.push((
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_XR_RENDER_PATH,
                        ph,
                        "XR Render Path",
                        xr_render_path.value_label(),
                    )
                    .enabled(xr_render_path.supported_modes != crate::GameXrRenderModeSet::NONE)
                    .action(GameUiAction::SetXrRenderMode(xr_render_path.next_mode())),
                ));
            }
            rows
        }
        GameOptionsCategory::Fog => {
            let fog = state.fog.normalized();
            let exponential = fog.mode.uses_exponential();
            let ground_haze = fog.mode == crate::GameFogMode::GroundHaze;
            let custom_color = fog.color_mode == crate::GameFogColorMode::Custom;
            vec![
                (
                    20.0,
                    UiWidget::cycle(UI_V2_FOG_MODE, ph, "Mode", fog.mode.label()).action(
                        GameUiAction::SetFogSettings(crate::GameFogSettings {
                            mode: fog.mode.next(),
                            ..fog
                        }),
                    ),
                ),
                (
                    20.0,
                    UiWidget::cycle(UI_V2_FOG_COLOR_MODE, ph, "Color", fog.color_mode.label())
                        .enabled(fog.mode != crate::GameFogMode::Off)
                        .action(GameUiAction::SetFogSettings(crate::GameFogSettings {
                            color_mode: fog.color_mode.next(),
                            ..fog
                        })),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_FOG_COLOR_RED,
                        ph,
                        fog_color_component_label(fog, 0, "Red"),
                        fog_color_component_slider_value(fog, 0),
                    )
                    .enabled(fog.mode != crate::GameFogMode::Off && custom_color)
                    .slider_action(UiSliderAction::FogColorRed),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_FOG_COLOR_GREEN,
                        ph,
                        fog_color_component_label(fog, 1, "Green"),
                        fog_color_component_slider_value(fog, 1),
                    )
                    .enabled(fog.mode != crate::GameFogMode::Off && custom_color)
                    .slider_action(UiSliderAction::FogColorGreen),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_FOG_COLOR_BLUE,
                        ph,
                        fog_color_component_label(fog, 2, "Blue"),
                        fog_color_component_slider_value(fog, 2),
                    )
                    .enabled(fog.mode != crate::GameFogMode::Off && custom_color)
                    .slider_action(UiSliderAction::FogColorBlue),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_FOG_VISIBILITY,
                        ph,
                        fog_visibility_label(fog),
                        fog_visibility_slider_value(fog),
                    )
                    .enabled(fog.mode != crate::GameFogMode::Off)
                    .slider_action(UiSliderAction::FogVisibility),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_FOG_CLASSIC_START,
                        ph,
                        fog_classic_start_label(fog),
                        fog_classic_start_slider_value(fog),
                    )
                    .enabled(fog.mode == crate::GameFogMode::Classic)
                    .slider_action(UiSliderAction::FogClassicStart),
                ),
                (
                    18.0,
                    UiWidget::checkbox(
                        UI_V2_FOG_COVERAGE_GUARD,
                        ph,
                        "Coverage Guard",
                        fog.coverage_guard,
                    )
                    .enabled(fog.mode != crate::GameFogMode::Off)
                    .action(GameUiAction::SetFogSettings(
                        crate::GameFogSettings {
                            coverage_guard: !fog.coverage_guard,
                            ..fog
                        },
                    )),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_FOG_GUARD_START,
                        ph,
                        fog_guard_start_label(fog),
                        fog_guard_start_slider_value(fog),
                    )
                    .enabled(fog.mode != crate::GameFogMode::Off && fog.coverage_guard)
                    .slider_action(UiSliderAction::FogGuardStart),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_FOG_GROUND_BASE,
                        ph,
                        fog_ground_base_label(fog),
                        fog_ground_base_slider_value(fog),
                    )
                    .enabled(ground_haze)
                    .slider_action(UiSliderAction::FogGroundBase),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_FOG_GROUND_FALLOFF,
                        ph,
                        fog_ground_falloff_label(fog),
                        fog_ground_falloff_slider_value(fog),
                    )
                    .enabled(ground_haze)
                    .slider_action(UiSliderAction::FogGroundFalloff),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_FOG_MAX_OPACITY,
                        ph,
                        fog_max_opacity_label(fog),
                        fog_max_opacity_slider_value(fog),
                    )
                    .enabled(exponential)
                    .slider_action(UiSliderAction::FogMaxOpacity),
                ),
                (
                    18.0,
                    UiWidget::checkbox(
                        UI_V2_FOG_EXPONENTIAL_SQUARED,
                        ph,
                        "Exponential Squared",
                        fog.exponential_squared,
                    )
                    .enabled(exponential)
                    .action(GameUiAction::SetFogSettings(
                        crate::GameFogSettings {
                            exponential_squared: !fog.exponential_squared,
                            ..fog
                        },
                    )),
                ),
                (
                    18.0,
                    UiWidget::checkbox(UI_V2_FOG_FAR_CULL, ph, "Far Cull", fog.far_cull)
                        .enabled(fog.mode != crate::GameFogMode::Off)
                        .action(GameUiAction::SetFogSettings(crate::GameFogSettings {
                            far_cull: !fog.far_cull,
                            ..fog
                        })),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_FOG_WEATHER,
                        ph,
                        "Weather Influence",
                        fog.weather_influence.label(),
                    )
                    .action(GameUiAction::SetFogSettings(
                        crate::GameFogSettings {
                            weather_influence: fog.weather_influence.next(),
                            ..fog
                        },
                    )),
                ),
            ]
        }
        GameOptionsCategory::Movement => {
            let turn_mode = state
                .turn_mode
                .or_else(|| state.xr_turn_mode.map(GameTurnMode::from));
            vec![
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_MOVEMENT_MODE,
                        ph,
                        "Movement",
                        state.movement_mode.label(),
                    )
                    .action(GameUiAction::SetMovementMode(state.movement_mode.next())),
                ),
                (
                    20.0,
                    optional_cycle(
                        UI_V2_OPTIONS_COLLISION_MODE,
                        "Collision",
                        state.collision_mode,
                        |mode| mode.label(),
                        |mode| GameUiAction::SetCollisionMode(mode.next()),
                    ),
                ),
                (
                    20.0,
                    optional_cycle(
                        UI_V2_OPTIONS_TRAVEL_ASSIST,
                        "Travel Assist",
                        state.travel_assist_mode,
                        |mode| mode.label(),
                        |mode| GameUiAction::SetTravelAssistMode(mode.next()),
                    ),
                ),
                (
                    20.0,
                    optional_cycle(
                        UI_V2_OPTIONS_TURN_MODE,
                        "Turn",
                        turn_mode,
                        |mode| mode.label(),
                        |mode| GameUiAction::SetTurnMode(mode.next()),
                    ),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_OPTIONS_FLY_SPEED,
                        ph,
                        fly_speed_label(state),
                        fly_speed_slider_value(state),
                    )
                    .slider_action(UiSliderAction::FlySpeed),
                ),
                (
                    20.0,
                    UiWidget::slider(
                        UI_V2_OPTIONS_MOVEMENT_SPEED,
                        ph,
                        movement_speed_label(state),
                        movement_speed_slider_value(state),
                    )
                    .slider_action(UiSliderAction::MovementSpeed),
                ),
                (
                    20.0,
                    optional_cycle(
                        UI_V2_OPTIONS_TOUCH_CONTROLS,
                        "Touch Controls",
                        state.touch_controls_mode,
                        |mode| touch_controls_mode_label(mode),
                        |mode| GameUiAction::SetTouchControlsMode(next_touch_controls_mode(mode)),
                    ),
                ),
                (20.0, {
                    let mut widget = UiWidget::slider(
                        UI_V2_OPTIONS_TOUCH_LOOK,
                        ph,
                        state
                            .touch_settings
                            .map(touch_look_label)
                            .unwrap_or_else(|| "Touch Look".to_string()),
                        state
                            .touch_settings
                            .map(touch_look_slider_value)
                            .unwrap_or(0.0),
                    )
                    .enabled(state.touch_settings.is_some());
                    if state.touch_settings.is_some() {
                        widget = widget.slider_action(UiSliderAction::TouchLook);
                    }
                    widget
                }),
            ]
        }
        GameOptionsCategory::Display => vec![
            (
                20.0,
                UiWidget::cycle(
                    UI_V2_OPTIONS_PLAYER_MODEL,
                    ph,
                    "Player Model",
                    state.player_model.label(),
                )
                .action(GameUiAction::SetPlayerModel(state.player_model.next())),
            ),
            (
                18.0,
                UiWidget::checkbox(
                    UI_V2_OPTIONS_FIRST_PERSON_PLAYER,
                    ph,
                    "First Person Body",
                    state.first_person_player_visible,
                )
                .action(GameUiAction::ToggleFirstPersonPlayer),
            ),
            (18.0, {
                let mut widget = UiWidget::checkbox(
                    UI_V2_OPTIONS_CROSSHAIR,
                    ph,
                    "Crosshair",
                    state.crosshair_visible.unwrap_or(false),
                )
                .enabled(state.crosshair_visible.is_some());
                if state.crosshair_visible.is_some() {
                    widget = widget.action(GameUiAction::ToggleCrosshair);
                }
                widget
            }),
        ],
        GameOptionsCategory::LocalPlay => {
            let local_play = state.local_play;
            let local_play_state = local_play.unwrap_or_default();
            vec![
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_LOCAL_PLAY_LAYOUT,
                        ph,
                        "Layout",
                        local_play
                            .map(|state| state.layout.label())
                            .unwrap_or("N/A"),
                    )
                    .enabled(local_play.is_some())
                    .action(GameUiAction::SetLocalPlayLayout(
                        local_play_state.layout.next(),
                    )),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_LOCAL_PLAY_PLAYER_ONE,
                        ph,
                        "Player 1",
                        if local_play.is_some() {
                            "Keyboard + Mouse"
                        } else {
                            "N/A"
                        },
                    )
                    .enabled(false),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_LOCAL_PLAY_GUEST,
                        ph,
                        "Guest 2",
                        local_play
                            .map(|state| state.guest_input.label())
                            .unwrap_or_else(|| "N/A".to_owned()),
                    )
                    .enabled(local_play.is_some())
                    .action(GameUiAction::ToggleLocalPlayGuest),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_LOCAL_PLAY_STATUS,
                        ph,
                        "Guest 2 Profile",
                        if local_play.is_some() {
                            "Session Only"
                        } else {
                            "N/A"
                        },
                    )
                    .enabled(false),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_OPTIONS_LOCAL_PLAY_ACCESS,
                        ph,
                        "Guest Access",
                        if local_play.is_some() {
                            "View Only"
                        } else {
                            "N/A"
                        },
                    )
                    .enabled(false),
                ),
            ]
        }
        GameOptionsCategory::Debug => vec![
            (
                20.0,
                optional_cycle(
                    UI_V2_OPTIONS_AUXILIARY_SPLIT,
                    "Auxiliary View",
                    state.auxiliary_split_mode,
                    |mode| mode.label(),
                    |mode| GameUiAction::SetAuxiliarySplitMode(mode.next()),
                ),
            ),
            (
                18.0,
                UiWidget::checkbox(
                    UI_V2_OPTIONS_PLAYER_BOX,
                    ph,
                    "Player Box",
                    state.player_collision_box_visible,
                )
                .action(GameUiAction::TogglePlayerCollisionBox),
            ),
            (
                18.0,
                UiWidget::checkbox(
                    UI_V2_OPTIONS_FRAME_PIPELINE_OVERLAY,
                    ph,
                    "Frame Metrics",
                    state.frame_pipeline_overlay_visible,
                )
                .action(GameUiAction::ToggleFramePipelineOverlay),
            ),
            (
                18.0,
                UiWidget::checkbox(
                    UI_V2_OPTIONS_DEBUG_DIAGNOSTICS,
                    ph,
                    "Debug Pane",
                    state.debug_diagnostics_visible,
                )
                .action(GameUiAction::ToggleDebugDiagnostics),
            ),
            (
                18.0,
                UiWidget::checkbox(
                    UI_V2_OPTIONS_FULLBRIGHT,
                    ph,
                    "Force Fullbright",
                    state.force_fullbright,
                )
                .action(GameUiAction::ToggleFullbright),
            ),
        ],
        GameOptionsCategory::StorageProfile => {
            let title_only = parent == GameOptionsParent::Title;
            let storage = state.storage_profile;
            let profile_id = storage
                .profile_id
                .map(format_profile_id)
                .unwrap_or_else(|| "Unavailable".to_owned());
            let world_count = state.world_catalog.entry_count();
            let delete_all_available = title_only
                && state.world_catalog.delete_supported
                && state.world_catalog.active.is_none()
                && !state.world_catalog.loading
                && world_count > 0;
            vec![
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_STORAGE_PROFILE_NAME,
                        ph,
                        "Player",
                        if storage.display_name.is_empty() {
                            "Unavailable"
                        } else {
                            storage.display_name.as_str()
                        },
                    )
                    .enabled(false),
                ),
                (
                    20.0,
                    UiWidget::cycle(UI_V2_STORAGE_PROFILE_ID, ph, "UUID", profile_id)
                        .enabled(false),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_STORAGE_BACKEND,
                        ph,
                        "Profile Store",
                        storage.backend.label(),
                    )
                    .enabled(false),
                ),
                (
                    20.0,
                    UiWidget::cycle(
                        UI_V2_STORAGE_WORLD_COUNT,
                        ph,
                        "Local Worlds",
                        world_count.to_string(),
                    )
                    .enabled(false),
                ),
                (
                    20.0,
                    UiWidget::button(UI_V2_STORAGE_CLEAR_CACHE, ph, "Clear Rebuildable Cache")
                        .enabled(title_only && storage.clear_cache_available)
                        .action(GameUiAction::ClearRebuildableCache),
                ),
                (
                    20.0,
                    UiWidget::button(UI_V2_STORAGE_RESET_IDENTITY, ph, "Reset Player Identity")
                        .enabled(title_only && storage.profile_actions_available)
                        .action(GameUiAction::ConfirmStorageAction(
                            parent,
                            GameStorageAction::ResetPlayerIdentity,
                        )),
                ),
                (
                    20.0,
                    UiWidget::button(
                        UI_V2_STORAGE_DELETE_ALL_WORLDS,
                        ph,
                        "Delete All Local Worlds",
                    )
                    .enabled(delete_all_available)
                    .action(GameUiAction::ConfirmStorageAction(
                        parent,
                        GameStorageAction::DeleteAllLocalWorlds,
                    )),
                ),
                (
                    20.0,
                    UiWidget::button(UI_V2_STORAGE_FACTORY_RESET, ph, "Factory Reset")
                        .enabled(
                            title_only
                                && storage.factory_reset_available
                                && state.world_catalog.delete_supported
                                && state.world_catalog.active.is_none()
                                && !state.world_catalog.loading,
                        )
                        .action(GameUiAction::ConfirmStorageAction(
                            parent,
                            GameStorageAction::FactoryReset,
                        )),
                ),
            ]
        }
    }
}

fn format_profile_id(bytes: [u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}

/// Build a cycle row backed by an `Option<T>` platform value. When present it
/// shows the value/label and cycles on click; when absent it renders disabled
/// with an "N/A" placeholder and no action.
fn optional_cycle<T: Copy>(
    id: UiWidgetId,
    label: &'static str,
    value: Option<T>,
    value_label: impl Fn(T) -> &'static str,
    action: impl Fn(T) -> GameUiAction,
) -> UiWidget {
    let display = value.map(&value_label).unwrap_or("N/A");
    let mut widget =
        UiWidget::cycle(id, Rect::new(0.0, 0.0, 0.0, 0.0), label, display).enabled(value.is_some());
    if let Some(value) = value {
        widget = widget.action(action(value));
    }
    widget
}

/// Per-category sub-panel: flows the category's rows into (by default) two
/// columns, with a shared Back/Done button below.
fn options_category_layout(
    scale: GuiScale,
    revision: u64,
    parent: GameOptionsParent,
    category: GameOptionsCategory,
    state: GameUiRenderState,
    scroll_offset: f32,
) -> UiLayout {
    let mut layout = UiLayout::new(
        Some(UiScreenId::OptionsCategory { parent, category }),
        revision,
    );
    let panel = options_category_panel_rect(scale, category);
    let rows = options_category_rows(parent, category, state);
    let scroll_clip = Rect::new(
        panel.x + 12.0,
        panel.y + 28.0,
        panel.width - 24.0,
        (panel.height - 60.0).max(1.0),
    );
    let first_widget = layout.widgets.len();
    let content_height = if category == GameOptionsCategory::StorageProfile {
        place_storage_profile_rows(&mut layout, panel, rows, scroll_offset)
    } else {
        place_option_rows(
            &mut layout,
            panel,
            options_category_columns(panel, category),
            rows,
            scroll_offset,
        )
    };
    let widget_count = layout.widgets.len() - first_widget;
    layout.set_scroll_region(UiScrollRegion {
        clip: scroll_clip,
        first_widget,
        widget_count,
        content_height,
        offset: scroll_offset,
    });
    layout.push(
        UiWidget::button(
            UI_V2_OPTIONS_BACK,
            Rect::new(panel.center_x() - 55.0, panel.bottom() - 26.0, 110.0, 20.0),
            match parent {
                GameOptionsParent::Title => "Back",
                GameOptionsParent::Pause => "Done",
            },
        )
        .action(if category == GameOptionsCategory::Fog {
            GameUiAction::OpenOptionsCategory(parent, GameOptionsCategory::Graphics)
        } else {
            GameUiAction::OpenOptions(parent)
        }),
    );
    layout
}

fn place_storage_profile_rows(
    layout: &mut UiLayout,
    panel: Rect,
    rows: Vec<(f32, UiWidget)>,
    scroll_offset: f32,
) -> f32 {
    debug_assert_eq!(
        rows.len(),
        options_category_row_count(GameOptionsCategory::StorageProfile)
    );
    let x0 = panel.x + 18.0;
    let y0 = panel.y + 30.0 - scroll_offset;
    let pitch = 24.0;
    let column_gap = 10.0;
    let full_width = panel.width - 36.0;
    let action_width = (full_width - column_gap) * 0.5;
    for (index, (height, mut widget)) in rows.into_iter().enumerate() {
        widget.rect = if index < 4 {
            Rect::new(x0, y0 + index as f32 * pitch, full_width, height)
        } else {
            let action_index = index - 4;
            let column = action_index % 2;
            let row = action_index / 2;
            Rect::new(
                x0 + column as f32 * (action_width + column_gap),
                y0 + (4 + row) as f32 * pitch,
                action_width,
                height,
            )
        };
        layout.push(widget);
    }
    6.0 * pitch
}

/// Flow `rows` (each `(height, widget)`) into `cols` columns within `panel`,
/// filling each column top-to-bottom (column-major) so related rows stay
/// adjacent. Returns the y just below the tallest column for placing a footer.
fn place_option_rows(
    layout: &mut UiLayout,
    panel: Rect,
    cols: usize,
    rows: Vec<(f32, UiWidget)>,
    scroll_offset: f32,
) -> f32 {
    let cols = cols.max(1);
    let column_gap = 10.0;
    let column_width =
        ((panel.width - 36.0 - column_gap * (cols as f32 - 1.0)) / cols as f32).max(110.0);
    let x0 = panel.x + 18.0;
    let y0 = panel.y + 30.0 - scroll_offset;
    let pitch = 24.0;
    let per_col = rows.len().div_ceil(cols).max(1);
    for (index, (height, mut widget)) in rows.into_iter().enumerate() {
        let col = index / per_col;
        let row_in_col = index % per_col;
        widget.rect = Rect::new(
            x0 + col as f32 * (column_width + column_gap),
            y0 + row_in_col as f32 * pitch,
            column_width,
            height,
        );
        layout.push(widget);
    }
    per_col as f32 * pitch
}

fn server_settings_layout(
    scale: GuiScale,
    revision: u64,
    parent: GameOptionsParent,
    state: GameUiRenderState,
) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::ServerSettings { parent }), revision);
    let panel = server_settings_panel_rect(scale, state);
    let row_x = panel.x + 25.0;
    let mut y = panel.y + 38.0;

    if let Some(cadence) = state.server_cadence {
        push_cycle(
            &mut layout,
            UI_V2_SERVER_SETTINGS_HOST_RATE,
            Rect::new(row_x, y, 192.0, 20.0),
            "Host Rate",
            format!("{} Hz", cadence.host_rate_hz),
            GameUiAction::SetServerSimulationCadence(cadence.next_host_rate()),
        );
        y += 22.0;
        push_cycle(
            &mut layout,
            UI_V2_SERVER_SETTINGS_GAMEPLAY_RATE,
            Rect::new(row_x, y, 192.0, 20.0),
            "World Tick Rate",
            format!("{} Hz", cadence.gameplay_rate_hz),
            GameUiAction::SetServerSimulationCadence(cadence.next_gameplay_rate()),
        );
        y += 22.0;
        push_cycle(
            &mut layout,
            UI_V2_SERVER_SETTINGS_PHYSICS_RATE,
            Rect::new(row_x, y, 192.0, 20.0),
            "Physics Rate",
            format!("{} Hz", cadence.physics_rate_hz),
            GameUiAction::SetServerSimulationCadence(cadence.next_physics_rate()),
        );
        y += 28.0;
    } else {
        y += 34.0;
    }

    layout.push(
        UiWidget::button(
            UI_V2_SERVER_SETTINGS_BACK,
            Rect::new(panel.center_x() - 55.0, y, 110.0, 20.0),
            match parent {
                GameOptionsParent::Title => "Back",
                GameOptionsParent::Pause => "Done",
            },
        )
        .action(GameUiAction::OpenOptions(parent)),
    );

    layout
}

fn push_cycle(
    layout: &mut UiLayout,
    id: UiWidgetId,
    rect: Rect,
    label: &'static str,
    value: impl Into<String>,
    action: GameUiAction,
) {
    layout.push(UiWidget::cycle(id, rect, label, value).action(action));
}

fn options_panel_rect(scale: GuiScale, _state: GameUiRenderState) -> Rect {
    // The hub is a fixed short list: categories + Visual Profiles + Server
    // Settings + Controls Help + Back.
    let panel_width = (scale.width - 18.0).clamp(242.0, 360.0);
    let panel_height = 262.0f32.min((scale.height - 4.0).max(1.0));
    centered_panel(scale, panel_width, panel_height)
}

fn asset_packs_panel_rect(scale: GuiScale) -> Rect {
    centered_panel(
        scale,
        (scale.width - 12.0).clamp(300.0, 456.0),
        (scale.height - 4.0).clamp(220.0, 286.0),
    )
}

fn asset_pack_row_id(index: usize) -> UiWidgetId {
    UiWidgetId(UI_V2_ASSET_PACK_ROW_BASE + index as u64)
}

fn asset_packs_layout(
    scale: GuiScale,
    revision: u64,
    parent: GameOptionsParent,
    state: AssetPacksUiState,
) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::AssetPacks { parent }), revision);
    let panel = asset_packs_panel_rect(scale);
    let row_count = state.row_count().max(1);
    let row_gap = 3.0;
    let rows_top = 28.0;
    let presentation_top = panel.height - 52.0;
    let available_height =
        presentation_top - rows_top - 8.0 - row_gap * row_count.saturating_sub(1) as f32;
    let row_height = (available_height / row_count as f32).clamp(24.0, 42.0);
    for (index, row) in state.rows.iter().flatten().copied().enumerate() {
        let mut widget = UiWidget::asset_pack_row(
            asset_pack_row_id(index),
            Rect::new(
                panel.x + 14.0,
                panel.y + rows_top + index as f32 * (row_height + row_gap),
                panel.width - 28.0,
                row_height,
            ),
            row,
        );
        widget.enabled &= !state.apply_state.is_preparing();
        layout.push(widget);
    }
    layout.push(
        UiWidget::button(
            UiWidgetId(UI_V2_ASSET_PACK_APPLY.0 - 2),
            Rect::new(
                panel.x + 14.0,
                panel.bottom() - 52.0,
                panel.width - 28.0,
                20.0,
            ),
            format!("Presentation: {}", state.presentation_label.as_str()),
        )
        .enabled(!state.apply_state.is_preparing())
        .action(GameUiAction::CycleTexturePresentation),
    );
    let footer_y = panel.bottom() - 27.0;
    layout.push(
        UiWidget::button(
            UI_V2_ASSET_PACK_CANCEL,
            Rect::new(panel.center_x() - 96.0, footer_y, 90.0, 20.0),
            "Cancel",
        )
        .enabled(!state.apply_state.is_preparing())
        .action(GameUiAction::CancelAssetPacks),
    );
    layout.push(
        UiWidget::button(
            UI_V2_ASSET_PACK_APPLY,
            Rect::new(panel.center_x() + 6.0, footer_y, 90.0, 20.0),
            "Apply",
        )
        .enabled(state.can_apply())
        .action(GameUiAction::ApplyAssetPacks),
    );
    layout
}

fn options_category_panel_rect(scale: GuiScale, category: GameOptionsCategory) -> Rect {
    let panel_width = (scale.width - 18.0).clamp(242.0, 420.0);
    let row_count = if category == GameOptionsCategory::StorageProfile {
        6
    } else {
        options_category_row_count(category)
            .div_ceil(options_category_columns(
                Rect::new(0.0, 0.0, panel_width, 1.0),
                category,
            ))
            .max(1)
    };
    // title band + flowed rows + Back button + bottom padding
    let content = 30.0 + row_count as f32 * 24.0 + 34.0;
    let panel_height = content.min((scale.height - 4.0).max(1.0));
    centered_panel(scale, panel_width, panel_height)
}

fn options_category_columns(panel: Rect, category: GameOptionsCategory) -> usize {
    if category == GameOptionsCategory::LocalPlay || panel.width < 340.0 {
        1
    } else {
        2
    }
}

fn server_settings_panel_rect(scale: GuiScale, state: GameUiRenderState) -> Rect {
    let height = if state.server_cadence.is_some() {
        138.0
    } else {
        96.0
    };
    centered_panel(scale, 242.0, height)
}

const fn help_parent_for_options(parent: GameOptionsParent) -> GameHelpParent {
    match parent {
        GameOptionsParent::Title => GameHelpParent::OptionsTitle,
        GameOptionsParent::Pause => GameHelpParent::OptionsPause,
    }
}

#[cfg(test)]
mod tests;
