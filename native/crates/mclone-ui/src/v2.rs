use crate::{
    Button, Checkbox, Color, CycleButton, Font, GameHelpParent, GameOptionsParent, GameScreen,
    GameUiAction, GameUiRenderState, GuiDrawList, GuiKey, GuiScale, Interaction, Point, Rect,
    Slider, WidgetId, centered_panel, far_lod_range_from_slider_value, far_lod_range_label,
    far_lod_range_slider_value, fly_speed_from_slider_value, fly_speed_label,
    fly_speed_slider_value, movement_speed_from_slider_value, movement_speed_label,
    movement_speed_slider_value, next_touch_controls_mode, render_distance_from_slider_value,
    render_distance_label, render_distance_slider_value, touch_controls_mode_label,
    touch_look_from_slider_value, touch_look_label, touch_look_slider_value,
};
use mclone_input::{
    ShortcutHelpGroup, ShortcutHelpRow, default_keyboard_mouse_shortcut_rows,
    flat_runtime_shortcut_rows,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiScreenId {
    Pause,
    Options { parent: GameOptionsParent },
    Help { parent: GameHelpParent },
}

impl UiScreenId {
    pub fn from_game_screen(screen: Option<GameScreen>) -> Option<Self> {
        match screen {
            Some(GameScreen::Pause) => Some(Self::Pause),
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

#[derive(Clone, Debug, PartialEq)]
pub struct UiSurface {
    screen: Option<UiScreenId>,
    scale: GuiScale,
    frame_revision: u64,
    layout_revision: u64,
    layout_dirty: bool,
    layout: UiLayout,
    render_state: GameUiRenderState,
    pointer: Option<Point>,
    hovered: Option<UiWidgetId>,
    captured: Option<UiWidgetId>,
    font: Font,
    debug_overlay: bool,
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
            layout_revision: 0,
            layout_dirty: true,
            layout: UiLayout::new(None, 0),
            render_state: GameUiRenderState::default(),
            pointer: None,
            hovered: None,
            captured: None,
            font: Font::default(),
            debug_overlay: false,
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
        self.pointer = None;
        self.hovered = None;
        self.captured = None;
    }

    pub fn set_scale(&mut self, scale: GuiScale) {
        if self.scale == scale {
            return;
        }
        self.scale = scale;
        self.frame_revision = self.frame_revision.wrapping_add(1);
        self.layout_dirty = true;
        self.pointer = self.pointer.map(|point| Point {
            x: point.x.clamp(0.0, scale.width),
            y: point.y.clamp(0.0, scale.height),
        });
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
        self.debug_overlay = enabled;
    }

    pub fn clear_input(&mut self) {
        self.pointer = None;
        self.hovered = None;
        self.captured = None;
    }

    pub fn frame_state(&self) -> Option<UiFrameState> {
        self.screen.map(|screen| {
            UiFrameState::new(screen, self.scale, self.render_state, self.frame_revision)
        })
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
        self.pointer = Some(point);
        self.hovered = self.layout.hit_test(point);
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
        self.pointer = Some(point);
        self.hovered = self.layout.hit_test(point);
        self.captured = self.hovered;
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
        self.pointer = Some(point);
        self.hovered = self.layout.hit_test(point);
        let captured = self.captured.take();
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
            self.captured = None;
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction {
            pointer: self.pointer,
            pressed: self.captured.map(UiWidgetId::legacy_widget_id),
            focused: None,
        }
    }

    fn render_pause(&self, draw: &mut GuiDrawList) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 135),
        );
        self.font.draw_centered(
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
        self.font.draw_centered(
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
        self.font.draw_centered(
            draw,
            "CONTROLS",
            panel.center_x(),
            panel.y + 8.0,
            Color::rgba(245, 252, 234, 255),
        );
        self.font.draw_centered(
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
                self.font.draw_shadow(
                    draw,
                    group.label(),
                    row.x,
                    row.y,
                    Color::rgba(220, 238, 220, 255),
                );
            }
            UiHelpRowKind::Shortcut(shortcut) => {
                self.font.draw_shadow(
                    draw,
                    &shortcut.control,
                    row.x,
                    row.y,
                    Color::rgba(185, 212, 198, 255),
                );
                self.font.draw_shadow(
                    draw,
                    &shortcut.action,
                    row.x + row.control_width,
                    row.y,
                    Color::rgba(214, 226, 218, 255),
                );
            }
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
            .render(draw, &self.font, interaction),
            UiWidgetKind::Checkbox { checked } => {
                let mut checkbox = Checkbox::new(
                    widget.id.legacy_widget_id(),
                    widget.rect,
                    widget.label.as_str(),
                    *checked,
                );
                checkbox.enabled = widget.enabled;
                checkbox.render(draw, &self.font, interaction);
            }
            UiWidgetKind::Cycle => {
                let mut cycle = CycleButton::new(
                    widget.id.legacy_widget_id(),
                    widget.rect,
                    widget.label.as_str(),
                    widget.value.as_deref().unwrap_or(""),
                );
                cycle.enabled = widget.enabled;
                cycle.render(draw, &self.font, interaction);
            }
            UiWidgetKind::Slider { value } => Slider::new(
                widget.id.legacy_widget_id(),
                widget.rect,
                widget.label.as_str(),
                *value,
            )
            .enabled(widget.enabled)
            .render(draw, &self.font, interaction),
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
const UI_V2_HELP_BACK: UiWidgetId = UiWidgetId(201);

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
    let right_rows_height = (7 + usize::from(state.touch_settings.is_some())) as f32 * 22.0;
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
    use crate::{GameSimulationCadence, GameTouchSettings, GameUi};
    use mclone_input::TouchControlsMode;

    fn point_in(rect: Rect) -> Point {
        Point {
            x: rect.center_x(),
            y: rect.y + rect.height * 0.5,
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
            touch_controls_mode: Some(TouchControlsMode::Auto),
            touch_settings: Some(GameTouchSettings::new(2.0, 1.0, 5.0)),
            server_cadence: Some(GameSimulationCadence::default()),
            ..GameUiRenderState::default()
        });

        let layout = surface.layout();

        assert!(layout.widget(UI_V2_OPTIONS_CROSSHAIR).is_none());
        assert!(layout.widget(UI_V2_OPTIONS_TOUCH_CONTROLS).is_some());
        assert!(layout.widget(UI_V2_OPTIONS_TOUCH_LOOK).is_some());
        assert!(layout.widget(UI_V2_OPTIONS_SERVER_SETTINGS).is_some());
    }

    #[test]
    fn options_buttons_emit_expected_actions_from_committed_rects() {
        let mut surface = UiSurface::new();
        surface.set_screen(Some(UiScreenId::Options {
            parent: GameOptionsParent::Pause,
        }));
        surface.set_scale(GuiScale::from_pixels(960, 540));
        surface.set_render_state(GameUiRenderState {
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
    fn help_render_uses_committed_rows_and_matches_legacy_command_scale() {
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

        assert_eq!(v2_draw.commands().len(), legacy_draw.commands().len());
    }
}
