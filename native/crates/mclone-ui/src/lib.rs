#![forbid(unsafe_code)]

use mclone_input::{FLAT_HOTBAR_SLOT_COUNT, InputPromptKind, ResolvedFlatInput, TouchControlsMode};

mod frame_pipeline_overlay;
mod v2;
pub use frame_pipeline_overlay::{FramePipelineHudOverlay, render_frame_pipeline_overlay};
pub use v2::{
    FlatHudDrawList, GameUiHost, LoadingProgressDrawList, LoadingProgressOverlayLayer,
    UiDebugSnapshot, UiDebugWidget, UiDrawCacheStats, UiFrameState, UiLayout, UiPanelDrawList,
    UiPanelRevision, UiScreenId, UiSurface, UiWidget, UiWidgetId, UiWidgetKind,
};

pub const HOTBAR_SLOT_COUNT_USIZE: usize = FLAT_HOTBAR_SLOT_COUNT as usize;
pub const EMPTY_HOTBAR_ICONS: [Option<GuiTextureUv>; HOTBAR_SLOT_COUNT_USIZE] =
    [None; HOTBAR_SLOT_COUNT_USIZE];
pub const BLOCK_PALETTE_ENTRY_CAPACITY: usize = 60;
pub const EMPTY_BLOCK_PALETTE_ENTRIES: [Option<BlockPaletteEntry>; BLOCK_PALETTE_ENTRY_CAPACITY] =
    [None; BLOCK_PALETTE_ENTRY_CAPACITY];

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
pub struct GuiTextureUv {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

impl GuiTextureUv {
    pub const fn new(u0: f32, v0: f32, u1: f32, v1: f32) -> Self {
        Self { u0, v0, u1, v1 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockPaletteEntry {
    pub block_state: u32,
    pub icon: Option<GuiTextureUv>,
    pub label: &'static str,
}

impl BlockPaletteEntry {
    pub const fn new(block_state: u32, icon: Option<GuiTextureUv>, label: &'static str) -> Self {
        Self {
            block_state,
            icon,
            label,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockPaletteOverlay {
    pub visible: bool,
    pub selected_hotbar_slot: u8,
    pub entries: [Option<BlockPaletteEntry>; BLOCK_PALETTE_ENTRY_CAPACITY],
}

impl BlockPaletteOverlay {
    pub const fn hidden() -> Self {
        Self {
            visible: false,
            selected_hotbar_slot: 0,
            entries: EMPTY_BLOCK_PALETTE_ENTRIES,
        }
    }

    pub const fn visible(
        selected_hotbar_slot: u8,
        entries: [Option<BlockPaletteEntry>; BLOCK_PALETTE_ENTRY_CAPACITY],
    ) -> Self {
        Self {
            visible: true,
            selected_hotbar_slot,
            entries,
        }
    }

    pub fn entry_count(self) -> usize {
        self.entries.iter().filter(|entry| entry.is_some()).count()
    }
}

impl Default for BlockPaletteOverlay {
    fn default() -> Self {
        Self::hidden()
    }
}

#[derive(Clone, Debug, PartialEq)]
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
    TextureRect {
        rect: Rect,
        uv: GuiTextureUv,
        color: Color,
        clip: Option<ClipRect>,
    },
    Text {
        text: String,
        x: f32,
        y: f32,
        color: Color,
        shadow: bool,
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

    pub fn append(&mut self, other: &GuiDrawList) {
        self.commands.extend(other.commands.iter().cloned());
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

    pub fn texture(&mut self, rect: Rect, uv: GuiTextureUv, color: Color) {
        if rect.width <= 0.0 || rect.height <= 0.0 || color.a == 0 {
            return;
        }
        self.commands.push(GuiDrawCommand::TextureRect {
            rect,
            uv,
            color,
            clip: self.current_clip(),
        });
    }

    pub fn text(&mut self, text: &str, x: f32, y: f32, color: Color, shadow: bool) {
        if text.is_empty() || color.a == 0 {
            return;
        }
        self.commands.push(GuiDrawCommand::Text {
            text: text.to_owned(),
            x,
            y,
            color,
            shadow,
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
    pub const fn glyph_width(&self) -> f32 {
        self.glyph_width as f32
    }

    pub const fn glyph_height(&self) -> f32 {
        self.glyph_height as f32
    }

    pub const fn advance(&self) -> f32 {
        self.advance as f32
    }

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

    pub fn draw_atlas(&self, draw: &mut GuiDrawList, text: &str, x: f32, y: f32, color: Color) {
        draw.text(text, x.floor(), y.floor(), color, false);
    }

    pub fn draw_shadow_atlas(
        &self,
        draw: &mut GuiDrawList,
        text: &str,
        x: f32,
        y: f32,
        color: Color,
    ) {
        draw.text(text, x.floor(), y.floor(), color, true);
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

    pub fn draw_centered_atlas(
        &self,
        draw: &mut GuiDrawList,
        text: &str,
        center_x: f32,
        y: f32,
        color: Color,
    ) {
        self.draw_shadow_atlas(draw, text, center_x - self.width(text) * 0.5, y, color);
    }

    pub fn glyph_rows(ch: char) -> [u8; 7] {
        glyph_rows(ch)
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
        self.render_with_text_mode(draw, font, interaction, false);
    }

    pub fn render_atlas_text(&self, draw: &mut GuiDrawList, font: &Font, interaction: Interaction) {
        self.render_with_text_mode(draw, font, interaction, true);
    }

    fn render_with_text_mode(
        &self,
        draw: &mut GuiDrawList,
        font: &Font,
        interaction: Interaction,
        atlas_text: bool,
    ) {
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
        let text_y =
            self.rect.y + ((self.rect.height - font.line_height()) * 0.5).floor() + y_offset;
        if atlas_text {
            font.draw_centered_atlas(draw, &self.label, self.rect.center_x(), text_y, text);
        } else {
            font.draw_centered(draw, &self.label, self.rect.center_x(), text_y, text);
        }
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
        self.render_with_text_mode(draw, font, interaction, false);
    }

    pub fn render_atlas_text(&self, draw: &mut GuiDrawList, font: &Font, interaction: Interaction) {
        self.render_with_text_mode(draw, font, interaction, true);
    }

    fn render_with_text_mode(
        &self,
        draw: &mut GuiDrawList,
        font: &Font,
        interaction: Interaction,
        atlas_text: bool,
    ) {
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
        let text_color = if self.enabled {
            Color::rgba(235, 242, 232, 255)
        } else {
            Color::rgba(135, 140, 136, 255)
        };
        if atlas_text {
            font.draw_shadow_atlas(
                draw,
                &self.label,
                self.rect.x + 18.0,
                self.rect.y + 4.0,
                text_color,
            );
        } else {
            font.draw_shadow(
                draw,
                &self.label,
                self.rect.x + 18.0,
                self.rect.y + 4.0,
                text_color,
            );
        }
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

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn contains(&self, point: Point) -> bool {
        self.enabled && self.rect.contains(point)
    }

    pub fn value_from_point(&self, point: Point) -> f32 {
        ((point.x - self.rect.x) / self.rect.width.max(1.0)).clamp(0.0, 1.0)
    }

    pub fn render(&self, draw: &mut GuiDrawList, font: &Font, interaction: Interaction) {
        self.render_with_text_mode(draw, font, interaction, false);
    }

    pub fn render_atlas_text(&self, draw: &mut GuiDrawList, font: &Font, interaction: Interaction) {
        self.render_with_text_mode(draw, font, interaction, true);
    }

    fn render_with_text_mode(
        &self,
        draw: &mut GuiDrawList,
        font: &Font,
        interaction: Interaction,
        atlas_text: bool,
    ) {
        Button::new(self.id, self.rect, &self.label)
            .enabled(self.enabled)
            .render_with_text_mode(draw, font, interaction, atlas_text);
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
        self.render_with_text_mode(draw, font, interaction, false);
    }

    pub fn render_atlas_text(&self, draw: &mut GuiDrawList, font: &Font, interaction: Interaction) {
        self.render_with_text_mode(draw, font, interaction, true);
    }

    fn render_with_text_mode(
        &self,
        draw: &mut GuiDrawList,
        font: &Font,
        interaction: Interaction,
        atlas_text: bool,
    ) {
        Button::new(
            self.id,
            self.rect,
            format!("{}: {}", self.label, self.value),
        )
        .enabled(self.enabled)
        .render_with_text_mode(draw, font, interaction, atlas_text);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuiKey {
    Escape,
    F1,
}

pub const WORLD_CATALOG_UI_ROW_CAPACITY: usize = 8;
pub const WORLD_CATALOG_UI_TEXT_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct WorldCatalogUiWorldId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldCatalogUiText {
    bytes: [u8; WORLD_CATALOG_UI_TEXT_CAPACITY],
    len: u8,
}

impl WorldCatalogUiText {
    pub const fn empty() -> Self {
        Self {
            bytes: [0; WORLD_CATALOG_UI_TEXT_CAPACITY],
            len: 0,
        }
    }

    pub fn new(value: &str) -> Self {
        let mut text = Self::empty();
        for ch in value.chars() {
            let mut encoded = [0; 4];
            let encoded = ch.encode_utf8(&mut encoded);
            let len = text.len as usize;
            if len + encoded.len() > WORLD_CATALOG_UI_TEXT_CAPACITY {
                break;
            }
            text.bytes[len..len + encoded.len()].copy_from_slice(encoded.as_bytes());
            text.len += encoded.len() as u8;
        }
        text
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("")
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Default for WorldCatalogUiText {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldCatalogUiEntry {
    pub id: WorldCatalogUiWorldId,
    pub display_name: WorldCatalogUiText,
    pub seed: i64,
    pub created_unix_millis: u64,
    pub last_played_unix_millis: Option<u64>,
    pub locked: bool,
    pub compatible: bool,
}

impl WorldCatalogUiEntry {
    pub fn new(id: WorldCatalogUiWorldId, display_name: &str, seed: i64) -> Self {
        Self {
            id,
            display_name: WorldCatalogUiText::new(display_name),
            seed,
            created_unix_millis: 0,
            last_played_unix_millis: None,
            locked: false,
            compatible: true,
        }
    }

    pub const fn with_created_unix_millis(mut self, created_unix_millis: u64) -> Self {
        self.created_unix_millis = created_unix_millis;
        self
    }

    pub const fn with_last_played_unix_millis(
        mut self,
        last_played_unix_millis: Option<u64>,
    ) -> Self {
        self.last_played_unix_millis = last_played_unix_millis;
        self
    }

    pub const fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    pub const fn compatible(mut self, compatible: bool) -> Self {
        self.compatible = compatible;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldCatalogUiStatus {
    pub visible: bool,
    pub ok: bool,
    pub message: WorldCatalogUiText,
}

impl WorldCatalogUiStatus {
    pub const fn hidden() -> Self {
        Self {
            visible: false,
            ok: true,
            message: WorldCatalogUiText::empty(),
        }
    }

    pub fn new(message: &str, ok: bool) -> Self {
        let message = WorldCatalogUiText::new(message);
        Self {
            visible: !message.is_empty(),
            ok,
            message,
        }
    }
}

impl Default for WorldCatalogUiStatus {
    fn default() -> Self {
        Self::hidden()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldCatalogUiState {
    pub persistent: bool,
    pub list_supported: bool,
    pub create_supported: bool,
    pub open_supported: bool,
    pub delete_supported: bool,
    pub loading: bool,
    pub entries: [Option<WorldCatalogUiEntry>; WORLD_CATALOG_UI_ROW_CAPACITY],
    pub selected: Option<WorldCatalogUiWorldId>,
    pub active: Option<WorldCatalogUiWorldId>,
    pub status: WorldCatalogUiStatus,
    pub create_display_name: WorldCatalogUiText,
}

impl WorldCatalogUiState {
    pub const fn empty() -> Self {
        Self {
            persistent: false,
            list_supported: false,
            create_supported: false,
            open_supported: false,
            delete_supported: false,
            loading: false,
            entries: [None; WORLD_CATALOG_UI_ROW_CAPACITY],
            selected: None,
            active: None,
            status: WorldCatalogUiStatus::hidden(),
            create_display_name: WorldCatalogUiText::empty(),
        }
    }

    pub fn persistent_local(entries: &[WorldCatalogUiEntry]) -> Self {
        let mut state = Self {
            persistent: true,
            list_supported: true,
            create_supported: true,
            open_supported: true,
            delete_supported: true,
            create_display_name: WorldCatalogUiText::new("New World"),
            ..Self::empty()
        };
        state.set_entries(entries);
        state
    }

    pub fn set_entries(&mut self, entries: &[WorldCatalogUiEntry]) {
        self.entries = [None; WORLD_CATALOG_UI_ROW_CAPACITY];
        for (slot, entry) in self.entries.iter_mut().zip(entries.iter().copied()) {
            *slot = Some(entry);
        }
        if self.selected.is_none_or(|id| self.entry(id).is_none()) {
            self.selected = self.entries.iter().flatten().next().map(|entry| entry.id);
        }
    }

    pub fn entry_count(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    pub fn entry(&self, id: WorldCatalogUiWorldId) -> Option<&WorldCatalogUiEntry> {
        self.entries.iter().flatten().find(|entry| entry.id == id)
    }

    pub fn selected_entry(&self) -> Option<&WorldCatalogUiEntry> {
        self.selected.and_then(|id| self.entry(id))
    }

    pub fn can_open_world(&self, id: WorldCatalogUiWorldId) -> bool {
        self.open_supported
            && self
                .entry(id)
                .is_some_and(|entry| !entry.locked && entry.compatible)
    }

    pub fn can_delete_world(&self, id: WorldCatalogUiWorldId) -> bool {
        self.delete_supported
            && self.active != Some(id)
            && self.entry(id).is_some_and(|entry| !entry.locked)
    }
}

impl Default for WorldCatalogUiState {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameScreen {
    Title,
    WorldList,
    WorldCreate,
    WorldDeleteConfirm { id: WorldCatalogUiWorldId },
    NewWorld,
    JoinRemote,
    Pause,
    Help { parent: GameHelpParent },
    BlockPalette,
    Options { parent: GameOptionsParent },
    ServerSettings { parent: GameOptionsParent },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameOptionsParent {
    Title,
    Pause,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameHelpParent {
    Game,
    Title,
    WorldList,
    WorldCreate,
    WorldDeleteConfirm,
    NewWorld,
    JoinRemote,
    Pause,
    OptionsTitle,
    OptionsPause,
}

impl GameHelpParent {
    const fn screen(self) -> Option<GameScreen> {
        match self {
            Self::Game => None,
            Self::Title => Some(GameScreen::Title),
            Self::WorldList => Some(GameScreen::WorldList),
            Self::WorldCreate => Some(GameScreen::WorldCreate),
            Self::WorldDeleteConfirm => Some(GameScreen::WorldList),
            Self::NewWorld => Some(GameScreen::NewWorld),
            Self::JoinRemote => Some(GameScreen::JoinRemote),
            Self::Pause => Some(GameScreen::Pause),
            Self::OptionsTitle => Some(GameScreen::Options {
                parent: GameOptionsParent::Title,
            }),
            Self::OptionsPause => Some(GameScreen::Options {
                parent: GameOptionsParent::Pause,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GameMovementMode {
    #[default]
    Walk,
    Fly,
    HandPush,
}

impl GameMovementMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Walk => "Walk",
            Self::Fly => "Fly",
            Self::HandPush => "Hand Push",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Walk => Self::Fly,
            Self::Fly => Self::HandPush,
            Self::HandPush => Self::Walk,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GameCollisionMode {
    #[default]
    Normal,
    NoClip,
}

impl GameCollisionMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::NoClip => "NoClip",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Normal => Self::NoClip,
            Self::NoClip => Self::Normal,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GameTravelAssistMode {
    #[default]
    Off,
    Blink,
    Warp,
}

impl GameTravelAssistMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Blink => "Blink",
            Self::Warp => "Warp",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Blink,
            Self::Blink => Self::Warp,
            Self::Warp => Self::Off,
        }
    }

    pub const fn is_enabled(self) -> bool {
        !matches!(self, Self::Off)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GameTurnMode {
    #[default]
    Snap15,
    Snap30,
    Smooth,
}

impl GameTurnMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Snap15 => "Snap 15",
            Self::Snap30 => "Snap 30",
            Self::Smooth => "Smooth",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Snap15 => Self::Snap30,
            Self::Snap30 => Self::Smooth,
            Self::Smooth => Self::Snap15,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GameXrTurnMode {
    #[default]
    Snap15,
    Snap30,
    Smooth,
}

impl GameXrTurnMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Snap15 => "Snap 15",
            Self::Snap30 => "Snap 30",
            Self::Smooth => "Smooth",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Snap15 => Self::Snap30,
            Self::Snap30 => Self::Smooth,
            Self::Smooth => Self::Snap15,
        }
    }
}

impl From<GameXrTurnMode> for GameTurnMode {
    fn from(value: GameXrTurnMode) -> Self {
        match value {
            GameXrTurnMode::Snap15 => Self::Snap15,
            GameXrTurnMode::Snap30 => Self::Snap30,
            GameXrTurnMode::Smooth => Self::Smooth,
        }
    }
}

impl From<GameTurnMode> for GameXrTurnMode {
    fn from(value: GameTurnMode) -> Self {
        match value {
            GameTurnMode::Snap15 => Self::Snap15,
            GameTurnMode::Snap30 => Self::Snap30,
            GameTurnMode::Smooth => Self::Smooth,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GamePlayerModel {
    #[default]
    Player,
    UprightBear,
}

impl GamePlayerModel {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Player => "Player",
            Self::UprightBear => "Bear",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Player => Self::UprightBear,
            Self::UprightBear => Self::Player,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GameSimulationCadence {
    pub host_rate_hz: u32,
    pub gameplay_rate_hz: u32,
    pub physics_rate_hz: u32,
}

impl GameSimulationCadence {
    pub const fn new(host_rate_hz: u32, gameplay_rate_hz: u32, physics_rate_hz: u32) -> Self {
        Self {
            host_rate_hz,
            gameplay_rate_hz,
            physics_rate_hz,
        }
    }

    pub const fn is_valid(self) -> bool {
        self.host_rate_hz > 0
            && self.gameplay_rate_hz > 0
            && self.physics_rate_hz > 0
            && lane_rate_is_clean(self.host_rate_hz, self.gameplay_rate_hz)
            && lane_rate_is_clean(self.host_rate_hz, self.physics_rate_hz)
    }

    pub fn next_host_rate(self) -> Self {
        let host_rate_hz = next_rate(self.host_rate_hz, &SERVER_HOST_RATE_OPTIONS);
        Self {
            host_rate_hz,
            gameplay_rate_hz: closest_clean_rate(
                host_rate_hz,
                self.gameplay_rate_hz,
                &SERVER_GAMEPLAY_RATE_OPTIONS,
            ),
            physics_rate_hz: closest_clean_rate(
                host_rate_hz,
                self.physics_rate_hz,
                &SERVER_PHYSICS_RATE_OPTIONS,
            ),
        }
    }

    pub fn next_gameplay_rate(self) -> Self {
        Self {
            gameplay_rate_hz: next_clean_rate(
                self.host_rate_hz,
                self.gameplay_rate_hz,
                &SERVER_GAMEPLAY_RATE_OPTIONS,
            ),
            ..self
        }
    }

    pub fn next_physics_rate(self) -> Self {
        Self {
            physics_rate_hz: next_clean_rate(
                self.host_rate_hz,
                self.physics_rate_hz,
                &SERVER_PHYSICS_RATE_OPTIONS,
            ),
            ..self
        }
    }

    pub fn label(self) -> String {
        format!(
            "{}/{}/{} Hz",
            self.host_rate_hz, self.gameplay_rate_hz, self.physics_rate_hz
        )
    }
}

impl Default for GameSimulationCadence {
    fn default() -> Self {
        Self::new(20, 20, 60)
    }
}

pub const SERVER_HOST_RATE_OPTIONS: [u32; 3] = [20, 30, 60];
pub const SERVER_GAMEPLAY_RATE_OPTIONS: [u32; 4] = [10, 20, 30, 60];
pub const SERVER_PHYSICS_RATE_OPTIONS: [u32; 3] = [30, 60, 120];

fn next_rate(current: u32, options: &[u32]) -> u32 {
    if options.is_empty() {
        return current;
    }
    let index = options
        .iter()
        .position(|rate| *rate == current)
        .map_or(0, |index| {
            if index + 1 >= options.len() {
                0
            } else {
                index + 1
            }
        });
    options[index]
}

fn next_clean_rate(host_rate_hz: u32, current: u32, options: &[u32]) -> u32 {
    let clean = options
        .iter()
        .copied()
        .filter(|rate| lane_rate_is_clean(host_rate_hz, *rate))
        .collect::<Vec<_>>();
    next_rate(current, &clean)
}

fn closest_clean_rate(host_rate_hz: u32, desired: u32, options: &[u32]) -> u32 {
    options
        .iter()
        .copied()
        .filter(|rate| lane_rate_is_clean(host_rate_hz, *rate))
        .min_by_key(|rate| (rate.abs_diff(desired), *rate < desired))
        .unwrap_or(desired)
}

const fn lane_rate_is_clean(host_rate_hz: u32, lane_rate_hz: u32) -> bool {
    if host_rate_hz == 0 || lane_rate_hz == 0 {
        false
    } else if lane_rate_hz >= host_rate_hz {
        lane_rate_hz % host_rate_hz == 0
    } else {
        host_rate_hz % lane_rate_hz == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameUiAction {
    StartWorld,
    OpenWorldList,
    OpenWorldCreate,
    SelectWorld(WorldCatalogUiWorldId),
    OpenWorld(WorldCatalogUiWorldId),
    CreateCatalogWorld,
    ConfirmDeleteWorld(WorldCatalogUiWorldId),
    DeleteWorld(WorldCatalogUiWorldId),
    CancelDeleteWorld,
    OpenNewWorld,
    OpenJoinRemote,
    RerollSeed,
    CreateWorld(i64),
    JoinRemote,
    Resume,
    OpenBlockPalette,
    OpenHelp(GameHelpParent),
    CloseHelp(GameHelpParent),
    AssignHotbarBlock { slot: u8, block_state: u32 },
    OpenOptions(GameOptionsParent),
    OpenServerSettings(GameOptionsParent),
    BackToTitle,
    BackToPause,
    QuitToTitle,
    ToggleSectionOcclusion,
    ToggleFullbright,
    ToggleFarLod,
    SetFarLodRange(i32),
    TogglePlayerCollisionBox,
    ToggleFirstPersonPlayer,
    ToggleCrosshair,
    ToggleFramePipelineOverlay,
    SetPlayerModel(GamePlayerModel),
    SetMovementMode(GameMovementMode),
    SetCollisionMode(GameCollisionMode),
    SetTravelAssistMode(GameTravelAssistMode),
    SetTurnMode(GameTurnMode),
    SetXrTurnMode(GameXrTurnMode),
    CycleFramePacing,
    CycleFpsCap,
    SetRenderDistance(i32),
    SetFlySpeed(f32),
    SetMovementSpeed(f32),
    SetTouchLookSensitivity(f32),
    SetTouchControlsMode(TouchControlsMode),
    SetServerSimulationCadence(GameSimulationCadence),
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

pub fn touch_controls_mode_label(mode: TouchControlsMode) -> &'static str {
    match mode {
        TouchControlsMode::Auto => "Auto",
        TouchControlsMode::On => "On",
        TouchControlsMode::Off => "Off",
    }
}

pub fn next_touch_controls_mode(mode: TouchControlsMode) -> TouchControlsMode {
    match mode {
        TouchControlsMode::Auto => TouchControlsMode::On,
        TouchControlsMode::On => TouchControlsMode::Off,
        TouchControlsMode::Off => TouchControlsMode::Auto,
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
    pub world_catalog: WorldCatalogUiState,
    pub render_distance: i32,
    pub min_render_distance: i32,
    pub max_render_distance: i32,
    pub section_occlusion_culling: bool,
    pub force_fullbright: bool,
    pub far_lod_enabled: bool,
    pub far_lod_range_chunks: i32,
    pub min_far_lod_range_chunks: i32,
    pub max_far_lod_range_chunks: i32,
    pub player_collision_box_visible: bool,
    pub first_person_player_visible: bool,
    pub crosshair_visible: Option<bool>,
    pub frame_pipeline_overlay_visible: bool,
    pub player_model: GamePlayerModel,
    pub movement_mode: GameMovementMode,
    pub collision_mode: Option<GameCollisionMode>,
    pub travel_assist_mode: Option<GameTravelAssistMode>,
    pub turn_mode: Option<GameTurnMode>,
    pub xr_turn_mode: Option<GameXrTurnMode>,
    pub fly_speed_multiplier: f32,
    pub min_fly_speed_multiplier: f32,
    pub max_fly_speed_multiplier: f32,
    pub movement_speed_multiplier: f32,
    pub min_movement_speed_multiplier: f32,
    pub max_movement_speed_multiplier: f32,
    pub frame_pacing_mode: GameFramePacingMode,
    pub fps_cap: u32,
    pub server_cadence: Option<GameSimulationCadence>,
    pub touch_controls_mode: Option<TouchControlsMode>,
    pub touch_settings: Option<GameTouchSettings>,
    pub block_palette: BlockPaletteOverlay,
}

impl Default for GameUiRenderState {
    fn default() -> Self {
        Self {
            world_catalog: WorldCatalogUiState::default(),
            render_distance: 2,
            min_render_distance: 2,
            max_render_distance: 16,
            section_occlusion_culling: true,
            force_fullbright: false,
            far_lod_enabled: false,
            far_lod_range_chunks: 12,
            min_far_lod_range_chunks: 1,
            max_far_lod_range_chunks: 64,
            player_collision_box_visible: false,
            first_person_player_visible: false,
            crosshair_visible: Some(true),
            frame_pipeline_overlay_visible: false,
            player_model: GamePlayerModel::Player,
            movement_mode: GameMovementMode::Walk,
            collision_mode: None,
            travel_assist_mode: None,
            turn_mode: None,
            xr_turn_mode: None,
            fly_speed_multiplier: 1.0,
            min_fly_speed_multiplier: 0.5,
            max_fly_speed_multiplier: 8.0,
            movement_speed_multiplier: 1.0,
            min_movement_speed_multiplier: 0.125,
            max_movement_speed_multiplier: 8.0,
            frame_pacing_mode: GameFramePacingMode::Vsync,
            fps_cap: 120,
            server_cadence: None,
            touch_controls_mode: None,
            touch_settings: None,
            block_palette: BlockPaletteOverlay::hidden(),
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

    pub fn far_lod_range_limits(self) -> (i32, i32) {
        (
            self.min_far_lod_range_chunks
                .min(self.max_far_lod_range_chunks),
            self.min_far_lod_range_chunks
                .max(self.max_far_lod_range_chunks),
        )
    }

    pub fn clamped_far_lod_range(self) -> i32 {
        let (min, max) = self.far_lod_range_limits();
        self.far_lod_range_chunks.clamp(min, max)
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

    pub fn movement_speed_multiplier_limits(self) -> (f32, f32) {
        let min = finite_or(self.min_movement_speed_multiplier, 0.5);
        let max = finite_or(self.max_movement_speed_multiplier, min);
        (min.min(max), min.max(max))
    }

    pub fn clamped_movement_speed_multiplier(self) -> f32 {
        let (min, max) = self.movement_speed_multiplier_limits();
        finite_or(self.movement_speed_multiplier, min).clamp(min, max)
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
pub struct FlatDebugOverlay {
    pub position: [f32; 3],
    pub chunk: [i32; 2],
    pub seed: Option<i64>,
    pub speed: f32,
    pub movement_mode: String,
    pub on_ground: bool,
    pub view: FlatDebugView,
    pub runner: Option<FlatDebugRunner>,
    pub chunks: Option<FlatDebugChunkCounts>,
    pub draw: Option<FlatDebugDrawCounts>,
    pub actors: Option<FlatDebugActorCounts>,
    pub mesh: Option<FlatDebugMeshCounts>,
    pub pending_compile_jobs: Option<usize>,
    pub day_time: Option<u64>,
    pub time_of_day: Option<f32>,
    pub selected_hotbar_slot: Option<u8>,
    pub target: Option<FlatDebugTarget>,
    pub render_options: Option<FlatDebugRenderOptions>,
    pub extra_lines: Vec<String>,
}

impl FlatDebugOverlay {
    pub fn new(
        position: [f32; 3],
        chunk: [i32; 2],
        speed: f32,
        movement_mode: impl Into<String>,
        on_ground: bool,
        view: FlatDebugView,
    ) -> Self {
        Self {
            position,
            chunk,
            seed: None,
            speed,
            movement_mode: movement_mode.into(),
            on_ground,
            view,
            runner: None,
            chunks: None,
            draw: None,
            actors: None,
            mesh: None,
            pending_compile_jobs: None,
            day_time: None,
            time_of_day: None,
            selected_hotbar_slot: None,
            target: None,
            render_options: None,
            extra_lines: Vec::new(),
        }
    }

    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "POS {:.1} {:.1} {:.1}",
                self.position[0], self.position[1], self.position[2]
            ),
            format!(
                "CHUNK {} {} SPEED {:.1}",
                self.chunk[0], self.chunk[1], self.speed
            ),
        ];
        if let Some(seed) = self.seed {
            lines.push(format!("SEED {seed}"));
        }
        lines.extend([
            format!(
                "MODE {} GROUND {}",
                self.movement_mode,
                if self.on_ground { "Y" } else { "N" }
            ),
            self.view.line(),
        ]);
        if let Some(runner) = &self.runner {
            lines.push(runner.line());
        }
        if let Some(chunks) = self.chunks {
            lines.push(chunks.line());
        }
        if let Some(draw) = self.draw {
            lines.push(draw.line());
        }
        if let Some(actors) = self.actors {
            lines.push(actors.line());
        }
        if let Some(mesh) = self.mesh {
            lines.push(mesh.line());
        }
        if let Some(pending_compile_jobs) = self.pending_compile_jobs {
            lines.push(format!("PENDING {pending_compile_jobs}"));
        }
        if let (Some(day_time), Some(time_of_day)) = (self.day_time, self.time_of_day) {
            lines.push(format!("TIME {day_time} {time_of_day:.3}"));
        }
        if let Some(selected_hotbar_slot) = self.selected_hotbar_slot {
            lines.push(format!("SLOT {}", u16::from(selected_hotbar_slot) + 1));
        }
        if let Some(target) = self.target {
            lines.push(target.line());
        }
        if let Some(render_options) = self.render_options {
            lines.push(render_options.line());
        }
        lines.extend(self.extra_lines.iter().cloned());
        lines
    }

    pub fn to_debug_overlay(&self) -> DebugOverlay {
        DebugOverlay::new("DEBUG", self.lines())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlatDebugView {
    pub render_distance: i32,
    pub tracking_radius: Option<i32>,
    pub center: Option<[i32; 2]>,
}

impl FlatDebugView {
    pub const fn render_distance(render_distance: i32) -> Self {
        Self {
            render_distance,
            tracking_radius: None,
            center: None,
        }
    }

    pub const fn with_tracking_radius(render_distance: i32, tracking_radius: i32) -> Self {
        Self {
            render_distance,
            tracking_radius: Some(tracking_radius),
            center: None,
        }
    }

    pub const fn with_center(render_distance: i32, center: [i32; 2]) -> Self {
        Self {
            render_distance,
            tracking_radius: None,
            center: Some(center),
        }
    }

    fn line(self) -> String {
        if let Some(center) = self.center {
            format!(
                "VIEW R{} CENTER {} {}",
                self.render_distance, center[0], center[1]
            )
        } else if let Some(tracking_radius) = self.tracking_radius {
            format!("VIEW R{} T{}", self.render_distance, tracking_radius)
        } else {
            format!("VIEW R{}", self.render_distance)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatDebugRunner {
    pub label: String,
    pub command_queue_depth: usize,
    pub update_queue_depth: usize,
}

impl FlatDebugRunner {
    pub fn new(
        label: impl Into<String>,
        command_queue_depth: usize,
        update_queue_depth: usize,
    ) -> Self {
        Self {
            label: label.into(),
            command_queue_depth,
            update_queue_depth,
        }
    }

    fn line(&self) -> String {
        format!(
            "RUN {} CQ{} UQ{}",
            self.label, self.command_queue_depth, self.update_queue_depth
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlatDebugChunkCounts {
    pub loaded: usize,
    pub visible: usize,
    pub pending_jobs: Option<usize>,
}

impl FlatDebugChunkCounts {
    pub const fn loaded_visible(loaded: usize, visible: usize) -> Self {
        Self {
            loaded,
            visible,
            pending_jobs: None,
        }
    }

    pub const fn loaded_visible_pending(
        loaded: usize,
        visible: usize,
        pending_jobs: usize,
    ) -> Self {
        Self {
            loaded,
            visible,
            pending_jobs: Some(pending_jobs),
        }
    }

    fn line(self) -> String {
        if let Some(pending_jobs) = self.pending_jobs {
            format!(
                "CHUNKS L{} V{} P{}",
                self.loaded, self.visible, pending_jobs
            )
        } else {
            format!("CHUNKS L{} V{}", self.loaded, self.visible)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlatDebugDrawCounts {
    pub drawn_sections: usize,
    pub section_count: usize,
    pub drawn_faces: u32,
    pub face_count: u32,
}

impl FlatDebugDrawCounts {
    fn line(self) -> String {
        format!(
            "DRAW S {}/{} F {}/{}",
            self.drawn_sections, self.section_count, self.drawn_faces, self.face_count
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlatDebugActorCounts {
    pub drawn_actors: usize,
    pub actor_count: usize,
    pub drawn_actor_indices: u32,
}

impl FlatDebugActorCounts {
    fn line(self) -> String {
        format!(
            "ACTOR R {}/{} I{}",
            self.drawn_actors, self.actor_count, self.drawn_actor_indices
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlatDebugMeshCounts {
    pub build_count: usize,
    pub upload_count: usize,
    pub render_count: usize,
}

impl FlatDebugMeshCounts {
    fn line(self) -> String {
        format!(
            "MESH B{} U{} R{}",
            self.build_count, self.upload_count, self.render_count
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatDebugTarget {
    Miss,
    Block { x: i32, y: i32, z: i32 },
}

impl FlatDebugTarget {
    fn line(self) -> String {
        match self {
            Self::Miss => "TARGET MISS".to_owned(),
            Self::Block { x, y, z } => format!("TARGET {x} {y} {z}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlatDebugRenderOptions {
    pub section_occlusion_culling: bool,
    pub force_fullbright: bool,
    pub color_profile: &'static str,
}

impl FlatDebugRenderOptions {
    fn line(self) -> String {
        let occlusion = if self.section_occlusion_culling {
            "ON"
        } else {
            "OFF"
        };
        let lighting = if self.force_fullbright {
            "FULL"
        } else {
            "LIGHT"
        };
        format!("OCC {occlusion}  {lighting}  COLOR {}", self.color_profile)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadingProgressCellStatus {
    None,
    Terrain,
    Surface,
    Features,
    Light,
    TargetReady,
}

impl LoadingProgressCellStatus {
    pub const fn color(self) -> Color {
        match self {
            Self::None => Color::rgba(0, 0, 0, 255),
            Self::Terrain => Color::rgba(209, 209, 209, 255),
            Self::Surface => Color::rgba(114, 104, 9, 255),
            Self::Features => Color::rgba(33, 198, 0, 255),
            Self::Light => Color::rgba(204, 204, 204, 255),
            Self::TargetReady => Color::WHITE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoadingProgressCell {
    pub relative_x: i32,
    pub relative_z: i32,
    pub status: LoadingProgressCellStatus,
    pub playable: bool,
}

impl LoadingProgressCell {
    pub const fn new(relative_x: i32, relative_z: i32, status: LoadingProgressCellStatus) -> Self {
        Self {
            relative_x,
            relative_z,
            status,
            playable: false,
        }
    }

    pub const fn playable(mut self, playable: bool) -> Self {
        self.playable = playable;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadingProgressOverlay {
    pub display_radius: u32,
    pub target_ready_chunks: usize,
    pub target_chunk_count: usize,
    pub playable_ready: bool,
    pub cells: Vec<LoadingProgressCell>,
    status_grid: Vec<LoadingProgressCellStatus>,
    playable_cell: Option<LoadingProgressCell>,
}

impl LoadingProgressOverlay {
    pub fn new(
        display_radius: u32,
        target_ready_chunks: usize,
        target_chunk_count: usize,
        playable_ready: bool,
        cells: impl IntoIterator<Item = LoadingProgressCell>,
    ) -> Self {
        let cells = cells.into_iter().collect::<Vec<_>>();
        let mut status_grid = vec![
            LoadingProgressCellStatus::None;
            loading_progress_grid_len(display_radius).unwrap_or(0)
        ];
        let mut playable_cell = None;
        for cell in &cells {
            if let Some(status) =
                loading_progress_grid_index(display_radius, cell.relative_x, cell.relative_z)
                    .and_then(|index| status_grid.get_mut(index))
            {
                *status = cell.status;
            }
            if cell.playable {
                playable_cell = Some(*cell);
            }
        }

        Self {
            display_radius,
            target_ready_chunks,
            target_chunk_count,
            playable_ready,
            cells,
            status_grid,
            playable_cell,
        }
    }

    pub fn grid_side(&self) -> usize {
        loading_progress_grid_side(self.display_radius).unwrap_or(0)
    }

    pub fn percent(&self) -> u8 {
        if self.target_chunk_count == 0 {
            return 0;
        }
        let ready = self.target_ready_chunks.min(self.target_chunk_count);
        ((ready * 100) / self.target_chunk_count) as u8
    }

    pub fn status_at(&self, relative_x: i32, relative_z: i32) -> LoadingProgressCellStatus {
        loading_progress_grid_index(self.display_radius, relative_x, relative_z)
            .and_then(|index| self.status_grid.get(index))
            .copied()
            .unwrap_or(LoadingProgressCellStatus::None)
    }

    pub fn playable_cell(&self) -> Option<LoadingProgressCell> {
        self.playable_cell
    }
}

fn loading_progress_grid_len(display_radius: u32) -> Option<usize> {
    let side = loading_progress_grid_side(display_radius)?;
    side.checked_mul(side)
}

fn loading_progress_grid_side(display_radius: u32) -> Option<usize> {
    let radius = usize::try_from(display_radius).ok()?;
    radius.checked_mul(2)?.checked_add(1)
}

fn loading_progress_grid_index(
    display_radius: u32,
    relative_x: i32,
    relative_z: i32,
) -> Option<usize> {
    let radius = i32::try_from(display_radius).ok()?;
    if relative_x < -radius || relative_x > radius || relative_z < -radius || relative_z > radius {
        return None;
    }

    let radius = i64::from(radius);
    let col = usize::try_from(i64::from(relative_x) + radius).ok()?;
    let row = usize::try_from(i64::from(relative_z) + radius).ok()?;
    let side = loading_progress_grid_side(display_radius)?;
    row.checked_mul(side)?.checked_add(col)
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
    pub hotbar_icons: [Option<GuiTextureUv>; HOTBAR_SLOT_COUNT_USIZE],
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlatHotbarOverlay {
    pub visible: bool,
    pub selected_slot: u8,
    pub icons: [Option<GuiTextureUv>; HOTBAR_SLOT_COUNT_USIZE],
}

impl FlatHotbarOverlay {
    pub fn hidden() -> Self {
        Self {
            visible: false,
            selected_slot: 0,
            icons: EMPTY_HOTBAR_ICONS,
        }
    }

    pub fn selected(selected_slot: u8) -> Self {
        Self {
            visible: true,
            selected_slot,
            icons: EMPTY_HOTBAR_ICONS,
        }
    }

    pub fn selected_with_icons(
        selected_slot: u8,
        icons: [Option<GuiTextureUv>; HOTBAR_SLOT_COUNT_USIZE],
    ) -> Self {
        Self {
            visible: true,
            selected_slot,
            icons,
        }
    }
}

impl Default for FlatHotbarOverlay {
    fn default() -> Self {
        Self::hidden()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GamepadHudOverlay {
    pub visible: bool,
    pub hotbar_hints_visible: bool,
    pub action_hints_visible: bool,
}

impl GamepadHudOverlay {
    pub fn hidden() -> Self {
        Self {
            visible: false,
            hotbar_hints_visible: false,
            action_hints_visible: false,
        }
    }

    pub fn visible() -> Self {
        Self {
            visible: true,
            hotbar_hints_visible: true,
            action_hints_visible: true,
        }
    }
}

impl Default for GamepadHudOverlay {
    fn default() -> Self {
        Self::hidden()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlatHud {
    pub input: ResolvedFlatInput,
    pub world_hud_visible: bool,
    pub crosshair_visible: bool,
    pub hotbar: FlatHotbarOverlay,
    pub gamepad: GamepadHudOverlay,
    pub touch: TouchOverlay,
    pub status: StatusOverlay,
    pub debug: Option<FlatHudDebugOverlay>,
    pub frame_pipeline: Option<FramePipelineHudOverlay>,
}

impl FlatHud {
    pub fn new(input: ResolvedFlatInput) -> Self {
        Self {
            input,
            world_hud_visible: true,
            crosshair_visible: true,
            hotbar: FlatHotbarOverlay::hidden(),
            gamepad: GamepadHudOverlay::visible(),
            touch: TouchOverlay::hidden(),
            status: StatusOverlay::hidden(),
            debug: None,
            frame_pipeline: None,
        }
    }

    pub fn debug_only(debug: FlatHudDebugOverlay) -> Self {
        let mut hud = Self::new(ResolvedFlatInput {
            preferred_prompt: None,
            touch_controls_visible: false,
            accepts_keyboard_mouse: false,
            accepts_touch: false,
            accepts_gamepad: false,
            accepts_xr_controller: false,
        });
        hud.world_hud_visible = false;
        hud.crosshair_visible = false;
        hud.debug = Some(debug);
        hud
    }

    pub fn has_visible_commands(&self) -> bool {
        (self.world_hud_visible && self.crosshair_visible)
            || (self.world_hud_visible && self.should_render_flat_hotbar())
            || self.effective_gamepad_overlay().visible
            || self.effective_touch_overlay().visible
            || self.status.visible
            || self
                .debug
                .as_ref()
                .is_some_and(FlatHudDebugOverlay::visible)
            || self
                .frame_pipeline
                .as_ref()
                .is_some_and(FramePipelineHudOverlay::visible)
    }

    pub(crate) fn should_render_flat_hotbar(&self) -> bool {
        let touch = self.effective_touch_overlay();
        self.hotbar.visible && !(touch.visible && touch.hotbar_visible)
    }

    pub(crate) fn effective_touch_overlay(&self) -> TouchOverlay {
        let mut touch = self.touch;
        touch.visible &= self.world_hud_visible && self.input.touch_controls_visible;
        touch
    }

    pub(crate) fn effective_gamepad_overlay(&self) -> GamepadHudOverlay {
        let mut gamepad = self.gamepad;
        gamepad.visible &= self.world_hud_visible
            && self.input.accepts_gamepad
            && self.input.preferred_prompt == Some(InputPromptKind::Gamepad);
        gamepad
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlatHudDebugOverlay {
    pub overlay: DebugOverlay,
    pub origin: Point,
}

impl FlatHudDebugOverlay {
    pub fn new(overlay: DebugOverlay) -> Self {
        Self::at(overlay, Point { x: 4.0, y: 4.0 })
    }

    pub fn at(overlay: DebugOverlay, origin: Point) -> Self {
        Self { overlay, origin }
    }

    pub fn visible(&self) -> bool {
        !self.overlay.is_empty()
    }
}

pub fn render_flat_hud(scale: GuiScale, draw: &mut GuiDrawList, hud: &FlatHud) {
    render_flat_hud_retained_layer(scale, draw, hud);
    render_flat_hud_hotbar_layer(scale, draw, hud);
    render_flat_hud_status_layer(scale, draw, hud);
    render_flat_hud_prompt_layer(scale, draw, hud);
    render_flat_hud_debug_layer(scale, draw, hud);
    render_flat_hud_frame_pipeline_layer(scale, draw, hud);
    render_flat_hud_transient_layers(scale, draw, hud);
}

pub(crate) fn render_flat_hud_retained_layer(
    scale: GuiScale,
    draw: &mut GuiDrawList,
    hud: &FlatHud,
) {
    if hud.world_hud_visible {
        if hud.crosshair_visible {
            render_crosshair(scale, draw);
        }
        if hud.should_render_flat_hotbar() {
            render_flat_hotbar_frame(draw, scale);
        }
    }
}

pub(crate) fn render_flat_hud_hotbar_layer(scale: GuiScale, draw: &mut GuiDrawList, hud: &FlatHud) {
    if hud.world_hud_visible && hud.should_render_flat_hotbar() {
        render_flat_hotbar_selection(draw, scale, hud.hotbar.selected_slot);
        render_flat_hotbar_contents(draw, &Font::default(), scale, hud.hotbar);
    }
}

pub(crate) fn render_flat_hud_status_layer(scale: GuiScale, draw: &mut GuiDrawList, hud: &FlatHud) {
    render_status_overlay(scale, draw, &hud.status);
}

pub(crate) fn render_flat_hud_prompt_layer(scale: GuiScale, draw: &mut GuiDrawList, hud: &FlatHud) {
    let touch = hud.effective_touch_overlay();
    let gamepad = hud.effective_gamepad_overlay();
    render_gamepad_hud(scale, draw, gamepad, hud.should_render_flat_hotbar());
    render_touch_overlay(scale, draw, &touch);
}

pub(crate) fn render_flat_hud_debug_layer(scale: GuiScale, draw: &mut GuiDrawList, hud: &FlatHud) {
    let Some(debug) = hud.debug.as_ref().filter(|debug| debug.visible()) else {
        return;
    };
    render_debug_overlay_at(scale, draw, &debug.overlay, debug.origin);
}

pub(crate) fn render_flat_hud_frame_pipeline_layer(
    scale: GuiScale,
    draw: &mut GuiDrawList,
    hud: &FlatHud,
) {
    let Some(overlay) = hud
        .frame_pipeline
        .as_ref()
        .filter(|overlay| overlay.visible())
    else {
        return;
    };
    render_frame_pipeline_overlay(scale, draw, overlay);
}

pub(crate) fn render_flat_hud_transient_layers(
    _scale: GuiScale,
    _draw: &mut GuiDrawList,
    _hud: &FlatHud,
) {
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

pub fn render_loading_progress_overlay(
    scale: GuiScale,
    draw: &mut GuiDrawList,
    progress: &LoadingProgressOverlay,
) {
    let side = progress.grid_side().max(1) as f32;
    let max_grid = (scale.width - 32.0).min(scale.height - 64.0).max(1.0);
    let cell_size = (max_grid / side).min(2.0).max(1.0);
    let grid_size = side * cell_size;
    let grid = Rect::new(
        ((scale.width - grid_size) * 0.5).floor(),
        ((scale.height - grid_size) * 0.5).floor(),
        grid_size,
        grid_size,
    );
    let font = Font::default();

    draw.fill(
        Rect::new(0.0, 0.0, scale.width, scale.height),
        Color::rgba(0, 0, 0, 190),
    );
    font.draw_centered(
        draw,
        &format!("{}%", progress.percent()),
        scale.width * 0.5,
        (grid.y - font.line_height() - 8.0).max(8.0).floor(),
        Color::WHITE,
    );

    let radius = progress.display_radius as i32;
    for relative_z in -radius..=radius {
        for relative_x in -radius..=radius {
            let status = progress.status_at(relative_x, relative_z);
            let col = (relative_x + radius) as f32;
            let row = (relative_z + radius) as f32;
            draw.fill(
                Rect::new(
                    grid.x + col * cell_size,
                    grid.y + row * cell_size,
                    cell_size,
                    cell_size,
                ),
                status.color(),
            );
        }
    }

    if let Some(playable) = progress.playable_cell() {
        let col = (playable.relative_x + radius) as f32;
        let row = (playable.relative_z + radius) as f32;
        if col >= 0.0 && row >= 0.0 && col < side && row < side {
            let outline = if progress.playable_ready {
                Color::WHITE
            } else {
                Color::rgba(242, 96, 96, 255)
            };
            draw.outline(
                Rect::new(
                    grid.x + col * cell_size,
                    grid.y + row * cell_size,
                    cell_size,
                    cell_size,
                ),
                outline,
            );
        }
    }
}

pub fn render_loading_progress_panel_at(
    scale: GuiScale,
    draw: &mut GuiDrawList,
    progress: &LoadingProgressOverlay,
    origin: Point,
    label: &str,
) {
    let side = progress.grid_side().max(1) as f32;
    let available_width = (scale.width - origin.x - 4.0).max(1.0);
    let available_height = (scale.height - origin.y - 4.0).max(1.0);
    let font = Font::default();
    let title = format!("{label} {}%", progress.percent());
    let max_grid = available_width
        .min(available_height - 28.0)
        .min(96.0)
        .max(1.0);
    let cell_size = (max_grid / side).min(3.0).max(1.0).floor();
    let grid_size = side * cell_size;
    let panel_width = (grid_size + 14.0)
        .max(font.width(&title) + 14.0)
        .min(available_width);
    let panel_height = (grid_size + font.line_height() + 15.0).min(available_height);
    let panel = Rect::new(origin.x, origin.y, panel_width, panel_height);
    let grid = Rect::new(
        panel.x + ((panel.width - grid_size) * 0.5).floor(),
        panel.y + font.line_height() + 10.0,
        grid_size,
        grid_size,
    );

    draw.fill(panel, Color::rgba(6, 9, 10, 185));
    draw.outline(panel, Color::rgba(110, 140, 136, 230));
    draw.push_clip(panel.inset(4.0));
    font.draw_centered(
        draw,
        &title,
        panel.x + panel.width * 0.5,
        panel.y + 5.0,
        Color::rgba(220, 238, 220, 255),
    );

    let radius = progress.display_radius as i32;
    for relative_z in -radius..=radius {
        for relative_x in -radius..=radius {
            let status = progress.status_at(relative_x, relative_z);
            let col = (relative_x + radius) as f32;
            let row = (relative_z + radius) as f32;
            draw.fill(
                Rect::new(
                    grid.x + col * cell_size,
                    grid.y + row * cell_size,
                    cell_size,
                    cell_size,
                ),
                status.color(),
            );
        }
    }

    if let Some(playable) = progress.playable_cell() {
        let col = (playable.relative_x + radius) as f32;
        let row = (playable.relative_z + radius) as f32;
        if col >= 0.0 && row >= 0.0 && col < side && row < side {
            let outline = if progress.playable_ready {
                Color::WHITE
            } else {
                Color::rgba(242, 96, 96, 255)
            };
            draw.outline(
                Rect::new(
                    grid.x + col * cell_size,
                    grid.y + row * cell_size,
                    cell_size,
                    cell_size,
                ),
                outline,
            );
        }
    }
    draw.pop_clip();
}

pub fn render_crosshair(scale: GuiScale, draw: &mut GuiDrawList) {
    let center_x = (scale.width * 0.5).floor();
    let center_y = (scale.height * 0.5).floor();
    let shadow = Color::rgba(0, 0, 0, 115);
    let color = Color::rgba(238, 244, 250, 220);
    draw.fill(Rect::new(center_x - 1.0, center_y - 7.0, 3.0, 15.0), shadow);
    draw.fill(Rect::new(center_x - 7.0, center_y - 1.0, 15.0, 3.0), shadow);
    draw.fill(Rect::new(center_x, center_y - 6.0, 1.0, 13.0), color);
    draw.fill(Rect::new(center_x - 6.0, center_y, 13.0, 1.0), color);
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

const BLOCK_PALETTE_COLUMNS: usize = 10;
const BLOCK_PALETTE_SLOT_SIZE: f32 = 22.0;
const BLOCK_PALETTE_GAP: f32 = 3.0;
const BLOCK_PALETTE_PADDING: f32 = 7.0;
const BLOCK_PALETTE_HEADER_HEIGHT: f32 = 14.0;

pub const DEFAULT_JOIN_REMOTE_ADDR: &str = "127.0.0.1:25565";

fn block_palette_occupied_span(overlay: BlockPaletteOverlay) -> usize {
    overlay
        .entries
        .iter()
        .rposition(Option::is_some)
        .map_or(0, |index| index + 1)
}

fn block_palette_rows(overlay: BlockPaletteOverlay) -> usize {
    let occupied = block_palette_occupied_span(overlay).max(1);
    (occupied + BLOCK_PALETTE_COLUMNS - 1) / BLOCK_PALETTE_COLUMNS
}

fn block_palette_panel_rect(scale: GuiScale, overlay: BlockPaletteOverlay) -> Rect {
    let rows = block_palette_rows(overlay) as f32;
    let columns = BLOCK_PALETTE_COLUMNS as f32;
    let grid_width = columns * BLOCK_PALETTE_SLOT_SIZE + (columns - 1.0) * BLOCK_PALETTE_GAP;
    let grid_height = rows * BLOCK_PALETTE_SLOT_SIZE + (rows - 1.0).max(0.0) * BLOCK_PALETTE_GAP;
    let panel_width = grid_width + BLOCK_PALETTE_PADDING * 2.0;
    let panel_height = grid_height + BLOCK_PALETTE_HEADER_HEIGHT + BLOCK_PALETTE_PADDING * 2.0;
    let hotbar = flat_hotbar_slot_rects(scale);
    let x = ((scale.width - panel_width) * 0.5)
        .floor()
        .clamp(4.0, (scale.width - panel_width - 4.0).max(4.0));
    let y = (hotbar[0].y - panel_height - 8.0)
        .floor()
        .clamp(4.0, (scale.height - panel_height - 4.0).max(4.0));
    Rect::new(x, y, panel_width.min(scale.width - 8.0), panel_height)
}

fn block_palette_slot_rect(
    scale: GuiScale,
    overlay: BlockPaletteOverlay,
    index: usize,
) -> Option<Rect> {
    overlay
        .entries
        .get(index)
        .and_then(|entry| entry.as_ref())?;
    let panel = block_palette_panel_rect(scale, overlay);
    let column = index % BLOCK_PALETTE_COLUMNS;
    let row = index / BLOCK_PALETTE_COLUMNS;
    Some(Rect::new(
        panel.x
            + BLOCK_PALETTE_PADDING
            + column as f32 * (BLOCK_PALETTE_SLOT_SIZE + BLOCK_PALETTE_GAP),
        panel.y
            + BLOCK_PALETTE_PADDING
            + BLOCK_PALETTE_HEADER_HEIGHT
            + row as f32 * (BLOCK_PALETTE_SLOT_SIZE + BLOCK_PALETTE_GAP),
        BLOCK_PALETTE_SLOT_SIZE,
        BLOCK_PALETTE_SLOT_SIZE,
    ))
}

fn render_palette_slot_contents(
    draw: &mut GuiDrawList,
    font: &Font,
    rect: Rect,
    icon: Option<GuiTextureUv>,
) {
    if let Some(icon) = icon {
        let icon_size = 16.0_f32.min(rect.width - 4.0).min(rect.height - 4.0);
        draw.texture(
            Rect::new(
                (rect.center_x() - icon_size * 0.5).floor(),
                (rect.y + (rect.height - icon_size) * 0.5).floor(),
                icon_size,
                icon_size,
            ),
            icon,
            Color::WHITE,
        );
    } else {
        font.draw_centered(
            draw,
            "?",
            rect.center_x(),
            rect.y + ((rect.height - font.line_height()) * 0.5).floor(),
            Color::rgba(245, 250, 255, 210),
        );
    }
}

fn render_block_palette_tooltip(
    draw: &mut GuiDrawList,
    font: &Font,
    scale: GuiScale,
    pointer: Point,
    label: &str,
) {
    let width = (font.width(label) + 12.0).min((scale.width - 8.0).max(0.0));
    let height = font.line_height() + 8.0;
    let x = (pointer.x + 8.0)
        .floor()
        .clamp(4.0, (scale.width - width - 4.0).max(4.0));
    let y = (pointer.y - height - 8.0)
        .floor()
        .clamp(4.0, (scale.height - height - 4.0).max(4.0));
    let rect = Rect::new(x, y, width, height);
    draw.fill(rect, Color::rgba(8, 11, 12, 230));
    draw.outline(rect, Color::rgba(178, 196, 188, 230));
    font.draw_shadow(
        draw,
        label,
        rect.x + 6.0,
        rect.y + 5.0,
        Color::rgba(238, 246, 240, 255),
    );
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

fn far_lod_range_slider_value(state: GameUiRenderState) -> f32 {
    let (min, max) = state.far_lod_range_limits();
    if max <= min {
        0.0
    } else {
        (state.clamped_far_lod_range() - min) as f32 / (max - min) as f32
    }
}

fn far_lod_range_from_slider_value(value: f32, state: GameUiRenderState) -> i32 {
    let (min, max) = state.far_lod_range_limits();
    if max <= min {
        min
    } else {
        min + (value.clamp(0.0, 1.0) * (max - min) as f32).round() as i32
    }
}

fn far_lod_range_label(state: GameUiRenderState) -> String {
    let range = state.clamped_far_lod_range();
    let suffix = if range == 1 { "chunk" } else { "chunks" };
    format!("Far LOD Range: {range} {suffix}")
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

fn movement_speed_slider_value(state: GameUiRenderState) -> f32 {
    let (min, max) = state.movement_speed_multiplier_limits();
    if max <= min || min <= 0.0 {
        0.0
    } else {
        (state.clamped_movement_speed_multiplier().ln() - min.ln()) / (max.ln() - min.ln())
    }
}

fn movement_speed_from_slider_value(value: f32, state: GameUiRenderState) -> f32 {
    let (min, max) = state.movement_speed_multiplier_limits();
    if max <= min || min <= 0.0 {
        min
    } else {
        let raw = (min.ln() + value.clamp(0.0, 1.0) * (max.ln() - min.ln())).exp();
        ((raw * 10.0).round() / 10.0).clamp(min, max)
    }
}

fn movement_speed_label(state: GameUiRenderState) -> String {
    format!(
        "Movement Speed: {:.1}x",
        state.clamped_movement_speed_multiplier()
    )
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

pub fn flat_hotbar_slot_rects(scale: GuiScale) -> [Rect; 9] {
    let slot = 22.0;
    let gap = 3.0;
    let total_width = slot * 9.0 + gap * 8.0;
    let x0 = ((scale.width - total_width) * 0.5).max(4.0);
    let y = (scale.height - 34.0).max(58.0);
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
        render_hotbar_slot_contents(draw, font, rect, overlay.hotbar_icons[index], index, 20.0);
    }
}

fn render_flat_hotbar_frame(draw: &mut GuiDrawList, scale: GuiScale) {
    for rect in flat_hotbar_slot_rects(scale) {
        render_touch_panel(draw, rect, false);
    }
}

fn render_flat_hotbar_selection(draw: &mut GuiDrawList, scale: GuiScale, selected_slot: u8) {
    let selected_slot = selected_slot.min(FLAT_HOTBAR_SLOT_COUNT.saturating_sub(1));
    if let Some(rect) = flat_hotbar_slot_rects(scale)
        .into_iter()
        .nth(usize::from(selected_slot))
    {
        draw.outline(rect.inset(-2.0), Color::rgba(245, 250, 255, 225));
    }
}

fn render_flat_hotbar_contents(
    draw: &mut GuiDrawList,
    font: &Font,
    scale: GuiScale,
    hotbar: FlatHotbarOverlay,
) {
    for (index, rect) in flat_hotbar_slot_rects(scale).into_iter().enumerate() {
        render_hotbar_slot_contents(draw, font, rect, hotbar.icons[index], index, 16.0);
    }
}

fn render_hotbar_slot_contents(
    draw: &mut GuiDrawList,
    font: &Font,
    rect: Rect,
    icon: Option<GuiTextureUv>,
    index: usize,
    icon_size: f32,
) {
    if let Some(icon) = icon {
        let icon_size = icon_size.min(rect.width - 4.0).min(rect.height - 4.0);
        draw.texture(
            Rect::new(
                (rect.center_x() - icon_size * 0.5).floor(),
                (rect.y + (rect.height - icon_size) * 0.5).floor(),
                icon_size,
                icon_size,
            ),
            icon,
            Color::WHITE,
        );
        return;
    }

    font.draw_centered(
        draw,
        &(index + 1).to_string(),
        rect.center_x(),
        rect.y + ((rect.height - font.line_height()) * 0.5).floor(),
        Color::rgba(245, 250, 255, 210),
    );
}

fn render_gamepad_hud(
    scale: GuiScale,
    draw: &mut GuiDrawList,
    overlay: GamepadHudOverlay,
    flat_hotbar_visible: bool,
) {
    if !overlay.visible {
        return;
    }
    let font = Font::default();
    if overlay.hotbar_hints_visible && flat_hotbar_visible {
        let hotbar = flat_hotbar_slot_rects(scale);
        let first = hotbar[0];
        let last = hotbar[hotbar.len() - 1];
        let y = first.y + ((first.height - 16.0) * 0.5).floor();
        let left = Rect::new((first.x - 34.0).max(4.0), y, 28.0, 16.0);
        let right = Rect::new(
            (last.right() + 6.0).min(scale.width - 32.0).max(4.0),
            y,
            28.0,
            16.0,
        );
        render_gamepad_prompt_chip(draw, &font, left, "LB");
        render_gamepad_prompt_chip(draw, &font, right, "RB");
    }
    if overlay.action_hints_visible {
        for (rect, label) in gamepad_action_prompt_rects(scale) {
            render_gamepad_prompt_chip(draw, &font, rect, label);
        }
    }
}

fn render_gamepad_prompt_chip(draw: &mut GuiDrawList, font: &Font, rect: Rect, label: &str) {
    draw.fill(rect, Color::rgba(0, 0, 0, 92));
    draw.outline(rect, Color::rgba(210, 230, 244, 92));
    draw.outline(rect.inset(1.0), Color::rgba(0, 0, 0, 72));
    font.draw_centered(
        draw,
        label,
        rect.center_x(),
        rect.y + ((rect.height - font.line_height()) * 0.5).floor(),
        Color::rgba(235, 244, 248, 220),
    );
}

fn gamepad_action_prompt_rects(scale: GuiScale) -> [(Rect, &'static str); 4] {
    let size = 18.0;
    let gap = 4.0;
    let right = 12.0;
    let bottom = 46.0;
    let x1 = (scale.width - right - size).max(4.0);
    let x0 = (x1 - gap - size).max(4.0);
    let y1 = (scale.height - bottom - size).max(4.0);
    let y0 = (y1 - gap - size).max(4.0);
    [
        (Rect::new(x0, y0, size, size), "Y"),
        (Rect::new(x0, y1, size, size), "X"),
        (Rect::new(x1, y0, size, size), "B"),
        (Rect::new(x1, y1, size, size), "A"),
    ]
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
mod tests;
