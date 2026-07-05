use serde::{Deserialize, Serialize};

use crate::stage::StageSpan;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PercentileMethod {
    /// Android XR's current nearest-rank helper: ceil(n * p) - 1.
    NearestRank,
    /// Native benchmark helper used before this crate: ceil((n - 1) * p).
    InclusiveCeil,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameAccountingConfig {
    pub target_hz: Option<f64>,
    pub target_period_ms: Option<f64>,
    pub percentile_method: PercentileMethod,
    pub worst_frame_capacity: usize,
}

impl FrameAccountingConfig {
    pub const DEFAULT_WORST_FRAME_CAPACITY: usize = 8;

    pub fn from_target_hz(target_hz: f64) -> Self {
        let target_hz = finite_positive(target_hz);
        Self {
            target_hz,
            target_period_ms: target_hz.map(|hz| 1000.0 / hz),
            percentile_method: PercentileMethod::NearestRank,
            worst_frame_capacity: Self::DEFAULT_WORST_FRAME_CAPACITY,
        }
    }

    pub fn from_target_period_ms(target_period_ms: f64) -> Self {
        let target_period_ms = finite_positive(target_period_ms);
        Self {
            target_hz: target_period_ms.map(|period| 1000.0 / period),
            target_period_ms,
            percentile_method: PercentileMethod::NearestRank,
            worst_frame_capacity: Self::DEFAULT_WORST_FRAME_CAPACITY,
        }
    }

    pub fn without_budget() -> Self {
        Self {
            target_hz: None,
            target_period_ms: None,
            percentile_method: PercentileMethod::NearestRank,
            worst_frame_capacity: Self::DEFAULT_WORST_FRAME_CAPACITY,
        }
    }

    pub fn with_percentile_method(mut self, method: PercentileMethod) -> Self {
        self.percentile_method = method;
        self
    }

    pub fn with_worst_frame_capacity(mut self, capacity: usize) -> Self {
        self.worst_frame_capacity = capacity;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameObservation {
    pub frame_index: u64,
    pub rendered: bool,
    pub frame_wall_ms: f64,
    pub wait_ms: f64,
    pub app_work_ms: Option<f64>,
    pub thread_cpu_ms: Option<f64>,
    pub dropped: bool,
    pub stale: bool,
    pub stage_spans: Vec<StageSpan>,
}

impl FrameObservation {
    pub fn new(frame_index: u64, frame_wall_ms: f64) -> Self {
        Self {
            frame_index,
            rendered: true,
            frame_wall_ms: sanitize_ms(frame_wall_ms),
            wait_ms: 0.0,
            app_work_ms: None,
            thread_cpu_ms: None,
            dropped: false,
            stale: false,
            stage_spans: Vec::new(),
        }
    }

    pub fn with_wait_ms(mut self, wait_ms: f64) -> Self {
        self.wait_ms = sanitize_ms(wait_ms);
        self
    }

    pub fn with_app_work_ms(mut self, app_work_ms: f64) -> Self {
        self.app_work_ms = Some(sanitize_ms(app_work_ms));
        self
    }

    pub fn with_thread_cpu_ms(mut self, thread_cpu_ms: Option<f64>) -> Self {
        self.thread_cpu_ms = thread_cpu_ms.map(sanitize_ms);
        self
    }

    pub fn with_rendered(mut self, rendered: bool) -> Self {
        self.rendered = rendered;
        self
    }

    pub fn with_dropped(mut self, dropped: bool) -> Self {
        self.dropped = dropped;
        self
    }

    pub fn with_stale(mut self, stale: bool) -> Self {
        self.stale = stale;
        self
    }

    pub fn with_stage_span(mut self, span: StageSpan) -> Self {
        self.stage_spans.push(span);
        self
    }

    pub fn computed_app_work_ms(&self) -> f64 {
        self.app_work_ms
            .map(sanitize_ms)
            .unwrap_or_else(|| (self.frame_wall_ms - self.wait_ms).max(0.0))
    }

    pub fn blocked_ms(&self) -> Option<f64> {
        self.thread_cpu_ms
            .map(|thread_cpu_ms| (self.computed_app_work_ms() - thread_cpu_ms).max(0.0))
    }

    pub fn headroom_ms(&self, target_period_ms: Option<f64>) -> Option<f64> {
        target_period_ms.map(|target| target - self.computed_app_work_ms())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverBudgetTiers {
    pub over_budget_frames: u64,
    pub over_2x_budget_frames: u64,
    pub over_4x_budget_frames: u64,
}

impl OverBudgetTiers {
    pub fn observe(&mut self, frame_wall_ms: f64, target_period_ms: Option<f64>) {
        let Some(target_period_ms) = target_period_ms.filter(|target| *target > 0.0) else {
            return;
        };
        let frame_wall_ms = sanitize_ms(frame_wall_ms);
        if frame_wall_ms > target_period_ms {
            self.over_budget_frames = self.over_budget_frames.saturating_add(1);
        }
        if frame_wall_ms > target_period_ms * 2.0 {
            self.over_2x_budget_frames = self.over_2x_budget_frames.saturating_add(1);
        }
        if frame_wall_ms > target_period_ms * 4.0 {
            self.over_4x_budget_frames = self.over_4x_budget_frames.saturating_add(1);
        }
    }

    pub const fn single_period_frames(self) -> u64 {
        self.over_budget_frames
    }

    pub const fn double_period_frames(self) -> u64 {
        self.over_2x_budget_frames
    }

    pub const fn quad_period_frames(self) -> u64 {
        self.over_4x_budget_frames
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PercentileSummary {
    pub count: u64,
    pub average_ms: f64,
    pub min_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

impl PercentileSummary {
    pub fn from_samples(samples: &[f64], method: PercentileMethod) -> Self {
        if samples.is_empty() {
            return Self::default();
        }
        let mut sorted = samples.iter().copied().map(sanitize_ms).collect::<Vec<_>>();
        sorted.sort_by(f64::total_cmp);
        let sum = sorted.iter().sum::<f64>();
        Self {
            count: sorted.len() as u64,
            average_ms: sum / sorted.len() as f64,
            min_ms: sorted.first().copied().unwrap_or(0.0),
            p50_ms: percentile_sorted_ms(&sorted, 0.50, method),
            p95_ms: percentile_sorted_ms(&sorted, 0.95, method),
            p99_ms: percentile_sorted_ms(&sorted, 0.99, method),
            max_ms: sorted.last().copied().unwrap_or(0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PercentileRing {
    capacity: usize,
    values: Vec<f64>,
    next_index: usize,
}

impl PercentileRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            values: Vec::with_capacity(capacity),
            next_index: 0,
        }
    }

    pub fn push(&mut self, value_ms: f64) {
        if self.capacity == 0 {
            return;
        }
        let value_ms = sanitize_ms(value_ms);
        if self.values.len() < self.capacity {
            self.values.push(value_ms);
        } else {
            self.values[self.next_index] = value_ms;
            self.next_index = (self.next_index + 1) % self.capacity;
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn summary(&self, method: PercentileMethod) -> PercentileSummary {
        PercentileSummary::from_samples(&self.values, method)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadroomSummary {
    pub count: u64,
    pub average_ms: f64,
    pub p50_ms: f64,
    pub p05_ms: f64,
    pub p01_ms: f64,
    pub min_ms: f64,
}

impl HeadroomSummary {
    fn from_samples(samples: &[f64], method: PercentileMethod) -> Self {
        if samples.is_empty() {
            return Self::default();
        }
        let mut sorted = samples
            .iter()
            .copied()
            .filter(|ms| ms.is_finite())
            .collect::<Vec<_>>();
        if sorted.is_empty() {
            return Self::default();
        }
        sorted.sort_by(f64::total_cmp);
        let sum = sorted.iter().sum::<f64>();
        Self {
            count: sorted.len() as u64,
            average_ms: sum / sorted.len() as f64,
            p50_ms: percentile_sorted_ms(&sorted, 0.50, method),
            p05_ms: percentile_sorted_ms(&sorted, 0.05, method),
            p01_ms: percentile_sorted_ms(&sorted, 0.01, method),
            min_ms: sorted.first().copied().unwrap_or(0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorstFrameDetail {
    pub rank: u64,
    pub frame_index: u64,
    pub rendered: bool,
    pub frame_wall_ms: f64,
    pub wait_ms: f64,
    pub app_work_ms: f64,
    pub thread_cpu_ms: Option<f64>,
    pub blocked_ms: Option<f64>,
    pub headroom_ms: Option<f64>,
    pub over_budget: bool,
    pub over_2x_budget: bool,
    pub over_4x_budget: bool,
    pub stage_spans: Vec<StageSpan>,
}

impl WorstFrameDetail {
    pub const fn over_single_budget(&self) -> bool {
        self.over_budget
    }

    pub const fn over_double_budget(&self) -> bool {
        self.over_2x_budget
    }

    pub const fn over_quad_budget(&self) -> bool {
        self.over_4x_budget
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameSummaryReport {
    pub schema_version: u32,
    pub target_hz: Option<f64>,
    pub target_period_ms: Option<f64>,
    pub frames: u64,
    pub rendered_frames: u64,
    pub dropped_frames: u64,
    pub stale_frames: u64,
    pub frame_wall: PercentileSummary,
    pub wait: PercentileSummary,
    pub app_work: PercentileSummary,
    pub thread_cpu: PercentileSummary,
    pub blocked: PercentileSummary,
    pub headroom: HeadroomSummary,
    pub over_budget: OverBudgetTiers,
    pub app_over_period_frames: u64,
    pub app_over_period_pct: f64,
    pub worst_frames: Vec<WorstFrameDetail>,
    pub latest_stage_spans: Vec<StageSpan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameAccumulator {
    config: FrameAccountingConfig,
    observations: Vec<FrameObservation>,
}

impl FrameAccumulator {
    pub fn new(config: FrameAccountingConfig) -> Self {
        Self {
            config,
            observations: Vec::new(),
        }
    }

    pub fn record_frame(&mut self, observation: FrameObservation) {
        self.observations.push(observation);
    }

    pub fn len(&self) -> usize {
        self.observations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.observations.is_empty()
    }

    pub fn summary_report(&self) -> FrameSummaryReport {
        let method = self.config.percentile_method;
        let frame_wall = self
            .observations
            .iter()
            .map(|frame| frame.frame_wall_ms)
            .collect::<Vec<_>>();
        let wait = self
            .observations
            .iter()
            .map(|frame| frame.wait_ms)
            .collect::<Vec<_>>();
        let app_work = self
            .observations
            .iter()
            .map(FrameObservation::computed_app_work_ms)
            .collect::<Vec<_>>();
        let thread_cpu = self
            .observations
            .iter()
            .filter_map(|frame| frame.thread_cpu_ms)
            .collect::<Vec<_>>();
        let blocked = self
            .observations
            .iter()
            .filter_map(FrameObservation::blocked_ms)
            .collect::<Vec<_>>();
        let headroom = self
            .observations
            .iter()
            .filter_map(|frame| frame.headroom_ms(self.config.target_period_ms))
            .collect::<Vec<_>>();
        let mut over_budget = OverBudgetTiers::default();
        let mut app_over_period_frames = 0_u64;
        for frame in &self.observations {
            over_budget.observe(frame.frame_wall_ms, self.config.target_period_ms);
            if let Some(target) = self.config.target_period_ms {
                if frame.computed_app_work_ms() > target {
                    app_over_period_frames = app_over_period_frames.saturating_add(1);
                }
            }
        }
        let frames = self.observations.len() as u64;
        let latest_stage_spans = self
            .observations
            .last()
            .map(|frame| frame.stage_spans.clone())
            .unwrap_or_default();
        FrameSummaryReport {
            schema_version: crate::FRAME_PIPELINE_SCHEMA_VERSION,
            target_hz: self.config.target_hz,
            target_period_ms: self.config.target_period_ms,
            frames,
            rendered_frames: self
                .observations
                .iter()
                .filter(|frame| frame.rendered)
                .count() as u64,
            dropped_frames: self
                .observations
                .iter()
                .filter(|frame| frame.dropped)
                .count() as u64,
            stale_frames: self.observations.iter().filter(|frame| frame.stale).count() as u64,
            frame_wall: PercentileSummary::from_samples(&frame_wall, method),
            wait: PercentileSummary::from_samples(&wait, method),
            app_work: PercentileSummary::from_samples(&app_work, method),
            thread_cpu: PercentileSummary::from_samples(&thread_cpu, method),
            blocked: PercentileSummary::from_samples(&blocked, method),
            headroom: HeadroomSummary::from_samples(&headroom, method),
            over_budget,
            app_over_period_frames,
            app_over_period_pct: percent(app_over_period_frames, frames),
            worst_frames: self.worst_frames(),
            latest_stage_spans,
        }
    }

    fn worst_frames(&self) -> Vec<WorstFrameDetail> {
        let target = self.config.target_period_ms;
        let mut worst = self
            .observations
            .iter()
            .map(|frame| {
                let app_work_ms = frame.computed_app_work_ms();
                let over_budget = target.is_some_and(|target| frame.frame_wall_ms > target);
                let over_2x_budget =
                    target.is_some_and(|target| frame.frame_wall_ms > target * 2.0);
                let over_4x_budget =
                    target.is_some_and(|target| frame.frame_wall_ms > target * 4.0);
                WorstFrameDetail {
                    rank: 0,
                    frame_index: frame.frame_index,
                    rendered: frame.rendered,
                    frame_wall_ms: frame.frame_wall_ms,
                    wait_ms: frame.wait_ms,
                    app_work_ms,
                    thread_cpu_ms: frame.thread_cpu_ms,
                    blocked_ms: frame.blocked_ms(),
                    headroom_ms: frame.headroom_ms(target),
                    over_budget,
                    over_2x_budget,
                    over_4x_budget,
                    stage_spans: frame.stage_spans.clone(),
                }
            })
            .collect::<Vec<_>>();
        worst.sort_by(|left, right| {
            right
                .app_work_ms
                .total_cmp(&left.app_work_ms)
                .then_with(|| right.frame_wall_ms.total_cmp(&left.frame_wall_ms))
                .then_with(|| left.frame_index.cmp(&right.frame_index))
        });
        worst.truncate(self.config.worst_frame_capacity);
        for (index, frame) in worst.iter_mut().enumerate() {
            frame.rank = index as u64 + 1;
        }
        worst
    }
}

pub fn percentile_sorted_ms(
    sorted_samples: &[f64],
    percentile: f64,
    method: PercentileMethod,
) -> f64 {
    if sorted_samples.is_empty() {
        return 0.0;
    }
    let clamped = percentile.clamp(0.0, 1.0);
    let last_index = sorted_samples.len() - 1;
    let index = match method {
        PercentileMethod::NearestRank => ((sorted_samples.len() as f64 * clamped).ceil() as usize)
            .saturating_sub(1)
            .min(last_index),
        PercentileMethod::InclusiveCeil => {
            ((last_index as f64 * clamped).ceil() as usize).min(last_index)
        }
    };
    sorted_samples[index]
}

fn finite_positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

fn sanitize_ms(ms: f64) -> f64 {
    if ms.is_finite() { ms.max(0.0) } else { 0.0 }
}

fn percent(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 * 100.0 / denominator as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StageId, StageSpan};

    #[test]
    fn percentile_methods_match_existing_call_sites() {
        let sorted = [10.0, 20.0, 30.0, 40.0];
        assert_eq!(
            percentile_sorted_ms(&sorted, 0.50, PercentileMethod::NearestRank),
            20.0
        );
        assert_eq!(
            percentile_sorted_ms(&sorted, 0.50, PercentileMethod::InclusiveCeil),
            30.0
        );
        assert_eq!(
            percentile_sorted_ms(&sorted, 0.95, PercentileMethod::NearestRank),
            40.0
        );
        assert_eq!(
            percentile_sorted_ms(&sorted, 0.95, PercentileMethod::InclusiveCeil),
            40.0
        );
    }

    #[test]
    fn frame_summary_tracks_tiers_headroom_and_worst_frames() {
        let mut accumulator = FrameAccumulator::new(
            FrameAccountingConfig::from_target_period_ms(10.0).with_worst_frame_capacity(2),
        );
        accumulator.record_frame(
            FrameObservation::new(1, 8.0)
                .with_wait_ms(2.0)
                .with_stage_span(StageSpan::new(StageId::DrawEncode, 3.0)),
        );
        accumulator.record_frame(FrameObservation::new(2, 11.0).with_wait_ms(1.0));
        accumulator.record_frame(
            FrameObservation::new(3, 21.0)
                .with_wait_ms(5.0)
                .with_thread_cpu_ms(Some(10.0)),
        );
        accumulator.record_frame(FrameObservation::new(4, 41.0).with_app_work_ms(25.0));

        let report = accumulator.summary_report();
        assert_eq!(report.schema_version, crate::FRAME_PIPELINE_SCHEMA_VERSION);
        assert_eq!(report.frames, 4);
        assert_eq!(report.rendered_frames, 4);
        assert_eq!(report.over_budget.over_budget_frames, 3);
        assert_eq!(report.over_budget.over_2x_budget_frames, 2);
        assert_eq!(report.over_budget.over_4x_budget_frames, 1);
        assert_eq!(report.app_over_period_frames, 2);
        assert_eq!(report.app_work.max_ms, 25.0);
        assert_eq!(report.blocked.max_ms, 6.0);
        assert_eq!(report.headroom.min_ms, -15.0);
        assert_eq!(report.worst_frames.len(), 2);
        assert_eq!(report.worst_frames[0].frame_index, 4);
        assert_eq!(report.worst_frames[0].rank, 1);
        assert!(report.worst_frames[0].over_4x_budget);
    }

    #[test]
    fn percentile_ring_keeps_latest_window() {
        let mut ring = PercentileRing::new(3);
        ring.push(1.0);
        ring.push(2.0);
        ring.push(3.0);
        ring.push(100.0);
        let summary = ring.summary(PercentileMethod::NearestRank);
        assert_eq!(summary.count, 3);
        assert_eq!(summary.min_ms, 2.0);
        assert_eq!(summary.max_ms, 100.0);
    }
}
