use crate::{
    Button, Color, Font, GameHelpParent, GameOptionsParent, GameScreen, GameUiAction, GuiDrawList,
    GuiKey, GuiScale, Interaction, Point, Rect, WidgetId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiScreenId {
    Pause,
}

impl UiScreenId {
    pub fn from_game_screen(screen: Option<GameScreen>) -> Option<Self> {
        match screen {
            Some(GameScreen::Pause) => Some(Self::Pause),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiFrameState {
    pub screen: UiScreenId,
    pub scale: GuiScale,
    pub revision: u64,
}

impl UiFrameState {
    pub const fn new(screen: UiScreenId, scale: GuiScale, revision: u64) -> Self {
        Self {
            screen,
            scale,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiWidgetKind {
    Button,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiWidget {
    pub id: UiWidgetId,
    pub kind: UiWidgetKind,
    pub rect: Rect,
    pub label: String,
    pub enabled: bool,
    pub action: Option<GameUiAction>,
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
        }
    }

    pub fn action(mut self, action: GameUiAction) -> Self {
        self.action = Some(action);
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiLayout {
    pub screen: Option<UiScreenId>,
    pub revision: u64,
    widgets: Vec<UiWidget>,
}

impl UiLayout {
    pub fn new(screen: Option<UiScreenId>, revision: u64) -> Self {
        Self {
            screen,
            revision,
            widgets: Vec::new(),
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiDebugWidget {
    pub id: UiWidgetId,
    pub kind: UiWidgetKind,
    pub rect: Rect,
    pub label: String,
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

    pub fn set_debug_overlay(&mut self, enabled: bool) {
        self.debug_overlay = enabled;
    }

    pub fn clear_input(&mut self) {
        self.pointer = None;
        self.hovered = None;
        self.captured = None;
    }

    pub fn frame_state(&self) -> Option<UiFrameState> {
        self.screen
            .map(|screen| UiFrameState::new(screen, self.scale, self.frame_revision))
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
                    enabled: widget.enabled,
                })
                .collect(),
        })
    }

    pub fn pointer_move(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.ensure_layout();
        self.pointer = Some(point);
        self.hovered = self.layout.hit_test(point);
        (true, None)
    }

    pub fn pointer_down(&mut self, point: Point) -> bool {
        if !self.is_active() {
            return false;
        }
        self.ensure_layout();
        self.pointer = Some(point);
        self.hovered = self.layout.hit_test(point);
        self.captured = self.hovered;
        true
    }

    pub fn pointer_up(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.ensure_layout();
        self.pointer = Some(point);
        self.hovered = self.layout.hit_test(point);
        let captured = self.captured.take();
        let action = match (captured, self.hovered) {
            (Some(captured), Some(released)) if captured == released => self
                .layout
                .widget(captured)
                .and_then(|widget| widget.action),
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
            (None, _) => (false, None),
        }
    }

    pub fn render_draw_list(&mut self) -> GuiDrawList {
        self.ensure_layout();
        let mut draw = GuiDrawList::new();
        match self.screen {
            Some(UiScreenId::Pause) => self.render_pause(&mut draw),
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
            match widget.kind {
                UiWidgetKind::Button => Button::new(
                    widget.id.legacy_widget_id(),
                    widget.rect,
                    widget.label.as_str(),
                )
                .enabled(widget.enabled)
                .render(draw, &self.font, interaction),
            }
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
}

const UI_V2_PAUSE_RESUME: UiWidgetId = UiWidgetId(1);
const UI_V2_PAUSE_OPTIONS: UiWidgetId = UiWidgetId(2);
const UI_V2_PAUSE_QUIT_TO_TITLE: UiWidgetId = UiWidgetId(3);

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

#[cfg(test)]
mod tests {
    use super::*;

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

        assert!(surface.pointer_down(point_in(resume)));
        let (_handled, action) = surface.pointer_up(point_in(options));

        assert_eq!(action, None);
    }

    #[test]
    fn pointer_move_updates_hover_without_action() {
        let mut surface = UiSurface::new();
        surface.set_screen(Some(UiScreenId::Pause));
        surface.set_scale(GuiScale::from_pixels(960, 540));
        let options = surface.layout().widgets()[1].rect;

        let (handled, action) = surface.pointer_move(point_in(options));
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
            assert!(surface.pointer_down(point));
            let (_handled, action) = surface.pointer_up(point);
            assert_eq!(action, Some(expected));
        }
    }

    #[test]
    fn pause_render_uses_committed_layout() {
        let mut surface = UiSurface::new();
        surface.set_screen(Some(UiScreenId::Pause));
        surface.set_scale(GuiScale::from_pixels(960, 540));
        let layout_revision = surface.layout().revision;

        let draw = surface.render_draw_list();

        assert_eq!(surface.layout().revision, layout_revision);
        assert!(!draw.commands().is_empty());
    }
}
