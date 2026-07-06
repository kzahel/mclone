use std::sync::Arc;

use mclone_diagnostics::{
    CriticalPathLabel, FramePipelineReport, QueueAgeReport, QueueId, StageId,
};

use crate::{Color, Font, GuiDrawList, GuiScale, Rect};

const PANEL_MARGIN: f32 = 4.0;
const PANEL_WIDTH: f32 = 314.0;
const PANEL_MIN_WIDTH: f32 = 236.0;
const ROW_HEIGHT: f32 = 10.0;
const SECTION_GAP: f32 = 5.0;
const MAX_STAGE_SEGMENTS: usize = 12;
const MAX_STAGE_LABELS: usize = 3;
const MAX_QUEUE_ROWS: usize = 6;

const PANEL_FILL: Color = Color::rgba(5, 8, 10, 214);
const PANEL_BORDER: Color = Color::rgba(125, 151, 142, 235);
const PANEL_INNER_BORDER: Color = Color::rgba(16, 24, 25, 230);
const TEXT_TITLE: Color = Color::rgba(229, 242, 230, 255);
const TEXT_MAIN: Color = Color::rgba(202, 220, 211, 255);
const TEXT_MUTED: Color = Color::rgba(151, 172, 164, 255);
const BAR_BG: Color = Color::rgba(22, 28, 30, 232);
const BAR_TICK: Color = Color::rgba(228, 235, 215, 220);
const APP_OK: Color = Color::rgba(89, 184, 132, 235);
const APP_WARN: Color = Color::rgba(224, 185, 76, 238);
const APP_OVER: Color = Color::rgba(216, 89, 86, 240);
const WAIT: Color = Color::rgba(87, 145, 202, 226);
const OTHER: Color = Color::rgba(111, 118, 123, 220);

#[derive(Clone, Debug)]
pub struct FramePipelineHudOverlay {
    report: Arc<FramePipelineReport>,
    revision: u64,
}

impl FramePipelineHudOverlay {
    pub fn new(report: Arc<FramePipelineReport>, revision: u64) -> Self {
        Self { report, revision }
    }

    pub fn report(&self) -> &FramePipelineReport {
        &self.report
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn visible(&self) -> bool {
        self.report.frame_summary.frames > 0
    }
}

impl PartialEq for FramePipelineHudOverlay {
    fn eq(&self, other: &Self) -> bool {
        self.revision == other.revision && Arc::ptr_eq(&self.report, &other.report)
    }
}

pub fn render_frame_pipeline_overlay(
    scale: GuiScale,
    draw: &mut GuiDrawList,
    overlay: &FramePipelineHudOverlay,
) {
    if !overlay.visible() {
        return;
    }

    let report = overlay.report();
    let font = Font::default();
    let queue_rows = report.queue_panel.queues.len().min(MAX_QUEUE_ROWS);
    let height = 96.0 + queue_rows as f32 * ROW_HEIGHT;
    let width = PANEL_WIDTH.min((scale.width - PANEL_MARGIN * 2.0).max(PANEL_MIN_WIDTH));
    let panel = Rect::new(
        (scale.width - width - PANEL_MARGIN)
            .max(PANEL_MARGIN)
            .floor(),
        PANEL_MARGIN,
        width,
        height.min((scale.height - PANEL_MARGIN * 2.0).max(1.0)),
    );
    draw.fill(panel, PANEL_FILL);
    draw.outline(panel, PANEL_BORDER);
    draw.outline(panel.inset(1.0), PANEL_INNER_BORDER);
    draw.push_clip(panel.inset(4.0));

    let mut y = panel.y + 5.0;
    font.draw_shadow_atlas(draw, "FRAME PIPELINE", panel.x + 6.0, y, TEXT_TITLE);
    font.draw_shadow_atlas(
        draw,
        &format!(
            "F{} REV{}",
            report.frame_summary.frames,
            overlay.revision().min(999_999)
        ),
        panel.right() - 82.0,
        y,
        TEXT_MUTED,
    );
    y += 12.0;

    y = render_frame_budget_bar(&font, draw, report, panel.inset(6.0), y);
    y += SECTION_GAP;
    y = render_stage_waterfall(&font, draw, report, panel.inset(6.0), y);
    y += SECTION_GAP;
    render_queue_panel(&font, draw, report, panel.inset(6.0), y);

    draw.pop_clip();
}

fn render_frame_budget_bar(
    font: &Font,
    draw: &mut GuiDrawList,
    report: &FramePipelineReport,
    content: Rect,
    y: f32,
) -> f32 {
    let summary = &report.frame_summary;
    let target_ms = summary.target_period_ms.unwrap_or(0.0).max(0.0);
    let app_ms = summary.latest_app_work_ms.max(0.0);
    let wait_ms = summary.latest_wait_ms.max(0.0);
    let frame_ms = summary.latest_frame_wall_ms.max(app_ms + wait_ms).max(0.0);
    let budget_ms = target_ms.max(frame_ms).max(1.0);
    let over = target_ms > 0.0 && app_ms > target_ms;
    let app_color = if over {
        APP_OVER
    } else if target_ms > 0.0 && app_ms > target_ms * 0.85 {
        APP_WARN
    } else {
        APP_OK
    };

    let bar = Rect::new(content.x, y + 12.0, content.width, 7.0);
    draw.fill(bar, BAR_BG);
    draw.fill(
        Rect::new(
            bar.x,
            bar.y,
            (bar.width * ratio(app_ms, budget_ms)).max(1.0),
            bar.height,
        ),
        app_color,
    );
    let wait_x = bar.x + bar.width * ratio(app_ms, budget_ms);
    let wait_width = (bar.width * ratio(wait_ms, budget_ms)).min(bar.right() - wait_x);
    draw.fill(Rect::new(wait_x, bar.y, wait_width, bar.height), WAIT);
    if target_ms > 0.0 {
        let target_x = bar.x + bar.width * ratio(target_ms, budget_ms);
        draw.fill(
            Rect::new(target_x.floor(), bar.y - 1.0, 1.0, bar.height + 2.0),
            BAR_TICK,
        );
    }

    let target_label = if target_ms > 0.0 {
        format!("{target_ms:.1}")
    } else {
        "UNCAP".to_owned()
    };
    let headroom = summary
        .latest_headroom_ms
        .map(|ms| format!("{ms:+.1}"))
        .unwrap_or_else(|| "N/A".to_owned());
    font.draw_shadow_atlas(
        draw,
        &format!(
            "BUDGET APP {:.1} WAIT {:.1} HEAD {} TARGET {}",
            app_ms, wait_ms, headroom, target_label
        ),
        content.x,
        y,
        if over { APP_OVER } else { TEXT_MAIN },
    );
    y + 22.0
}

fn render_stage_waterfall(
    font: &Font,
    draw: &mut GuiDrawList,
    report: &FramePipelineReport,
    content: Rect,
    y: f32,
) -> f32 {
    let spans = report.stage_spans.as_slice();
    font.draw_shadow_atlas(draw, "CURRENT FRAME WATERFALL", content.x, y, TEXT_MAIN);
    let bar = Rect::new(content.x, y + 12.0, content.width, 7.0);
    draw.fill(bar, BAR_BG);
    let denominator_ms = report
        .frame_summary
        .latest_frame_wall_ms
        .max(report.frame_summary.latest_app_work_ms)
        .max(1.0);
    let mut x = bar.x;
    for span in spans.iter().take(MAX_STAGE_SEGMENTS) {
        let width = (bar.width * ratio(span.elapsed_ms, denominator_ms)).max(1.0);
        draw.fill(
            Rect::new(x, bar.y, width.min(bar.right() - x), bar.height),
            critical_path_color(span.label),
        );
        x += width;
        if x >= bar.right() {
            break;
        }
    }
    if spans.len() > MAX_STAGE_SEGMENTS && x < bar.right() {
        draw.fill(Rect::new(x, bar.y, bar.right() - x, bar.height), OTHER);
    }

    let mut label_y = y + 22.0;
    for span in spans.iter().take(MAX_STAGE_LABELS) {
        font.draw_shadow_atlas(
            draw,
            &format!("{} {:.1}", stage_label(span.stage), span.elapsed_ms),
            content.x,
            label_y,
            TEXT_MUTED,
        );
        label_y += ROW_HEIGHT;
    }
    if spans.is_empty() {
        font.draw_shadow_atlas(draw, "NO STAGES", content.x, label_y, TEXT_MUTED);
        label_y += ROW_HEIGHT;
    } else if spans.len() > MAX_STAGE_LABELS {
        font.draw_shadow_atlas(
            draw,
            &format!("+{} STAGES", spans.len() - MAX_STAGE_LABELS),
            content.x,
            label_y,
            TEXT_MUTED,
        );
        label_y += ROW_HEIGHT;
    }
    label_y
}

fn render_queue_panel(
    font: &Font,
    draw: &mut GuiDrawList,
    report: &FramePipelineReport,
    content: Rect,
    y: f32,
) {
    font.draw_shadow_atlas(draw, "QUEUES", content.x, y, TEXT_MAIN);
    let rows = report.queue_panel.queues.as_slice();
    if rows.is_empty() {
        font.draw_shadow_atlas(draw, "NONE", content.x + 48.0, y, TEXT_MUTED);
        return;
    }
    let max_depth = rows
        .iter()
        .map(|queue| queue.depth)
        .max()
        .unwrap_or(1)
        .max(1);
    let mut row_y = y + 11.0;
    for queue in rows.iter().take(MAX_QUEUE_ROWS) {
        render_queue_row(font, draw, queue, max_depth, content, row_y);
        row_y += ROW_HEIGHT;
    }
    if rows.len() > MAX_QUEUE_ROWS {
        font.draw_shadow_atlas(
            draw,
            &format!("+{} QUEUES", rows.len() - MAX_QUEUE_ROWS),
            content.x,
            row_y,
            TEXT_MUTED,
        );
    }
}

fn render_queue_row(
    font: &Font,
    draw: &mut GuiDrawList,
    queue: &QueueAgeReport,
    max_depth: u64,
    content: Rect,
    y: f32,
) {
    let label_width = 88.0;
    let bar = Rect::new(content.x + label_width, y + 1.0, 54.0, 5.0);
    draw.fill(bar, BAR_BG);
    if queue.depth > 0 {
        draw.fill(
            Rect::new(
                bar.x,
                bar.y,
                (bar.width * ratio(queue.depth as f64, max_depth as f64)).max(1.0),
                bar.height,
            ),
            if queue.conservation_violations > 0 {
                APP_OVER
            } else {
                WAIT
            },
        );
    }
    font.draw_shadow_atlas(draw, queue_label(&queue.queue), content.x, y, TEXT_MUTED);
    font.draw_shadow_atlas(
        draw,
        &format!(
            "D{} AGE {}",
            queue.depth,
            queue
                .oldest_age_ms
                .map(|age| format!("{age:.0}"))
                .unwrap_or_else(|| "-".to_owned())
        ),
        bar.right() + 6.0,
        y,
        TEXT_MUTED,
    );
}

fn ratio(value: f64, denominator: f64) -> f32 {
    if denominator <= 0.0 || !denominator.is_finite() || !value.is_finite() {
        return 0.0;
    }
    (value.max(0.0) / denominator).clamp(0.0, 1.0) as f32
}

fn critical_path_color(label: CriticalPathLabel) -> Color {
    match label {
        CriticalPathLabel::CurrentFrameCritical => APP_OK,
        CriticalPathLabel::NextFrameSlack => APP_WARN,
        CriticalPathLabel::ParallelCpuPeer => Color::rgba(160, 122, 214, 232),
        CriticalPathLabel::RemoteHostWork => Color::rgba(92, 174, 194, 230),
        CriticalPathLabel::QueuedBackpressured => APP_OVER,
    }
}

fn stage_label(stage: StageId) -> &'static str {
    match stage {
        StageId::InputPoseEvents => "INPUT",
        StageId::HostSessionCommands => "HOST",
        StageId::TerrainGeneration => "TERRAIN",
        StageId::LightComputeStatus => "LIGHT",
        StageId::SchedulerPublication => "PUBLISH",
        StageId::TransportDecode => "DECODE",
        StageId::ClientUpdateApply => "APPLY",
        StageId::RenderSectionAdmission => "ADMIT",
        StageId::RenderAdmissionDirtyReadyScan => "DIRTY",
        StageId::RenderAdmissionRequestBuild => "REQ",
        StageId::RenderAdmissionWorkerSubmit => "SUBMIT",
        StageId::RenderAdmissionPreparedRecordMaintenance => "RECORDS",
        StageId::CpuMeshCompile => "MESH",
        StageId::CompletedResultAcceptance => "ACCEPT",
        StageId::GpuUpload => "GPU UP",
        StageId::UploadApply => "UPLOAD",
        StageId::PreparedDrawRecords => "DRAW REC",
        StageId::DrawEncode => "ENCODE",
        StageId::GpuExecutionPresentationWait => "GPU/PRESENT",
        StageId::UiDebug => "UI",
    }
}

fn queue_label(queue: &QueueId) -> &'static str {
    match queue {
        QueueId::InboundUpdates => "INBOUND",
        QueueId::CompletedRenderResults => "COMPLETE",
        QueueId::UploadWork => "UPLOAD",
        QueueId::HostPublication => "PUBLISH",
        QueueId::RenderCompileJobs => "COMPILE",
        QueueId::Custom(_) => "CUSTOM",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_diagnostics::{
        FrameAccountingConfig, FrameAccumulator, FrameObservation, QueueAgeReport,
        QueuePanelReport, StageSpan,
    };

    #[test]
    fn overlay_renders_bounded_command_count() {
        let mut frames = FrameAccumulator::new(FrameAccountingConfig::from_target_period_ms(16.0));
        let mut observation = FrameObservation::new(1, 16.0)
            .with_wait_ms(4.0)
            .with_app_work_ms(12.0);
        for _ in 0..32 {
            observation = observation.with_stage_span(StageSpan::new(StageId::DrawEncode, 0.25));
        }
        frames.record_frame(observation);
        let queues = (0..18)
            .map(|index| {
                QueueAgeReport::snapshot(
                    QueueId::Custom(format!("q{index}")),
                    index + 4,
                    index,
                    4,
                    Some(8.0),
                    8.0,
                    0,
                )
            })
            .collect();
        let report = Arc::new(FramePipelineReport::new(
            frames.summary_report(),
            QueuePanelReport::new(queues),
        ));
        let overlay = FramePipelineHudOverlay::new(report, 7);
        let mut draw = GuiDrawList::new();

        render_frame_pipeline_overlay(GuiScale::from_pixels(960, 540), &mut draw, &overlay);

        assert!(!draw.commands().is_empty());
        assert!(
            draw.commands().len() <= 64,
            "unexpected overlay command count: {}",
            draw.commands().len()
        );
    }

    #[test]
    fn overlay_equality_uses_revision_and_report_identity() {
        let mut frames = FrameAccumulator::new(FrameAccountingConfig::from_target_period_ms(16.0));
        frames.record_frame(FrameObservation::new(1, 10.0));
        let report = Arc::new(FramePipelineReport::new(
            frames.summary_report(),
            QueuePanelReport::new(Vec::new()),
        ));

        let first = FramePipelineHudOverlay::new(report.clone(), 1);
        let same = FramePipelineHudOverlay::new(report.clone(), 1);
        let changed_revision = FramePipelineHudOverlay::new(report.clone(), 2);
        let changed_identity = FramePipelineHudOverlay::new(Arc::new((*report).clone()), 1);

        assert_eq!(first, same);
        assert_ne!(first, changed_revision);
        assert_ne!(first, changed_identity);
    }
}
