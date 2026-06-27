#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuiScale {
    pub scale: u32,
    pub width: f32,
    pub height: f32,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

impl GuiScale {
    pub fn from_pixels(pixel_width: u32, pixel_height: u32) -> Self {
        let pixel_width = pixel_width.max(1);
        let pixel_height = pixel_height.max(1);
        let mut scale = 1;
        while scale < 4 && pixel_width / (scale + 1) >= 320 && pixel_height / (scale + 1) >= 240 {
            scale += 1;
        }
        Self {
            scale,
            width: (pixel_width as f32 / scale as f32).ceil(),
            height: (pixel_height as f32 / scale as f32).ceil(),
            pixel_width,
            pixel_height,
        }
    }

    pub fn client_to_gui(self, x: f64, y: f64) -> Point {
        Point {
            x: x as f32 / self.scale as f32,
            y: y as f32 / self.scale as f32,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub fn center_x(self) -> f32 {
        self.x + self.width * 0.5
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x && point.y >= self.y && point.x < self.right() && point.y < self.bottom()
    }

    pub fn inset(self, amount: f32) -> Self {
        Self {
            x: self.x + amount,
            y: self.y + amount,
            width: (self.width - amount * 2.0).max(0.0),
            height: (self.height - amount * 2.0).max(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
    pub const WHITE: Self = Self::rgba(255, 255, 255, 255);
    pub const BLACK: Self = Self::rgba(0, 0, 0, 255);

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_linear_f32(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    pub fn scale_rgb(self, scale: f32) -> Self {
        let scale = scale.max(0.0);
        Self {
            r: ((self.r as f32 * scale).round()).clamp(0.0, 255.0) as u8,
            g: ((self.g as f32 * scale).round()).clamp(0.0, 255.0) as u8,
            b: ((self.b as f32 * scale).round()).clamp(0.0, 255.0) as u8,
            a: self.a,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<Rect> for ClipRect {
    fn from(value: Rect) -> Self {
        Self {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GuiDrawCommand {
    SolidRect {
        rect: Rect,
        color: Color,
        clip: Option<ClipRect>,
    },
    GradientRect {
        rect: Rect,
        top: Color,
        bottom: Color,
        clip: Option<ClipRect>,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuiDrawList {
    commands: Vec<GuiDrawCommand>,
    clip_stack: Vec<ClipRect>,
}

impl GuiDrawList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.commands.clear();
        self.clip_stack.clear();
    }

    pub fn commands(&self) -> &[GuiDrawCommand] {
        &self.commands
    }

    pub fn fill(&mut self, rect: Rect, color: Color) {
        if rect.width <= 0.0 || rect.height <= 0.0 || color.a == 0 {
            return;
        }
        self.commands.push(GuiDrawCommand::SolidRect {
            rect,
            color,
            clip: self.current_clip(),
        });
    }

    pub fn fill_gradient(&mut self, rect: Rect, top: Color, bottom: Color) {
        if rect.width <= 0.0 || rect.height <= 0.0 || (top.a == 0 && bottom.a == 0) {
            return;
        }
        self.commands.push(GuiDrawCommand::GradientRect {
            rect,
            top,
            bottom,
            clip: self.current_clip(),
        });
    }

    pub fn outline(&mut self, rect: Rect, color: Color) {
        self.fill(Rect::new(rect.x, rect.y, rect.width, 1.0), color);
        self.fill(
            Rect::new(rect.x, rect.bottom() - 1.0, rect.width, 1.0),
            color,
        );
        self.fill(Rect::new(rect.x, rect.y, 1.0, rect.height), color);
        self.fill(
            Rect::new(rect.right() - 1.0, rect.y, 1.0, rect.height),
            color,
        );
    }

    pub fn push_clip(&mut self, rect: Rect) {
        let next = ClipRect::from(normalized_rect(rect));
        let clip = self
            .current_clip()
            .map_or(next, |current| intersect_clip(current, next));
        self.clip_stack.push(clip);
    }

    pub fn pop_clip(&mut self) {
        let _ = self.clip_stack.pop();
    }

    fn current_clip(&self) -> Option<ClipRect> {
        self.clip_stack.last().copied()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct WidgetId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Interaction {
    pub pointer: Option<Point>,
    pub pressed: Option<WidgetId>,
    pub focused: Option<WidgetId>,
}

impl Interaction {
    pub fn is_hovered(self, rect: Rect) -> bool {
        self.pointer.is_some_and(|point| rect.contains(point))
    }

    pub fn is_pressed(self, id: WidgetId) -> bool {
        self.pressed == Some(id)
    }

    pub fn is_focused(self, id: WidgetId) -> bool {
        self.focused == Some(id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonVisual {
    Normal,
    Hovered,
    Pressed,
    Disabled,
    Focused,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Font {
    glyph_width: u8,
    glyph_height: u8,
    advance: u8,
    line_height: u8,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            glyph_width: 5,
            glyph_height: 7,
            advance: 6,
            line_height: 9,
        }
    }
}

impl Font {
    pub fn line_height(&self) -> f32 {
        self.line_height as f32
    }

    pub fn width(&self, text: &str) -> f32 {
        text.chars().count() as f32 * self.advance as f32
    }

    pub fn draw(&self, draw: &mut GuiDrawList, text: &str, x: f32, y: f32, color: Color) {
        self.draw_internal(draw, text, x, y, color, false);
    }

    pub fn draw_shadow(&self, draw: &mut GuiDrawList, text: &str, x: f32, y: f32, color: Color) {
        self.draw_internal(draw, text, x, y, color, true);
    }

    pub fn draw_centered(
        &self,
        draw: &mut GuiDrawList,
        text: &str,
        center_x: f32,
        y: f32,
        color: Color,
    ) {
        self.draw_shadow(draw, text, center_x - self.width(text) * 0.5, y, color);
    }

    fn draw_internal(
        &self,
        draw: &mut GuiDrawList,
        text: &str,
        x: f32,
        y: f32,
        color: Color,
        shadow: bool,
    ) {
        if shadow {
            self.draw_glyphs(draw, text, x + 1.0, y + 1.0, color.scale_rgb(0.22));
        }
        self.draw_glyphs(draw, text, x, y, color);
    }

    fn draw_glyphs(&self, draw: &mut GuiDrawList, text: &str, x: f32, y: f32, color: Color) {
        let mut cursor = x.floor();
        for ch in text.chars() {
            self.draw_glyph(draw, ch, cursor, y.floor(), color);
            cursor += self.advance as f32;
        }
    }

    fn draw_glyph(&self, draw: &mut GuiDrawList, ch: char, x: f32, y: f32, color: Color) {
        if ch == ' ' || color.a == 0 {
            return;
        }
        let rows = glyph_rows(ch);
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..self.glyph_width {
                let mask = 1 << (self.glyph_width - 1 - col);
                if bits & mask != 0 {
                    draw.fill(Rect::new(x + col as f32, y + row as f32, 1.0, 1.0), color);
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub id: WidgetId,
    pub rect: Rect,
    pub label: String,
    pub enabled: bool,
}

impl Button {
    pub fn new(id: WidgetId, rect: Rect, label: impl Into<String>) -> Self {
        Self {
            id,
            rect,
            label: label.into(),
            enabled: true,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn contains(&self, point: Point) -> bool {
        self.enabled && self.rect.contains(point)
    }

    pub fn visual(&self, interaction: Interaction) -> ButtonVisual {
        if !self.enabled {
            ButtonVisual::Disabled
        } else if interaction.is_pressed(self.id) && interaction.is_hovered(self.rect) {
            ButtonVisual::Pressed
        } else if interaction.is_hovered(self.rect) {
            ButtonVisual::Hovered
        } else if interaction.is_focused(self.id) {
            ButtonVisual::Focused
        } else {
            ButtonVisual::Normal
        }
    }

    pub fn render(&self, draw: &mut GuiDrawList, font: &Font, interaction: Interaction) {
        let visual = self.visual(interaction);
        let (top, bottom, border, text, y_offset) = match visual {
            ButtonVisual::Normal => (
                Color::rgba(65, 80, 86, 235),
                Color::rgba(36, 46, 50, 235),
                Color::rgba(140, 170, 170, 255),
                Color::rgba(240, 246, 238, 255),
                0.0,
            ),
            ButtonVisual::Hovered | ButtonVisual::Focused => (
                Color::rgba(84, 112, 106, 245),
                Color::rgba(45, 62, 60, 245),
                Color::rgba(196, 224, 180, 255),
                Color::rgba(255, 255, 225, 255),
                0.0,
            ),
            ButtonVisual::Pressed => (
                Color::rgba(32, 42, 44, 245),
                Color::rgba(55, 70, 68, 245),
                Color::rgba(92, 128, 122, 255),
                Color::rgba(210, 222, 206, 255),
                1.0,
            ),
            ButtonVisual::Disabled => (
                Color::rgba(42, 46, 48, 170),
                Color::rgba(30, 32, 34, 170),
                Color::rgba(75, 78, 78, 210),
                Color::rgba(135, 140, 136, 255),
                0.0,
            ),
        };
        draw.fill_gradient(self.rect, top, bottom);
        draw.outline(self.rect, border);
        draw.outline(self.rect.inset(1.0), Color::rgba(8, 10, 12, 180));
        font.draw_centered(
            draw,
            &self.label,
            self.rect.center_x(),
            self.rect.y + ((self.rect.height - font.line_height()) * 0.5).floor() + y_offset,
            text,
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Checkbox {
    pub id: WidgetId,
    pub rect: Rect,
    pub label: String,
    pub checked: bool,
    pub enabled: bool,
}

impl Checkbox {
    pub fn new(id: WidgetId, rect: Rect, label: impl Into<String>, checked: bool) -> Self {
        Self {
            id,
            rect,
            label: label.into(),
            checked,
            enabled: true,
        }
    }

    pub fn contains(&self, point: Point) -> bool {
        self.enabled && self.rect.contains(point)
    }

    pub fn render(&self, draw: &mut GuiDrawList, font: &Font, interaction: Interaction) {
        let hovered = interaction.is_hovered(self.rect);
        let box_rect = Rect::new(self.rect.x, self.rect.y + 2.0, 12.0, 12.0);
        let border = if hovered {
            Color::rgba(196, 224, 180, 255)
        } else {
            Color::rgba(130, 154, 152, 255)
        };
        draw.fill(box_rect, Color::rgba(18, 24, 25, 235));
        draw.outline(box_rect, border);
        if self.checked {
            draw.fill(
                Rect::new(box_rect.x + 3.0, box_rect.y + 6.0, 2.0, 3.0),
                Color::WHITE,
            );
            draw.fill(
                Rect::new(box_rect.x + 5.0, box_rect.y + 8.0, 2.0, 2.0),
                Color::WHITE,
            );
            draw.fill(
                Rect::new(box_rect.x + 7.0, box_rect.y + 4.0, 2.0, 6.0),
                Color::WHITE,
            );
        }
        font.draw_shadow(
            draw,
            &self.label,
            self.rect.x + 18.0,
            self.rect.y + 4.0,
            if self.enabled {
                Color::rgba(235, 242, 232, 255)
            } else {
                Color::rgba(135, 140, 136, 255)
            },
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Slider {
    pub id: WidgetId,
    pub rect: Rect,
    pub label: String,
    pub value: f32,
    pub enabled: bool,
}

impl Slider {
    pub fn new(id: WidgetId, rect: Rect, label: impl Into<String>, value: f32) -> Self {
        Self {
            id,
            rect,
            label: label.into(),
            value: value.clamp(0.0, 1.0),
            enabled: true,
        }
    }

    pub fn contains(&self, point: Point) -> bool {
        self.enabled && self.rect.contains(point)
    }

    pub fn value_from_point(&self, point: Point) -> f32 {
        ((point.x - self.rect.x) / self.rect.width.max(1.0)).clamp(0.0, 1.0)
    }

    pub fn render(&self, draw: &mut GuiDrawList, font: &Font, interaction: Interaction) {
        Button::new(self.id, self.rect, &self.label)
            .enabled(self.enabled)
            .render(draw, font, interaction);
        let track = Rect::new(
            self.rect.x + 8.0,
            self.rect.bottom() - 6.0,
            self.rect.width - 16.0,
            2.0,
        );
        draw.fill(track, Color::rgba(15, 18, 18, 230));
        let knob_x = track.x + self.value * track.width;
        draw.fill(
            Rect::new(knob_x - 2.0, track.y - 4.0, 4.0, 10.0),
            Color::rgba(215, 230, 198, 255),
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CycleButton {
    pub id: WidgetId,
    pub rect: Rect,
    pub label: String,
    pub value: String,
    pub enabled: bool,
}

impl CycleButton {
    pub fn new(
        id: WidgetId,
        rect: Rect,
        label: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            id,
            rect,
            label: label.into(),
            value: value.into(),
            enabled: true,
        }
    }

    pub fn contains(&self, point: Point) -> bool {
        self.enabled && self.rect.contains(point)
    }

    pub fn render(&self, draw: &mut GuiDrawList, font: &Font, interaction: Interaction) {
        Button::new(
            self.id,
            self.rect,
            format!("{}: {}", self.label, self.value),
        )
        .enabled(self.enabled)
        .render(draw, font, interaction);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuiKey {
    Escape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameScreen {
    Title,
    NewWorld,
    JoinRemote,
    Pause,
    Options { parent: GameOptionsParent },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameOptionsParent {
    Title,
    Pause,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameUiAction {
    StartWorld,
    OpenNewWorld,
    OpenJoinRemote,
    RerollSeed,
    CreateWorld(i64),
    JoinRemote,
    Resume,
    OpenOptions(GameOptionsParent),
    BackToTitle,
    BackToPause,
    QuitToTitle,
    ToggleSectionOcclusion,
    ToggleFullbright,
    ToggleFly,
    CycleFramePacing,
    CycleFpsCap,
    SetRenderDistance(i32),
    SetFlySpeed(f32),
    SetTouchLookSensitivity(f32),
    Quit,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GameFramePacingMode {
    #[default]
    Vsync,
    Capped,
    Uncapped,
}

impl GameFramePacingMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Vsync => "VSync",
            Self::Capped => "Max FPS",
            Self::Uncapped => "Uncapped",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameTouchSettings {
    pub look_sensitivity: f32,
    pub min_look_sensitivity: f32,
    pub max_look_sensitivity: f32,
}

impl GameTouchSettings {
    pub fn new(
        look_sensitivity: f32,
        min_look_sensitivity: f32,
        max_look_sensitivity: f32,
    ) -> Self {
        Self {
            look_sensitivity,
            min_look_sensitivity,
            max_look_sensitivity,
        }
    }

    pub fn look_sensitivity_limits(self) -> (f32, f32) {
        let min = finite_or(self.min_look_sensitivity, 0.0);
        let max = finite_or(self.max_look_sensitivity, min);
        (min.min(max), min.max(max))
    }

    pub fn clamped_look_sensitivity(self) -> f32 {
        let (min, max) = self.look_sensitivity_limits();
        finite_or(self.look_sensitivity, min).clamp(min, max)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameUiRenderState {
    pub render_distance: i32,
    pub min_render_distance: i32,
    pub max_render_distance: i32,
    pub section_occlusion_culling: bool,
    pub force_fullbright: bool,
    pub fly_enabled: bool,
    pub fly_speed_multiplier: f32,
    pub min_fly_speed_multiplier: f32,
    pub max_fly_speed_multiplier: f32,
    pub frame_pacing_mode: GameFramePacingMode,
    pub fps_cap: u32,
    pub touch_settings: Option<GameTouchSettings>,
}

impl Default for GameUiRenderState {
    fn default() -> Self {
        Self {
            render_distance: 2,
            min_render_distance: 2,
            max_render_distance: 16,
            section_occlusion_culling: true,
            force_fullbright: false,
            fly_enabled: false,
            fly_speed_multiplier: 1.0,
            min_fly_speed_multiplier: 0.5,
            max_fly_speed_multiplier: 8.0,
            frame_pacing_mode: GameFramePacingMode::Vsync,
            fps_cap: 120,
            touch_settings: None,
        }
    }
}

impl GameUiRenderState {
    pub fn render_distance_limits(self) -> (i32, i32) {
        (
            self.min_render_distance.min(self.max_render_distance),
            self.min_render_distance.max(self.max_render_distance),
        )
    }

    pub fn clamped_render_distance(self) -> i32 {
        let (min, max) = self.render_distance_limits();
        self.render_distance.clamp(min, max)
    }

    pub fn fly_speed_multiplier_limits(self) -> (f32, f32) {
        let min = finite_or(self.min_fly_speed_multiplier, 0.5);
        let max = finite_or(self.max_fly_speed_multiplier, min);
        (min.min(max), min.max(max))
    }

    pub fn clamped_fly_speed_multiplier(self) -> f32 {
        let (min, max) = self.fly_speed_multiplier_limits();
        finite_or(self.fly_speed_multiplier, min).clamp(min, max)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DebugOverlay {
    pub title: String,
    pub lines: Vec<String>,
}

impl DebugOverlay {
    pub fn new<I, S>(title: impl Into<String>, lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            title: title.into(),
            lines: lines.into_iter().map(Into::into).collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.lines.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StatusOverlay {
    pub message: String,
    pub ok: bool,
    pub visible: bool,
}

impl StatusOverlay {
    pub fn hidden() -> Self {
        Self {
            message: String::new(),
            ok: true,
            visible: false,
        }
    }

    pub fn new(message: impl Into<String>, ok: bool) -> Self {
        let message = message.into();
        Self {
            visible: !message.is_empty(),
            message,
            ok,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TouchOverlay {
    pub visible: bool,
    pub menu_pressed: bool,
    pub movement: TouchJoystickOverlay,
    pub jump_pressed: bool,
    pub sprint_pressed: bool,
    pub descend_pressed: bool,
    pub interaction_visible: bool,
    pub attack_pressed: bool,
    pub use_pressed: bool,
    pub hotbar_visible: bool,
    pub selected_hotbar_slot: u8,
    pub hotbar_pressed_slot: Option<u8>,
}

impl TouchOverlay {
    pub fn hidden() -> Self {
        Self::default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TouchJoystickOverlay {
    pub active: bool,
    pub base: Point,
    pub thumb: Point,
}

pub fn render_debug_overlay(scale: GuiScale, draw: &mut GuiDrawList, overlay: &DebugOverlay) {
    render_debug_overlay_at(scale, draw, overlay, Point { x: 4.0, y: 4.0 });
}

pub fn render_debug_overlay_at(
    scale: GuiScale,
    draw: &mut GuiDrawList,
    overlay: &DebugOverlay,
    origin: Point,
) {
    if overlay.is_empty() {
        return;
    }
    let font = Font::default();
    let line_height = font.line_height();
    let max_width = std::iter::once(overlay.title.as_str())
        .chain(overlay.lines.iter().map(String::as_str))
        .map(|line| font.width(line))
        .fold(0.0_f32, f32::max);
    let panel_width = (max_width + 14.0)
        .max(120.0)
        .min((scale.width - origin.x - 4.0).max(0.0));
    let line_count = 1 + overlay.lines.len();
    let panel_height = 8.0 + line_height * line_count as f32;
    let panel = Rect::new(
        origin.x,
        origin.y,
        panel_width,
        panel_height.min((scale.height - origin.y - 4.0).max(0.0)),
    );
    draw.fill(panel, Color::rgba(6, 9, 10, 185));
    draw.outline(panel, Color::rgba(110, 140, 136, 230));
    draw.push_clip(panel.inset(4.0));
    let title = Color::rgba(220, 238, 220, 255);
    let muted = Color::rgba(165, 186, 176, 255);
    let mut y = panel.y + 5.0;
    font.draw_shadow(draw, &overlay.title, panel.x + 6.0, y, title);
    y += line_height;
    for line in &overlay.lines {
        font.draw_shadow(draw, line, panel.x + 6.0, y, muted);
        y += line_height;
    }
    draw.pop_clip();
}

pub fn render_status_overlay(scale: GuiScale, draw: &mut GuiDrawList, status: &StatusOverlay) {
    if !status.visible || status.message.is_empty() {
        return;
    }
    let font = Font::default();
    let panel_width = (font.width(&status.message) + 16.0)
        .max(80.0)
        .min((scale.width - 16.0).max(0.0));
    let panel = Rect::new(
        (scale.width - panel_width - 8.0).max(4.0),
        8.0,
        panel_width,
        22.0,
    );
    let border = if status.ok {
        Color::rgba(110, 140, 136, 230)
    } else {
        Color::rgba(220, 120, 120, 240)
    };
    let text = if status.ok {
        Color::rgba(205, 220, 214, 255)
    } else {
        Color::rgba(255, 196, 196, 255)
    };
    draw.fill(panel, Color::rgba(8, 12, 14, 205));
    draw.outline(panel, border);
    draw.push_clip(panel.inset(4.0));
    font.draw_shadow(draw, &status.message, panel.x + 7.0, panel.y + 7.0, text);
    draw.pop_clip();
}

pub fn render_crosshair(scale: GuiScale, draw: &mut GuiDrawList) {
    let center_x = (scale.width * 0.5).floor();
    let center_y = (scale.height * 0.5).floor();
    let shadow = Color::rgba(0, 0, 0, 115);
    let color = Color::rgba(238, 244, 250, 220);
    draw.fill(Rect::new(center_x - 1.0, center_y - 8.0, 2.0, 16.0), shadow);
    draw.fill(Rect::new(center_x - 8.0, center_y - 1.0, 16.0, 2.0), shadow);
    draw.fill(Rect::new(center_x, center_y - 7.0, 1.0, 14.0), color);
    draw.fill(Rect::new(center_x - 7.0, center_y, 14.0, 1.0), color);
}

pub fn render_touch_overlay(scale: GuiScale, draw: &mut GuiDrawList, overlay: &TouchOverlay) {
    if !overlay.visible {
        return;
    }
    let font = Font::default();
    render_touch_menu_button(draw, touch_menu_button_rect(), overlay.menu_pressed);
    if overlay.movement.active {
        render_touch_joystick(draw, overlay.movement);
    }
    let buttons = touch_action_button_rects(scale);
    render_touch_action_button(draw, &font, buttons.jump, "UP", overlay.jump_pressed);
    render_touch_action_button(draw, &font, buttons.sprint, ">>", overlay.sprint_pressed);
    render_touch_action_button(draw, &font, buttons.descend, "DN", overlay.descend_pressed);
    if overlay.interaction_visible {
        render_touch_action_button(draw, &font, buttons.attack, "ATK", overlay.attack_pressed);
        render_touch_action_button(draw, &font, buttons.use_item, "USE", overlay.use_pressed);
    }
    if overlay.hotbar_visible {
        render_touch_hotbar(draw, &font, scale, overlay);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameUi {
    screen: Option<GameScreen>,
    new_world_seed: i64,
    join_remote_addr: String,
    pointer: Option<Point>,
    pressed: Option<WidgetId>,
    font: Font,
    scale: GuiScale,
}

const ID_TITLE_START: WidgetId = WidgetId(1);
const ID_TITLE_OPTIONS: WidgetId = WidgetId(2);
const ID_TITLE_QUIT: WidgetId = WidgetId(3);
const ID_PAUSE_RESUME: WidgetId = WidgetId(4);
const ID_PAUSE_OPTIONS: WidgetId = WidgetId(5);
const ID_PAUSE_TITLE: WidgetId = WidgetId(6);
const ID_OPTIONS_OCCLUSION: WidgetId = WidgetId(7);
const ID_OPTIONS_FULLBRIGHT: WidgetId = WidgetId(8);
const ID_OPTIONS_RADIUS: WidgetId = WidgetId(9);
const ID_OPTIONS_BACK: WidgetId = WidgetId(10);
const ID_OPTIONS_FRAME_PACING: WidgetId = WidgetId(11);
const ID_OPTIONS_FPS_CAP: WidgetId = WidgetId(12);
const ID_OPTIONS_TOUCH_LOOK: WidgetId = WidgetId(13);
const ID_OPTIONS_FLY: WidgetId = WidgetId(14);
const ID_OPTIONS_FLY_SPEED: WidgetId = WidgetId(15);
const ID_NEW_WORLD_REROLL: WidgetId = WidgetId(16);
const ID_NEW_WORLD_CREATE: WidgetId = WidgetId(17);
const ID_NEW_WORLD_BACK: WidgetId = WidgetId(18);
const ID_TITLE_JOIN_REMOTE: WidgetId = WidgetId(19);
const ID_JOIN_REMOTE_CONNECT: WidgetId = WidgetId(20);
const ID_JOIN_REMOTE_BACK: WidgetId = WidgetId(21);

pub const DEFAULT_JOIN_REMOTE_ADDR: &str = "127.0.0.1:25565";

impl Default for GameUi {
    fn default() -> Self {
        Self::new()
    }
}

impl GameUi {
    pub fn new() -> Self {
        Self {
            screen: Some(GameScreen::Title),
            new_world_seed: 0,
            join_remote_addr: DEFAULT_JOIN_REMOTE_ADDR.to_owned(),
            pointer: None,
            pressed: None,
            font: Font::default(),
            scale: GuiScale::from_pixels(1280, 900),
        }
    }

    pub fn new_ingame() -> Self {
        let mut ui = Self::new();
        ui.set_screen(None);
        ui
    }

    pub fn screen(&self) -> Option<GameScreen> {
        self.screen
    }

    pub fn new_world_seed(&self) -> i64 {
        self.new_world_seed
    }

    pub fn set_new_world_seed(&mut self, seed: i64) {
        self.new_world_seed = seed;
    }

    pub fn join_remote_addr(&self) -> &str {
        &self.join_remote_addr
    }

    pub fn set_join_remote_addr(&mut self, addr: impl Into<String>) {
        self.join_remote_addr = addr.into();
    }

    pub fn scale(&self) -> GuiScale {
        self.scale
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    pub fn set_scale(&mut self, scale: GuiScale) {
        self.scale = scale;
        self.pointer = self.pointer.map(|point| Point {
            x: point.x.clamp(0.0, scale.width),
            y: point.y.clamp(0.0, scale.height),
        });
    }

    pub fn is_active(&self) -> bool {
        self.screen.is_some()
    }

    pub fn covers_world(&self) -> bool {
        matches!(
            self.screen,
            Some(GameScreen::Title | GameScreen::NewWorld | GameScreen::JoinRemote)
        )
    }

    pub fn open_pause(&mut self) {
        self.screen = Some(GameScreen::Pause);
        self.pressed = None;
    }

    pub fn close(&mut self) {
        self.screen = None;
        self.pressed = None;
    }

    pub fn set_screen(&mut self, screen: Option<GameScreen>) {
        self.screen = screen;
        self.pressed = None;
    }

    pub fn clear_input(&mut self) {
        self.pointer = None;
        self.pressed = None;
    }

    pub fn pointer_move(
        &mut self,
        point: Point,
        state: GameUiRenderState,
    ) -> (bool, Option<GameUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.pointer = Some(point);
        let action = match self.pressed {
            Some(ID_OPTIONS_RADIUS) => Some(self.render_distance_action_at(point, state)),
            Some(ID_OPTIONS_FLY_SPEED) => Some(self.fly_speed_action_at(point, state)),
            Some(ID_OPTIONS_TOUCH_LOOK) => self.touch_look_action_at(point, state),
            _ => None,
        };
        (true, action)
    }

    pub fn pointer_down(&mut self, point: Point, state: GameUiRenderState) -> bool {
        if !self.is_active() {
            return false;
        }
        self.pointer = Some(point);
        self.pressed = self.widget_at(point, state);
        true
    }

    pub fn pointer_up(
        &mut self,
        point: Point,
        state: GameUiRenderState,
    ) -> (bool, Option<GameUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.pointer = Some(point);
        let pressed = self.pressed.take();
        let released = self.widget_at(point, state);
        let action = match (pressed, released) {
            (Some(ID_OPTIONS_RADIUS), Some(ID_OPTIONS_RADIUS)) => {
                Some(self.render_distance_action_at(point, state))
            }
            (Some(ID_OPTIONS_FLY_SPEED), Some(ID_OPTIONS_FLY_SPEED)) => {
                Some(self.fly_speed_action_at(point, state))
            }
            (Some(ID_OPTIONS_TOUCH_LOOK), Some(ID_OPTIONS_TOUCH_LOOK)) => {
                self.touch_look_action_at(point, state)
            }
            (Some(id), Some(released)) if id == released => self.action_for(id),
            _ => None,
        };
        (true, action)
    }

    pub fn key_pressed(&mut self, key: GuiKey) -> (bool, Option<GameUiAction>) {
        let Some(screen) = self.screen else {
            return (false, None);
        };
        match (screen, key) {
            (GameScreen::Pause, GuiKey::Escape) => (true, Some(GameUiAction::Resume)),
            (GameScreen::NewWorld, GuiKey::Escape) => (true, Some(GameUiAction::BackToTitle)),
            (GameScreen::JoinRemote, GuiKey::Escape) => (true, Some(GameUiAction::BackToTitle)),
            (GameScreen::Options { parent }, GuiKey::Escape) => match parent {
                GameOptionsParent::Title => (true, Some(GameUiAction::BackToTitle)),
                GameOptionsParent::Pause => (true, Some(GameUiAction::BackToPause)),
            },
            (GameScreen::Title, GuiKey::Escape) => (true, None),
        }
    }

    pub fn apply_action(&mut self, action: GameUiAction) {
        match action {
            GameUiAction::StartWorld | GameUiAction::Resume => self.close(),
            GameUiAction::OpenNewWorld => {
                self.screen = Some(GameScreen::NewWorld);
                self.pressed = None;
            }
            GameUiAction::OpenJoinRemote => {
                self.screen = Some(GameScreen::JoinRemote);
                self.pressed = None;
            }
            GameUiAction::OpenOptions(parent) => {
                self.screen = Some(GameScreen::Options { parent });
                self.pressed = None;
            }
            GameUiAction::BackToTitle => {
                self.screen = Some(GameScreen::Title);
                self.pressed = None;
            }
            GameUiAction::QuitToTitle => {
                self.screen = Some(GameScreen::Title);
                self.pressed = None;
            }
            GameUiAction::BackToPause => {
                self.screen = Some(GameScreen::Pause);
                self.pressed = None;
            }
            GameUiAction::CreateWorld(_) | GameUiAction::JoinRemote => self.close(),
            GameUiAction::RerollSeed => {}
            GameUiAction::ToggleSectionOcclusion
            | GameUiAction::ToggleFullbright
            | GameUiAction::ToggleFly
            | GameUiAction::CycleFramePacing
            | GameUiAction::CycleFpsCap
            | GameUiAction::SetRenderDistance(_)
            | GameUiAction::SetFlySpeed(_)
            | GameUiAction::SetTouchLookSensitivity(_)
            | GameUiAction::Quit => {}
        }
    }

    pub fn render_draw_list(&self, state: GameUiRenderState) -> GuiDrawList {
        let mut draw = GuiDrawList::new();
        match self.screen {
            Some(GameScreen::Title) => self.render_title(&mut draw),
            Some(GameScreen::NewWorld) => self.render_new_world(&mut draw),
            Some(GameScreen::JoinRemote) => self.render_join_remote(&mut draw),
            Some(GameScreen::Pause) => self.render_pause(&mut draw),
            Some(GameScreen::Options { parent }) => {
                self.render_options_screen(&mut draw, state, parent)
            }
            None => {}
        }
        draw
    }

    fn widget_at(&self, point: Point, state: GameUiRenderState) -> Option<WidgetId> {
        match self.screen? {
            GameScreen::Title => title_buttons(self.scale)
                .into_iter()
                .find(|button| button.contains(point))
                .map(|button| button.id),
            GameScreen::NewWorld => new_world_buttons(self.scale)
                .into_iter()
                .find(|button| button.contains(point))
                .map(|button| button.id),
            GameScreen::JoinRemote => join_remote_buttons(self.scale)
                .into_iter()
                .find(|button| button.contains(point))
                .map(|button| button.id),
            GameScreen::Pause => pause_buttons(self.scale)
                .into_iter()
                .find(|button| button.contains(point))
                .map(|button| button.id),
            GameScreen::Options { .. } => {
                let rects = option_widgets(self.scale, state);
                if rects.occlusion.contains(point) {
                    Some(ID_OPTIONS_OCCLUSION)
                } else if rects.fullbright.contains(point) {
                    Some(ID_OPTIONS_FULLBRIGHT)
                } else if rects.fly.contains(point) {
                    Some(ID_OPTIONS_FLY)
                } else if rects.frame_pacing.contains(point) {
                    Some(ID_OPTIONS_FRAME_PACING)
                } else if rects.fps_cap.contains(point) {
                    Some(ID_OPTIONS_FPS_CAP)
                } else if rects.radius.contains(point) {
                    Some(ID_OPTIONS_RADIUS)
                } else if rects.fly_speed.contains(point) {
                    Some(ID_OPTIONS_FLY_SPEED)
                } else if rects.touch_look.is_some_and(|rect| rect.contains(point)) {
                    Some(ID_OPTIONS_TOUCH_LOOK)
                } else if rects.back.contains(point) {
                    Some(ID_OPTIONS_BACK)
                } else {
                    None
                }
            }
        }
    }

    fn action_for(&self, id: WidgetId) -> Option<GameUiAction> {
        match id {
            ID_TITLE_START => Some(GameUiAction::OpenNewWorld),
            ID_TITLE_JOIN_REMOTE => Some(GameUiAction::OpenJoinRemote),
            ID_TITLE_OPTIONS => Some(GameUiAction::OpenOptions(GameOptionsParent::Title)),
            ID_TITLE_QUIT => Some(GameUiAction::Quit),
            ID_NEW_WORLD_REROLL => Some(GameUiAction::RerollSeed),
            ID_NEW_WORLD_CREATE => Some(GameUiAction::CreateWorld(self.new_world_seed)),
            ID_NEW_WORLD_BACK => Some(GameUiAction::BackToTitle),
            ID_JOIN_REMOTE_CONNECT => Some(GameUiAction::JoinRemote),
            ID_JOIN_REMOTE_BACK => Some(GameUiAction::BackToTitle),
            ID_PAUSE_RESUME => Some(GameUiAction::Resume),
            ID_PAUSE_OPTIONS => Some(GameUiAction::OpenOptions(GameOptionsParent::Pause)),
            ID_PAUSE_TITLE => Some(GameUiAction::QuitToTitle),
            ID_OPTIONS_OCCLUSION => Some(GameUiAction::ToggleSectionOcclusion),
            ID_OPTIONS_FULLBRIGHT => Some(GameUiAction::ToggleFullbright),
            ID_OPTIONS_FLY => Some(GameUiAction::ToggleFly),
            ID_OPTIONS_FRAME_PACING => Some(GameUiAction::CycleFramePacing),
            ID_OPTIONS_FPS_CAP => Some(GameUiAction::CycleFpsCap),
            ID_OPTIONS_BACK => match self.screen {
                Some(GameScreen::Options {
                    parent: GameOptionsParent::Title,
                }) => Some(GameUiAction::BackToTitle),
                Some(GameScreen::Options {
                    parent: GameOptionsParent::Pause,
                }) => Some(GameUiAction::BackToPause),
                _ => None,
            },
            _ => None,
        }
    }

    fn render_distance_action_at(&self, point: Point, state: GameUiRenderState) -> GameUiAction {
        let slider = Slider::new(
            ID_OPTIONS_RADIUS,
            option_widgets(self.scale, state).radius,
            "",
            render_distance_slider_value(state),
        );
        GameUiAction::SetRenderDistance(render_distance_from_slider_value(
            slider.value_from_point(point),
            state,
        ))
    }

    fn fly_speed_action_at(&self, point: Point, state: GameUiRenderState) -> GameUiAction {
        let slider = Slider::new(
            ID_OPTIONS_FLY_SPEED,
            option_widgets(self.scale, state).fly_speed,
            "",
            fly_speed_slider_value(state),
        );
        GameUiAction::SetFlySpeed(fly_speed_from_slider_value(
            slider.value_from_point(point),
            state,
        ))
    }

    fn touch_look_action_at(&self, point: Point, state: GameUiRenderState) -> Option<GameUiAction> {
        let settings = state.touch_settings?;
        let slider = Slider::new(
            ID_OPTIONS_TOUCH_LOOK,
            option_widgets(self.scale, state).touch_look?,
            "",
            touch_look_slider_value(settings),
        );
        Some(GameUiAction::SetTouchLookSensitivity(
            touch_look_from_slider_value(slider.value_from_point(point), settings),
        ))
    }

    fn interaction(&self) -> Interaction {
        Interaction {
            pointer: self.pointer,
            pressed: self.pressed,
            focused: None,
        }
    }

    fn render_title(&self, draw: &mut GuiDrawList) {
        draw.fill_gradient(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(24, 44, 51, 255),
            Color::rgba(7, 10, 12, 255),
        );
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 55),
        );
        self.font.draw_centered(
            draw,
            "MCLONE",
            self.scale.width * 0.5,
            34.0,
            Color::rgba(245, 252, 234, 255),
        );
        self.font.draw_centered(
            draw,
            "NATIVE RUST CLIENT",
            self.scale.width * 0.5,
            48.0,
            Color::rgba(185, 212, 198, 255),
        );
        for button in title_buttons(self.scale) {
            button.render(draw, &self.font, self.interaction());
        }
        self.font.draw_shadow(
            draw,
            "MINECRAFT 1.17.1 TARGET",
            4.0,
            self.scale.height - 12.0,
            Color::rgba(160, 176, 170, 255),
        );
    }

    fn render_new_world(&self, draw: &mut GuiDrawList) {
        draw.fill_gradient(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(24, 44, 51, 255),
            Color::rgba(7, 10, 12, 255),
        );
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 55),
        );
        self.font.draw_centered(
            draw,
            "NEW WORLD",
            self.scale.width * 0.5,
            self.scale.height * 0.28,
            Color::rgba(245, 252, 234, 255),
        );
        self.font.draw_centered(
            draw,
            &format!("Seed: {}", self.new_world_seed),
            self.scale.width * 0.5,
            self.scale.height * 0.28 + 22.0,
            Color::rgba(185, 212, 198, 255),
        );
        for button in new_world_buttons(self.scale) {
            button.render(draw, &self.font, self.interaction());
        }
    }

    fn render_join_remote(&self, draw: &mut GuiDrawList) {
        draw.fill_gradient(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(24, 44, 51, 255),
            Color::rgba(7, 10, 12, 255),
        );
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 55),
        );
        self.font.draw_centered(
            draw,
            "JOIN REMOTE",
            self.scale.width * 0.5,
            self.scale.height * 0.28,
            Color::rgba(245, 252, 234, 255),
        );
        self.font.draw_centered(
            draw,
            &format!("Server: {}", self.join_remote_addr),
            self.scale.width * 0.5,
            self.scale.height * 0.28 + 22.0,
            Color::rgba(185, 212, 198, 255),
        );
        for button in join_remote_buttons(self.scale) {
            button.render(draw, &self.font, self.interaction());
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
        for button in pause_buttons(self.scale) {
            button.render(draw, &self.font, self.interaction());
        }
    }

    fn render_options_screen(
        &self,
        draw: &mut GuiDrawList,
        state: GameUiRenderState,
        parent: GameOptionsParent,
    ) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 150),
        );
        let panel = options_panel(self.scale, state);
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
        let widgets = option_widgets(self.scale, state);
        Checkbox::new(
            ID_OPTIONS_OCCLUSION,
            widgets.occlusion,
            "Section Occlusion",
            state.section_occlusion_culling,
        )
        .render(draw, &self.font, self.interaction());
        Checkbox::new(
            ID_OPTIONS_FULLBRIGHT,
            widgets.fullbright,
            "Force Fullbright",
            state.force_fullbright,
        )
        .render(draw, &self.font, self.interaction());
        Checkbox::new(ID_OPTIONS_FLY, widgets.fly, "Fly Mode", state.fly_enabled).render(
            draw,
            &self.font,
            self.interaction(),
        );
        CycleButton::new(
            ID_OPTIONS_FRAME_PACING,
            widgets.frame_pacing,
            "Frame Pacing",
            state.frame_pacing_mode.label(),
        )
        .render(draw, &self.font, self.interaction());
        CycleButton::new(
            ID_OPTIONS_FPS_CAP,
            widgets.fps_cap,
            "FPS Cap",
            state.fps_cap.to_string(),
        )
        .render(draw, &self.font, self.interaction());
        Slider::new(
            ID_OPTIONS_RADIUS,
            widgets.radius,
            render_distance_label(state),
            render_distance_slider_value(state),
        )
        .render(draw, &self.font, self.interaction());
        Slider::new(
            ID_OPTIONS_FLY_SPEED,
            widgets.fly_speed,
            fly_speed_label(state),
            fly_speed_slider_value(state),
        )
        .render(draw, &self.font, self.interaction());
        if let (Some(settings), Some(rect)) = (state.touch_settings, widgets.touch_look) {
            Slider::new(
                ID_OPTIONS_TOUCH_LOOK,
                rect,
                touch_look_label(settings),
                touch_look_slider_value(settings),
            )
            .render(draw, &self.font, self.interaction());
        }
        Button::new(
            ID_OPTIONS_BACK,
            widgets.back,
            match parent {
                GameOptionsParent::Title => "Back",
                GameOptionsParent::Pause => "Done",
            },
        )
        .render(draw, &self.font, self.interaction());
    }
}

#[derive(Clone, Copy, Debug)]
struct OptionWidgetRects {
    occlusion: Rect,
    fullbright: Rect,
    fly: Rect,
    frame_pacing: Rect,
    fps_cap: Rect,
    radius: Rect,
    fly_speed: Rect,
    touch_look: Option<Rect>,
    back: Rect,
}

fn title_buttons(scale: GuiScale) -> [Button; 4] {
    let y = scale.height * 0.5 - 34.0;
    [
        Button::new(ID_TITLE_START, menu_button_rect(scale, y), "New World"),
        Button::new(
            ID_TITLE_JOIN_REMOTE,
            menu_button_rect(scale, y + 24.0),
            "Join Remote",
        ),
        Button::new(
            ID_TITLE_OPTIONS,
            menu_button_rect(scale, y + 48.0),
            "Options",
        ),
        Button::new(ID_TITLE_QUIT, menu_button_rect(scale, y + 72.0), "Quit"),
    ]
}

fn new_world_buttons(scale: GuiScale) -> [Button; 3] {
    let y = scale.height * 0.5 - 4.0;
    [
        Button::new(ID_NEW_WORLD_REROLL, menu_button_rect(scale, y), "Reroll"),
        Button::new(
            ID_NEW_WORLD_CREATE,
            menu_button_rect(scale, y + 24.0),
            "Create World",
        ),
        Button::new(ID_NEW_WORLD_BACK, menu_button_rect(scale, y + 48.0), "Back"),
    ]
}

fn join_remote_buttons(scale: GuiScale) -> [Button; 2] {
    let y = scale.height * 0.5 + 20.0;
    [
        Button::new(
            ID_JOIN_REMOTE_CONNECT,
            menu_button_rect(scale, y),
            "Connect",
        ),
        Button::new(
            ID_JOIN_REMOTE_BACK,
            menu_button_rect(scale, y + 24.0),
            "Back",
        ),
    ]
}

fn pause_buttons(scale: GuiScale) -> [Button; 3] {
    let y = scale.height * 0.5 - 22.0;
    [
        Button::new(ID_PAUSE_RESUME, menu_button_rect(scale, y), "Back To Game"),
        Button::new(
            ID_PAUSE_OPTIONS,
            menu_button_rect(scale, y + 24.0),
            "Options",
        ),
        Button::new(
            ID_PAUSE_TITLE,
            menu_button_rect(scale, y + 48.0),
            "Quit To Title",
        ),
    ]
}

fn option_widgets(scale: GuiScale, state: GameUiRenderState) -> OptionWidgetRects {
    let panel = options_panel(scale, state);
    let show_touch = state.touch_settings.is_some();
    let check_x = panel.x + 26.0;
    let row_x = panel.x + 25.0;
    let mut y = panel.y + 34.0;

    let occlusion = Rect::new(check_x, y, 190.0, 18.0);
    y += 20.0;
    let fullbright = Rect::new(check_x, y, 190.0, 18.0);
    y += 20.0;
    let fly = Rect::new(check_x, y, 190.0, 18.0);
    y += 24.0;
    let frame_pacing = Rect::new(row_x, y, 192.0, 20.0);
    y += 22.0;
    let fps_cap = Rect::new(row_x, y, 192.0, 20.0);
    y += 22.0;
    let radius = Rect::new(row_x, y, 192.0, 20.0);
    y += 22.0;
    let fly_speed = Rect::new(row_x, y, 192.0, 20.0);
    y += 22.0;
    let touch_look = if show_touch {
        let rect = Rect::new(row_x, y, 192.0, 20.0);
        y += 22.0;
        Some(rect)
    } else {
        None
    };
    y += 6.0;
    let back = Rect::new(panel.center_x() - 55.0, y, 110.0, 20.0);

    OptionWidgetRects {
        occlusion,
        fullbright,
        fly,
        frame_pacing,
        fps_cap,
        radius,
        fly_speed,
        touch_look,
        back,
    }
}

fn options_panel(scale: GuiScale, state: GameUiRenderState) -> Rect {
    centered_panel(
        scale,
        242.0,
        if state.touch_settings.is_some() {
            244.0
        } else {
            222.0
        },
    )
}

fn render_distance_slider_value(state: GameUiRenderState) -> f32 {
    let (min, max) = state.render_distance_limits();
    if max <= min {
        0.0
    } else {
        (state.clamped_render_distance() - min) as f32 / (max - min) as f32
    }
}

fn render_distance_from_slider_value(value: f32, state: GameUiRenderState) -> i32 {
    let (min, max) = state.render_distance_limits();
    if max <= min {
        min
    } else {
        min + (value.clamp(0.0, 1.0) * (max - min) as f32).round() as i32
    }
}

fn render_distance_label(state: GameUiRenderState) -> String {
    let radius = state.clamped_render_distance();
    let suffix = if radius == 1 { "chunk" } else { "chunks" };
    format!("Render Distance: {radius} {suffix}")
}

fn fly_speed_slider_value(state: GameUiRenderState) -> f32 {
    let (min, max) = state.fly_speed_multiplier_limits();
    if max <= min || min <= 0.0 {
        0.0
    } else {
        (state.clamped_fly_speed_multiplier().ln() - min.ln()) / (max.ln() - min.ln())
    }
}

fn fly_speed_from_slider_value(value: f32, state: GameUiRenderState) -> f32 {
    let (min, max) = state.fly_speed_multiplier_limits();
    if max <= min || min <= 0.0 {
        min
    } else {
        let raw = (min.ln() + value.clamp(0.0, 1.0) * (max.ln() - min.ln())).exp();
        ((raw * 10.0).round() / 10.0).clamp(min, max)
    }
}

fn fly_speed_label(state: GameUiRenderState) -> String {
    format!("Fly Speed: {:.1}x", state.clamped_fly_speed_multiplier())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchActionButtonRects {
    pub jump: Rect,
    pub sprint: Rect,
    pub descend: Rect,
    pub attack: Rect,
    pub use_item: Rect,
}

pub fn touch_menu_button_rect() -> Rect {
    Rect::new(10.0, 10.0, 40.0, 40.0)
}

pub fn touch_movement_zone_rect(scale: GuiScale) -> Rect {
    Rect::new(
        0.0,
        scale.height * 0.45,
        scale.width * 0.58,
        scale.height * 0.55,
    )
}

pub fn touch_action_button_rects(scale: GuiScale) -> TouchActionButtonRects {
    let size = 58.0;
    let gap = 12.0;
    let right = 18.0;
    let bottom = 24.0;
    let x1 = (scale.width - right - size).max(0.0);
    let x0 = (x1 - gap - size).max(0.0);
    let y1 = (scale.height - bottom - size).max(0.0);
    let y0 = (y1 - gap - size).max(0.0);
    let y_attack = (y0 - gap - size).max(0.0);
    TouchActionButtonRects {
        jump: Rect::new(x1, y0, size, size),
        sprint: Rect::new(x0, y1, size, size),
        descend: Rect::new(x1, y1, size, size),
        attack: Rect::new(x1, y_attack, size, size),
        use_item: Rect::new(x0, y_attack, size, size),
    }
}

pub fn touch_hotbar_slot_rects(scale: GuiScale) -> [Rect; 9] {
    let slot = 28.0;
    let gap = 4.0;
    let total_width = slot * 9.0 + gap * 8.0;
    let x0 = ((scale.width - total_width) * 0.5).max(4.0);
    let y = (scale.height - 98.0).max(58.0);
    std::array::from_fn(|index| Rect::new(x0 + index as f32 * (slot + gap), y, slot, slot))
}

fn render_touch_menu_button(draw: &mut GuiDrawList, rect: Rect, pressed: bool) {
    render_touch_panel(draw, rect, pressed);
    let color = if pressed {
        Color::rgba(24, 30, 34, 235)
    } else {
        Color::rgba(238, 246, 248, 220)
    };
    let line_x = rect.x + 11.0;
    let mut y = rect.y + 12.0;
    for _ in 0..3 {
        draw.fill(Rect::new(line_x, y, 18.0, 3.0), color);
        y += 7.0;
    }
}

fn render_touch_joystick(draw: &mut GuiDrawList, joystick: TouchJoystickOverlay) {
    let base = Rect::new(joystick.base.x - 50.0, joystick.base.y - 50.0, 100.0, 100.0);
    let thumb = Rect::new(joystick.thumb.x - 18.0, joystick.thumb.y - 18.0, 36.0, 36.0);
    draw.fill(base, Color::rgba(0, 0, 0, 70));
    draw.outline(base, Color::rgba(232, 240, 248, 58));
    draw.outline(base.inset(1.0), Color::rgba(0, 0, 0, 65));
    draw.fill(thumb, Color::rgba(245, 250, 255, 158));
    draw.outline(thumb, Color::rgba(245, 250, 255, 205));
}

fn render_touch_action_button(
    draw: &mut GuiDrawList,
    font: &Font,
    rect: Rect,
    label: &str,
    pressed: bool,
) {
    render_touch_panel(draw, rect, pressed);
    let color = if pressed {
        Color::rgba(12, 16, 20, 230)
    } else {
        Color::rgba(245, 250, 255, 210)
    };
    font.draw_centered(
        draw,
        label,
        rect.center_x(),
        rect.y + ((rect.height - font.line_height()) * 0.5).floor(),
        color,
    );
}

fn render_touch_hotbar(
    draw: &mut GuiDrawList,
    font: &Font,
    scale: GuiScale,
    overlay: &TouchOverlay,
) {
    for (index, rect) in touch_hotbar_slot_rects(scale).into_iter().enumerate() {
        let slot = index as u8;
        let pressed = overlay.hotbar_pressed_slot == Some(slot);
        render_touch_panel(draw, rect, pressed);
        if overlay.selected_hotbar_slot == slot {
            draw.outline(rect.inset(-2.0), Color::rgba(245, 250, 255, 215));
        }
        font.draw_centered(
            draw,
            &(index + 1).to_string(),
            rect.center_x(),
            rect.y + ((rect.height - font.line_height()) * 0.5).floor(),
            Color::rgba(245, 250, 255, 210),
        );
    }
}

fn render_touch_panel(draw: &mut GuiDrawList, rect: Rect, pressed: bool) {
    let (fill, border) = if pressed {
        (
            Color::rgba(245, 250, 255, 174),
            Color::rgba(245, 250, 255, 225),
        )
    } else {
        (Color::rgba(0, 0, 0, 86), Color::rgba(232, 240, 248, 60))
    };
    draw.fill(rect, fill);
    draw.outline(rect, border);
    draw.outline(rect.inset(1.0), Color::rgba(0, 0, 0, 64));
}

fn touch_look_slider_value(settings: GameTouchSettings) -> f32 {
    let (min, max) = settings.look_sensitivity_limits();
    if max <= min {
        0.0
    } else {
        (settings.clamped_look_sensitivity() - min) / (max - min)
    }
}

fn touch_look_from_slider_value(value: f32, settings: GameTouchSettings) -> f32 {
    let (min, max) = settings.look_sensitivity_limits();
    if max <= min {
        min
    } else {
        let raw = min + value.clamp(0.0, 1.0) * (max - min);
        ((raw * 10.0).round() / 10.0).clamp(min, max)
    }
}

fn touch_look_label(settings: GameTouchSettings) -> String {
    format!("Touch Look: {:.1}x", settings.clamped_look_sensitivity())
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

fn centered_panel(scale: GuiScale, width: f32, height: f32) -> Rect {
    Rect::new(
        (scale.width - width).max(0.0) * 0.5,
        (scale.height - height).max(0.0) * 0.5,
        width.min(scale.width),
        height.min(scale.height),
    )
}

fn menu_button_rect(scale: GuiScale, y: f32) -> Rect {
    Rect::new(scale.width * 0.5 - 90.0, y, 180.0, 20.0)
}

fn normalized_rect(rect: Rect) -> Rect {
    let x0 = rect.x.min(rect.right());
    let y0 = rect.y.min(rect.bottom());
    let x1 = rect.x.max(rect.right());
    let y1 = rect.y.max(rect.bottom());
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

fn intersect_clip(left: ClipRect, right: ClipRect) -> ClipRect {
    let x0 = left.x.max(right.x);
    let y0 = left.y.max(right.y);
    let x1 = (left.x + left.width).min(right.x + right.width);
    let y1 = (left.y + left.height).min(right.y + right.height);
    ClipRect {
        x: x0,
        y: y0,
        width: (x1 - x0).max(0.0),
        height: (y1 - y0).max(0.0),
    }
}

fn glyph_rows(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10011, 0b10101, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        ':' => [0, 0b00100, 0b00100, 0, 0b00100, 0b00100, 0],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        ',' => [0, 0, 0, 0, 0, 0b00100, 0b01000],
        '/' => [
            0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000,
        ],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        '%' => [
            0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011,
        ],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        '[' => [
            0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
        ],
        ']' => [
            0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
        ],
        '<' => [
            0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010,
        ],
        '>' => [
            0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000,
        ],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        _ => [
            0b11111, 0b10001, 0b00010, 0b00100, 0b00010, 0b10001, 0b11111,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_scale_matches_minecraft_style_thresholds() {
        assert_eq!(GuiScale::from_pixels(319, 239).scale, 1);
        assert_eq!(GuiScale::from_pixels(640, 480).scale, 2);
        assert_eq!(GuiScale::from_pixels(1280, 720).scale, 3);
        assert_eq!(GuiScale::from_pixels(1920, 1080).scale, 4);
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
    fn button_reports_hit_only_when_enabled() {
        let button = Button::new(WidgetId(1), Rect::new(10.0, 20.0, 50.0, 12.0), "Start");
        assert!(button.contains(Point { x: 12.0, y: 21.0 }));
        assert!(!button.contains(Point { x: 5.0, y: 21.0 }));
        assert!(!button.enabled(false).contains(Point { x: 12.0, y: 21.0 }));
    }

    #[test]
    fn game_ui_has_title_and_ingame_start_modes() {
        let title_ui = GameUi::new();
        assert!(title_ui.is_active());
        assert!(title_ui.covers_world());
        assert_eq!(title_ui.screen(), Some(GameScreen::Title));

        let ingame_ui = GameUi::new_ingame();
        assert!(!ingame_ui.is_active());
        assert!(!ingame_ui.covers_world());
        assert_eq!(ingame_ui.screen(), None);
    }

    #[test]
    fn game_ui_title_renders_draw_commands() {
        let mut ui = GameUi::new();
        ui.set_scale(GuiScale::from_pixels(960, 540));
        let draw = ui.render_draw_list(GameUiRenderState::default());
        assert!(!draw.commands().is_empty());
    }

    #[test]
    fn game_ui_new_world_flow_emits_seeded_create_action() {
        let mut ui = GameUi::new();
        ui.set_scale(GuiScale::from_pixels(960, 540));
        let state = GameUiRenderState::default();
        let click = |rect: Rect| Point {
            x: rect.center_x(),
            y: rect.y + rect.height * 0.5,
        };

        let start = click(title_buttons(ui.scale())[0].rect);
        assert!(ui.pointer_down(start, state));
        let (_handled, action) = ui.pointer_up(start, state);
        assert_eq!(action, Some(GameUiAction::OpenNewWorld));
        ui.apply_action(action.unwrap());
        assert_eq!(ui.screen(), Some(GameScreen::NewWorld));
        assert!(ui.covers_world());

        ui.set_new_world_seed(12345);
        let buttons = new_world_buttons(ui.scale());
        let reroll = click(buttons[0].rect);
        assert!(ui.pointer_down(reroll, state));
        let (_handled, action) = ui.pointer_up(reroll, state);
        assert_eq!(action, Some(GameUiAction::RerollSeed));

        let create = click(buttons[1].rect);
        assert!(ui.pointer_down(create, state));
        let (_handled, action) = ui.pointer_up(create, state);
        assert_eq!(action, Some(GameUiAction::CreateWorld(12345)));
        ui.apply_action(action.unwrap());
        assert_eq!(ui.screen(), None);

        ui.set_screen(Some(GameScreen::NewWorld));
        let back = click(buttons[2].rect);
        assert!(ui.pointer_down(back, state));
        let (_handled, action) = ui.pointer_up(back, state);
        assert_eq!(action, Some(GameUiAction::BackToTitle));
        ui.apply_action(action.unwrap());
        assert_eq!(ui.screen(), Some(GameScreen::Title));
    }

    #[test]
    fn game_ui_join_remote_flow_emits_connect_action() {
        let mut ui = GameUi::new();
        ui.set_scale(GuiScale::from_pixels(960, 540));
        ui.set_join_remote_addr("10.0.0.5:25565");
        let state = GameUiRenderState::default();
        let click = |rect: Rect| Point {
            x: rect.center_x(),
            y: rect.y + rect.height * 0.5,
        };

        let join = click(title_buttons(ui.scale())[1].rect);
        assert!(ui.pointer_down(join, state));
        let (_handled, action) = ui.pointer_up(join, state);
        assert_eq!(action, Some(GameUiAction::OpenJoinRemote));
        ui.apply_action(action.unwrap());
        assert_eq!(ui.screen(), Some(GameScreen::JoinRemote));
        assert!(ui.covers_world());
        assert_eq!(ui.join_remote_addr(), "10.0.0.5:25565");

        let buttons = join_remote_buttons(ui.scale());
        let connect = click(buttons[0].rect);
        assert!(ui.pointer_down(connect, state));
        let (_handled, action) = ui.pointer_up(connect, state);
        assert_eq!(action, Some(GameUiAction::JoinRemote));
        ui.apply_action(action.unwrap());
        assert_eq!(ui.screen(), None);

        ui.set_screen(Some(GameScreen::JoinRemote));
        let back = click(buttons[1].rect);
        assert!(ui.pointer_down(back, state));
        let (_handled, action) = ui.pointer_up(back, state);
        assert_eq!(action, Some(GameUiAction::BackToTitle));
        ui.apply_action(action.unwrap());
        assert_eq!(ui.screen(), Some(GameScreen::Title));
    }

    #[test]
    fn debug_overlay_renders_title_and_lines() {
        let overlay = DebugOverlay::new("DEBUG", ["POS 1.0 64.0 -2.0", "CHUNKS 9"]);
        let mut draw = GuiDrawList::new();

        render_debug_overlay(GuiScale::from_pixels(960, 540), &mut draw, &overlay);

        assert!(!draw.commands().is_empty());
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
            descend_pressed: false,
            interaction_visible: true,
            attack_pressed: false,
            use_pressed: true,
            hotbar_visible: true,
            selected_hotbar_slot: 2,
            hotbar_pressed_slot: Some(4),
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
            actions.sprint,
            actions.descend,
            actions.attack,
            actions.use_item,
        ] {
            assert!(rect.x >= 0.0);
            assert!(rect.y >= 0.0);
            assert!(rect.right() <= scale.width);
            assert!(rect.bottom() <= scale.height);
        }
        assert_eq!(hotbar.len(), 9);
        for rect in hotbar {
            assert!(rect.x >= 0.0);
            assert!(rect.y >= 0.0);
            assert!(rect.right() <= scale.width);
            assert!(rect.bottom() <= scale.height);
        }
    }

    #[test]
    fn game_ui_escape_maps_to_screen_actions() {
        let mut ui = GameUi::new();
        ui.set_screen(Some(GameScreen::Pause));
        assert_eq!(
            ui.key_pressed(GuiKey::Escape),
            (true, Some(GameUiAction::Resume))
        );

        ui.set_screen(Some(GameScreen::Options {
            parent: GameOptionsParent::Title,
        }));
        assert_eq!(
            ui.key_pressed(GuiKey::Escape),
            (true, Some(GameUiAction::BackToTitle))
        );
    }

    #[test]
    fn game_ui_options_radius_slider_emits_render_distance_action() {
        let mut ui = GameUi::new();
        ui.set_screen(Some(GameScreen::Options {
            parent: GameOptionsParent::Pause,
        }));
        ui.set_scale(GuiScale::from_pixels(960, 540));
        let state = GameUiRenderState {
            render_distance: 2,
            min_render_distance: 2,
            max_render_distance: 16,
            ..GameUiRenderState::default()
        };

        let radius = option_widgets(ui.scale(), state).radius;
        let point = Point {
            x: radius.right() - 0.1,
            y: radius.y + radius.height * 0.5,
        };
        assert!(ui.pointer_down(point, state));
        let (_handled, action) = ui.pointer_up(point, state);
        assert_eq!(action, Some(GameUiAction::SetRenderDistance(16)));

        let point = Point {
            x: radius.x,
            y: radius.y + radius.height * 0.5,
        };
        assert!(ui.pointer_down(point, state));
        let (_handled, action) = ui.pointer_up(point, state);
        assert_eq!(action, Some(GameUiAction::SetRenderDistance(2)));
    }

    #[test]
    fn game_ui_options_touch_look_slider_emits_sensitivity_action() {
        let mut ui = GameUi::new();
        ui.set_screen(Some(GameScreen::Options {
            parent: GameOptionsParent::Pause,
        }));
        ui.set_scale(GuiScale::from_pixels(960, 540));
        let state = GameUiRenderState {
            touch_settings: Some(GameTouchSettings::new(2.4, 0.5, 5.0)),
            ..GameUiRenderState::default()
        };

        let touch_look = option_widgets(ui.scale(), state)
            .touch_look
            .expect("touch settings should expose the touch look slider");
        let point = Point {
            x: touch_look.right() - 0.1,
            y: touch_look.y + touch_look.height * 0.5,
        };
        assert!(ui.pointer_down(point, state));
        let (_handled, action) = ui.pointer_up(point, state);
        assert_eq!(action, Some(GameUiAction::SetTouchLookSensitivity(5.0)));

        let state = GameUiRenderState::default();
        assert!(option_widgets(ui.scale(), state).touch_look.is_none());
    }

    #[test]
    fn game_ui_options_fly_checkbox_emits_toggle_fly_action() {
        let mut ui = GameUi::new();
        ui.set_screen(Some(GameScreen::Options {
            parent: GameOptionsParent::Pause,
        }));
        ui.set_scale(GuiScale::from_pixels(960, 540));
        let state = GameUiRenderState::default();

        let fly = option_widgets(ui.scale(), state).fly;
        let point = Point {
            x: fly.x + fly.width * 0.5,
            y: fly.y + fly.height * 0.5,
        };
        assert!(ui.pointer_down(point, state));
        let (_handled, action) = ui.pointer_up(point, state);
        assert_eq!(action, Some(GameUiAction::ToggleFly));
    }

    #[test]
    fn game_ui_options_fly_speed_slider_emits_fly_speed_action() {
        let mut ui = GameUi::new();
        ui.set_screen(Some(GameScreen::Options {
            parent: GameOptionsParent::Pause,
        }));
        ui.set_scale(GuiScale::from_pixels(960, 540));
        let state = GameUiRenderState {
            fly_speed_multiplier: 1.0,
            min_fly_speed_multiplier: 0.5,
            max_fly_speed_multiplier: 8.0,
            ..GameUiRenderState::default()
        };

        let fly_speed = option_widgets(ui.scale(), state).fly_speed;
        let point = Point {
            x: fly_speed.right() - 0.1,
            y: fly_speed.y + fly_speed.height * 0.5,
        };
        assert!(ui.pointer_down(point, state));
        let (_handled, action) = ui.pointer_up(point, state);
        assert_eq!(action, Some(GameUiAction::SetFlySpeed(8.0)));

        let point = Point {
            x: fly_speed.x,
            y: fly_speed.y + fly_speed.height * 0.5,
        };
        assert!(ui.pointer_down(point, state));
        let (_handled, action) = ui.pointer_up(point, state);
        assert_eq!(action, Some(GameUiAction::SetFlySpeed(0.5)));
    }
}
