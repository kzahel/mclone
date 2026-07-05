use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CriticalPathLabel {
    CurrentFrameCritical,
    NextFrameSlack,
    ParallelCpuPeer,
    RemoteHostWork,
    QueuedBackpressured,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StageId {
    InputPoseEvents,
    HostSessionCommands,
    TerrainGeneration,
    LightComputeStatus,
    SchedulerPublication,
    TransportDecode,
    ClientUpdateApply,
    RenderSectionAdmission,
    RenderAdmissionDirtyReadyScan,
    RenderAdmissionRequestBuild,
    RenderAdmissionWorkerSubmit,
    RenderAdmissionPreparedRecordMaintenance,
    CpuMeshCompile,
    CompletedResultAcceptance,
    GpuUpload,
    UploadApply,
    PreparedDrawRecords,
    DrawEncode,
    GpuExecutionPresentationWait,
    UiDebug,
}

impl StageId {
    pub const fn default_label(self) -> CriticalPathLabel {
        match self {
            Self::InputPoseEvents
            | Self::ClientUpdateApply
            | Self::GpuUpload
            | Self::UploadApply
            | Self::PreparedDrawRecords
            | Self::DrawEncode
            | Self::GpuExecutionPresentationWait => CriticalPathLabel::CurrentFrameCritical,
            Self::RenderSectionAdmission
            | Self::RenderAdmissionDirtyReadyScan
            | Self::RenderAdmissionRequestBuild
            | Self::RenderAdmissionWorkerSubmit
            | Self::RenderAdmissionPreparedRecordMaintenance
            | Self::CompletedResultAcceptance
            | Self::UiDebug => CriticalPathLabel::NextFrameSlack,
            Self::TerrainGeneration | Self::LightComputeStatus | Self::CpuMeshCompile => {
                CriticalPathLabel::ParallelCpuPeer
            }
            Self::HostSessionCommands | Self::SchedulerPublication => {
                CriticalPathLabel::RemoteHostWork
            }
            Self::TransportDecode => CriticalPathLabel::QueuedBackpressured,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageSpan {
    pub stage: StageId,
    pub label: CriticalPathLabel,
    pub elapsed_ms: f64,
    pub thread_cpu_ms: Option<f64>,
}

impl StageSpan {
    pub fn new(stage: StageId, elapsed_ms: f64) -> Self {
        Self {
            stage,
            label: stage.default_label(),
            elapsed_ms: sanitize_ms(elapsed_ms),
            thread_cpu_ms: None,
        }
    }

    pub fn with_label(mut self, label: CriticalPathLabel) -> Self {
        self.label = label;
        self
    }

    pub fn with_thread_cpu_ms(mut self, thread_cpu_ms: Option<f64>) -> Self {
        self.thread_cpu_ms = thread_cpu_ms.map(sanitize_ms);
        self
    }
}

fn sanitize_ms(ms: f64) -> f64 {
    if ms.is_finite() { ms.max(0.0) } else { 0.0 }
}
