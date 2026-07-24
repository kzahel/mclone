use std::collections::VecDeque;

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
    pub conservation_tolerance_ms: f64,
    pub debug_assert_conservation: bool,
}

impl FrameAccountingConfig {
    pub const DEFAULT_WORST_FRAME_CAPACITY: usize = 8;
    pub const DEFAULT_CONSERVATION_TOLERANCE_MS: f64 = 0.25;

    pub fn from_target_hz(target_hz: f64) -> Self {
        let target_hz = finite_positive(target_hz);
        Self {
            target_hz,
            target_period_ms: target_hz.map(|hz| 1000.0 / hz),
            percentile_method: PercentileMethod::NearestRank,
            worst_frame_capacity: Self::DEFAULT_WORST_FRAME_CAPACITY,
            conservation_tolerance_ms: Self::DEFAULT_CONSERVATION_TOLERANCE_MS,
            debug_assert_conservation: true,
        }
    }

    pub fn from_target_period_ms(target_period_ms: f64) -> Self {
        let target_period_ms = finite_positive(target_period_ms);
        Self {
            target_hz: target_period_ms.map(|period| 1000.0 / period),
            target_period_ms,
            percentile_method: PercentileMethod::NearestRank,
            worst_frame_capacity: Self::DEFAULT_WORST_FRAME_CAPACITY,
            conservation_tolerance_ms: Self::DEFAULT_CONSERVATION_TOLERANCE_MS,
            debug_assert_conservation: true,
        }
    }

    pub fn without_budget() -> Self {
        Self {
            target_hz: None,
            target_period_ms: None,
            percentile_method: PercentileMethod::NearestRank,
            worst_frame_capacity: Self::DEFAULT_WORST_FRAME_CAPACITY,
            conservation_tolerance_ms: Self::DEFAULT_CONSERVATION_TOLERANCE_MS,
            debug_assert_conservation: true,
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

    pub fn with_conservation_tolerance_ms(mut self, tolerance_ms: f64) -> Self {
        self.conservation_tolerance_ms =
            finite_positive(tolerance_ms).unwrap_or(Self::DEFAULT_CONSERVATION_TOLERANCE_MS);
        self
    }

    pub fn with_debug_conservation_assertions(mut self, enabled: bool) -> Self {
        self.debug_assert_conservation = enabled;
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConservationViolationCounts {
    pub stage_spans_over_frame_wall: u64,
    pub app_wait_mismatch: u64,
    pub frame_thread_cpu_over_app_work: u64,
    pub stage_thread_cpu_over_wall: u64,
    pub non_monotonic_frame_index: u64,
}

impl ConservationViolationCounts {
    pub const fn is_empty(self) -> bool {
        self.total() == 0
    }

    pub const fn total(self) -> u64 {
        self.stage_spans_over_frame_wall
            + self.app_wait_mismatch
            + self.frame_thread_cpu_over_app_work
            + self.stage_thread_cpu_over_wall
            + self.non_monotonic_frame_index
    }

    fn add_assign(&mut self, other: Self) {
        self.stage_spans_over_frame_wall = self
            .stage_spans_over_frame_wall
            .saturating_add(other.stage_spans_over_frame_wall);
        self.app_wait_mismatch = self
            .app_wait_mismatch
            .saturating_add(other.app_wait_mismatch);
        self.frame_thread_cpu_over_app_work = self
            .frame_thread_cpu_over_app_work
            .saturating_add(other.frame_thread_cpu_over_app_work);
        self.stage_thread_cpu_over_wall = self
            .stage_thread_cpu_over_wall
            .saturating_add(other.stage_thread_cpu_over_wall);
        self.non_monotonic_frame_index = self
            .non_monotonic_frame_index
            .saturating_add(other.non_monotonic_frame_index);
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
    /// Total observations recorded since this accumulator was created.
    pub frames: u64,
    /// Observations retained for the percentile fields in the metric summaries.
    ///
    /// Summary `count`, `average_ms`, `min_ms`, and `max_ms` fields remain
    /// lifetime aggregates. Percentile fields are calculated from this bounded
    /// recent window in rolling live mode.
    pub percentile_window_frames: u64,
    /// `None` identifies an exact finite accumulator. Live accumulators report
    /// their configured retained-observation bound here.
    pub percentile_window_capacity: Option<u64>,
    pub rendered_frames: u64,
    pub dropped_frames: u64,
    pub stale_frames: u64,
    pub latest_frame_wall_ms: f64,
    pub latest_wait_ms: f64,
    pub latest_app_work_ms: f64,
    pub latest_thread_cpu_ms: Option<f64>,
    pub latest_blocked_ms: Option<f64>,
    pub latest_headroom_ms: Option<f64>,
    pub frame_wall: PercentileSummary,
    pub wait: PercentileSummary,
    pub app_work: PercentileSummary,
    pub thread_cpu: PercentileSummary,
    pub blocked: PercentileSummary,
    pub headroom: HeadroomSummary,
    pub over_budget: OverBudgetTiers,
    pub app_over_period_frames: u64,
    pub app_over_period_pct: f64,
    pub conservation_violations: ConservationViolationCounts,
    pub worst_frames: Vec<WorstFrameDetail>,
    pub latest_stage_spans: Vec<StageSpan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameAccumulator {
    config: FrameAccountingConfig,
    observation_capacity: Option<usize>,
    observations: VecDeque<FrameObservation>,
    frames: u64,
    rendered_frames: u64,
    dropped_frames: u64,
    stale_frames: u64,
    frame_wall: MetricAggregate,
    wait: MetricAggregate,
    app_work: MetricAggregate,
    thread_cpu: MetricAggregate,
    blocked: MetricAggregate,
    headroom: MetricAggregate,
    over_budget: OverBudgetTiers,
    app_over_period_frames: u64,
    conservation_violations: ConservationViolationCounts,
    worst_frames: Vec<WorstFrameDetail>,
    last_frame_index: Option<u64>,
}

impl FrameAccumulator {
    pub fn new(config: FrameAccountingConfig) -> Self {
        Self::with_observation_capacity(config, None)
    }

    pub fn new_rolling(config: FrameAccountingConfig, observation_capacity: usize) -> Self {
        assert!(
            observation_capacity > 0,
            "rolling frame history capacity must be positive"
        );
        Self::with_observation_capacity(config, Some(observation_capacity))
    }

    fn with_observation_capacity(
        config: FrameAccountingConfig,
        observation_capacity: Option<usize>,
    ) -> Self {
        Self {
            config,
            observation_capacity,
            observations: observation_capacity.map_or_else(VecDeque::new, VecDeque::with_capacity),
            frames: 0,
            rendered_frames: 0,
            dropped_frames: 0,
            stale_frames: 0,
            frame_wall: MetricAggregate::default(),
            wait: MetricAggregate::default(),
            app_work: MetricAggregate::default(),
            thread_cpu: MetricAggregate::default(),
            blocked: MetricAggregate::default(),
            headroom: MetricAggregate::default(),
            over_budget: OverBudgetTiers::default(),
            app_over_period_frames: 0,
            conservation_violations: ConservationViolationCounts::default(),
            worst_frames: Vec::with_capacity(config.worst_frame_capacity),
            last_frame_index: None,
        }
    }

    pub fn record_frame(&mut self, observation: FrameObservation) {
        let violations = self.conservation_violations_for(&observation);
        if self.config.debug_assert_conservation {
            debug_assert!(
                violations.is_empty(),
                "frame accounting conservation violation: {violations:?}"
            );
        }
        self.conservation_violations.add_assign(violations);
        self.last_frame_index = Some(observation.frame_index);
        self.frames = self.frames.saturating_add(1);
        self.rendered_frames = self
            .rendered_frames
            .saturating_add(u64::from(observation.rendered));
        self.dropped_frames = self
            .dropped_frames
            .saturating_add(u64::from(observation.dropped));
        self.stale_frames = self
            .stale_frames
            .saturating_add(u64::from(observation.stale));
        self.frame_wall.observe(observation.frame_wall_ms);
        self.wait.observe(observation.wait_ms);
        let app_work_ms = observation.computed_app_work_ms();
        self.app_work.observe(app_work_ms);
        if let Some(thread_cpu_ms) = observation.thread_cpu_ms {
            self.thread_cpu.observe(thread_cpu_ms);
        }
        if let Some(blocked_ms) = observation.blocked_ms() {
            self.blocked.observe(blocked_ms);
        }
        if let Some(headroom_ms) = observation.headroom_ms(self.config.target_period_ms) {
            self.headroom.observe_signed(headroom_ms);
        }
        self.over_budget
            .observe(observation.frame_wall_ms, self.config.target_period_ms);
        if self
            .config
            .target_period_ms
            .is_some_and(|target| app_work_ms > target)
        {
            self.app_over_period_frames = self.app_over_period_frames.saturating_add(1);
        }
        self.retain_worst_frame(&observation);
        if self
            .observation_capacity
            .is_some_and(|capacity| self.observations.len() == capacity)
        {
            self.observations.pop_front();
        }
        self.observations.push_back(observation);
    }

    pub fn len(&self) -> usize {
        usize::try_from(self.frames).unwrap_or(usize::MAX)
    }

    pub fn is_empty(&self) -> bool {
        self.frames == 0
    }

    pub fn frames_recorded(&self) -> u64 {
        self.frames
    }

    pub fn retained_len(&self) -> usize {
        self.observations.len()
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
        let latest = self.observations.back();
        let latest_stage_spans = latest
            .map(|frame| frame.stage_spans.clone())
            .unwrap_or_default();
        FrameSummaryReport {
            schema_version: crate::FRAME_PIPELINE_SCHEMA_VERSION,
            target_hz: self.config.target_hz,
            target_period_ms: self.config.target_period_ms,
            frames: self.frames,
            percentile_window_frames: self.observations.len() as u64,
            percentile_window_capacity: self
                .observation_capacity
                .and_then(|capacity| u64::try_from(capacity).ok()),
            rendered_frames: self.rendered_frames,
            dropped_frames: self.dropped_frames,
            stale_frames: self.stale_frames,
            latest_frame_wall_ms: latest.map_or(0.0, |frame| frame.frame_wall_ms),
            latest_wait_ms: latest.map_or(0.0, |frame| frame.wait_ms),
            latest_app_work_ms: latest.map_or(0.0, FrameObservation::computed_app_work_ms),
            latest_thread_cpu_ms: latest.and_then(|frame| frame.thread_cpu_ms),
            latest_blocked_ms: latest.and_then(FrameObservation::blocked_ms),
            latest_headroom_ms: latest
                .and_then(|frame| frame.headroom_ms(self.config.target_period_ms)),
            frame_wall: self.frame_wall.percentile_summary(&frame_wall, method),
            wait: self.wait.percentile_summary(&wait, method),
            app_work: self.app_work.percentile_summary(&app_work, method),
            thread_cpu: self.thread_cpu.percentile_summary(&thread_cpu, method),
            blocked: self.blocked.percentile_summary(&blocked, method),
            headroom: self.headroom.headroom_summary(&headroom, method),
            over_budget: self.over_budget,
            app_over_period_frames: self.app_over_period_frames,
            app_over_period_pct: percent(self.app_over_period_frames, self.frames),
            conservation_violations: self.conservation_violations,
            worst_frames: self.worst_frames.clone(),
            latest_stage_spans,
        }
    }

    fn conservation_violations_for(
        &self,
        observation: &FrameObservation,
    ) -> ConservationViolationCounts {
        let tolerance_ms = sanitize_ms(self.config.conservation_tolerance_ms);
        let mut violations = ConservationViolationCounts::default();
        if let Some(last_frame_index) = self.last_frame_index
            && observation.frame_index <= last_frame_index
        {
            violations.non_monotonic_frame_index = 1;
        }

        let stage_elapsed_ms = observation
            .stage_spans
            .iter()
            .map(|span| span.elapsed_ms)
            .sum::<f64>();
        if exceeds_with_tolerance(stage_elapsed_ms, observation.frame_wall_ms, tolerance_ms) {
            violations.stage_spans_over_frame_wall = 1;
        }

        if observation.app_work_ms.is_some() && observation.wait_ms > 0.0 {
            let app_wait_ms = observation.computed_app_work_ms() + observation.wait_ms;
            if differs_by_more_than(app_wait_ms, observation.frame_wall_ms, tolerance_ms) {
                violations.app_wait_mismatch = 1;
            }
        }

        if let Some(thread_cpu_ms) = observation.thread_cpu_ms
            && exceeds_with_tolerance(
                thread_cpu_ms,
                observation.computed_app_work_ms(),
                tolerance_ms,
            )
        {
            violations.frame_thread_cpu_over_app_work = 1;
        }

        for span in &observation.stage_spans {
            if let Some(thread_cpu_ms) = span.thread_cpu_ms
                && exceeds_with_tolerance(thread_cpu_ms, span.elapsed_ms, tolerance_ms)
            {
                violations.stage_thread_cpu_over_wall =
                    violations.stage_thread_cpu_over_wall.saturating_add(1);
            }
        }
        violations
    }

    fn retain_worst_frame(&mut self, frame: &FrameObservation) {
        if self.config.worst_frame_capacity == 0 {
            return;
        }
        let target = self.config.target_period_ms;
        let app_work_ms = frame.computed_app_work_ms();
        let insert_at = self
            .worst_frames
            .iter()
            .position(|retained| {
                worst_frame_ordering_values(
                    app_work_ms,
                    frame.frame_wall_ms,
                    frame.frame_index,
                    retained,
                )
                .is_lt()
            })
            .unwrap_or(self.worst_frames.len());
        if insert_at >= self.config.worst_frame_capacity {
            return;
        }
        self.worst_frames.insert(
            insert_at,
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
                over_budget: target.is_some_and(|target| frame.frame_wall_ms > target),
                over_2x_budget: target.is_some_and(|target| frame.frame_wall_ms > target * 2.0),
                over_4x_budget: target.is_some_and(|target| frame.frame_wall_ms > target * 4.0),
                stage_spans: frame.stage_spans.clone(),
            },
        );
        self.worst_frames.truncate(self.config.worst_frame_capacity);
        for (index, frame) in self.worst_frames.iter_mut().enumerate() {
            frame.rank = index as u64 + 1;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct MetricAggregate {
    count: u64,
    sum_ms: f64,
    min_ms: Option<f64>,
    max_ms: Option<f64>,
}

impl MetricAggregate {
    fn observe(&mut self, value_ms: f64) {
        self.observe_value(sanitize_ms(value_ms));
    }

    fn observe_signed(&mut self, value_ms: f64) {
        self.observe_value(if value_ms.is_finite() { value_ms } else { 0.0 });
    }

    fn observe_value(&mut self, value_ms: f64) {
        self.count = self.count.saturating_add(1);
        self.sum_ms += value_ms;
        self.min_ms = Some(self.min_ms.map_or(value_ms, |min_ms| min_ms.min(value_ms)));
        self.max_ms = Some(self.max_ms.map_or(value_ms, |max_ms| max_ms.max(value_ms)));
    }

    fn percentile_summary(self, samples: &[f64], method: PercentileMethod) -> PercentileSummary {
        let mut summary = PercentileSummary::from_samples(samples, method);
        summary.count = self.count;
        summary.average_ms = self.average_ms();
        summary.min_ms = self.min_ms.unwrap_or(0.0);
        summary.max_ms = self.max_ms.unwrap_or(0.0);
        summary
    }

    fn headroom_summary(self, samples: &[f64], method: PercentileMethod) -> HeadroomSummary {
        let mut summary = HeadroomSummary::from_samples(samples, method);
        summary.count = self.count;
        summary.average_ms = self.average_ms();
        summary.min_ms = self.min_ms.unwrap_or(0.0);
        summary
    }

    fn average_ms(self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum_ms / self.count as f64
        }
    }
}

fn worst_frame_ordering_values(
    app_work_ms: f64,
    frame_wall_ms: f64,
    frame_index: u64,
    retained: &WorstFrameDetail,
) -> std::cmp::Ordering {
    retained
        .app_work_ms
        .total_cmp(&app_work_ms)
        .then_with(|| retained.frame_wall_ms.total_cmp(&frame_wall_ms))
        .then_with(|| frame_index.cmp(&retained.frame_index))
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

fn exceeds_with_tolerance(value: f64, limit: f64, tolerance: f64) -> bool {
    sanitize_ms(value) > sanitize_ms(limit) + sanitize_ms(tolerance)
}

fn differs_by_more_than(left: f64, right: f64, tolerance: f64) -> bool {
    (sanitize_ms(left) - sanitize_ms(right)).abs() > sanitize_ms(tolerance)
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
        assert_eq!(report.percentile_window_frames, 4);
        assert_eq!(report.percentile_window_capacity, None);
        assert_eq!(report.rendered_frames, 4);
        assert_eq!(report.latest_frame_wall_ms, 41.0);
        assert_eq!(report.latest_wait_ms, 0.0);
        assert_eq!(report.latest_app_work_ms, 25.0);
        assert_eq!(report.latest_headroom_ms, Some(-15.0));
        assert_eq!(report.over_budget.over_budget_frames, 3);
        assert_eq!(report.over_budget.over_2x_budget_frames, 2);
        assert_eq!(report.over_budget.over_4x_budget_frames, 1);
        assert_eq!(report.app_over_period_frames, 2);
        assert_eq!(report.app_work.max_ms, 25.0);
        assert_eq!(report.blocked.max_ms, 6.0);
        assert_eq!(report.headroom.min_ms, -15.0);
        assert_eq!(report.conservation_violations.total(), 0);
        assert_eq!(report.worst_frames.len(), 2);
        assert_eq!(report.worst_frames[0].frame_index, 4);
        assert_eq!(report.worst_frames[0].rank, 1);
        assert!(report.worst_frames[0].over_4x_budget);
    }

    #[test]
    fn frame_summary_counts_conservation_violations_when_assertions_are_disabled() {
        let mut accumulator = FrameAccumulator::new(
            FrameAccountingConfig::from_target_period_ms(10.0)
                .with_conservation_tolerance_ms(0.01)
                .with_debug_conservation_assertions(false),
        );
        accumulator.record_frame(FrameObservation::new(2, 10.0));
        accumulator.record_frame(
            FrameObservation::new(2, 10.0)
                .with_wait_ms(4.0)
                .with_app_work_ms(9.0)
                .with_thread_cpu_ms(Some(10.0))
                .with_stage_span(
                    StageSpan::new(StageId::DrawEncode, 8.0).with_thread_cpu_ms(Some(9.0)),
                )
                .with_stage_span(StageSpan::new(StageId::GpuUpload, 5.0)),
        );

        let violations = accumulator.summary_report().conservation_violations;
        assert_eq!(violations.stage_spans_over_frame_wall, 1);
        assert_eq!(violations.app_wait_mismatch, 1);
        assert_eq!(violations.frame_thread_cpu_over_app_work, 1);
        assert_eq!(violations.stage_thread_cpu_over_wall, 1);
        assert_eq!(violations.non_monotonic_frame_index, 1);
        assert_eq!(violations.total(), 5);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn frame_summary_debug_asserts_conservation_violations_by_default() {
        let result = std::panic::catch_unwind(|| {
            let mut accumulator = FrameAccumulator::new(
                FrameAccountingConfig::from_target_period_ms(10.0)
                    .with_conservation_tolerance_ms(0.01),
            );
            accumulator.record_frame(
                FrameObservation::new(1, 10.0)
                    .with_stage_span(StageSpan::new(StageId::DrawEncode, 11.0)),
            );
        });
        assert!(result.is_err());
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

    #[test]
    fn rolling_accumulator_bounds_history_but_keeps_lifetime_totals_and_worst_frames() {
        let mut accumulator = FrameAccumulator::new_rolling(
            FrameAccountingConfig::from_target_period_ms(4.0).with_worst_frame_capacity(2),
            3,
        );
        for frame_index in 1..=6 {
            accumulator.record_frame(FrameObservation::new(frame_index, frame_index as f64));
        }

        let report = accumulator.summary_report();
        assert_eq!(accumulator.frames_recorded(), 6);
        assert_eq!(accumulator.retained_len(), 3);
        assert_eq!(report.frames, 6);
        assert_eq!(report.percentile_window_frames, 3);
        assert_eq!(report.percentile_window_capacity, Some(3));
        assert_eq!(report.frame_wall.count, 6);
        assert_eq!(report.frame_wall.average_ms, 3.5);
        assert_eq!(report.frame_wall.min_ms, 1.0);
        assert_eq!(report.frame_wall.p50_ms, 5.0);
        assert_eq!(report.frame_wall.max_ms, 6.0);
        assert_eq!(report.latest_frame_wall_ms, 6.0);
        assert_eq!(report.over_budget.over_budget_frames, 2);
        assert_eq!(report.app_over_period_frames, 2);
        assert_eq!(
            report
                .worst_frames
                .iter()
                .map(|frame| frame.frame_index)
                .collect::<Vec<_>>(),
            vec![6, 5]
        );
    }
}
