use glam::Vec3;
#[cfg(test)]
use mclone_core::ChunkPos;
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_ui::{
    Button, Checkbox, Color, CycleButton, Font, GuiDrawList, GuiScale, Interaction, Point, Rect,
    Slider, WidgetId,
};
use winit::keyboard::KeyCode;

use crate::app::RenderStreamStats;
use crate::cli::HeadlessScreenshotUi;
use crate::frame_pacing::{
    FramePacingDebugStats, FramePacingMode, FramePacingUiState, FrameTimingStats,
};
use crate::scene_runtime::WindowRuntimeStats;
use crate::{DEFAULT_RENDER_DISTANCE, MAX_RENDER_DISTANCE, MIN_RENDER_DISTANCE};

const MIN_UI_RENDER_DISTANCE: i32 = MIN_RENDER_DISTANCE;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DebugPaneStats {
    pub(crate) position: Vec3,
    pub(crate) speed: f32,
    pub(crate) movement_mode: &'static str,
    pub(crate) on_ground: bool,
    pub(crate) runtime: WindowRuntimeStats,
    pub(crate) render: RenderStreamStats,
    pub(crate) frame: FrameTimingStats,
    pub(crate) pacing: FramePacingDebugStats,
    pub(crate) section_occlusion: bool,
    pub(crate) force_fullbright: bool,
}

impl DebugPaneStats {
    pub(crate) fn lines(self) -> Vec<String> {
        let occlusion = if self.section_occlusion { "ON" } else { "OFF" };
        let lighting = if self.force_fullbright {
            "FULL"
        } else {
            "LIGHT"
        };
        let budget = self
            .pacing
            .target_frame_ms
            .map(|ms| format!("{ms:.1}MS"))
            .unwrap_or_else(|| "UNCAPPED".to_owned());
        let refresh = self
            .pacing
            .monitor_refresh_hz
            .map(|hz| format!("{hz:.1}HZ"))
            .unwrap_or_else(|| "UNKNOWN".to_owned());
        let pacing_target = match self.pacing.mode {
            FramePacingMode::Capped => format!("{}FPS", self.pacing.fps_cap),
            FramePacingMode::Vsync | FramePacingMode::Uncapped => refresh,
        };
        vec![
            "DEBUG".to_string(),
            format!(
                "POS {:.1} {:.1} {:.1}",
                self.position.x, self.position.y, self.position.z
            ),
            format!(
                "CHUNK {} {} SPEED {:.1}",
                self.runtime.interest_center.x, self.runtime.interest_center.z, self.speed
            ),
            format!(
                "MODE {} GROUND {}",
                self.movement_mode,
                if self.on_ground { "Y" } else { "N" }
            ),
            format!(
                "VIEW R{} T{}",
                self.runtime.render_distance, self.runtime.chunk_tracking_radius
            ),
            format!("OCC {}  {}", occlusion, lighting),
            format!(
                "TICK {} SIM {}",
                self.runtime.last_tick, self.runtime.last_simulation_tick
            ),
            format!(
                "CHUNKS L{} V{} P{}",
                self.runtime.loaded_chunks,
                self.runtime.client_visible_chunks,
                self.runtime.pending_jobs
            ),
            format!("STREAM PUB{}", self.runtime.pending_publications),
            format!("MESH Q{}", self.runtime.pending_render_chunks),
            format!(
                "TICKING B{}:{} E{}:{}",
                self.runtime.block_ticking_chunks,
                self.runtime.last_simulation_block_tick_chunks,
                self.runtime.entity_ticking_chunks,
                self.runtime.last_simulation_entity_tick_chunks
            ),
            format!(
                "FLUID {}/{}/{}/{}",
                self.runtime.last_simulation_fluid_ticks_executed,
                self.runtime.last_simulation_deferred_fluid_ticks,
                self.runtime.last_simulation_fluid_mutated_blocks,
                self.runtime.scheduled_fluid_ticks
            ),
            format!(
                "DRAW S {}/{} F {}/{}",
                self.render.drawn_section_count,
                self.render.section_count,
                self.render.drawn_face_count,
                self.render.face_count
            ),
            format!(
                "MESH R{} U{} D{} SQ{} CQ{} X{} F {:.1}MS",
                self.render.last_rebuilt_section_count,
                self.render.last_uploaded_section_count,
                self.render.last_deferred_section_count,
                self.render.last_submitted_compile_section_count,
                self.render.last_completed_compile_section_count,
                self.render.last_stale_compile_section_count,
                self.render.last_frame_ms
            ),
            format!("BUDGET {} FRAME {:.1}MS", budget, self.frame.last_frame_ms),
            format!(
                "OVER {}/{}/{} WORST {:.1}",
                self.frame.over_budget_count,
                self.frame.over_2x_budget_count,
                self.frame.over_4x_budget_count,
                self.frame.worst_frame_ms
            ),
            format!(
                "STAGE POLL {:.1} MESH {:.1} UP {:.1}",
                self.frame.last_runtime_poll_ms,
                self.frame.last_remesh_ms,
                self.frame.last_upload_ms
            ),
            format!(
                "GPU ACQ {:.1} ENC {:.1} SUB {:.1} PRS {:.1}",
                self.frame.last_surface_acquire_ms,
                self.frame.last_surface_encode_ms,
                self.frame.last_surface_submit_ms,
                self.frame.last_surface_present_ms
            ),
            format!(
                "PACE {} {} {}",
                self.pacing.mode.label(),
                pacing_target,
                self.pacing.active_present_mode_label.to_ascii_uppercase()
            ),
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeScreen {
    Title,
    Pause,
    Options { parent: OptionsParent },
}

impl HeadlessScreenshotUi {
    pub(crate) fn native_screen(self) -> Option<NativeScreen> {
        match self {
            Self::None => None,
            Self::Title => Some(NativeScreen::Title),
            Self::Pause => Some(NativeScreen::Pause),
            Self::OptionsTitle => Some(NativeScreen::Options {
                parent: OptionsParent::Title,
            }),
            Self::OptionsPause => Some(NativeScreen::Options {
                parent: OptionsParent::Pause,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OptionsParent {
    Title,
    Pause,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeUiAction {
    StartWorld,
    Resume,
    OpenOptions(OptionsParent),
    BackToTitle,
    BackToPause,
    ToggleSectionOcclusion,
    ToggleFullbright,
    CycleFramePacing,
    CycleFpsCap,
    SetRenderDistance(i32),
    Quit,
}

pub(crate) struct NativeUi {
    pub(crate) screen: Option<NativeScreen>,
    pub(crate) pointer: Option<Point>,
    pub(crate) pressed: Option<WidgetId>,
    pub(crate) font: Font,
    pub(crate) render_distance: i32,
    pub(crate) scale: GuiScale,
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

impl NativeUi {
    pub(crate) fn new(render_distance: i32) -> Self {
        Self {
            screen: Some(NativeScreen::Title),
            pointer: None,
            pressed: None,
            font: Font::default(),
            render_distance,
            scale: GuiScale::from_pixels(1280, 900),
        }
    }

    pub(crate) fn new_ingame(render_distance: i32) -> Self {
        let mut ui = Self::new(render_distance);
        ui.set_screen(None);
        ui
    }

    pub(crate) fn set_scale(&mut self, scale: GuiScale) {
        self.scale = scale;
        self.pointer = self.pointer.map(|point| Point {
            x: point.x.clamp(0.0, scale.width),
            y: point.y.clamp(0.0, scale.height),
        });
    }

    pub(crate) fn is_active(&self) -> bool {
        self.screen.is_some()
    }

    pub(crate) fn covers_world(&self) -> bool {
        self.screen == Some(NativeScreen::Title)
    }

    pub(crate) fn open_pause(&mut self) {
        self.screen = Some(NativeScreen::Pause);
        self.pressed = None;
    }

    pub(crate) fn close(&mut self) {
        self.screen = None;
        self.pressed = None;
    }

    pub(crate) fn set_screen(&mut self, screen: Option<NativeScreen>) {
        self.screen = screen;
        self.pressed = None;
    }

    pub(crate) fn clear_input(&mut self) {
        self.pointer = None;
        self.pressed = None;
    }

    pub(crate) fn pointer_move(&mut self, point: Point) -> (bool, Option<NativeUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.pointer = Some(point);
        let action = if self.pressed == Some(ID_OPTIONS_RADIUS) {
            Some(self.render_distance_action_at(point))
        } else {
            None
        };
        (true, action)
    }

    pub(crate) fn pointer_down(&mut self, point: Point) -> bool {
        if !self.is_active() {
            return false;
        }
        self.pointer = Some(point);
        self.pressed = self.widget_at(point);
        true
    }

    pub(crate) fn pointer_up(&mut self, point: Point) -> (bool, Option<NativeUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.pointer = Some(point);
        let pressed = self.pressed.take();
        let released = self.widget_at(point);
        let action = match (pressed, released) {
            (Some(ID_OPTIONS_RADIUS), Some(ID_OPTIONS_RADIUS)) => {
                Some(self.render_distance_action_at(point))
            }
            (Some(id), Some(released)) if id == released => self.action_for(id),
            _ => None,
        };
        (true, action)
    }

    pub(crate) fn key_pressed(&mut self, key: KeyCode) -> (bool, Option<NativeUiAction>) {
        let Some(screen) = self.screen else {
            return (false, None);
        };
        match (screen, key) {
            (NativeScreen::Pause, KeyCode::Escape) => (true, Some(NativeUiAction::Resume)),
            (NativeScreen::Options { parent }, KeyCode::Escape) => match parent {
                OptionsParent::Title => (true, Some(NativeUiAction::BackToTitle)),
                OptionsParent::Pause => (true, Some(NativeUiAction::BackToPause)),
            },
            (NativeScreen::Title, KeyCode::Escape) => (true, None),
            _ => (false, None),
        }
    }

    pub(crate) fn apply_action(&mut self, action: NativeUiAction) {
        match action {
            NativeUiAction::StartWorld | NativeUiAction::Resume => self.close(),
            NativeUiAction::OpenOptions(parent) => {
                self.screen = Some(NativeScreen::Options { parent });
                self.pressed = None;
            }
            NativeUiAction::BackToTitle => {
                self.screen = Some(NativeScreen::Title);
                self.pressed = None;
            }
            NativeUiAction::BackToPause => {
                self.screen = Some(NativeScreen::Pause);
                self.pressed = None;
            }
            NativeUiAction::SetRenderDistance(radius) => {
                self.render_distance = radius.clamp(MIN_UI_RENDER_DISTANCE, MAX_RENDER_DISTANCE);
            }
            NativeUiAction::ToggleSectionOcclusion
            | NativeUiAction::ToggleFullbright
            | NativeUiAction::CycleFramePacing
            | NativeUiAction::CycleFpsCap
            | NativeUiAction::Quit => {}
        }
    }

    pub(crate) fn render_draw_list(
        &self,
        render_options: TexturedSectionRenderOptions,
        frame_pacing: FramePacingUiState,
    ) -> GuiDrawList {
        let mut draw = GuiDrawList::new();
        match self.screen {
            Some(NativeScreen::Title) => self.render_title(&mut draw),
            Some(NativeScreen::Pause) => self.render_pause(&mut draw),
            Some(NativeScreen::Options { parent }) => {
                self.render_options_screen(&mut draw, render_options, frame_pacing, parent)
            }
            None => {}
        }
        draw
    }

    pub(crate) fn render_debug_pane(&self, draw: &mut GuiDrawList, stats: &DebugPaneStats) {
        let line_height = self.font.line_height();
        let lines = stats.lines();
        let panel_width = 236.0_f32.min(self.scale.width - 8.0).max(120.0);
        let panel_height = 8.0 + line_height * lines.len() as f32;
        let panel = Rect::new(
            4.0,
            4.0,
            panel_width,
            panel_height.min((self.scale.height - 8.0).max(0.0)),
        );
        draw.fill(panel, Color::rgba(6, 9, 10, 185));
        draw.outline(panel, Color::rgba(110, 140, 136, 230));
        draw.push_clip(panel.inset(4.0));
        let text = Color::rgba(220, 238, 220, 255);
        let muted = Color::rgba(165, 186, 176, 255);
        let mut y = panel.y + 5.0;
        for (index, line) in lines.iter().enumerate() {
            self.font.draw_shadow(
                draw,
                line,
                panel.x + 6.0,
                y,
                if index == 0 { text } else { muted },
            );
            y += line_height;
        }
        draw.pop_clip();
    }

    fn widget_at(&self, point: Point) -> Option<WidgetId> {
        match self.screen? {
            NativeScreen::Title => title_buttons(self.scale)
                .into_iter()
                .find(|button| button.contains(point))
                .map(|button| button.id),
            NativeScreen::Pause => pause_buttons(self.scale)
                .into_iter()
                .find(|button| button.contains(point))
                .map(|button| button.id),
            NativeScreen::Options { .. } => {
                let rects = option_widgets(self.scale);
                if rects.occlusion.contains(point) {
                    Some(ID_OPTIONS_OCCLUSION)
                } else if rects.fullbright.contains(point) {
                    Some(ID_OPTIONS_FULLBRIGHT)
                } else if rects.frame_pacing.contains(point) {
                    Some(ID_OPTIONS_FRAME_PACING)
                } else if rects.fps_cap.contains(point) {
                    Some(ID_OPTIONS_FPS_CAP)
                } else if rects.radius.contains(point) {
                    Some(ID_OPTIONS_RADIUS)
                } else if rects.back.contains(point) {
                    Some(ID_OPTIONS_BACK)
                } else {
                    None
                }
            }
        }
    }

    fn action_for(&self, id: WidgetId) -> Option<NativeUiAction> {
        match id {
            ID_TITLE_START => Some(NativeUiAction::StartWorld),
            ID_TITLE_OPTIONS => Some(NativeUiAction::OpenOptions(OptionsParent::Title)),
            ID_TITLE_QUIT => Some(NativeUiAction::Quit),
            ID_PAUSE_RESUME => Some(NativeUiAction::Resume),
            ID_PAUSE_OPTIONS => Some(NativeUiAction::OpenOptions(OptionsParent::Pause)),
            ID_PAUSE_TITLE => Some(NativeUiAction::BackToTitle),
            ID_OPTIONS_OCCLUSION => Some(NativeUiAction::ToggleSectionOcclusion),
            ID_OPTIONS_FULLBRIGHT => Some(NativeUiAction::ToggleFullbright),
            ID_OPTIONS_FRAME_PACING => Some(NativeUiAction::CycleFramePacing),
            ID_OPTIONS_FPS_CAP => Some(NativeUiAction::CycleFpsCap),
            ID_OPTIONS_BACK => match self.screen {
                Some(NativeScreen::Options {
                    parent: OptionsParent::Title,
                }) => Some(NativeUiAction::BackToTitle),
                Some(NativeScreen::Options {
                    parent: OptionsParent::Pause,
                }) => Some(NativeUiAction::BackToPause),
                _ => None,
            },
            _ => None,
        }
    }

    fn render_distance_action_at(&self, point: Point) -> NativeUiAction {
        let slider = Slider::new(
            ID_OPTIONS_RADIUS,
            option_widgets(self.scale).radius,
            "",
            render_distance_slider_value(self.render_distance),
        );
        NativeUiAction::SetRenderDistance(render_distance_from_slider_value(
            slider.value_from_point(point),
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
        render_options: TexturedSectionRenderOptions,
        frame_pacing: FramePacingUiState,
        parent: OptionsParent,
    ) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 150),
        );
        let panel = centered_panel(self.scale, 242.0, 190.0);
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
        let widgets = option_widgets(self.scale);
        Checkbox::new(
            ID_OPTIONS_OCCLUSION,
            widgets.occlusion,
            "Section Occlusion",
            render_options.section_occlusion_culling,
        )
        .render(draw, &self.font, self.interaction());
        Checkbox::new(
            ID_OPTIONS_FULLBRIGHT,
            widgets.fullbright,
            "Force Fullbright",
            render_options.force_fullbright,
        )
        .render(draw, &self.font, self.interaction());
        CycleButton::new(
            ID_OPTIONS_FRAME_PACING,
            widgets.frame_pacing,
            "Frame Pacing",
            frame_pacing.mode.label(),
        )
        .render(draw, &self.font, self.interaction());
        CycleButton::new(
            ID_OPTIONS_FPS_CAP,
            widgets.fps_cap,
            "FPS Cap",
            frame_pacing.fps_cap.to_string(),
        )
        .render(draw, &self.font, self.interaction());
        Slider::new(
            ID_OPTIONS_RADIUS,
            widgets.radius,
            render_distance_label(self.render_distance),
            render_distance_slider_value(self.render_distance),
        )
        .render(draw, &self.font, self.interaction());
        Button::new(
            ID_OPTIONS_BACK,
            widgets.back,
            match parent {
                OptionsParent::Title => "Back",
                OptionsParent::Pause => "Done",
            },
        )
        .render(draw, &self.font, self.interaction());
    }
}

#[derive(Clone, Copy, Debug)]
struct OptionWidgetRects {
    occlusion: Rect,
    fullbright: Rect,
    frame_pacing: Rect,
    fps_cap: Rect,
    radius: Rect,
    back: Rect,
}

fn title_buttons(scale: GuiScale) -> [Button; 3] {
    let y = scale.height * 0.5 - 22.0;
    [
        Button::new(
            ID_TITLE_START,
            menu_button_rect(scale, y),
            "Start Local World",
        ),
        Button::new(
            ID_TITLE_OPTIONS,
            menu_button_rect(scale, y + 24.0),
            "Options",
        ),
        Button::new(ID_TITLE_QUIT, menu_button_rect(scale, y + 48.0), "Quit"),
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

fn option_widgets(scale: GuiScale) -> OptionWidgetRects {
    let panel = centered_panel(scale, 242.0, 190.0);
    OptionWidgetRects {
        occlusion: Rect::new(panel.x + 26.0, panel.y + 38.0, 190.0, 18.0),
        fullbright: Rect::new(panel.x + 26.0, panel.y + 60.0, 190.0, 18.0),
        frame_pacing: Rect::new(panel.x + 25.0, panel.y + 84.0, 192.0, 20.0),
        fps_cap: Rect::new(panel.x + 25.0, panel.y + 108.0, 192.0, 20.0),
        radius: Rect::new(panel.x + 25.0, panel.y + 132.0, 192.0, 20.0),
        back: Rect::new(panel.center_x() - 55.0, panel.y + 160.0, 110.0, 20.0),
    }
}

fn render_distance_slider_value(radius: i32) -> f32 {
    if MAX_RENDER_DISTANCE <= MIN_UI_RENDER_DISTANCE {
        0.0
    } else {
        (radius.clamp(MIN_UI_RENDER_DISTANCE, MAX_RENDER_DISTANCE) - MIN_UI_RENDER_DISTANCE) as f32
            / (MAX_RENDER_DISTANCE - MIN_UI_RENDER_DISTANCE) as f32
    }
}

fn render_distance_from_slider_value(value: f32) -> i32 {
    MIN_UI_RENDER_DISTANCE
        + (value.clamp(0.0, 1.0) * (MAX_RENDER_DISTANCE - MIN_UI_RENDER_DISTANCE) as f32).round()
            as i32
}

fn render_distance_label(radius: i32) -> String {
    let radius = radius.clamp(MIN_UI_RENDER_DISTANCE, MAX_RENDER_DISTANCE);
    let suffix = if radius == 1 { "chunk" } else { "chunks" };
    format!("Render Distance: {radius} {suffix}")
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

pub(crate) fn render_static_title_ui(width: u32, height: u32) -> GuiDrawList {
    let scale = GuiScale::from_pixels(width, height);
    let mut ui = NativeUi::new(DEFAULT_RENDER_DISTANCE);
    ui.set_scale(scale);
    ui.render_draw_list(
        TexturedSectionRenderOptions::default(),
        FramePacingUiState::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_ui_has_title_and_ingame_start_modes() {
        let title_ui = NativeUi::new(1);
        assert!(title_ui.is_active());
        assert!(title_ui.covers_world());

        let ingame_ui = NativeUi::new_ingame(1);
        assert!(!ingame_ui.is_active());
        assert!(!ingame_ui.covers_world());
    }

    #[test]
    fn debug_pane_formats_and_draws_runtime_stats() {
        let stats = DebugPaneStats {
            position: Vec3::new(1.25, 64.0, -2.5),
            speed: 32.0,
            movement_mode: "WALK",
            on_ground: true,
            runtime: WindowRuntimeStats {
                interest_center: ChunkPos::new(3, -4),
                render_distance: 2,
                chunk_tracking_radius: 3,
                loaded_chunks: 9,
                pending_jobs: 1,
                pending_publications: 2,
                pending_render_chunks: 3,
                pending_render_compile_jobs: 1,
                inflight_render_sections: 4,
                client_visible_chunks: 8,
                active_ticket_chunks: 9,
                pending_unload_chunks: 0,
                block_ticking_chunks: 4,
                entity_ticking_chunks: 2,
                last_tick: 12,
                last_simulation_tick: 11,
                last_tick_unloads_processed: 0,
                last_simulation_block_tick_chunks: 4,
                last_simulation_entity_tick_chunks: 2,
                last_simulation_scheduler_tick_ms: 0.1,
                last_simulation_block_tick_ms: 0.2,
                last_simulation_fluid_tick_ms: 0.3,
                last_simulation_entity_tick_ms: 0.4,
                last_simulation_fluid_ticks_executed: 5,
                last_simulation_deferred_fluid_ticks: 6,
                last_simulation_fluid_mutated_blocks: 7,
                scheduled_fluid_ticks: 8,
            },
            render: RenderStreamStats {
                section_count: 16,
                drawn_section_count: 10,
                face_count: 200,
                drawn_face_count: 120,
                last_rebuilt_section_count: 2,
                last_uploaded_section_count: 2,
                last_frame_ms: 16.7,
                ..RenderStreamStats::default()
            },
            frame: FrameTimingStats {
                frame_count: 12,
                over_budget_count: 3,
                over_2x_budget_count: 1,
                over_4x_budget_count: 0,
                last_frame_ms: 16.7,
                worst_frame_ms: 33.4,
                budget_ms: Some(8.3),
                last_runtime_poll_ms: 5.0,
                last_remesh_ms: 2.0,
                last_upload_ms: 1.0,
                last_surface_acquire_ms: 0.2,
                last_surface_encode_ms: 1.4,
                last_surface_submit_ms: 0.1,
                last_surface_present_ms: 0.0,
                ..FrameTimingStats::default()
            },
            pacing: FramePacingDebugStats {
                mode: FramePacingMode::Vsync,
                fps_cap: 120,
                monitor_refresh_hz: Some(120.0),
                target_frame_ms: Some(8.3),
                active_present_mode_label: "fifo",
            },
            section_occlusion: true,
            force_fullbright: false,
        };

        let lines = stats.lines();
        assert_eq!(lines[0], "DEBUG");
        assert_eq!(lines[1], "POS 1.2 64.0 -2.5");
        assert_eq!(lines[2], "CHUNK 3 -4 SPEED 32.0");
        assert_eq!(lines[3], "MODE WALK GROUND Y");
        assert_eq!(lines[4], "VIEW R2 T3");
        assert_eq!(lines[5], "OCC ON  LIGHT");
        assert!(lines.iter().any(|line| line == "BUDGET 8.3MS FRAME 16.7MS"));
        assert!(lines.iter().any(|line| line == "OVER 3/1/0 WORST 33.4"));

        let mut ui = NativeUi::new(1);
        ui.set_scale(GuiScale::from_pixels(960, 540));
        let mut draw = GuiDrawList::new();
        ui.render_debug_pane(&mut draw, &stats);
        assert!(!draw.commands().is_empty());
    }

    #[test]
    fn options_radius_slider_sets_render_distance() {
        let mut ui = NativeUi::new(1);
        ui.set_screen(Some(NativeScreen::Options {
            parent: OptionsParent::Pause,
        }));
        ui.set_scale(GuiScale::from_pixels(960, 540));

        let radius = option_widgets(ui.scale).radius;
        let point = Point {
            x: radius.right() - 0.1,
            y: radius.y + radius.height * 0.5,
        };
        assert!(ui.pointer_down(point));
        let (_handled, action) = ui.pointer_up(point);

        let action = action.expect("radius slider release should produce an action");
        assert_eq!(
            action,
            NativeUiAction::SetRenderDistance(MAX_RENDER_DISTANCE)
        );
        ui.apply_action(action);
        assert_eq!(ui.render_distance, MAX_RENDER_DISTANCE);

        let point = Point {
            x: radius.x,
            y: radius.y + radius.height * 0.5,
        };
        assert!(ui.pointer_down(point));
        let (_handled, action) = ui.pointer_up(point);

        let action = action.expect("radius slider release should produce an action");
        assert_eq!(
            action,
            NativeUiAction::SetRenderDistance(MIN_UI_RENDER_DISTANCE)
        );
        ui.apply_action(action);
        assert_eq!(ui.render_distance, MIN_UI_RENDER_DISTANCE);
    }
}
