use serde::{Deserialize, Serialize};

use crate::DiagnosticLaneAvailability;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PeerThreadId {
    ServerRunner,
    Worldgen,
    LightStatus,
    RenderCompileWorkers,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerThreadActivityReport {
    pub lane: PeerThreadId,
    #[serde(default)]
    pub availability: DiagnosticLaneAvailability,
    pub active: bool,
    pub pending_jobs: u64,
    pub request_frames: u64,
    pub response_frames: u64,
    pub request_bytes: u64,
    pub response_bytes: u64,
    pub max_pending_frames: u64,
    pub busy_ms: Option<f64>,
    pub idle_ms: Option<f64>,
    pub last_request_ms: Option<f64>,
    pub total_request_ms: f64,
    pub max_request_ms: f64,
    pub conservation_violations: u64,
}

impl PeerThreadActivityReport {
    pub fn new(lane: PeerThreadId) -> Self {
        Self {
            lane,
            availability: DiagnosticLaneAvailability::Local,
            active: false,
            pending_jobs: 0,
            request_frames: 0,
            response_frames: 0,
            request_bytes: 0,
            response_bytes: 0,
            max_pending_frames: 0,
            busy_ms: None,
            idle_ms: None,
            last_request_ms: None,
            total_request_ms: 0.0,
            max_request_ms: 0.0,
            conservation_violations: 0,
        }
    }

    pub fn with_pending_jobs(mut self, pending_jobs: u64) -> Self {
        self.pending_jobs = pending_jobs;
        self.active |= pending_jobs > 0;
        self
    }

    pub fn with_frames(mut self, request_frames: u64, response_frames: u64) -> Self {
        self.request_frames = request_frames;
        self.response_frames = response_frames;
        self.active |= request_frames > 0 || response_frames > 0;
        self
    }

    pub fn with_bytes(mut self, request_bytes: u64, response_bytes: u64) -> Self {
        self.request_bytes = request_bytes;
        self.response_bytes = response_bytes;
        self.active |= request_bytes > 0 || response_bytes > 0;
        self
    }

    pub fn with_max_pending_frames(mut self, max_pending_frames: u64) -> Self {
        self.max_pending_frames = max_pending_frames;
        self.active |= max_pending_frames > 0;
        self
    }

    pub fn with_busy_idle_ms(mut self, busy_ms: Option<f64>, idle_ms: Option<f64>) -> Self {
        self.busy_ms = busy_ms.map(sanitize_ms);
        self.idle_ms = idle_ms.map(sanitize_ms);
        self.active |= self.busy_ms.unwrap_or(0.0) > 0.0 || self.idle_ms.unwrap_or(0.0) > 0.0;
        self
    }

    pub fn with_request_timing_ms(
        mut self,
        last_request_ms: Option<f64>,
        total_request_ms: f64,
        max_request_ms: f64,
    ) -> Self {
        self.last_request_ms = last_request_ms.map(sanitize_ms);
        self.total_request_ms = sanitize_ms(total_request_ms);
        self.max_request_ms = sanitize_ms(max_request_ms);
        self.active |= self.last_request_ms.unwrap_or(0.0) > 0.0
            || self.total_request_ms > 0.0
            || self.max_request_ms > 0.0;
        self
    }

    pub fn with_conservation_violations(mut self, conservation_violations: u64) -> Self {
        self.conservation_violations = conservation_violations;
        self
    }

    pub fn with_availability(mut self, availability: DiagnosticLaneAvailability) -> Self {
        self.availability = availability;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerThreadPanelReport {
    pub schema_version: u32,
    pub peers: Vec<PeerThreadActivityReport>,
}

impl PeerThreadPanelReport {
    pub fn new(peers: Vec<PeerThreadActivityReport>) -> Self {
        Self {
            schema_version: crate::FRAME_PIPELINE_SCHEMA_VERSION,
            peers,
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::new())
    }
}

fn sanitize_ms(ms: f64) -> f64 {
    if ms.is_finite() { ms.max(0.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_activity_active_when_work_observed() {
        let report = PeerThreadActivityReport::new(PeerThreadId::Worldgen)
            .with_frames(2, 1)
            .with_request_timing_ms(Some(1.25), 3.0, 2.0);
        assert!(report.active);
        assert_eq!(report.request_frames, 2);
        assert_eq!(report.response_frames, 1);
        assert_eq!(report.last_request_ms, Some(1.25));
    }

    #[test]
    fn peer_activity_can_mark_remote_host_ownership() {
        let report = PeerThreadActivityReport::new(PeerThreadId::ServerRunner)
            .with_availability(DiagnosticLaneAvailability::RemoteHost);

        assert_eq!(report.availability, DiagnosticLaneAvailability::RemoteHost);
        assert!(!report.active);
    }

    #[test]
    fn peer_activity_deserializes_missing_availability_as_local() {
        let report: PeerThreadActivityReport = serde_json::from_str(
            r#"{
                "lane": "server-runner",
                "active": false,
                "pendingJobs": 0,
                "requestFrames": 0,
                "responseFrames": 0,
                "requestBytes": 0,
                "responseBytes": 0,
                "maxPendingFrames": 0,
                "busyMs": null,
                "idleMs": null,
                "lastRequestMs": null,
                "totalRequestMs": 0.0,
                "maxRequestMs": 0.0,
                "conservationViolations": 0
            }"#,
        )
        .expect("peer report without availability");

        assert_eq!(report.availability, DiagnosticLaneAvailability::Local);
    }
}
