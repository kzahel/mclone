use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::DiagnosticLaneAvailability;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueueId {
    InboundUpdates,
    CompletedRenderResults,
    UploadWork,
    HostPublication,
    HostPublicationRunner,
    HostPublicationWorldgen,
    HostPublicationLight,
    RenderCompileJobs,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueAgeReport {
    pub queue: QueueId,
    #[serde(default)]
    pub availability: DiagnosticLaneAvailability,
    pub enqueued_total: u64,
    pub dequeued_total: u64,
    pub depth: u64,
    pub oldest_age_ms: Option<f64>,
    pub max_oldest_age_ms: f64,
    pub conservation_violations: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuePanelReport {
    pub schema_version: u32,
    pub queues: Vec<QueueAgeReport>,
}

impl QueuePanelReport {
    pub fn new(queues: Vec<QueueAgeReport>) -> Self {
        Self {
            schema_version: crate::FRAME_PIPELINE_SCHEMA_VERSION,
            queues,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueueAgeTracker {
    queue: QueueId,
    enqueued_total: u64,
    dequeued_total: u64,
    enqueue_times_ms: VecDeque<f64>,
    max_oldest_age_ms: f64,
    conservation_violations: u64,
}

impl QueueAgeTracker {
    pub fn new(queue: QueueId) -> Self {
        Self {
            queue,
            enqueued_total: 0,
            dequeued_total: 0,
            enqueue_times_ms: VecDeque::new(),
            max_oldest_age_ms: 0.0,
            conservation_violations: 0,
        }
    }

    pub fn enqueue(&mut self, count: u64, now_ms: f64) {
        let now_ms = sanitize_ms(now_ms);
        self.enqueued_total = self.enqueued_total.saturating_add(count);
        for _ in 0..count {
            self.enqueue_times_ms.push_back(now_ms);
        }
    }

    pub fn dequeue(&mut self, count: u64, now_ms: f64) {
        let available = self.enqueue_times_ms.len() as u64;
        if count > available {
            self.conservation_violations = self.conservation_violations.saturating_add(1);
        }
        let actual = count.min(available);
        self.dequeued_total = self.dequeued_total.saturating_add(actual);
        for _ in 0..actual {
            self.enqueue_times_ms.pop_front();
        }
        self.observe_oldest_age(now_ms);
    }

    pub fn reconcile_depth(&mut self, depth: u64, now_ms: f64) {
        let actual = self.enqueue_times_ms.len() as u64;
        if depth > actual {
            self.enqueue(depth - actual, now_ms);
        } else if actual > depth {
            self.dequeue(actual - depth, now_ms);
        } else {
            self.observe_oldest_age(now_ms);
        }
    }

    pub fn observe_oldest_age(&mut self, now_ms: f64) -> Option<f64> {
        let age = self
            .enqueue_times_ms
            .front()
            .map(|queued_at| (sanitize_ms(now_ms) - *queued_at).max(0.0));
        if let Some(age) = age {
            self.max_oldest_age_ms = self.max_oldest_age_ms.max(age);
        }
        age
    }

    pub fn report(&mut self, now_ms: f64) -> QueueAgeReport {
        let oldest_age_ms = self.observe_oldest_age(now_ms);
        let expected_depth = self.enqueued_total.saturating_sub(self.dequeued_total);
        let actual_depth = self.enqueue_times_ms.len() as u64;
        if expected_depth != actual_depth {
            self.conservation_violations = self.conservation_violations.saturating_add(1);
        }
        QueueAgeReport {
            queue: self.queue.clone(),
            availability: DiagnosticLaneAvailability::Local,
            enqueued_total: self.enqueued_total,
            dequeued_total: self.dequeued_total,
            depth: actual_depth,
            oldest_age_ms,
            max_oldest_age_ms: self.max_oldest_age_ms,
            conservation_violations: self.conservation_violations,
        }
    }
}

impl QueueAgeReport {
    pub fn snapshot(
        queue: QueueId,
        enqueued_total: u64,
        dequeued_total: u64,
        depth: u64,
        oldest_age_ms: Option<f64>,
        max_oldest_age_ms: f64,
        conservation_violations: u64,
    ) -> Self {
        Self {
            queue,
            availability: DiagnosticLaneAvailability::Local,
            enqueued_total,
            dequeued_total,
            depth,
            oldest_age_ms: oldest_age_ms.map(sanitize_ms),
            max_oldest_age_ms: sanitize_ms(max_oldest_age_ms),
            conservation_violations,
        }
    }

    pub fn with_availability(mut self, availability: DiagnosticLaneAvailability) -> Self {
        self.availability = availability;
        self
    }
}

fn sanitize_ms(ms: f64) -> f64 {
    if ms.is_finite() { ms.max(0.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_age_tracks_depth_and_oldest_age() {
        let mut queue = QueueAgeTracker::new(QueueId::InboundUpdates);
        queue.enqueue(2, 100.0);
        let report = queue.report(125.0);
        assert_eq!(report.enqueued_total, 2);
        assert_eq!(report.dequeued_total, 0);
        assert_eq!(report.depth, 2);
        assert_eq!(report.oldest_age_ms, Some(25.0));

        queue.dequeue(1, 130.0);
        let report = queue.report(140.0);
        assert_eq!(report.enqueued_total, 2);
        assert_eq!(report.dequeued_total, 1);
        assert_eq!(report.depth, 1);
        assert_eq!(report.oldest_age_ms, Some(40.0));
        assert_eq!(report.max_oldest_age_ms, 40.0);
        assert_eq!(report.conservation_violations, 0);
    }

    #[test]
    fn queue_age_counts_over_dequeue_as_violation_without_breaking_conservation() {
        let mut queue = QueueAgeTracker::new(QueueId::UploadWork);
        queue.enqueue(1, 0.0);
        queue.dequeue(3, 1.0);
        let report = queue.report(2.0);
        assert_eq!(report.enqueued_total, 1);
        assert_eq!(report.dequeued_total, 1);
        assert_eq!(report.depth, 0);
        assert_eq!(report.conservation_violations, 1);
    }

    #[test]
    fn queue_age_reconciles_sampled_depth() {
        let mut queue = QueueAgeTracker::new(QueueId::RenderCompileJobs);
        queue.reconcile_depth(3, 10.0);
        queue.reconcile_depth(1, 20.0);
        let report = queue.report(25.0);
        assert_eq!(report.enqueued_total, 3);
        assert_eq!(report.dequeued_total, 2);
        assert_eq!(report.depth, 1);
        assert_eq!(report.oldest_age_ms, Some(15.0));
    }

    #[test]
    fn queue_report_can_mark_remote_host_ownership() {
        let report = QueueAgeReport::snapshot(QueueId::HostPublication, 0, 0, 0, None, 0.0, 0)
            .with_availability(DiagnosticLaneAvailability::RemoteHost);

        assert_eq!(report.availability, DiagnosticLaneAvailability::RemoteHost);
        assert_eq!(report.depth, 0);
    }

    #[test]
    fn queue_report_deserializes_missing_availability_as_local() {
        let report: QueueAgeReport = serde_json::from_str(
            r#"{
                "queue": "host-publication",
                "enqueuedTotal": 0,
                "dequeuedTotal": 0,
                "depth": 0,
                "oldestAgeMs": null,
                "maxOldestAgeMs": 0.0,
                "conservationViolations": 0
            }"#,
        )
        .expect("queue report without availability");

        assert_eq!(report.availability, DiagnosticLaneAvailability::Local);
    }
}
