use crate::{
    BLOCK_PALETTE_ENTRY_CAPACITY, BLOCK_PALETTE_PADDING, BlockPaletteEntry, BlockPaletteOverlay,
    Button, Checkbox, Color, CycleButton, FlatHud, Font, GameHelpParent, GameOptionsParent,
    GameScreen, GameUi, GameUiAction, GameUiRenderState, GuiDrawList, GuiKey, GuiScale,
    GuiTextureUv, HOTBAR_SLOT_COUNT_USIZE, Interaction, Point, Rect, Slider, WidgetId,
    block_palette_panel_rect, block_palette_slot_rect, centered_panel,
    far_lod_range_from_slider_value, far_lod_range_label, far_lod_range_slider_value,
    fly_speed_from_slider_value, fly_speed_label, fly_speed_slider_value,
    movement_speed_from_slider_value, movement_speed_label, movement_speed_slider_value,
    next_touch_controls_mode, render_block_palette_tooltip, render_distance_from_slider_value,
    render_distance_label, render_distance_slider_value, render_flat_hud_hotbar_layer,
    render_flat_hud_retained_layer, render_flat_hud_transient_layers, render_palette_slot_contents,
    render_touch_panel, touch_controls_mode_label, touch_look_from_slider_value, touch_look_label,
    touch_look_slider_value,
};
use mclone_input::{
    FLAT_HOTBAR_SLOT_COUNT, ShortcutHelpGroup, ShortcutHelpRow,
    default_keyboard_mouse_shortcut_rows, flat_runtime_shortcut_rows,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiScreenId {
    Pause,
    BlockPalette,
    Options { parent: GameOptionsParent },
    Help { parent: GameHelpParent },
}

impl UiScreenId {
    pub fn from_game_screen(screen: Option<GameScreen>) -> Option<Self> {
        match screen {
            Some(GameScreen::Pause) => Some(Self::Pause),
            Some(GameScreen::BlockPalette) => Some(Self::BlockPalette),
            Some(GameScreen::Options { parent }) => Some(Self::Options { parent }),
            Some(GameScreen::Help { parent }) => Some(Self::Help { parent }),
            _ => None,
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
    Checkbox { checked: bool },
    Cycle,
    PaletteSlot { icon: Option<GuiTextureUv> },
    Slider { value: f32 },
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
            Some(UiScreenId::Pause) => self.render_pause(&mut draw),
            Some(UiScreenId::BlockPalette) => self.render_block_palette(&mut draw),
            Some(UiScreenId::Options { parent }) => self.render_options(&mut draw, parent),
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
            Some(UiScreenId::Pause) => pause_layout(self.scale, self.layout_revision),
            Some(UiScreenId::BlockPalette) => block_palette_layout(
                self.scale,
                self.layout_revision,
                self.render_state.block_palette,
            ),
            Some(UiScreenId::Options { parent }) => {
                options_layout(self.scale, self.layout_revision, parent, self.render_state)
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
struct CachedFlatHudRetainedLayer {
    state: FlatHudRetainedState,
    draw: GuiDrawList,
}

#[derive(Clone, Debug, PartialEq)]
struct CachedFlatHudHotbarLayer {
    state: FlatHudHotbarState,
    draw: GuiDrawList,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FlatHudSurface {
    retained: Option<CachedFlatHudRetainedLayer>,
    hotbar: Option<CachedFlatHudHotbarLayer>,
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameUiHost {
    legacy: GameUi,
    surface: UiSurface,
    committed_render_state: GameUiRenderState,
    cached_v2_draw: Option<CachedV2DrawList>,
    hud_surface: FlatHudSurface,
}

impl Default for GameUiHost {
    fn default() -> Self {
        Self::new()
    }
}

impl GameUiHost {
    pub fn new() -> Self {
        Self::from_game_ui(GameUi::new())
    }

    pub fn new_ingame() -> Self {
        Self::from_game_ui(GameUi::new_ingame())
    }

    pub fn from_game_ui(legacy: GameUi) -> Self {
        let mut host = Self {
            legacy,
            surface: UiSurface::new(),
            committed_render_state: GameUiRenderState::default(),
            cached_v2_draw: None,
            hud_surface: FlatHudSurface::default(),
        };
        host.sync_surface_screen();
        host.surface.set_scale(host.legacy.scale());
        host
    }

    pub fn screen(&self) -> Option<GameScreen> {
        self.legacy.screen()
    }

    pub fn new_world_seed(&self) -> i64 {
        self.legacy.new_world_seed()
    }

    pub fn set_new_world_seed(&mut self, seed: i64) {
        self.legacy.set_new_world_seed(seed);
    }

    pub fn join_remote_addr(&self) -> &str {
        self.legacy.join_remote_addr()
    }

    pub fn set_join_remote_addr(&mut self, addr: impl Into<String>) {
        self.legacy.set_join_remote_addr(addr);
    }

    pub fn scale(&self) -> GuiScale {
        self.legacy.scale()
    }

    pub fn font(&self) -> &Font {
        self.legacy.font()
    }

    pub fn set_scale(&mut self, scale: GuiScale) {
        self.legacy.set_scale(scale);
        self.surface.set_scale(scale);
    }

    pub fn is_active(&self) -> bool {
        self.legacy.is_active()
    }

    pub fn covers_world(&self) -> bool {
        self.legacy.covers_world()
    }

    pub fn open_pause(&mut self) {
        self.legacy.open_pause();
        self.sync_surface_screen();
    }

    pub fn close(&mut self) {
        self.legacy.close();
        self.sync_surface_screen();
    }

    pub fn set_screen(&mut self, screen: Option<GameScreen>) {
        self.legacy.set_screen(screen);
        self.sync_surface_screen();
    }

    pub fn clear_input(&mut self) {
        self.legacy.clear_input();
        self.surface.clear_input();
    }

    pub fn key_pressed(&mut self, key: GuiKey) -> (bool, Option<GameUiAction>) {
        if self.sync_surface_screen() {
            return self.surface.key_pressed(key);
        }
        self.legacy.key_pressed(key)
    }

    pub fn pointer_move(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        let state = self.committed_render_state;
        if self.sync_surface_screen() {
            return self.surface.pointer_move(point, state);
        }
        self.legacy.pointer_move(point, state)
    }

    pub fn pointer_down(&mut self, point: Point) -> bool {
        let state = self.committed_render_state;
        if self.sync_surface_screen() {
            return self.surface.pointer_down(point, state);
        }
        self.legacy.pointer_down(point, state)
    }

    pub fn pointer_up(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        let state = self.committed_render_state;
        if self.sync_surface_screen() {
            return self.surface.pointer_up(point, state);
        }
        self.legacy.pointer_up(point, state)
    }

    pub fn apply_action(&mut self, action: GameUiAction) {
        self.legacy.apply_action(action);
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
            self.legacy.render_draw_list(self.committed_render_state)
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
        let mut draw = retained.draw.clone();
        draw.append(&hotbar.draw);
        render_flat_hud_transient_layers(scale, &mut draw, hud);
        let mut retained_cache = retained.retained_cache;
        retained_cache.add(hotbar.retained_cache);
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

    pub fn set_v2_debug_overlay(&mut self, enabled: bool) {
        self.surface.set_debug_overlay(enabled);
    }

    pub fn v2_is_active(&self) -> bool {
        UiScreenId::from_game_screen(self.legacy.screen()).is_some()
    }

    pub fn v2_debug_snapshot(&mut self) -> Option<UiDebugSnapshot> {
        self.sync_surface_screen();
        self.surface.debug_snapshot()
    }

    fn sync_surface_screen(&mut self) -> bool {
        let screen = UiScreenId::from_game_screen(self.legacy.screen());
        self.surface.set_screen(screen);
        screen.is_some()
    }
}

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
const UI_V2_OPTIONS_XR_TURN_MODE: UiWidgetId = UiWidgetId(120);
const UI_V2_HELP_BACK: UiWidgetId = UiWidgetId(201);
const UI_V2_BLOCK_PALETTE_BASE: u64 = 3000;

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
    Rect::new(scale.width * 0.5 - 90.0, y, 180.0, 20.0)
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
        GameHelpParent::Title | GameHelpParent::NewWorld | GameHelpParent::JoinRemote
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
    let mut left_y = panel.y + 34.0;
    let mut right_y = panel.y + 34.0;

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
    if let Some(turn_mode) = state.xr_turn_mode {
        push_cycle(
            &mut layout,
            UI_V2_OPTIONS_XR_TURN_MODE,
            Rect::new(right_x, right_y, column_width, 20.0),
            "XR Turn",
            turn_mode.label(),
            GameUiAction::SetXrTurnMode(turn_mode.next()),
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
    let right_rows_height = (7
        + usize::from(state.xr_turn_mode.is_some())
        + usize::from(state.touch_settings.is_some())) as f32
        * 22.0;
    let panel_width = (scale.width - 18.0).clamp(242.0, 420.0);
    let panel_height =
        (44.0 + left_rows_height.max(right_rows_height)).min((scale.height - 4.0).max(1.0));
    centered_panel(scale, panel_width, panel_height)
}

const fn help_parent_for_options(parent: GameOptionsParent) -> GameHelpParent {
    match parent {
        GameOptionsParent::Title => GameHelpParent::OptionsTitle,
        GameOptionsParent::Pause => GameHelpParent::OptionsPause,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EMPTY_BLOCK_PALETTE_ENTRIES, EMPTY_HOTBAR_ICONS, FlatHotbarOverlay, GameSimulationCadence,
        GameTouchSettings, GameUi, GameXrTurnMode, GuiDrawCommand, GuiTextureUv, render_flat_hud,
    };
    use mclone_input::{InputPromptKind, ResolvedFlatInput, TouchControlsMode};

    fn point_in(rect: Rect) -> Point {
        Point {
            x: rect.center_x(),
            y: rect.y + rect.height * 0.5,
        }
    }

    fn hovered_label(snapshot: &UiDebugSnapshot) -> Option<&str> {
        let hovered = snapshot.hovered?;
        snapshot
            .widgets
            .iter()
            .find(|widget| widget.id == hovered)
            .map(|widget| widget.label.as_str())
    }

    fn keyboard_mouse_input() -> ResolvedFlatInput {
        ResolvedFlatInput {
            preferred_prompt: Some(InputPromptKind::KeyboardMouse),
            touch_controls_visible: false,
            accepts_keyboard_mouse: true,
            accepts_touch: false,
            accepts_gamepad: false,
            accepts_xr_controller: false,
        }
    }

    fn block_palette_state(selected_hotbar_slot: u8) -> GameUiRenderState {
        let brick_icon = GuiTextureUv::new(0.1, 0.2, 0.3, 0.4);
        let log_icon = GuiTextureUv::new(0.4, 0.3, 0.2, 0.1);
        let mut entries = EMPTY_BLOCK_PALETTE_ENTRIES;
        entries[0] = Some(BlockPaletteEntry::new(91, Some(brick_icon), "Bricks"));
        entries[1] = Some(BlockPaletteEntry::new(41, Some(log_icon), "Oak Log"));
        GameUiRenderState {
            block_palette: BlockPaletteOverlay::visible(selected_hotbar_slot, entries),
            ..GameUiRenderState::default()
        }
    }

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
        let (_handled, action) =
            surface.pointer_up(point_in(options), GameUiRenderState::default());

        assert_eq!(action, None);
    }

    #[test]
    fn pointer_move_updates_hover_without_action() {
        let mut surface = UiSurface::new();
        surface.set_screen(Some(UiScreenId::Pause));
        surface.set_scale(GuiScale::from_pixels(960, 540));
        let options = surface.layout().widgets()[1].rect;

        let (handled, action) =
            surface.pointer_move(point_in(options), GameUiRenderState::default());
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
        host.set_screen(Some(GameScreen::Options {
            parent: GameOptionsParent::Pause,
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
    fn game_ui_host_v2_panel_draw_cache_ignores_legacy_screens() {
        let mut host = GameUiHost::new();
        host.set_screen(Some(GameScreen::Title));

        assert!(
            host.render_v2_panel_draw_list(GameUiRenderState::default())
                .is_none()
        );
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
        assert_eq!(widgets.len(), 2);
        assert_eq!(widgets[0].id, block_palette_slot_id(0));
        assert_eq!(widgets[0].label, "Bricks");
        assert_eq!(
            widgets[0].rect,
            block_palette_slot_rect(scale, state.block_palette, 0)
                .expect("first palette slot rect")
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
    fn game_ui_host_flat_hud_draw_matches_standalone_renderer() {
        let scale = GuiScale::from_pixels(960, 540);
        let mut host = GameUiHost::new_ingame();
        let mut hud = FlatHud::new(keyboard_mouse_input());
        let mut icons = EMPTY_HOTBAR_ICONS;
        icons[0] = Some(GuiTextureUv::new(0.1, 0.2, 0.3, 0.4));
        hud.hotbar = FlatHotbarOverlay::selected_with_icons(4, icons);
        hud.status = crate::StatusOverlay::new("ready", true);

        let retained = host.render_flat_hud_draw_list(scale, &hud);
        let mut standalone = GuiDrawList::new();
        render_flat_hud(scale, &mut standalone, &hud);

        assert_eq!(retained.draw, standalone);
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

        let mut legacy = GameUi::new();
        legacy.set_screen(Some(GameScreen::Help {
            parent: GameHelpParent::Game,
        }));
        legacy.set_scale(GuiScale::from_pixels(960, 540));
        let legacy_draw = legacy.render_draw_list(GameUiRenderState::default());

        assert!(
            v2_draw.commands().len() < legacy_draw.commands().len(),
            "v2={} legacy={}",
            v2_draw.commands().len(),
            legacy_draw.commands().len()
        );
        assert!(
            v2_draw
                .commands()
                .iter()
                .any(|command| matches!(command, GuiDrawCommand::Text { .. }))
        );
    }
}
