#![deny(unsafe_op_in_unsafe_fn)]

pub mod clock;
mod frame;
mod gpu;
pub mod legacy_json_keys;
mod peer;
mod queue;
mod schema;
mod stage;

pub use frame::{
    ConservationViolationCounts, FrameAccountingConfig, FrameAccumulator, FrameObservation,
    FrameSummaryReport, HeadroomSummary, OverBudgetTiers, PercentileMethod, PercentileRing,
    PercentileSummary, WorstFrameDetail, percentile_sorted_ms,
};
pub use gpu::{GpuPassId, GpuTimestampPanelReport, GpuTimestampPassReport};
pub use peer::{PeerThreadActivityReport, PeerThreadId, PeerThreadPanelReport};
pub use queue::{QueueAgeReport, QueueAgeTracker, QueueId, QueuePanelReport};
pub use schema::{FRAME_PIPELINE_SCHEMA_VERSION, FramePipelineReport};
pub use stage::{CriticalPathLabel, StageId, StageSpan};
