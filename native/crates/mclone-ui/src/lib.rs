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
}
