use std::sync::Arc;

use anyhow::Result;
use mclone_diagnostics::FramePipelineReport;
use mclone_render::chunk::ChunkRenderView;
use mclone_render::gui::{WorldGuiPanel, WorldGuiPanelRenderStats, WorldGuiRenderer};
use mclone_render::target::RenderFrameTarget;
use mclone_render::uniform::PerViewSlot;
use mclone_ui::{
    DebugOverlay, FramePipelineHudOverlay, GuiDrawList, GuiScale, Point, UiDrawCacheStats,
    UiPanelRevision, render_debug_overlay_at, render_frame_pipeline_overlay,
};

use crate::XR_DIAGNOSTIC_PANEL_PIXELS;

pub(crate) struct XrDiagnosticPanel {
    renderer: WorldGuiRenderer,
    frame_metrics_visible: bool,
    debug_diagnostics_visible: bool,
    frame_metrics: Option<FramePipelineHudOverlay>,
    frame_metrics_cache: XrFrameMetricsPanelCache,
    debug_overlay: Option<DebugOverlay>,
    debug_overlay_cache: XrDebugOverlayPanelCache,
}

impl XrDiagnosticPanel {
    pub(crate) fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        Self {
            renderer: WorldGuiRenderer::new(device, color_format),
            frame_metrics_visible: false,
            debug_diagnostics_visible: false,
            frame_metrics: None,
            frame_metrics_cache: XrFrameMetricsPanelCache::default(),
            debug_overlay: None,
            debug_overlay_cache: XrDebugOverlayPanelCache::default(),
        }
    }

    pub(crate) const fn frame_metrics_visible(&self) -> bool {
        self.frame_metrics_visible
    }

    pub(crate) fn set_frame_metrics_visible(&mut self, visible: bool) {
        self.frame_metrics_visible = visible;
    }

    pub(crate) const fn debug_diagnostics_visible(&self) -> bool {
        self.debug_diagnostics_visible
    }

    pub(crate) fn set_debug_diagnostics_visible(&mut self, visible: bool) {
        self.debug_diagnostics_visible = visible;
    }

    pub(crate) fn set_frame_pipeline_report(
        &mut self,
        report: Arc<FramePipelineReport>,
        revision: u64,
    ) {
        self.frame_metrics = Some(FramePipelineHudOverlay::new(report, revision));
    }

    pub(crate) fn clear_frame_pipeline_report(&mut self) {
        self.frame_metrics = None;
        self.frame_metrics_cache.clear();
    }

    pub(crate) fn set_debug_overlay(&mut self, overlay: DebugOverlay) {
        self.debug_overlay = Some(overlay);
    }

    pub(crate) fn clear_debug_overlay(&mut self) {
        self.debug_overlay = None;
        self.debug_overlay_cache.clear();
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_in_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        panel: WorldGuiPanel,
        view_slot: PerViewSlot,
    ) -> Result<(WorldGuiPanelRenderStats, UiDrawCacheStats)> {
        let Some(draw) = self.prepare_diagnostic_draw() else {
            return Ok(Default::default());
        };
        let panel_stats = self.renderer.render_panel_cached_in_slot(
            device,
            queue,
            encoder,
            target,
            render_view,
            XR_DIAGNOSTIC_PANEL_PIXELS,
            [draw.gui_scale.width, draw.gui_scale.height],
            &draw.draw,
            panel,
            &[],
            view_slot,
            draw.cache_revision,
        )?;
        Ok((panel_stats, draw.draw_cache))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_views: [ChunkRenderView; 2],
        panel: WorldGuiPanel,
    ) -> Result<(WorldGuiPanelRenderStats, UiDrawCacheStats)> {
        let Some(draw) = self.prepare_diagnostic_draw() else {
            return Ok(Default::default());
        };
        let panel_stats = self.renderer.render_panel_cached_multiview(
            device,
            queue,
            encoder,
            target,
            render_views,
            XR_DIAGNOSTIC_PANEL_PIXELS,
            [draw.gui_scale.width, draw.gui_scale.height],
            &draw.draw,
            panel,
            &[],
            draw.cache_revision,
        )?;
        Ok((panel_stats, draw.draw_cache))
    }

    fn prepare_diagnostic_draw(&mut self) -> Option<XrDiagnosticPanelDraw> {
        let gui_scale =
            GuiScale::from_pixels(XR_DIAGNOSTIC_PANEL_PIXELS[0], XR_DIAGNOSTIC_PANEL_PIXELS[1]);
        if self.debug_diagnostics_visible {
            let overlay = self.debug_overlay.as_ref()?;
            if !overlay.is_empty() {
                return self.debug_overlay_cache.render(gui_scale, overlay);
            }
        }
        if !self.frame_metrics_visible {
            return None;
        }
        let overlay = self.frame_metrics.as_ref()?;
        if !overlay.visible() {
            return None;
        }
        self.frame_metrics_cache.render(gui_scale, overlay)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct XrFrameMetricsPanelState {
    gui_scale: GuiScale,
    overlay: FramePipelineHudOverlay,
}

#[derive(Clone, Debug, PartialEq)]
struct XrDebugOverlayPanelState {
    gui_scale: GuiScale,
    overlay: DebugOverlay,
}

#[derive(Clone, Debug, PartialEq)]
struct XrDiagnosticPanelDraw {
    draw: GuiDrawList,
    gui_scale: GuiScale,
    cache_revision: UiPanelRevision,
    draw_cache: UiDrawCacheStats,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct XrFrameMetricsPanelCache {
    state: Option<XrFrameMetricsPanelState>,
    draw: GuiDrawList,
    revision_content: u64,
}

impl XrFrameMetricsPanelCache {
    fn clear(&mut self) {
        self.state = None;
        self.draw.clear();
        self.revision_content = 0;
    }

    fn render(
        &mut self,
        gui_scale: GuiScale,
        overlay: &FramePipelineHudOverlay,
    ) -> Option<XrDiagnosticPanelDraw> {
        let state = XrFrameMetricsPanelState {
            gui_scale,
            overlay: overlay.clone(),
        };
        if self.state.as_ref() == Some(&state) {
            return Some(XrDiagnosticPanelDraw {
                draw: self.draw.clone(),
                gui_scale,
                cache_revision: UiPanelRevision::new(self.revision_content, 0),
                draw_cache: UiDrawCacheStats::cache_hit(),
            });
        }

        let mut draw = GuiDrawList::new();
        render_frame_pipeline_overlay(gui_scale, &mut draw, overlay);
        if draw.commands().is_empty() {
            self.clear();
            return None;
        }
        self.revision_content = self.revision_content.wrapping_add(1).max(1);
        self.state = Some(state);
        self.draw = draw.clone();
        Some(XrDiagnosticPanelDraw {
            draw,
            gui_scale,
            cache_revision: UiPanelRevision::new(self.revision_content, 0),
            draw_cache: UiDrawCacheStats::rebuild(),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct XrDebugOverlayPanelCache {
    state: Option<XrDebugOverlayPanelState>,
    draw: GuiDrawList,
    revision_content: u64,
}

impl XrDebugOverlayPanelCache {
    fn clear(&mut self) {
        self.state = None;
        self.draw.clear();
        self.revision_content = 0;
    }

    fn render(
        &mut self,
        gui_scale: GuiScale,
        overlay: &DebugOverlay,
    ) -> Option<XrDiagnosticPanelDraw> {
        let state = XrDebugOverlayPanelState {
            gui_scale,
            overlay: overlay.clone(),
        };
        if self.state.as_ref() == Some(&state) {
            return Some(XrDiagnosticPanelDraw {
                draw: self.draw.clone(),
                gui_scale,
                cache_revision: UiPanelRevision::new(self.revision_content, 0),
                draw_cache: UiDrawCacheStats::cache_hit(),
            });
        }

        let mut draw = GuiDrawList::new();
        render_debug_overlay_at(gui_scale, &mut draw, overlay, Point { x: 4.0, y: 4.0 });
        if draw.commands().is_empty() {
            self.clear();
            return None;
        }
        self.revision_content = self.revision_content.wrapping_add(1).max(1);
        self.state = Some(state);
        self.draw = draw.clone();
        Some(XrDiagnosticPanelDraw {
            draw,
            gui_scale,
            cache_revision: UiPanelRevision::new(self.revision_content, 0),
            draw_cache: UiDrawCacheStats::rebuild(),
        })
    }
}
