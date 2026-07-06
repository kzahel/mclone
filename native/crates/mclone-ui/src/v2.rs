use crate::{
    BLOCK_PALETTE_ENTRY_CAPACITY, BLOCK_PALETTE_PADDING, BlockPaletteEntry, BlockPaletteOverlay,
    Button, Checkbox, Color, CycleButton, FlatHud, Font, GameHelpParent, GameOptionsParent,
    GameScreen, GameTurnMode, GameUiAction, GameUiRenderState, GuiDrawList, GuiKey, GuiScale,
    GuiTextureUv, HOTBAR_SLOT_COUNT_USIZE, Interaction, LoadingProgressOverlay, Point, Rect,
    Slider, WidgetId, WorldCatalogUiEntry, WorldCatalogUiState, WorldCatalogUiWorldId,
    block_palette_panel_rect, block_palette_slot_rect, centered_panel,
    far_lod_range_from_slider_value, far_lod_range_label, far_lod_range_slider_value,
    fly_speed_from_slider_value, fly_speed_label, fly_speed_slider_value,
    movement_speed_from_slider_value, movement_speed_label, movement_speed_slider_value,
    next_touch_controls_mode, render_block_palette_tooltip, render_distance_from_slider_value,
    render_distance_label, render_distance_slider_value, render_flat_hud_debug_layer,
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
    WorldList,
    WorldCreate,
    WorldDeleteConfirm { id: WorldCatalogUiWorldId },
    NewWorld,
    JoinRemote,
    Pause,
    BlockPalette,
    Options { parent: GameOptionsParent },
    ServerSettings { parent: GameOptionsParent },
    Help { parent: GameHelpParent },
}

impl UiScreenId {
    pub fn from_game_screen(screen: Option<GameScreen>) -> Option<Self> {
        match screen {
            Some(GameScreen::Title) => Some(Self::Title),
            Some(GameScreen::WorldList) => Some(Self::WorldList),
            Some(GameScreen::WorldCreate) => Some(Self::WorldCreate),
            Some(GameScreen::WorldDeleteConfirm { id }) => Some(Self::WorldDeleteConfirm { id }),
            Some(GameScreen::NewWorld) => Some(Self::NewWorld),
            Some(GameScreen::JoinRemote) => Some(Self::JoinRemote),
            Some(GameScreen::Pause) => Some(Self::Pause),
            Some(GameScreen::BlockPalette) => Some(Self::BlockPalette),
            Some(GameScreen::Options { parent }) => Some(Self::Options { parent }),
            Some(GameScreen::ServerSettings { parent }) => Some(Self::ServerSettings { parent }),
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
    FarLodRange,
    FlySpeed,
    MovementSpeed,
    TouchLook,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiLayout {
    pub screen: Option<UiScreenId>,
    pub revision: u64,
    widgets: Vec<UiWidget>,
    help_rows: Vec<UiHelpRow>,
}

impl UiLayout {
    pub fn new(screen: Option<UiScreenId>, revision: u64) -> Self {
        Self {
            screen,
            revision,
            widgets: Vec::new(),
            help_rows: Vec::new(),
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
            .rev()
            .find(|widget| widget.contains(point))
            .map(|widget| widget.id)
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
    pub widgets: Vec<UiDebugWidget>,
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
        self.set_interaction_state(None, None, None);
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
        let action = match (captured, self.hovered) {
            (Some(captured), Some(released)) if captured == released => self
                .layout
                .widget(captured)
                .and_then(|widget| self.action_for_widget(widget, point)),
            _ => None,
        };
        (true, action)
    }

    pub fn key_pressed(&mut self, key: GuiKey) -> (bool, Option<GameUiAction>) {
        match (self.screen, key) {
            (Some(UiScreenId::Title), GuiKey::Escape) => (true, None),
            (Some(UiScreenId::Title), GuiKey::F1) => {
                (true, Some(GameUiAction::OpenHelp(GameHelpParent::Title)))
            }
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
            (Some(UiScreenId::ServerSettings { parent }), GuiKey::Escape) => {
                (true, Some(GameUiAction::OpenOptions(parent)))
            }
            (Some(UiScreenId::ServerSettings { parent }), GuiKey::F1) => (
                true,
                Some(GameUiAction::OpenHelp(help_parent_for_options(parent))),
            ),
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
            Some(UiScreenId::WorldList) => self.render_world_list(&mut draw),
            Some(UiScreenId::WorldCreate) => self.render_world_create(&mut draw),
            Some(UiScreenId::WorldDeleteConfirm { id }) => {
                self.render_world_delete_confirm(&mut draw, id)
            }
            Some(UiScreenId::NewWorld) => self.render_new_world(&mut draw),
            Some(UiScreenId::JoinRemote) => self.render_join_remote(&mut draw),
            Some(UiScreenId::Pause) => self.render_pause(&mut draw),
            Some(UiScreenId::BlockPalette) => self.render_block_palette(&mut draw),
            Some(UiScreenId::Options { parent }) => self.render_options(&mut draw, parent),
            Some(UiScreenId::ServerSettings { parent }) => {
                self.render_server_settings(&mut draw, parent)
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
            Some(UiScreenId::Title) => title_layout(self.scale, self.layout_revision),
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
            Some(UiScreenId::BlockPalette) => block_palette_layout(
                self.scale,
                self.layout_revision,
                self.render_state.block_palette,
            ),
            Some(UiScreenId::Options { parent }) => {
                options_layout(self.scale, self.layout_revision, parent, self.render_state)
            }
            Some(UiScreenId::ServerSettings { parent }) => {
                server_settings_layout(self.scale, self.layout_revision, parent, self.render_state)
            }
            Some(UiScreenId::Help { parent }) => {
                help_layout(self.scale, self.layout_revision, parent)
            }
            None => UiLayout::new(None, self.layout_revision),
        };
        self.layout_dirty = false;
        self.hovered = self.pointer.and_then(|point| self.layout.hit_test(point));
        if self
            .captured
            .is_some_and(|captured| self.layout.widget(captured).is_none())
        {
            self.set_interaction_state(self.pointer, self.hovered, None);
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
            focused: None,
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
        if !catalog.create_supported {
            self.font.draw_centered_atlas(
                draw,
                "CREATE IS UNAVAILABLE",
                panel.center_x(),
                panel.y + 72.0,
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
        for row in self.layout.help_rows() {
            self.render_help_row(draw, row);
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
        for widget in self.layout.widgets() {
            let UiWidgetKind::PaletteSlot { icon } = widget.kind else {
                continue;
            };
            if self.captured == Some(widget.id) {
                render_touch_panel(draw, widget.rect, true);
                render_palette_slot_contents(draw, &self.font, widget.rect, icon);
            }
            if self.hovered == Some(widget.id) {
                draw.outline(widget.rect.inset(-1.0), Color::rgba(245, 250, 255, 205));
            }
        }

        let Some(hovered) = self.hovered.and_then(|id| self.layout.widget(id)) else {
            return;
        };
        if matches!(hovered.kind, UiWidgetKind::PaletteSlot { .. }) {
            let tooltip_anchor = Point {
                x: hovered.rect.right(),
                y: hovered.rect.y,
            };
            render_block_palette_tooltip(
                draw,
                &self.font,
                self.scale,
                tooltip_anchor,
                hovered.label.as_str(),
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
                let hovered = interaction.is_hovered(widget.rect);
                let fill = if *selected {
                    Color::rgba(64, 90, 84, 230)
                } else if hovered {
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
            UiWidgetKind::PaletteSlot { icon } => {
                render_touch_panel(
                    draw,
                    widget.rect,
                    interaction.pressed == Some(widget.id.legacy_widget_id()),
                );
                render_palette_slot_contents(draw, &self.font, widget.rect, *icon);
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
                    UiSliderAction::FarLodRange => GameUiAction::SetFarLodRange(
                        far_lod_range_from_slider_value(value, self.render_state),
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
                    | GameScreen::NewWorld
                    | GameScreen::JoinRemote
            )
        ) || matches!(
            self.screen,
            Some(GameScreen::Help { parent }) if help_parent_covers_world(parent)
        )
    }

    pub fn open_pause(&mut self) {
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
        match action {
            GameUiAction::StartWorld
            | GameUiAction::Resume
            | GameUiAction::OpenWorld(_)
            | GameUiAction::CreateCatalogWorld
            | GameUiAction::AssignHotbarBlock { .. } => self.screen = None,
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
            GameUiAction::OpenServerSettings(parent) => {
                self.screen = Some(GameScreen::ServerSettings { parent });
            }
            GameUiAction::BackToTitle | GameUiAction::QuitToTitle => {
                self.screen = Some(GameScreen::Title);
            }
            GameUiAction::BackToPause => self.screen = Some(GameScreen::Pause),
            GameUiAction::CreateWorld(_) | GameUiAction::JoinRemote => self.screen = None,
            GameUiAction::RerollSeed => {}
            GameUiAction::ToggleSectionOcclusion
            | GameUiAction::ToggleFullbright
            | GameUiAction::ToggleFarLod
            | GameUiAction::TogglePlayerCollisionBox
            | GameUiAction::ToggleFirstPersonPlayer
            | GameUiAction::ToggleCrosshair
            | GameUiAction::ToggleFramePipelineOverlay
            | GameUiAction::ToggleDebugDiagnostics
            | GameUiAction::SetPlayerModel(_)
            | GameUiAction::SetMovementMode(_)
            | GameUiAction::SetCollisionMode(_)
            | GameUiAction::SetTravelAssistMode(_)
            | GameUiAction::SetTurnMode(_)
            | GameUiAction::SetXrTurnMode(_)
            | GameUiAction::CycleFramePacing
            | GameUiAction::CycleFpsCap
            | GameUiAction::SetRenderDistance(_)
            | GameUiAction::SetFarLodRange(_)
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
const UI_V2_WORLD_LIST_OPEN: UiWidgetId = UiWidgetId(801);
const UI_V2_WORLD_LIST_CREATE: UiWidgetId = UiWidgetId(802);
const UI_V2_WORLD_LIST_DELETE: UiWidgetId = UiWidgetId(803);
const UI_V2_WORLD_LIST_BACK: UiWidgetId = UiWidgetId(804);
const UI_V2_WORLD_LIST_ROW_BASE: u64 = 820;
const UI_V2_WORLD_CREATE_REROLL: UiWidgetId = UiWidgetId(901);
const UI_V2_WORLD_CREATE_CREATE: UiWidgetId = UiWidgetId(902);
const UI_V2_WORLD_CREATE_BACK: UiWidgetId = UiWidgetId(903);
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
const UI_V2_OPTIONS_OCCLUSION: UiWidgetId = UiWidgetId(101);
const UI_V2_OPTIONS_FULLBRIGHT: UiWidgetId = UiWidgetId(102);
const UI_V2_OPTIONS_FAR_LOD: UiWidgetId = UiWidgetId(103);
const UI_V2_OPTIONS_FAR_LOD_RANGE: UiWidgetId = UiWidgetId(104);
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
const UI_V2_SERVER_SETTINGS_HOST_RATE: UiWidgetId = UiWidgetId(701);
const UI_V2_SERVER_SETTINGS_GAMEPLAY_RATE: UiWidgetId = UiWidgetId(702);
const UI_V2_SERVER_SETTINGS_PHYSICS_RATE: UiWidgetId = UiWidgetId(703);
const UI_V2_SERVER_SETTINGS_BACK: UiWidgetId = UiWidgetId(704);
const UI_V2_HELP_BACK: UiWidgetId = UiWidgetId(201);
const UI_V2_BLOCK_PALETTE_BASE: u64 = 3000;

fn title_layout(scale: GuiScale, revision: u64) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::Title), revision);
    let y = scale.height * 0.5 - 34.0;
    layout.push(
        UiWidget::button(
            UI_V2_TITLE_START,
            menu_button_rect(scale, y),
            "Singleplayer",
        )
        .action(GameUiAction::OpenWorldList),
    );
    layout.push(
        UiWidget::button(
            UI_V2_TITLE_JOIN_REMOTE,
            menu_button_rect(scale, y + 24.0),
            "Join Remote",
        )
        .action(GameUiAction::OpenJoinRemote),
    );
    layout.push(
        UiWidget::button(
            UI_V2_TITLE_OPTIONS,
            menu_button_rect(scale, y + 48.0),
            "Options",
        )
        .action(GameUiAction::OpenOptions(GameOptionsParent::Title)),
    );
    layout.push(
        UiWidget::button(UI_V2_TITLE_QUIT, menu_button_rect(scale, y + 72.0), "Quit")
            .action(GameUiAction::Quit),
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
            UI_V2_WORLD_CREATE_CREATE,
            menu_button_rect_at(panel.center_x(), y + 24.0),
            "Create World",
        )
        .enabled(catalog.create_supported)
        .action(GameUiAction::CreateCatalogWorld),
    );
    layout.push(
        UiWidget::button(
            UI_V2_WORLD_CREATE_BACK,
            menu_button_rect_at(panel.center_x(), y + 48.0),
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

fn new_world_layout(scale: GuiScale, revision: u64) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::NewWorld), revision);
    let y = scale.height * 0.5 - 4.0;
    layout.push(
        UiWidget::button(UI_V2_NEW_WORLD_REROLL, menu_button_rect(scale, y), "Reroll")
            .action(GameUiAction::RerollSeed),
    );
    layout.push(
        UiWidget::button(
            UI_V2_NEW_WORLD_CREATE,
            menu_button_rect(scale, y + 24.0),
            "Create World",
        )
        .action(GameUiAction::CreateWorld(0)),
    );
    layout.push(
        UiWidget::button(
            UI_V2_NEW_WORLD_BACK,
            menu_button_rect(scale, y + 48.0),
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
    centered_panel(scale, 320.0, 178.0)
}

fn world_delete_confirm_panel_rect(scale: GuiScale) -> Rect {
    centered_panel(scale, 340.0, 158.0)
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
    if active == Some(entry.id) {
        "Active".to_owned()
    } else {
        format!("Seed {}", entry.seed)
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
        layout.push(
            UiWidget::palette_slot(block_palette_slot_id(index), rect, entry.label, entry.icon)
                .action(GameUiAction::AssignHotbarBlock {
                    slot: overlay.selected_hotbar_slot,
                    block_state: entry.block_state,
                }),
        );
    }
    layout
}

fn help_layout(scale: GuiScale, revision: u64, parent: GameHelpParent) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::Help { parent }), revision);
    let panel = help_panel_rect(scale);
    let font = Font::default();
    layout.set_help_rows(help_text_rows(panel, font.line_height()));
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

fn options_layout(
    scale: GuiScale,
    revision: u64,
    parent: GameOptionsParent,
    state: GameUiRenderState,
) -> UiLayout {
    let mut layout = UiLayout::new(Some(UiScreenId::Options { parent }), revision);
    let panel = options_panel_rect(scale, state);
    let column_gap = 10.0;
    let column_width = ((panel.width - 42.0 - column_gap) * 0.5).max(110.0);
    let left_x = panel.x + 18.0;
    let right_x = left_x + column_width + column_gap;
    let mut left_y = panel.y + 30.0;
    let mut right_y = panel.y + 30.0;

    push_checkbox(
        &mut layout,
        UI_V2_OPTIONS_OCCLUSION,
        Rect::new(left_x, left_y, column_width, 18.0),
        "Section Occlusion",
        state.section_occlusion_culling,
        GameUiAction::ToggleSectionOcclusion,
    );
    left_y += 20.0;
    push_checkbox(
        &mut layout,
        UI_V2_OPTIONS_FULLBRIGHT,
        Rect::new(left_x, left_y, column_width, 18.0),
        "Force Fullbright",
        state.force_fullbright,
        GameUiAction::ToggleFullbright,
    );
    left_y += 20.0;
    push_checkbox(
        &mut layout,
        UI_V2_OPTIONS_FAR_LOD,
        Rect::new(left_x, left_y, column_width, 18.0),
        "Far LOD",
        state.far_lod_enabled,
        GameUiAction::ToggleFarLod,
    );
    left_y += 20.0;
    layout.push(
        UiWidget::slider(
            UI_V2_OPTIONS_FAR_LOD_RANGE,
            Rect::new(left_x, left_y, column_width, 20.0),
            far_lod_range_label(state),
            far_lod_range_slider_value(state),
        )
        .enabled(state.far_lod_enabled)
        .slider_action(UiSliderAction::FarLodRange),
    );
    left_y += 22.0;
    push_checkbox(
        &mut layout,
        UI_V2_OPTIONS_PLAYER_BOX,
        Rect::new(left_x, left_y, column_width, 18.0),
        "Player Box",
        state.player_collision_box_visible,
        GameUiAction::TogglePlayerCollisionBox,
    );
    left_y += 20.0;
    push_checkbox(
        &mut layout,
        UI_V2_OPTIONS_FIRST_PERSON_PLAYER,
        Rect::new(left_x, left_y, column_width, 18.0),
        "First Person Body",
        state.first_person_player_visible,
        GameUiAction::ToggleFirstPersonPlayer,
    );
    left_y += 20.0;
    if let Some(crosshair_visible) = state.crosshair_visible {
        push_checkbox(
            &mut layout,
            UI_V2_OPTIONS_CROSSHAIR,
            Rect::new(left_x, left_y, column_width, 18.0),
            "Crosshair",
            crosshair_visible,
            GameUiAction::ToggleCrosshair,
        );
        left_y += 20.0;
    }

    push_cycle(
        &mut layout,
        UI_V2_OPTIONS_PLAYER_MODEL,
        Rect::new(right_x, right_y, column_width, 20.0),
        "Player Model",
        state.player_model.label(),
        GameUiAction::SetPlayerModel(state.player_model.next()),
    );
    right_y += 22.0;
    push_cycle(
        &mut layout,
        UI_V2_OPTIONS_MOVEMENT_MODE,
        Rect::new(right_x, right_y, column_width, 20.0),
        "Movement",
        state.movement_mode.label(),
        GameUiAction::SetMovementMode(state.movement_mode.next()),
    );
    right_y += 22.0;
    if let Some(collision_mode) = state.collision_mode {
        push_cycle(
            &mut layout,
            UI_V2_OPTIONS_COLLISION_MODE,
            Rect::new(right_x, right_y, column_width, 20.0),
            "Collision",
            collision_mode.label(),
            GameUiAction::SetCollisionMode(collision_mode.next()),
        );
        right_y += 22.0;
    }
    if let Some(travel_assist_mode) = state.travel_assist_mode {
        push_cycle(
            &mut layout,
            UI_V2_OPTIONS_TRAVEL_ASSIST,
            Rect::new(right_x, right_y, column_width, 20.0),
            "Travel Assist",
            travel_assist_mode.label(),
            GameUiAction::SetTravelAssistMode(travel_assist_mode.next()),
        );
        right_y += 22.0;
    }
    let turn_mode = state
        .turn_mode
        .or_else(|| state.xr_turn_mode.map(GameTurnMode::from));
    if let Some(turn_mode) = turn_mode {
        push_cycle(
            &mut layout,
            UI_V2_OPTIONS_TURN_MODE,
            Rect::new(right_x, right_y, column_width, 20.0),
            "Turn",
            turn_mode.label(),
            GameUiAction::SetTurnMode(turn_mode.next()),
        );
        right_y += 22.0;
    }
    push_cycle(
        &mut layout,
        UI_V2_OPTIONS_FRAME_PACING,
        Rect::new(right_x, right_y, column_width, 20.0),
        "Frame Pacing",
        state.frame_pacing_mode.label(),
        GameUiAction::CycleFramePacing,
    );
    right_y += 22.0;
    push_cycle(
        &mut layout,
        UI_V2_OPTIONS_FPS_CAP,
        Rect::new(right_x, right_y, column_width, 20.0),
        "FPS Cap",
        state.fps_cap.to_string(),
        GameUiAction::CycleFpsCap,
    );
    right_y += 22.0;
    push_checkbox(
        &mut layout,
        UI_V2_OPTIONS_FRAME_PIPELINE_OVERLAY,
        Rect::new(right_x, right_y, column_width, 18.0),
        "Frame Metrics",
        state.frame_pipeline_overlay_visible,
        GameUiAction::ToggleFramePipelineOverlay,
    );
    right_y += 20.0;
    push_checkbox(
        &mut layout,
        UI_V2_OPTIONS_DEBUG_DIAGNOSTICS,
        Rect::new(right_x, right_y, column_width, 18.0),
        "Debug Pane",
        state.debug_diagnostics_visible,
        GameUiAction::ToggleDebugDiagnostics,
    );
    right_y += 20.0;
    layout.push(
        UiWidget::slider(
            UI_V2_OPTIONS_RADIUS,
            Rect::new(right_x, right_y, column_width, 20.0),
            render_distance_label(state),
            render_distance_slider_value(state),
        )
        .slider_action(UiSliderAction::RenderDistance),
    );
    right_y += 22.0;
    layout.push(
        UiWidget::slider(
            UI_V2_OPTIONS_FLY_SPEED,
            Rect::new(right_x, right_y, column_width, 20.0),
            fly_speed_label(state),
            fly_speed_slider_value(state),
        )
        .slider_action(UiSliderAction::FlySpeed),
    );
    right_y += 22.0;
    layout.push(
        UiWidget::slider(
            UI_V2_OPTIONS_MOVEMENT_SPEED,
            Rect::new(right_x, right_y, column_width, 20.0),
            movement_speed_label(state),
            movement_speed_slider_value(state),
        )
        .slider_action(UiSliderAction::MovementSpeed),
    );
    right_y += 22.0;

    if let Some(mode) = state.touch_controls_mode {
        push_cycle(
            &mut layout,
            UI_V2_OPTIONS_TOUCH_CONTROLS,
            Rect::new(left_x, left_y, column_width, 20.0),
            "Touch Controls",
            touch_controls_mode_label(mode),
            GameUiAction::SetTouchControlsMode(next_touch_controls_mode(mode)),
        );
        left_y += 22.0;
    }
    if let Some(settings) = state.touch_settings {
        layout.push(
            UiWidget::slider(
                UI_V2_OPTIONS_TOUCH_LOOK,
                Rect::new(right_x, right_y, column_width, 20.0),
                touch_look_label(settings),
                touch_look_slider_value(settings),
            )
            .slider_action(UiSliderAction::TouchLook),
        );
    }
    if state.server_cadence.is_some() {
        layout.push(
            UiWidget::button(
                UI_V2_OPTIONS_SERVER_SETTINGS,
                Rect::new(left_x, left_y, column_width, 20.0),
                "Server Settings",
            )
            .action(GameUiAction::OpenServerSettings(parent)),
        );
        left_y += 22.0;
    }
    left_y += 6.0;
    layout.push(
        UiWidget::button(
            UI_V2_OPTIONS_CONTROLS,
            Rect::new(left_x, left_y, column_width, 20.0),
            "Controls",
        )
        .action(GameUiAction::OpenHelp(help_parent_for_options(parent))),
    );
    left_y += 22.0;
    layout.push(
        UiWidget::button(
            UI_V2_OPTIONS_BACK,
            Rect::new(left_x, left_y, column_width, 20.0),
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

fn push_checkbox(
    layout: &mut UiLayout,
    id: UiWidgetId,
    rect: Rect,
    label: &'static str,
    checked: bool,
    action: GameUiAction,
) {
    layout.push(UiWidget::checkbox(id, rect, label, checked).action(action));
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

fn options_panel_rect(scale: GuiScale, state: GameUiRenderState) -> Rect {
    let left_rows_height = 122.0
        + f32::from(u8::from(state.crosshair_visible.is_some())) * 20.0
        + f32::from(
            u8::from(state.touch_controls_mode.is_some())
                + u8::from(state.server_cadence.is_some()),
        ) * 22.0
        + 48.0;
    let right_rows_height = (9
        + usize::from(state.collision_mode.is_some())
        + usize::from(state.travel_assist_mode.is_some())
        + usize::from(state.turn_mode.is_some() || state.xr_turn_mode.is_some())
        + usize::from(state.touch_settings.is_some())) as f32
        * 22.0;
    let panel_width = (scale.width - 18.0).clamp(242.0, 420.0);
    let panel_height =
        (44.0 + left_rows_height.max(right_rows_height)).min((scale.height - 4.0).max(1.0));
    centered_panel(scale, panel_width, panel_height)
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
