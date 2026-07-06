use serde::{Deserialize, Serialize};

use crate::{FrameHostKind, StageId, WorkWindow};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetDecisionFamily {
    FeaturePublication,
    LightPublication,
    FeatureJobAdmission,
    RenderAdmission,
    CompletedResultAcceptance,
    SectionUpload,
    RenderCompileWorkers,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetHostMode {
    LocalIntegrated,
    RemoteHost,
    DedicatedServer,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetDecisionAddress {
    pub host_kind: FrameHostKind,
    pub work_window: WorkWindow,
    pub stage: StageId,
}

impl BudgetDecisionAddress {
    pub const fn new(host_kind: FrameHostKind, work_window: WorkWindow, stage: StageId) -> Self {
        Self {
            host_kind,
            work_window,
            stage,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetDecisionReason {
    ColdStartFloor,
    MissingInputFloor,
    TargetPeriodChangedFloor,
    CutFrameMiss,
    CutNegativeHeadroom,
    CutQueueAge,
    RaiseSustainedHeadroom,
    HoldHysteresis,
    HoldAtCap,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetGrantReport {
    pub elapsed_ms: f64,
    pub min_units: u32,
    pub max_units: u32,
    pub max_pending_units: Option<u32>,
}

impl BudgetGrantReport {
    pub fn new(
        elapsed_ms: f64,
        min_units: u32,
        max_units: u32,
        max_pending_units: Option<u32>,
    ) -> Self {
        Self {
            elapsed_ms: sanitize_ms(elapsed_ms),
            min_units,
            max_units: max_units.max(min_units),
            max_pending_units,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetInputSnapshotReport {
    pub host_mode: BudgetHostMode,
    pub catch_up_gameplay_ticks: u32,
    pub target_period_ms: f64,
    pub app_work_p95_ms: Option<f64>,
    pub headroom_p05_ms: Option<f64>,
    pub app_over_period_pct: f64,
    pub missed_frames: u64,
    pub dropped_frames_delta: u64,
    pub stale_frames_delta: u64,
    pub queue_depth: u64,
    pub oldest_queue_age_ms: Option<f64>,
    pub per_unit_cost_ms: Option<f64>,
}

impl BudgetInputSnapshotReport {
    pub fn new(target_period_ms: f64) -> Self {
        Self {
            host_mode: BudgetHostMode::LocalIntegrated,
            catch_up_gameplay_ticks: 1,
            target_period_ms: sanitize_ms(target_period_ms),
            app_work_p95_ms: None,
            headroom_p05_ms: None,
            app_over_period_pct: 0.0,
            missed_frames: 0,
            dropped_frames_delta: 0,
            stale_frames_delta: 0,
            queue_depth: 0,
            oldest_queue_age_ms: None,
            per_unit_cost_ms: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetDecisionTraceReport {
    pub reason: BudgetDecisionReason,
    pub input_snapshot: BudgetInputSnapshotReport,
    pub previous_max_units: u32,
    pub consecutive_clean_windows: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetDecisionReport {
    pub family: BudgetDecisionFamily,
    pub address: BudgetDecisionAddress,
    pub grant: BudgetGrantReport,
    pub trace: BudgetDecisionTraceReport,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetDecisionPanelReport {
    pub schema_version: u32,
    pub decisions: Vec<BudgetDecisionReport>,
}

impl BudgetDecisionPanelReport {
    pub fn new(decisions: Vec<BudgetDecisionReport>) -> Self {
        Self {
            schema_version: crate::FRAME_PIPELINE_SCHEMA_VERSION,
            decisions,
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::new())
    }
}

impl Default for BudgetDecisionPanelReport {
    fn default() -> Self {
        Self::empty()
    }
}

fn sanitize_ms(ms: f64) -> f64 {
    if ms.is_finite() { ms.max(0.0) } else { 0.0 }
}
