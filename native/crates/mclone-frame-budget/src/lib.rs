#![deny(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;

pub use mclone_diagnostics::{
    BudgetDecisionAddress, BudgetDecisionFamily, BudgetDecisionPanelReport, BudgetDecisionReason,
    BudgetDecisionReport, BudgetDecisionTraceReport, BudgetGrantReport, BudgetHostMode,
    BudgetInputSnapshotReport, FrameHostKind, StageId, WorkWindow,
};
use serde::{Deserialize, Serialize};

const TARGET_PERIOD_EPSILON_MS: f64 = 0.001;
pub const DEFAULT_COST_EWMA_ALPHA: f64 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetTelemetryWindow {
    pub host_kind: FrameHostKind,
    pub work_window: WorkWindow,
    pub app_work_p95_ms: Option<f64>,
    pub headroom_p05_ms: Option<f64>,
    pub app_over_period_pct: f64,
    pub missed_frames: u64,
    pub over_2x_frames: u64,
    pub dropped_frames_delta: u64,
    pub stale_frames_delta: u64,
    pub queue_depth: u64,
    pub oldest_queue_age_ms: Option<f64>,
}

impl BudgetTelemetryWindow {
    pub const fn new(host_kind: FrameHostKind, work_window: WorkWindow) -> Self {
        Self {
            host_kind,
            work_window,
            app_work_p95_ms: None,
            headroom_p05_ms: None,
            app_over_period_pct: 0.0,
            missed_frames: 0,
            over_2x_frames: 0,
            dropped_frames_delta: 0,
            stale_frames_delta: 0,
            queue_depth: 0,
            oldest_queue_age_ms: None,
        }
    }

    pub fn with_headroom_p05_ms(mut self, headroom_p05_ms: f64) -> Self {
        self.headroom_p05_ms = Some(headroom_p05_ms);
        self
    }

    pub fn with_app_work_p95_ms(mut self, app_work_p95_ms: f64) -> Self {
        self.app_work_p95_ms = Some(sanitize_ms(app_work_p95_ms));
        self
    }

    pub fn with_missed_frames(mut self, missed_frames: u64) -> Self {
        self.missed_frames = missed_frames;
        self
    }

    pub fn with_queue_age_ms(mut self, queue_depth: u64, oldest_queue_age_ms: f64) -> Self {
        self.queue_depth = queue_depth;
        self.oldest_queue_age_ms = Some(sanitize_ms(oldest_queue_age_ms));
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetCostEstimates {
    pub feature_publish_ms: Option<f64>,
    pub light_publish_ms: Option<f64>,
    pub completed_result_accept_ms: Option<f64>,
    pub section_upload_ms: Option<f64>,
    pub render_admission_scan_ms: Option<f64>,
}

impl BudgetCostEstimates {
    pub fn cost_for(self, family: BudgetDecisionFamily) -> Option<f64> {
        match family {
            BudgetDecisionFamily::FeaturePublication => self.feature_publish_ms,
            BudgetDecisionFamily::LightPublication => self.light_publish_ms,
            BudgetDecisionFamily::CompletedResultAcceptance => self.completed_result_accept_ms,
            BudgetDecisionFamily::SectionUpload => self.section_upload_ms,
            BudgetDecisionFamily::RenderAdmission
            | BudgetDecisionFamily::FeatureJobAdmission
            | BudgetDecisionFamily::RenderCompileWorkers => self.render_admission_scan_ms,
        }
        .map(sanitize_ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwmaCostEstimator {
    pub alpha: f64,
    pub estimates: BudgetCostEstimates,
    pub target_period_ms: Option<f64>,
}

impl EwmaCostEstimator {
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
            estimates: BudgetCostEstimates::default(),
            target_period_ms: None,
        }
    }

    pub fn reset(&mut self) {
        self.estimates = BudgetCostEstimates::default();
    }

    pub fn observe_for_target_period(
        &mut self,
        target_period_ms: f64,
        family: BudgetDecisionFamily,
        elapsed_ms: f64,
        units: u32,
    ) -> BudgetCostEstimates {
        if self
            .target_period_ms
            .is_some_and(|last| (last - target_period_ms).abs() > TARGET_PERIOD_EPSILON_MS)
        {
            self.reset();
        }
        if finite_positive(target_period_ms).is_some() {
            self.target_period_ms = Some(target_period_ms);
        }
        self.observe(family, elapsed_ms, units)
    }

    pub fn observe(
        &mut self,
        family: BudgetDecisionFamily,
        elapsed_ms: f64,
        units: u32,
    ) -> BudgetCostEstimates {
        if units == 0 {
            return self.estimates;
        }
        let sample = sanitize_ms(elapsed_ms) / f64::from(units);
        let alpha = self.alpha;
        let slot = match family {
            BudgetDecisionFamily::FeaturePublication => &mut self.estimates.feature_publish_ms,
            BudgetDecisionFamily::LightPublication => &mut self.estimates.light_publish_ms,
            BudgetDecisionFamily::CompletedResultAcceptance => {
                &mut self.estimates.completed_result_accept_ms
            }
            BudgetDecisionFamily::SectionUpload => &mut self.estimates.section_upload_ms,
            BudgetDecisionFamily::RenderAdmission
            | BudgetDecisionFamily::FeatureJobAdmission
            | BudgetDecisionFamily::RenderCompileWorkers => {
                &mut self.estimates.render_admission_scan_ms
            }
        };
        *slot = Some(slot.map_or(sample, |current| current + alpha * (sample - current)));
        self.estimates
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetControllerInput {
    pub target_period_ms: f64,
    pub host_mode: BudgetHostMode,
    pub catch_up_gameplay_ticks: u32,
    pub windows: Vec<BudgetTelemetryWindow>,
    pub costs: BudgetCostEstimates,
}

impl BudgetControllerInput {
    pub fn new(target_period_ms: f64) -> Self {
        Self {
            target_period_ms: sanitize_ms(target_period_ms),
            host_mode: BudgetHostMode::LocalIntegrated,
            catch_up_gameplay_ticks: 1,
            windows: Vec::new(),
            costs: BudgetCostEstimates::default(),
        }
    }

    pub fn with_window(mut self, window: BudgetTelemetryWindow) -> Self {
        self.windows.push(window);
        self
    }

    pub fn with_costs(mut self, costs: BudgetCostEstimates) -> Self {
        self.costs = costs;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyBudgetConfig {
    pub family: BudgetDecisionFamily,
    pub address: BudgetDecisionAddress,
    pub floor_units: u32,
    pub cap_units: u32,
    pub raise_units: u32,
    pub floor_period_fraction: f64,
    pub cap_period_fraction: f64,
    pub raise_period_fraction: f64,
    pub floor_pending_units: Option<u32>,
    pub cap_pending_units: Option<u32>,
    pub raise_pending_units: u32,
}

impl FamilyBudgetConfig {
    pub const fn new(
        family: BudgetDecisionFamily,
        address: BudgetDecisionAddress,
        floor_units: u32,
        cap_units: u32,
        floor_period_fraction: f64,
        cap_period_fraction: f64,
    ) -> Self {
        Self {
            family,
            address,
            floor_units,
            cap_units,
            raise_units: 1,
            floor_period_fraction,
            cap_period_fraction,
            raise_period_fraction: 0.02,
            floor_pending_units: None,
            cap_pending_units: None,
            raise_pending_units: 1,
        }
    }

    pub const fn with_pending_units(mut self, floor: u32, cap: u32) -> Self {
        self.floor_pending_units = Some(floor);
        self.cap_pending_units = Some(cap);
        self
    }

    fn floor_units(self) -> u32 {
        self.floor_units.max(1)
    }

    fn cap_units(self) -> u32 {
        self.cap_units.max(self.floor_units())
    }

    fn floor_period_fraction(self) -> f64 {
        sanitize_fraction(self.floor_period_fraction)
    }

    fn cap_period_fraction(self) -> f64 {
        sanitize_fraction(self.cap_period_fraction).max(self.floor_period_fraction())
    }

    fn raise_period_fraction(self) -> f64 {
        sanitize_fraction(self.raise_period_fraction)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetControllerConfig {
    pub raise_after_clean_windows: u32,
    pub over_period_pct_cut_threshold: f64,
    pub queue_age_limit_ms: Option<f64>,
    pub queue_depth_limit: Option<u64>,
    pub families: Vec<FamilyBudgetConfig>,
}

impl Default for BudgetControllerConfig {
    fn default() -> Self {
        use BudgetDecisionFamily::*;
        use FrameHostKind::*;
        use StageId::*;
        use WorkWindow::*;

        Self {
            raise_after_clean_windows: 3,
            over_period_pct_cut_threshold: 0.0,
            queue_age_limit_ms: Some(250.0),
            queue_depth_limit: Some(1_000),
            families: vec![
                FamilyBudgetConfig::new(
                    FeaturePublication,
                    BudgetDecisionAddress::new(
                        IntegratedServerRunner,
                        GameplayTick,
                        SchedulerPublication,
                    ),
                    1,
                    4,
                    0.02,
                    0.20,
                ),
                FamilyBudgetConfig::new(
                    LightPublication,
                    BudgetDecisionAddress::new(
                        IntegratedServerRunner,
                        GameplayTick,
                        SchedulerPublication,
                    ),
                    1,
                    4,
                    0.02,
                    0.20,
                ),
                FamilyBudgetConfig::new(
                    FeatureJobAdmission,
                    BudgetDecisionAddress::new(
                        IntegratedServerRunner,
                        GameplayTick,
                        TerrainGeneration,
                    ),
                    1,
                    4,
                    0.0,
                    0.10,
                )
                .with_pending_units(1, 8),
                FamilyBudgetConfig::new(
                    RenderAdmission,
                    BudgetDecisionAddress::new(
                        DesktopFlatWinit,
                        BeforeRender,
                        RenderSectionAdmission,
                    ),
                    1,
                    4,
                    0.02,
                    0.20,
                ),
                FamilyBudgetConfig::new(
                    BudgetDecisionFamily::CompletedResultAcceptance,
                    BudgetDecisionAddress::new(
                        AndroidXrOpenXr,
                        BeforeRender,
                        StageId::CompletedResultAcceptance,
                    ),
                    1,
                    16,
                    0.02,
                    0.20,
                ),
                FamilyBudgetConfig::new(
                    SectionUpload,
                    BudgetDecisionAddress::new(AndroidXrOpenXr, BeforeRender, UploadApply),
                    1,
                    16,
                    0.02,
                    0.20,
                ),
                FamilyBudgetConfig::new(
                    RenderCompileWorkers,
                    BudgetDecisionAddress::new(DesktopFlatWinit, WorkerPoll, CpuMeshCompile),
                    1,
                    4,
                    0.0,
                    0.0,
                ),
            ],
        }
    }
}

pub fn render_frame_budget_controller_config(
    host_kind: FrameHostKind,
    admission_window: WorkWindow,
) -> BudgetControllerConfig {
    use BudgetDecisionFamily::*;
    use WorkWindow::*;

    BudgetControllerConfig {
        raise_after_clean_windows: 3,
        over_period_pct_cut_threshold: 0.0,
        queue_age_limit_ms: Some(250.0),
        queue_depth_limit: Some(1_000),
        families: vec![
            FamilyBudgetConfig::new(
                RenderAdmission,
                BudgetDecisionAddress::new(
                    host_kind,
                    admission_window,
                    StageId::RenderSectionAdmission,
                ),
                1,
                4,
                0.02,
                0.20,
            ),
            FamilyBudgetConfig::new(
                BudgetDecisionFamily::CompletedResultAcceptance,
                BudgetDecisionAddress::new(
                    host_kind,
                    admission_window,
                    StageId::CompletedResultAcceptance,
                ),
                1,
                16,
                0.02,
                0.20,
            ),
            FamilyBudgetConfig::new(
                SectionUpload,
                BudgetDecisionAddress::new(host_kind, admission_window, StageId::UploadApply),
                1,
                16,
                0.02,
                0.20,
            ),
            FamilyBudgetConfig::new(
                RenderCompileWorkers,
                BudgetDecisionAddress::new(host_kind, WorkerPoll, StageId::CpuMeshCompile),
                1,
                4,
                0.0,
                0.0,
            ),
        ],
    }
}

pub fn decision_for_family(
    panel: &BudgetDecisionPanelReport,
    family: BudgetDecisionFamily,
) -> Option<&BudgetDecisionReport> {
    panel
        .decisions
        .iter()
        .find(|decision| decision.family == family)
}

pub const DEFAULT_RENDER_COMPILE_CAPACITY_WORKER_FLOOR: usize = 1;
pub const DEFAULT_RENDER_COMPILE_CAPACITY_MAX_PENDING_FLOOR: usize = 4;
pub const VANILLA_RENDER_COMPILE_BACKGROUND_WORKER_CAP: usize = 7;
pub const VANILLA_RENDER_COMPILE_MEMORY_BUDGET_FRACTION: f64 = 0.30;
pub const VANILLA_RENDER_COMPILE_BUFFER_SAFETY_MULTIPLIER: u64 = 4;
pub const VANILLA_RENDER_COMPILE_RESERVED_BUFFER_PACKS: usize = 1;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderCompileThreadReservation {
    pub frame_thread_count: usize,
    pub server_runner_count: usize,
    pub worldgen_actor_count: usize,
    pub light_actor_count: usize,
    pub xr_runtime_count: usize,
    pub extra_reserved_count: usize,
}

impl RenderCompileThreadReservation {
    pub const fn local_integrated_flat() -> Self {
        Self {
            frame_thread_count: 1,
            server_runner_count: 1,
            worldgen_actor_count: 1,
            light_actor_count: 1,
            xr_runtime_count: 0,
            extra_reserved_count: 0,
        }
    }

    pub const fn local_integrated_xr() -> Self {
        Self {
            xr_runtime_count: 1,
            ..Self::local_integrated_flat()
        }
    }

    pub const fn total(self) -> usize {
        self.frame_thread_count
            + self.server_runner_count
            + self.worldgen_actor_count
            + self.light_actor_count
            + self.xr_runtime_count
            + self.extra_reserved_count
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderCompileMeshFootprint {
    pub compile_request_bytes: Option<u64>,
    pub mesh_vertex_bytes: Option<u64>,
    pub mesh_index_bytes: Option<u64>,
}

impl RenderCompileMeshFootprint {
    pub fn total_pack_bytes(self) -> Option<u64> {
        let total = self
            .compile_request_bytes
            .unwrap_or(0)
            .saturating_add(self.mesh_vertex_bytes.unwrap_or(0))
            .saturating_add(self.mesh_index_bytes.unwrap_or(0));
        (total > 0).then_some(total)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderCompileCapacityInput {
    pub available_parallelism: Option<usize>,
    pub total_memory_bytes: Option<u64>,
    pub reservation: RenderCompileThreadReservation,
    pub mesh_footprint: RenderCompileMeshFootprint,
    pub worker_floor: usize,
    pub max_pending_floor: usize,
}

impl RenderCompileCapacityInput {
    pub const fn new(
        available_parallelism: Option<usize>,
        total_memory_bytes: Option<u64>,
        reservation: RenderCompileThreadReservation,
        mesh_footprint: RenderCompileMeshFootprint,
    ) -> Self {
        Self {
            available_parallelism,
            total_memory_bytes,
            reservation,
            mesh_footprint,
            worker_floor: DEFAULT_RENDER_COMPILE_CAPACITY_WORKER_FLOOR,
            max_pending_floor: DEFAULT_RENDER_COMPILE_CAPACITY_MAX_PENDING_FLOOR,
        }
    }

    pub const fn with_floors(mut self, worker_floor: usize, max_pending_floor: usize) -> Self {
        self.worker_floor = worker_floor;
        self.max_pending_floor = max_pending_floor;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderCompileCapacityFallbackReason {
    MissingParallelism,
    MissingMemoryBudgetOrFootprint,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderCompileCapacityReport {
    pub available_parallelism: Option<usize>,
    pub reserved_parallelism: usize,
    pub usable_parallelism: Option<usize>,
    pub cpu_worker_cap: usize,
    pub cpu_pack_cap: Option<usize>,
    pub total_memory_bytes: Option<u64>,
    pub memory_budget_fraction: f64,
    pub memory_budget_bytes: Option<u64>,
    pub mesh_footprint: RenderCompileMeshFootprint,
    pub mesh_buffer_pack_bytes: Option<u64>,
    pub memory_pack_cap: Option<usize>,
    pub worker_floor: usize,
    pub max_pending_floor: usize,
    pub derived_worker_count: usize,
    pub derived_max_pending_jobs: usize,
    pub fallback_reason: Option<RenderCompileCapacityFallbackReason>,
}

pub fn derive_render_compile_capacity(
    input: RenderCompileCapacityInput,
) -> RenderCompileCapacityReport {
    let worker_floor = input.worker_floor.max(1);
    let max_pending_floor = input.max_pending_floor.max(worker_floor);
    let available_parallelism = input.available_parallelism.filter(|value| *value > 0);
    let reserved_parallelism = input.reservation.total();
    let usable_parallelism = available_parallelism.map(|available| {
        available
            .saturating_sub(reserved_parallelism)
            .max(worker_floor)
    });
    let cpu_worker_cap = usable_parallelism
        .map(|usable| {
            usable.clamp(
                worker_floor,
                VANILLA_RENDER_COMPILE_BACKGROUND_WORKER_CAP.max(worker_floor),
            )
        })
        .unwrap_or(worker_floor);
    let cpu_pack_cap = available_parallelism.map(|available| {
        if usize::BITS >= 64 {
            available.max(worker_floor)
        } else {
            available.min(4).max(worker_floor)
        }
    });
    let memory_budget_bytes = input
        .total_memory_bytes
        .filter(|value| *value > 0)
        .map(|bytes| (bytes as f64 * VANILLA_RENDER_COMPILE_MEMORY_BUDGET_FRACTION) as u64)
        .filter(|value| *value > 0);
    let mesh_buffer_pack_bytes = input.mesh_footprint.total_pack_bytes();
    let memory_pack_cap = memory_budget_bytes
        .zip(mesh_buffer_pack_bytes)
        .and_then(|(budget, pack)| {
            let guarded_pack = pack.saturating_mul(VANILLA_RENDER_COMPILE_BUFFER_SAFETY_MULTIPLIER);
            (guarded_pack > 0).then_some(budget / guarded_pack)
        })
        .map(|raw_packs| {
            usize::try_from(raw_packs)
                .unwrap_or(usize::MAX)
                .saturating_sub(VANILLA_RENDER_COMPILE_RESERVED_BUFFER_PACKS)
                .max(worker_floor)
        });
    let fallback_reason = if available_parallelism.is_none() {
        Some(RenderCompileCapacityFallbackReason::MissingParallelism)
    } else if memory_pack_cap.is_none() {
        Some(RenderCompileCapacityFallbackReason::MissingMemoryBudgetOrFootprint)
    } else {
        None
    };
    let (derived_worker_count, derived_max_pending_jobs) = if fallback_reason.is_none() {
        let pack_count = cpu_pack_cap
            .zip(memory_pack_cap)
            .map(|(cpu, memory)| cpu.min(memory).max(worker_floor))
            .unwrap_or(worker_floor);
        let worker_count = cpu_worker_cap.min(pack_count).max(worker_floor);
        let max_pending_jobs = max_pending_floor.max(worker_count).max(pack_count);
        (worker_count, max_pending_jobs)
    } else {
        (worker_floor, max_pending_floor)
    };

    RenderCompileCapacityReport {
        available_parallelism,
        reserved_parallelism,
        usable_parallelism,
        cpu_worker_cap,
        cpu_pack_cap,
        total_memory_bytes: input.total_memory_bytes.filter(|value| *value > 0),
        memory_budget_fraction: VANILLA_RENDER_COMPILE_MEMORY_BUDGET_FRACTION,
        memory_budget_bytes,
        mesh_footprint: input.mesh_footprint,
        mesh_buffer_pack_bytes,
        memory_pack_cap,
        worker_floor,
        max_pending_floor,
        derived_worker_count,
        derived_max_pending_jobs,
        fallback_reason,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BudgetController {
    config: BudgetControllerConfig,
    states: BTreeMap<BudgetDecisionFamily, FamilyState>,
    last_target_period_ms: Option<f64>,
}

impl BudgetController {
    pub fn new(config: BudgetControllerConfig) -> Self {
        let mut states = BTreeMap::new();
        for family in &config.families {
            states.insert(family.family, FamilyState::floor(*family));
        }
        Self {
            config,
            states,
            last_target_period_ms: None,
        }
    }

    pub fn decide(&mut self, input: &BudgetControllerInput) -> BudgetDecisionPanelReport {
        let target_period_ms = finite_positive(input.target_period_ms);
        let missing_input = target_period_ms.is_none() || input.windows.is_empty();
        let target_period_changed = target_period_ms.is_some_and(|target| {
            self.last_target_period_ms
                .is_some_and(|last| (last - target).abs() > TARGET_PERIOD_EPSILON_MS)
        });
        let pressure = (!missing_input && !target_period_changed).then(|| self.pressure(input));
        let mut decisions = Vec::with_capacity(self.config.families.len());

        for family_config in &self.config.families {
            let state = self
                .states
                .entry(family_config.family)
                .or_insert_with(|| FamilyState::floor(*family_config));
            let previous_max_units = state.current_units;
            let reason = if missing_input {
                state.reset_to_floor(*family_config);
                state.initialized = true;
                BudgetDecisionReason::MissingInputFloor
            } else if !state.initialized {
                state.reset_to_floor(*family_config);
                state.initialized = true;
                BudgetDecisionReason::ColdStartFloor
            } else if target_period_changed {
                state.reset_to_floor(*family_config);
                BudgetDecisionReason::TargetPeriodChangedFloor
            } else if let Some(Some(pressure)) = pressure {
                state.cut_toward_floor(*family_config);
                pressure.reason()
            } else {
                state.raise_or_hold(*family_config, self.config.raise_after_clean_windows)
            };
            let snapshot =
                input_snapshot(input, target_period_ms.unwrap_or(0.0), family_config.family);
            decisions.push(BudgetDecisionReport {
                family: family_config.family,
                address: family_config.address,
                grant: state.grant(
                    *family_config,
                    target_period_ms.unwrap_or(0.0),
                    input.costs.cost_for(family_config.family),
                ),
                trace: BudgetDecisionTraceReport {
                    reason,
                    input_snapshot: snapshot,
                    previous_max_units,
                    consecutive_clean_windows: state.clean_windows,
                },
            });
        }

        if let Some(target) = target_period_ms {
            self.last_target_period_ms = Some(target);
        }

        BudgetDecisionPanelReport::new(decisions)
    }

    fn pressure(&self, input: &BudgetControllerInput) -> Option<BudgetPressure> {
        if input.windows.iter().any(|window| {
            window.missed_frames > 0
                || window.over_2x_frames > 0
                || window.dropped_frames_delta > 0
                || window.stale_frames_delta > 0
                || window.app_over_period_pct > self.config.over_period_pct_cut_threshold
        }) {
            return Some(BudgetPressure::FrameMiss);
        }
        if input
            .windows
            .iter()
            .filter_map(|window| window.headroom_p05_ms)
            .any(|headroom| headroom < 0.0)
        {
            return Some(BudgetPressure::NegativeHeadroom);
        }
        let over_age = self.config.queue_age_limit_ms.is_some_and(|limit| {
            input
                .windows
                .iter()
                .filter_map(|window| window.oldest_queue_age_ms)
                .any(|age| age > limit)
        });
        let over_depth = self.config.queue_depth_limit.is_some_and(|limit| {
            input
                .windows
                .iter()
                .any(|window| window.queue_depth > limit)
        });
        if over_age || over_depth {
            return Some(BudgetPressure::QueueAge);
        }
        None
    }
}

impl Default for BudgetController {
    fn default() -> Self {
        Self::new(BudgetControllerConfig::default())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BudgetPressure {
    FrameMiss,
    NegativeHeadroom,
    QueueAge,
}

impl BudgetPressure {
    const fn reason(self) -> BudgetDecisionReason {
        match self {
            Self::FrameMiss => BudgetDecisionReason::CutFrameMiss,
            Self::NegativeHeadroom => BudgetDecisionReason::CutNegativeHeadroom,
            Self::QueueAge => BudgetDecisionReason::CutQueueAge,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FamilyState {
    initialized: bool,
    current_units: u32,
    current_period_fraction: f64,
    current_pending_units: Option<u32>,
    clean_windows: u32,
}

impl FamilyState {
    fn floor(config: FamilyBudgetConfig) -> Self {
        Self {
            initialized: false,
            current_units: config.floor_units(),
            current_period_fraction: config.floor_period_fraction(),
            current_pending_units: config.floor_pending_units,
            clean_windows: 0,
        }
    }

    fn reset_to_floor(&mut self, config: FamilyBudgetConfig) {
        self.current_units = config.floor_units();
        self.current_period_fraction = config.floor_period_fraction();
        self.current_pending_units = config.floor_pending_units;
        self.clean_windows = 0;
    }

    fn cut_toward_floor(&mut self, config: FamilyBudgetConfig) {
        self.current_units = halve_toward_floor(self.current_units, config.floor_units());
        self.current_period_fraction = config.floor_period_fraction()
            + (self.current_period_fraction - config.floor_period_fraction()).max(0.0) * 0.5;
        if let (Some(current), Some(floor)) =
            (self.current_pending_units, config.floor_pending_units)
        {
            self.current_pending_units = Some(halve_toward_floor(current, floor));
        }
        self.clean_windows = 0;
    }

    fn raise_or_hold(
        &mut self,
        config: FamilyBudgetConfig,
        raise_after_clean_windows: u32,
    ) -> BudgetDecisionReason {
        if self.current_units >= config.cap_units()
            && self.current_period_fraction >= config.cap_period_fraction()
            && pending_at_cap(self.current_pending_units, config.cap_pending_units)
        {
            self.clean_windows = 0;
            return BudgetDecisionReason::HoldAtCap;
        }
        self.clean_windows = self.clean_windows.saturating_add(1);
        if self.clean_windows < raise_after_clean_windows.max(1) {
            return BudgetDecisionReason::HoldHysteresis;
        }
        self.current_units = self
            .current_units
            .saturating_add(config.raise_units.max(1))
            .min(config.cap_units());
        self.current_period_fraction = (self.current_period_fraction
            + config.raise_period_fraction())
        .min(config.cap_period_fraction());
        if let (Some(current), Some(cap)) = (self.current_pending_units, config.cap_pending_units) {
            self.current_pending_units = Some(
                current
                    .saturating_add(config.raise_pending_units.max(1))
                    .min(cap.max(config.floor_pending_units.unwrap_or(1))),
            );
        }
        self.clean_windows = 0;
        BudgetDecisionReason::RaiseSustainedHeadroom
    }

    fn grant(
        self,
        config: FamilyBudgetConfig,
        target_period_ms: f64,
        per_unit_cost_ms: Option<f64>,
    ) -> BudgetGrantReport {
        let elapsed_ms = sanitize_ms(target_period_ms) * self.current_period_fraction;
        let current_max_units = self.current_units.max(config.floor_units());
        let max_units = per_unit_cost_ms
            .and_then(finite_positive)
            .filter(|_| elapsed_ms > 0.0)
            .map(|cost| {
                let elapsed_units = (elapsed_ms / cost).floor() as u32;
                elapsed_units.clamp(config.floor_units(), current_max_units)
            })
            .unwrap_or(current_max_units);
        BudgetGrantReport::new(
            elapsed_ms,
            config.floor_units(),
            max_units,
            self.current_pending_units,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetSpendObservation {
    pub family: BudgetDecisionFamily,
    pub elapsed_ms: f64,
    pub units: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetLedgerEntryReport {
    pub family: BudgetDecisionFamily,
    pub granted_elapsed_ms: f64,
    pub spent_elapsed_ms: f64,
    pub elapsed_over_grant_ms: f64,
    pub granted_units: u32,
    pub spent_units: u32,
    pub units_over_grant: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetLedgerReport {
    pub entries: Vec<BudgetLedgerEntryReport>,
    pub conservation_violations: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BudgetGrantLedger {
    entries: BTreeMap<BudgetDecisionFamily, LedgerEntry>,
}

impl BudgetGrantLedger {
    pub fn from_panel(panel: &BudgetDecisionPanelReport) -> Self {
        let mut ledger = Self::default();
        for decision in &panel.decisions {
            ledger.entries.insert(
                decision.family,
                LedgerEntry {
                    granted_elapsed_ms: decision.grant.elapsed_ms,
                    granted_units: decision.grant.max_units,
                    ..LedgerEntry::default()
                },
            );
        }
        ledger
    }

    pub fn record_spend(&mut self, observation: BudgetSpendObservation) {
        let entry = self.entries.entry(observation.family).or_default();
        entry.spent_elapsed_ms += sanitize_ms(observation.elapsed_ms);
        entry.spent_units = entry.spent_units.saturating_add(observation.units);
    }

    pub fn report(&self) -> BudgetLedgerReport {
        let mut entries = Vec::with_capacity(self.entries.len());
        let mut conservation_violations = 0_u64;
        for (family, entry) in &self.entries {
            let elapsed_over_grant_ms =
                (entry.spent_elapsed_ms - entry.granted_elapsed_ms).max(0.0);
            let units_over_grant = entry.spent_units.saturating_sub(entry.granted_units);
            if elapsed_over_grant_ms > 0.000_001 || units_over_grant > 0 {
                conservation_violations = conservation_violations.saturating_add(1);
            }
            entries.push(BudgetLedgerEntryReport {
                family: *family,
                granted_elapsed_ms: entry.granted_elapsed_ms,
                spent_elapsed_ms: entry.spent_elapsed_ms,
                elapsed_over_grant_ms,
                granted_units: entry.granted_units,
                spent_units: entry.spent_units,
                units_over_grant,
            });
        }
        BudgetLedgerReport {
            entries,
            conservation_violations,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct LedgerEntry {
    granted_elapsed_ms: f64,
    spent_elapsed_ms: f64,
    granted_units: u32,
    spent_units: u32,
}

fn input_snapshot(
    input: &BudgetControllerInput,
    target_period_ms: f64,
    family: BudgetDecisionFamily,
) -> BudgetInputSnapshotReport {
    let mut snapshot = BudgetInputSnapshotReport::new(target_period_ms);
    snapshot.host_mode = input.host_mode;
    snapshot.catch_up_gameplay_ticks = input.catch_up_gameplay_ticks.max(1);
    snapshot.app_work_p95_ms = max_option(input.windows.iter().filter_map(|w| w.app_work_p95_ms));
    snapshot.headroom_p05_ms = min_option(input.windows.iter().filter_map(|w| w.headroom_p05_ms));
    snapshot.app_over_period_pct = input
        .windows
        .iter()
        .map(|w| w.app_over_period_pct.max(0.0))
        .fold(0.0, f64::max);
    snapshot.missed_frames = input
        .windows
        .iter()
        .map(|w| w.missed_frames.saturating_add(w.over_2x_frames))
        .sum();
    snapshot.dropped_frames_delta = input.windows.iter().map(|w| w.dropped_frames_delta).sum();
    snapshot.stale_frames_delta = input.windows.iter().map(|w| w.stale_frames_delta).sum();
    snapshot.queue_depth = input
        .windows
        .iter()
        .map(|w| w.queue_depth)
        .max()
        .unwrap_or_default();
    snapshot.oldest_queue_age_ms =
        max_option(input.windows.iter().filter_map(|w| w.oldest_queue_age_ms));
    snapshot.per_unit_cost_ms = input.costs.cost_for(family);
    snapshot
}

fn halve_toward_floor(current: u32, floor: u32) -> u32 {
    floor.saturating_add(current.saturating_sub(floor) / 2)
}

fn pending_at_cap(current: Option<u32>, cap: Option<u32>) -> bool {
    match (current, cap) {
        (Some(current), Some(cap)) => current >= cap,
        (None, None) => true,
        _ => false,
    }
}

fn finite_positive(value: f64) -> Option<f64> {
    value.is_finite().then_some(value).filter(|v| *v > 0.0)
}

fn sanitize_ms(ms: f64) -> f64 {
    if ms.is_finite() { ms.max(0.0) } else { 0.0 }
}

fn sanitize_fraction(fraction: f64) -> f64 {
    if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn max_option(values: impl Iterator<Item = f64>) -> Option<f64> {
    values.map(sanitize_ms).reduce(f64::max)
}

fn min_option(values: impl Iterator<Item = f64>) -> Option<f64> {
    values.reduce(f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> BudgetControllerConfig {
        use BudgetDecisionFamily::*;
        use FrameHostKind::*;
        use StageId::*;
        use WorkWindow::*;

        BudgetControllerConfig {
            raise_after_clean_windows: 2,
            over_period_pct_cut_threshold: 0.0,
            queue_age_limit_ms: Some(100.0),
            queue_depth_limit: Some(100),
            families: vec![
                FamilyBudgetConfig {
                    raise_period_fraction: 0.10,
                    ..FamilyBudgetConfig::new(
                        FeaturePublication,
                        BudgetDecisionAddress::new(
                            IntegratedServerRunner,
                            GameplayTick,
                            SchedulerPublication,
                        ),
                        1,
                        4,
                        0.10,
                        0.40,
                    )
                },
                FamilyBudgetConfig {
                    raise_period_fraction: 0.05,
                    ..FamilyBudgetConfig::new(
                        RenderAdmission,
                        BudgetDecisionAddress::new(
                            DesktopFlatWinit,
                            BeforeRender,
                            RenderSectionAdmission,
                        ),
                        1,
                        3,
                        0.05,
                        0.15,
                    )
                },
                FamilyBudgetConfig {
                    raise_period_fraction: 0.0,
                    ..FamilyBudgetConfig::new(
                        RenderCompileWorkers,
                        BudgetDecisionAddress::new(DesktopFlatWinit, WorkerPoll, CpuMeshCompile),
                        1,
                        2,
                        0.0,
                        0.0,
                    )
                },
            ],
        }
    }

    fn clean_input(period_ms: f64) -> BudgetControllerInput {
        BudgetControllerInput::new(period_ms).with_window(
            BudgetTelemetryWindow::new(FrameHostKind::DesktopFlatWinit, WorkWindow::BeforeRender)
                .with_app_work_p95_ms(period_ms * 0.5)
                .with_headroom_p05_ms(period_ms * 0.25),
        )
    }

    fn decision(
        panel: &BudgetDecisionPanelReport,
        family: BudgetDecisionFamily,
    ) -> &BudgetDecisionReport {
        panel
            .decisions
            .iter()
            .find(|decision| decision.family == family)
            .expect("decision family present")
    }

    #[test]
    fn cold_start_uses_exact_fail_safe_floors() {
        let mut controller = BudgetController::new(test_config());
        let panel = controller.decide(&clean_input(10.0));

        let feature = decision(&panel, BudgetDecisionFamily::FeaturePublication);
        assert_eq!(feature.trace.reason, BudgetDecisionReason::ColdStartFloor);
        assert_eq!(feature.grant.min_units, 1);
        assert_eq!(feature.grant.max_units, 1);
        assert_eq!(feature.grant.elapsed_ms, 1.0);

        let workers = decision(&panel, BudgetDecisionFamily::RenderCompileWorkers);
        assert_eq!(workers.grant.min_units, 1);
        assert_eq!(workers.grant.max_units, 1);
        assert_eq!(workers.grant.elapsed_ms, 0.0);
    }

    #[test]
    fn clean_windows_raise_slowly_and_spike_cuts_fast() {
        let mut controller = BudgetController::new(test_config());
        controller.decide(&clean_input(10.0));
        let hold = controller.decide(&clean_input(10.0));
        assert_eq!(
            decision(&hold, BudgetDecisionFamily::FeaturePublication)
                .trace
                .reason,
            BudgetDecisionReason::HoldHysteresis
        );
        assert_eq!(
            decision(&hold, BudgetDecisionFamily::FeaturePublication)
                .grant
                .max_units,
            1
        );

        let raised_once = controller.decide(&clean_input(10.0));
        assert_eq!(
            decision(&raised_once, BudgetDecisionFamily::FeaturePublication)
                .trace
                .reason,
            BudgetDecisionReason::RaiseSustainedHeadroom
        );
        assert_eq!(
            decision(&raised_once, BudgetDecisionFamily::FeaturePublication)
                .grant
                .max_units,
            2
        );
        assert_eq!(
            decision(&raised_once, BudgetDecisionFamily::FeaturePublication)
                .grant
                .elapsed_ms,
            2.0
        );

        controller.decide(&clean_input(10.0));
        let raised_twice = controller.decide(&clean_input(10.0));
        assert_eq!(
            decision(&raised_twice, BudgetDecisionFamily::FeaturePublication)
                .grant
                .max_units,
            3
        );

        let bad_input = BudgetControllerInput::new(10.0).with_window(
            BudgetTelemetryWindow::new(FrameHostKind::DesktopFlatWinit, WorkWindow::BeforeRender)
                .with_missed_frames(1),
        );
        let cut = controller.decide(&bad_input);
        let feature = decision(&cut, BudgetDecisionFamily::FeaturePublication);
        assert_eq!(feature.trace.reason, BudgetDecisionReason::CutFrameMiss);
        assert_eq!(feature.grant.max_units, 2);
        assert_eq!(feature.grant.elapsed_ms, 2.0);
    }

    #[test]
    fn steady_clean_input_holds_at_cap_without_oscillation() {
        let mut controller = BudgetController::new(test_config());
        let mut last = controller.decide(&clean_input(10.0));
        for _ in 0..16 {
            last = controller.decide(&clean_input(10.0));
        }
        let feature = decision(&last, BudgetDecisionFamily::FeaturePublication);
        assert_eq!(feature.grant.max_units, 4);
        assert_eq!(feature.grant.elapsed_ms, 4.0);
        assert!(matches!(
            feature.trace.reason,
            BudgetDecisionReason::HoldAtCap | BudgetDecisionReason::RaiseSustainedHeadroom
        ));
    }

    #[test]
    fn target_period_change_resets_estimators_to_floor() {
        let mut controller = BudgetController::new(test_config());
        controller.decide(&clean_input(10.0));
        controller.decide(&clean_input(10.0));
        controller.decide(&clean_input(10.0));
        assert_eq!(
            decision(
                &controller.decide(&clean_input(10.0)),
                BudgetDecisionFamily::FeaturePublication
            )
            .grant
            .max_units,
            2
        );

        let reset = controller.decide(&clean_input(20.0));
        let feature = decision(&reset, BudgetDecisionFamily::FeaturePublication);
        assert_eq!(
            feature.trace.reason,
            BudgetDecisionReason::TargetPeriodChangedFloor
        );
        assert_eq!(feature.grant.max_units, 1);
        assert_eq!(feature.grant.elapsed_ms, 2.0);
    }

    #[test]
    fn missing_input_degrades_to_floor() {
        let mut controller = BudgetController::new(test_config());
        controller.decide(&clean_input(10.0));
        controller.decide(&clean_input(10.0));
        controller.decide(&clean_input(10.0));

        let missing = controller.decide(&BudgetControllerInput::new(10.0));
        let feature = decision(&missing, BudgetDecisionFamily::FeaturePublication);
        assert_eq!(
            feature.trace.reason,
            BudgetDecisionReason::MissingInputFloor
        );
        assert_eq!(feature.grant.max_units, 1);
    }

    #[test]
    fn period_relative_elapsed_budget_scales_with_target_period() {
        let mut controller = BudgetController::new(test_config());
        let first = controller.decide(&clean_input(8.0));
        assert_eq!(
            decision(&first, BudgetDecisionFamily::FeaturePublication)
                .grant
                .elapsed_ms,
            0.8
        );

        let reset = controller.decide(&clean_input(16.0));
        assert_eq!(
            decision(&reset, BudgetDecisionFamily::FeaturePublication)
                .grant
                .elapsed_ms,
            1.6
        );
    }

    #[test]
    fn measured_unit_cost_caps_grant_to_elapsed_budget() {
        let mut controller = BudgetController::new(test_config());
        for _ in 0..8 {
            controller.decide(&clean_input(10.0));
        }

        let with_cost = clean_input(10.0).with_costs(BudgetCostEstimates {
            feature_publish_ms: Some(1.5),
            ..BudgetCostEstimates::default()
        });
        let panel = controller.decide(&with_cost);
        let feature = decision(&panel, BudgetDecisionFamily::FeaturePublication);

        assert_eq!(feature.grant.elapsed_ms, 4.0);
        assert_eq!(feature.grant.max_units, 2);
        assert_eq!(feature.trace.input_snapshot.per_unit_cost_ms, Some(1.5));
    }

    #[test]
    fn queue_age_cuts_without_frame_miss() {
        let mut controller = BudgetController::new(test_config());
        controller.decide(&clean_input(10.0));
        controller.decide(&clean_input(10.0));
        controller.decide(&clean_input(10.0));

        let queued = BudgetControllerInput::new(10.0).with_window(
            BudgetTelemetryWindow::new(FrameHostKind::DesktopFlatWinit, WorkWindow::BeforeRender)
                .with_headroom_p05_ms(1.0)
                .with_queue_age_ms(10, 150.0),
        );
        let panel = controller.decide(&queued);
        assert_eq!(
            decision(&panel, BudgetDecisionFamily::FeaturePublication)
                .trace
                .reason,
            BudgetDecisionReason::CutQueueAge
        );
    }

    #[test]
    fn render_frame_budget_config_addresses_lane_without_changing_floors() {
        let config = render_frame_budget_controller_config(
            FrameHostKind::AndroidXrOpenXr,
            WorkWindow::PostSubmitOverlapSlack,
        );
        let mut controller = BudgetController::new(config);
        let panel = controller.decide(&BudgetControllerInput::new(11.1));

        let render = decision(&panel, BudgetDecisionFamily::RenderAdmission);
        assert_eq!(render.address.host_kind, FrameHostKind::AndroidXrOpenXr);
        assert_eq!(
            render.address.work_window,
            WorkWindow::PostSubmitOverlapSlack
        );
        assert_eq!(render.address.stage, StageId::RenderSectionAdmission);
        assert_eq!(render.grant.min_units, 1);
        assert_eq!(render.grant.max_units, 1);

        let workers = decision(&panel, BudgetDecisionFamily::RenderCompileWorkers);
        assert_eq!(workers.address.host_kind, FrameHostKind::AndroidXrOpenXr);
        assert_eq!(workers.address.work_window, WorkWindow::WorkerPoll);
        assert_eq!(workers.address.stage, StageId::CpuMeshCompile);
        assert_eq!(workers.grant.min_units, 1);
        assert_eq!(workers.grant.max_units, 1);
    }

    #[test]
    fn render_compile_capacity_missing_inputs_stays_on_shipped_floors() {
        let report = derive_render_compile_capacity(RenderCompileCapacityInput::new(
            None,
            None,
            RenderCompileThreadReservation::local_integrated_flat(),
            RenderCompileMeshFootprint::default(),
        ));

        assert_eq!(report.derived_worker_count, 1);
        assert_eq!(report.derived_max_pending_jobs, 4);
        assert_eq!(
            report.fallback_reason,
            Some(RenderCompileCapacityFallbackReason::MissingParallelism)
        );
    }

    #[test]
    fn render_compile_capacity_derives_from_cpu_reservation_and_memory() {
        let footprint = RenderCompileMeshFootprint {
            compile_request_bytes: Some(256 * 1024),
            mesh_vertex_bytes: Some(512 * 1024),
            mesh_index_bytes: Some(128 * 1024),
        };
        let report = derive_render_compile_capacity(RenderCompileCapacityInput::new(
            Some(12),
            Some(8 * 1024 * 1024 * 1024),
            RenderCompileThreadReservation::local_integrated_flat(),
            footprint,
        ));

        assert_eq!(report.reserved_parallelism, 4);
        assert_eq!(report.usable_parallelism, Some(8));
        assert_eq!(report.cpu_worker_cap, 7);
        assert_eq!(report.derived_worker_count, 7);
        assert!(report.derived_max_pending_jobs >= report.derived_worker_count);
        assert_eq!(report.fallback_reason, None);
    }

    #[test]
    fn render_compile_capacity_memory_can_limit_workers_but_not_pending_floor() {
        let footprint = RenderCompileMeshFootprint {
            compile_request_bytes: Some(128 * 1024),
            mesh_vertex_bytes: Some(128 * 1024),
            mesh_index_bytes: Some(64 * 1024),
        };
        let guarded_pack = footprint
            .total_pack_bytes()
            .unwrap()
            .saturating_mul(VANILLA_RENDER_COMPILE_BUFFER_SAFETY_MULTIPLIER);
        let two_pack_budget = guarded_pack * 3;
        let total_memory =
            (two_pack_budget as f64 / VANILLA_RENDER_COMPILE_MEMORY_BUDGET_FRACTION) as u64;
        let report = derive_render_compile_capacity(RenderCompileCapacityInput::new(
            Some(16),
            Some(total_memory),
            RenderCompileThreadReservation::local_integrated_flat(),
            footprint,
        ));

        assert_eq!(report.memory_pack_cap, Some(2));
        assert_eq!(report.derived_worker_count, 2);
        assert_eq!(report.derived_max_pending_jobs, 4);
    }

    #[test]
    fn ewma_cost_estimator_updates_per_unit_costs() {
        let mut estimator = EwmaCostEstimator::new(0.5);
        estimator.observe(BudgetDecisionFamily::FeaturePublication, 4.0, 2);
        assert_eq!(estimator.estimates.feature_publish_ms, Some(2.0));
        estimator.observe(BudgetDecisionFamily::FeaturePublication, 6.0, 2);
        assert_eq!(estimator.estimates.feature_publish_ms, Some(2.5));
    }

    #[test]
    fn ewma_cost_estimator_resets_on_target_period_change() {
        let mut estimator = EwmaCostEstimator::new(0.5);
        estimator.observe_for_target_period(10.0, BudgetDecisionFamily::FeaturePublication, 4.0, 2);
        estimator.observe_for_target_period(10.0, BudgetDecisionFamily::FeaturePublication, 6.0, 2);
        assert_eq!(estimator.estimates.feature_publish_ms, Some(2.5));

        estimator.observe_for_target_period(20.0, BudgetDecisionFamily::FeaturePublication, 8.0, 2);
        assert_eq!(estimator.target_period_ms, Some(20.0));
        assert_eq!(estimator.estimates.feature_publish_ms, Some(4.0));
    }

    #[test]
    fn catch_up_ticks_do_not_multiply_host_frame_grant() {
        let mut normal = BudgetController::new(test_config());
        let normal_panel = normal.decide(&clean_input(10.0));

        let mut bunched = BudgetController::new(test_config());
        let mut input = clean_input(10.0);
        input.catch_up_gameplay_ticks = 4;
        let bunched_panel = bunched.decide(&input);

        let normal_feature = decision(&normal_panel, BudgetDecisionFamily::FeaturePublication);
        let bunched_feature = decision(&bunched_panel, BudgetDecisionFamily::FeaturePublication);
        assert_eq!(
            bunched_feature.grant.max_units,
            normal_feature.grant.max_units
        );
        assert_eq!(
            bunched_feature.grant.elapsed_ms,
            normal_feature.grant.elapsed_ms
        );
        assert_eq!(
            bunched_feature.trace.input_snapshot.catch_up_gameplay_ticks,
            4
        );
    }

    #[test]
    fn ledger_reports_granted_vs_spent_conservation() {
        let mut controller = BudgetController::new(test_config());
        let panel = controller.decide(&clean_input(10.0));
        let mut ledger = BudgetGrantLedger::from_panel(&panel);
        ledger.record_spend(BudgetSpendObservation {
            family: BudgetDecisionFamily::FeaturePublication,
            elapsed_ms: 0.5,
            units: 1,
        });
        assert_eq!(ledger.report().conservation_violations, 0);

        ledger.record_spend(BudgetSpendObservation {
            family: BudgetDecisionFamily::FeaturePublication,
            elapsed_ms: 0.75,
            units: 1,
        });
        let report = ledger.report();
        let feature = report
            .entries
            .iter()
            .find(|entry| entry.family == BudgetDecisionFamily::FeaturePublication)
            .expect("feature ledger entry");
        assert_eq!(report.conservation_violations, 1);
        assert_eq!(feature.spent_units, 2);
        assert_eq!(feature.units_over_grant, 1);
        assert_eq!(feature.elapsed_over_grant_ms, 0.25);
    }
}
