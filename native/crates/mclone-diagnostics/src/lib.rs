#![deny(unsafe_op_in_unsafe_fn)]

pub mod clock;
mod frame;
mod queue;
mod schema;
mod stage;

pub use frame::{
    FrameAccountingConfig, FrameAccumulator, FrameObservation, FrameSummaryReport, HeadroomSummary,
    OverBudgetTiers, PercentileMethod, PercentileRing, PercentileSummary, WorstFrameDetail,
    percentile_sorted_ms,
};
pub use queue::{QueueAgeReport, QueueAgeTracker, QueueId, QueuePanelReport};
pub use schema::{FRAME_PIPELINE_SCHEMA_VERSION, FramePipelineReport};
pub use stage::{CriticalPathLabel, StageId, StageSpan};
