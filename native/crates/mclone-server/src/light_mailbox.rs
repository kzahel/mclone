//! Light-status mailbox ownership.
//!
//! Native desktop computes `ChunkStatus::Light` payloads off the foreground
//! scheduler path, mirroring the reference split where `ThreadedLevelLightEngine`
//! keeps propagation work outside `ServerChunkCache`'s immediate tick body. WASM
//! keeps an inline backend until worker plumbing exists there.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
};

use mclone_core::{ChunkPos, ChunkSnapshot, PackedLightSection};

#[cfg(target_arch = "wasm32")]
use crate::WasmServerJobWorkerConfig;
#[cfg(target_arch = "wasm32")]
use crate::job_codec::{
    ServerJobActorKind, decode_light_status_response, encode_light_status_request,
    encode_light_status_unload_request,
};
use crate::level_light_bridge::LevelLightComputationTiming;
#[cfg(any(not(target_arch = "wasm32"), test))]
use crate::light_status::PendingLightStatus;
use crate::light_status::{
    LightRequestToken, PendingLightStatusBatch, snapshot_heap_bytes_estimate,
    ticks_heap_bytes_estimate,
};
use crate::light_world::RetainedInitialLightState;
use crate::persistence::ScheduledTickRecord;
#[cfg(not(target_arch = "wasm32"))]
use crate::timing::TimingSample;
use crate::timing::{timing_elapsed_us, timing_start};
#[cfg(target_arch = "wasm32")]
use crate::wasm_job_worker::WasmJobWorker;
use crate::{LightStatusMailboxKind, LightStatusMailboxMetrics, WorkerFrameMetrics};

pub(crate) const DEFAULT_MAX_ADMITTED_LIGHT_STATUSES: usize = 18;
pub(crate) const DEFAULT_MAX_ADMITTED_LIGHT_OWNED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct CompletedLightStatus {
    pub(crate) token: LightRequestToken,
    pub(crate) pos: ChunkPos,
    pub(crate) feature_snapshot: ChunkSnapshot,
    pub(crate) scheduled_block_ticks: Vec<ScheduledTickRecord>,
    pub(crate) scheduled_fluid_ticks: Vec<ScheduledTickRecord>,
    pub(crate) light_sections: Vec<PackedLightSection>,
    pub(crate) batch_compute_leader: bool,
    pub(crate) compute_us: u128,
    pub(crate) timing: LevelLightComputationTiming,
    pub(crate) cancelled: bool,
}

impl CompletedLightStatus {
    pub(crate) fn from_batch(
        light_state: &mut RetainedInitialLightState,
        batch: PendingLightStatusBatch,
    ) -> Vec<Self> {
        let start = timing_start();
        let completed = light_state.compute_batch(batch);
        let compute_us = timing_elapsed_us(start);
        completed
            .into_iter()
            .enumerate()
            .map(|(index, (pending, light_sections, timing))| {
                let batch_compute_leader = index == 0;
                Self {
                    token: pending.token,
                    pos: pending.pos,
                    feature_snapshot: pending.feature_snapshot,
                    scheduled_block_ticks: pending.scheduled_block_ticks,
                    scheduled_fluid_ticks: pending.scheduled_fluid_ticks,
                    light_sections,
                    batch_compute_leader,
                    compute_us: if batch_compute_leader { compute_us } else { 0 },
                    timing,
                    cancelled: false,
                }
            })
            .collect()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn cancelled(pending: PendingLightStatus) -> Self {
        Self {
            token: pending.token,
            pos: pending.pos,
            feature_snapshot: pending.feature_snapshot,
            scheduled_block_ticks: pending.scheduled_block_ticks,
            scheduled_fluid_ticks: pending.scheduled_fluid_ticks,
            light_sections: Vec::new(),
            batch_compute_leader: false,
            compute_us: 0,
            timing: LevelLightComputationTiming::default(),
            cancelled: true,
        }
    }

    pub(crate) fn owned_bytes_estimate(&self) -> usize {
        let light_bytes = self.light_sections.iter().fold(
            self.light_sections
                .capacity()
                .saturating_mul(std::mem::size_of::<PackedLightSection>()),
            |bytes, section| {
                bytes
                    .saturating_add(section.sky.as_ref().map_or(0, |values| values.capacity()))
                    .saturating_add(section.block.as_ref().map_or(0, |values| values.capacity()))
            },
        );
        std::mem::size_of_val(self)
            .saturating_add(snapshot_heap_bytes_estimate(&self.feature_snapshot))
            .saturating_add(ticks_heap_bytes_estimate(&self.scheduled_block_ticks))
            .saturating_add(ticks_heap_bytes_estimate(&self.scheduled_fluid_ticks))
            .saturating_add(light_bytes)
    }
}

pub(crate) struct LightStatusMailbox {
    backend: LightStatusMailboxBackend,
    pending_count: usize,
    pending_owned_bytes: usize,
    max_pending_owned_bytes: usize,
    max_admitted_statuses: usize,
    max_admitted_owned_bytes: usize,
    reservations: BTreeMap<LightRequestToken, usize>,
    max_batch_unique_input_chunks: usize,
    max_batch_input_bytes: usize,
    max_completed_owned_bytes: usize,
    admission_rejections: usize,
    oversize_admissions: usize,
}

impl LightStatusMailbox {
    pub(crate) fn new() -> Self {
        Self::with_limits(
            DEFAULT_MAX_ADMITTED_LIGHT_STATUSES,
            DEFAULT_MAX_ADMITTED_LIGHT_OWNED_BYTES,
        )
    }

    fn with_limits(max_admitted_statuses: usize, max_admitted_owned_bytes: usize) -> Self {
        Self {
            backend: LightStatusMailboxBackend::new(None),
            pending_count: 0,
            pending_owned_bytes: 0,
            max_pending_owned_bytes: 0,
            max_admitted_statuses: max_admitted_statuses.max(1),
            max_admitted_owned_bytes: max_admitted_owned_bytes.max(1),
            reservations: BTreeMap::new(),
            max_batch_unique_input_chunks: 0,
            max_batch_input_bytes: 0,
            max_completed_owned_bytes: 0,
            admission_rejections: 0,
            oversize_admissions: 0,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn with_wasm_job_worker(config: WasmServerJobWorkerConfig) -> Self {
        Self {
            backend: LightStatusMailboxBackend::new(Some(config)),
            pending_count: 0,
            pending_owned_bytes: 0,
            max_pending_owned_bytes: 0,
            max_admitted_statuses: DEFAULT_MAX_ADMITTED_LIGHT_STATUSES,
            max_admitted_owned_bytes: DEFAULT_MAX_ADMITTED_LIGHT_OWNED_BYTES,
            reservations: BTreeMap::new(),
            max_batch_unique_input_chunks: 0,
            max_batch_input_bytes: 0,
            max_completed_owned_bytes: 0,
            admission_rejections: 0,
            oversize_admissions: 0,
        }
    }

    pub(crate) fn try_enqueue_batch(
        &mut self,
        batch: PendingLightStatusBatch,
    ) -> Result<(), PendingLightStatusBatch> {
        if batch.is_empty() {
            return Ok(());
        }
        let target_count = batch.target_count();
        let owned_bytes = batch.lifecycle_owned_bytes_estimate();
        let unique_input_chunks = batch.unique_input_count();
        let input_bytes = batch.owned_input_bytes();
        let fits_normal_limit = self.pending_count.saturating_add(target_count)
            <= self.max_admitted_statuses
            && self.pending_owned_bytes.saturating_add(owned_bytes)
                <= self.max_admitted_owned_bytes;
        let oversize = !fits_normal_limit
            && self.pending_count == 0
            && target_count == 1
            && owned_bytes > self.max_admitted_owned_bytes;
        if !fits_normal_limit && !oversize {
            self.admission_rejections = self.admission_rejections.saturating_add(1);
            return Err(batch);
        }

        let tokens = batch.tokens().collect::<Vec<_>>();
        let base_reservation = owned_bytes / target_count;
        let remainder = owned_bytes % target_count;
        for (index, token) in tokens.into_iter().enumerate() {
            let reservation = base_reservation + usize::from(index < remainder);
            assert!(
                self.reservations.insert(token, reservation).is_none(),
                "Light request token was admitted twice"
            );
        }
        self.pending_count = self.pending_count.saturating_add(target_count);
        self.pending_owned_bytes = self.pending_owned_bytes.saturating_add(owned_bytes);
        self.max_pending_owned_bytes = self.max_pending_owned_bytes.max(self.pending_owned_bytes);
        self.max_batch_unique_input_chunks =
            self.max_batch_unique_input_chunks.max(unique_input_chunks);
        self.max_batch_input_bytes = self.max_batch_input_bytes.max(input_bytes);
        if oversize {
            self.oversize_admissions = self.oversize_admissions.saturating_add(1);
        }
        self.backend.enqueue_batch(batch, self.pending_count);
        Ok(())
    }

    pub(crate) fn drain_completed(&mut self) -> Vec<CompletedLightStatus> {
        let completed = self.backend.drain_completed();
        for status in &completed {
            self.max_completed_owned_bytes = self
                .max_completed_owned_bytes
                .max(status.owned_bytes_estimate());
            if let Some(owned_bytes) = self.reservations.remove(&status.token) {
                self.pending_count = self.pending_count.saturating_sub(1);
                self.pending_owned_bytes = self.pending_owned_bytes.saturating_sub(owned_bytes);
            }
            self.backend.acknowledge_terminal(status.token);
        }
        completed
    }

    /// Evict unloaded chunks from the retained light state (155 P0). Fire-and-
    /// forget: unloads produce no completion, so `pending_count` is untouched.
    pub(crate) fn enqueue_unload(&mut self, positions: Vec<ChunkPos>) {
        if positions.is_empty() {
            return;
        }
        self.backend.enqueue_unload(positions);
    }

    pub(crate) fn cancel_token(&mut self, token: LightRequestToken) {
        if self.reservations.contains_key(&token) {
            self.backend.cancel_token(token);
        }
    }

    /// Block until the light worker has drained every request enqueued so far
    /// (including unloads), so the retained gauge reflects them. Returns `false`
    /// on timeout / dead worker.
    pub(crate) fn wait_for_light_idle(&mut self, timeout: Duration) -> bool {
        self.backend.wait_for_light_idle(timeout)
    }

    pub(crate) const fn pending_count(&self) -> usize {
        self.pending_count
    }

    pub(crate) const fn remaining_status_capacity(&self) -> usize {
        self.max_admitted_statuses
            .saturating_sub(self.pending_count)
    }

    pub(crate) fn wait_for_completed(&mut self, timeout: Duration) -> bool {
        self.backend.wait_for_completed(timeout)
    }

    pub(crate) fn kind(&self) -> LightStatusMailboxKind {
        self.backend.kind()
    }

    pub(crate) fn frame_metrics(&self) -> WorkerFrameMetrics {
        self.backend.frame_metrics()
    }

    pub(crate) fn mailbox_metrics(&self) -> LightStatusMailboxMetrics {
        let mut metrics = self.backend.mailbox_metrics();
        metrics.admitted_statuses = self.pending_count;
        metrics.admitted_owned_bytes = self.pending_owned_bytes;
        metrics.max_admitted_owned_bytes = self.max_pending_owned_bytes;
        metrics.max_batch_unique_input_chunks = self.max_batch_unique_input_chunks;
        metrics.max_batch_input_bytes = self.max_batch_input_bytes;
        metrics.max_completed_owned_bytes = self.max_completed_owned_bytes;
        metrics.admission_rejections = self.admission_rejections;
        metrics.oversize_admissions = self.oversize_admissions;
        metrics
    }
}

impl fmt::Debug for LightStatusMailbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LightStatusMailbox")
            .field("backend", &self.backend)
            .field("pending_count", &self.pending_count)
            .field("pending_owned_bytes", &self.pending_owned_bytes)
            .finish()
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
struct LightStatusMailboxBackend {
    worker: Option<WasmJobWorker>,
    light_state: RetainedInitialLightState,
    completed: VecDeque<CompletedLightStatus>,
    mailbox_metrics: LightStatusMailboxMetrics,
}

#[cfg(target_arch = "wasm32")]
impl LightStatusMailboxBackend {
    fn new(config: Option<WasmServerJobWorkerConfig>) -> Self {
        let worker = config.map(|config| {
            WasmJobWorker::new(
                "mclone-light-status",
                ServerJobActorKind::LightStatus,
                &config,
            )
            .expect("failed to spawn wasm light-status worker")
        });
        Self {
            worker,
            light_state: RetainedInitialLightState::new(),
            completed: VecDeque::new(),
            mailbox_metrics: LightStatusMailboxMetrics::default(),
        }
    }

    fn kind(&self) -> LightStatusMailboxKind {
        if self.worker.is_some() {
            LightStatusMailboxKind::WebWorker
        } else {
            LightStatusMailboxKind::Inline
        }
    }

    fn frame_metrics(&self) -> WorkerFrameMetrics {
        self.worker
            .as_ref()
            .map_or_else(WorkerFrameMetrics::default, WasmJobWorker::frame_metrics)
    }

    fn mailbox_metrics(&self) -> LightStatusMailboxMetrics {
        self.mailbox_metrics
    }

    fn enqueue_batch(&mut self, batch: PendingLightStatusBatch, pending_statuses: usize) {
        let batch_statuses = batch.target_count();
        let pending_batches = self
            .mailbox_metrics
            .enqueued_batches
            .saturating_sub(self.mailbox_metrics.completed_batches)
            .saturating_add(1);
        self.mailbox_metrics
            .record_enqueue(batch_statuses, pending_batches, pending_statuses);
        if let Some(worker) = &mut self.worker {
            let frame = encode_light_status_request(batch)
                .expect("failed to encode wasm light-status worker request");
            worker
                .post_frame(frame)
                .expect("wasm light-status worker stopped before receiving job");
            return;
        }

        let start = timing_start();
        self.completed.extend(CompletedLightStatus::from_batch(
            &mut self.light_state,
            batch,
        ));
        let compute_us = timing_elapsed_us(start);
        self.mailbox_metrics.record_worker_start(0);
        self.mailbox_metrics
            .record_compute(batch_statuses, compute_us);
        self.mailbox_metrics.retained_light_chunk_count = self.light_state.retained_chunk_count();
    }

    fn enqueue_unload(&mut self, positions: Vec<ChunkPos>) {
        if let Some(worker) = &mut self.worker {
            let frame = encode_light_status_unload_request(&positions)
                .expect("failed to encode wasm light-status unload request");
            worker
                .post_frame(frame)
                .expect("wasm light-status worker stopped before receiving unload");
            return;
        }
        self.light_state.evict_chunks(&positions);
        self.mailbox_metrics.retained_light_chunk_count = self.light_state.retained_chunk_count();
    }

    fn cancel_token(&mut self, _token: LightRequestToken) {
        // An inline job has already completed synchronously. A Web Worker
        // completion is still token-checked by the scheduler; capacity remains
        // reserved until that terminal frame is drained.
    }

    fn acknowledge_terminal(&mut self, _token: LightRequestToken) {}

    fn wait_for_light_idle(&mut self, _timeout: Duration) -> bool {
        self.worker
            .as_ref()
            .is_none_or(|worker| worker.pending_count() == 0)
    }

    fn drain_completed(&mut self) -> Vec<CompletedLightStatus> {
        if let Some(worker) = &mut self.worker {
            for frame in worker
                .drain_frames()
                .expect("wasm light-status worker failed while draining jobs")
            {
                let response = decode_light_status_response(&frame)
                    .expect("failed to decode wasm light-status job");
                self.mailbox_metrics.retained_light_chunk_count = response.retained_chunk_count;
                self.completed.extend(response.completed);
            }
        }
        self.completed.drain(..).collect()
    }

    fn wait_for_completed(&mut self, _timeout: Duration) -> bool {
        !self.completed.is_empty()
            || self
                .worker
                .as_ref()
                .is_some_and(WasmJobWorker::has_completed_frame)
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct LightStatusMailboxBackend {
    sender: mpsc::Sender<LightStatusRequest>,
    completion_receiver: mpsc::Receiver<CompletedLightStatusMessage>,
    completed: VecDeque<CompletedLightStatusMessage>,
    worker: Option<thread::JoinHandle<()>>,
    metrics: Arc<Mutex<WorkerFrameMetrics>>,
    mailbox_metrics: Arc<Mutex<LightStatusMailboxMetrics>>,
    control: Arc<Mutex<LightStatusControlState>>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default)]
struct LightStatusControlState {
    cancelled_tokens: std::collections::BTreeSet<LightRequestToken>,
    pending_unloads: std::collections::BTreeSet<ChunkPos>,
}

#[cfg(not(target_arch = "wasm32"))]
impl fmt::Debug for LightStatusMailboxBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LightStatusMailboxBackend")
            .field("kind", &"native-thread")
            .field("worker_running", &self.worker.is_some())
            .finish()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl LightStatusMailboxBackend {
    fn new(_config: Option<()>) -> Self {
        let (sender, receiver) = mpsc::channel::<LightStatusRequest>();
        let (completion_sender, completion_receiver) =
            mpsc::channel::<CompletedLightStatusMessage>();
        let metrics = Arc::new(Mutex::new(WorkerFrameMetrics::message_transfer()));
        let worker_metrics = Arc::clone(&metrics);
        let mailbox_metrics = Arc::new(Mutex::new(LightStatusMailboxMetrics::default()));
        let worker_mailbox_metrics = Arc::clone(&mailbox_metrics);
        let control = Arc::new(Mutex::new(LightStatusControlState::default()));
        let worker_control = Arc::clone(&control);
        let worker = thread::Builder::new()
            .name("mclone-light-status".to_owned())
            .spawn(move || {
                let mut light_state = RetainedInitialLightState::new();
                while let Ok(request) = receiver.recv() {
                    match request {
                        LightStatusRequest::ComputeBatch(request) => {
                            let queue_wait_us = timing_elapsed_us(request.enqueued_at);
                            if let Ok(mut metrics) = worker_mailbox_metrics.lock() {
                                metrics.record_worker_start(queue_wait_us);
                            }
                            let request_start = timing_start();
                            let mut completed = Vec::with_capacity(request.target_count);
                            let mut cancelled_statuses = 0_usize;
                            let mut active = Vec::with_capacity(request.target_count);
                            for pending in request.batch.into_statuses() {
                                apply_pending_light_unloads(&worker_control, &mut light_state);
                                let cancelled = worker_control
                                    .lock()
                                    .map(|mut control| {
                                        control.cancelled_tokens.remove(&pending.token)
                                    })
                                    .unwrap_or(false);
                                if cancelled {
                                    cancelled_statuses = cancelled_statuses.saturating_add(1);
                                    completed.push(CompletedLightStatus::cancelled(pending));
                                } else {
                                    active.push(pending);
                                }
                            }
                            if !active.is_empty() {
                                completed.extend(CompletedLightStatus::from_batch(
                                    &mut light_state,
                                    PendingLightStatusBatch::new(active),
                                ));
                            }
                            let compute_us = timing_elapsed_us(request_start);
                            let completed_at = timing_start();
                            for completed in completed {
                                if completion_sender
                                    .send(CompletedLightStatusMessage {
                                        completed,
                                        completed_at,
                                    })
                                    .is_err()
                                {
                                    return;
                                }
                            }
                            if let Ok(mut metrics) = worker_metrics.lock() {
                                metrics.record_inbound(0);
                                metrics.record_request_time_us(compute_us);
                                let pending_frames = metrics
                                    .request_frames
                                    .saturating_sub(metrics.inbound_frames);
                                metrics.observe_pending_frames(pending_frames);
                            }
                            if let Ok(mut metrics) = worker_mailbox_metrics.lock() {
                                metrics.record_compute(request.target_count, compute_us);
                                metrics.cancelled_statuses = metrics
                                    .cancelled_statuses
                                    .saturating_add(cancelled_statuses);
                                metrics.retained_light_chunk_count =
                                    light_state.retained_chunk_count();
                            }
                        }
                        LightStatusRequest::Sync(ack) => {
                            apply_pending_light_unloads(&worker_control, &mut light_state);
                            if let Ok(mut metrics) = worker_mailbox_metrics.lock() {
                                metrics.retained_light_chunk_count =
                                    light_state.retained_chunk_count();
                            }
                            let _ = ack.send(());
                        }
                        LightStatusRequest::Shutdown => break,
                    }
                }
            })
            .expect("failed to spawn native light status worker");

        Self {
            sender,
            completion_receiver,
            completed: VecDeque::new(),
            worker: Some(worker),
            metrics,
            mailbox_metrics,
            control,
        }
    }

    fn kind(&self) -> LightStatusMailboxKind {
        LightStatusMailboxKind::NativeThread
    }

    fn frame_metrics(&self) -> WorkerFrameMetrics {
        self.metrics
            .lock()
            .map(|metrics| *metrics)
            .unwrap_or_default()
    }

    fn mailbox_metrics(&self) -> LightStatusMailboxMetrics {
        self.mailbox_metrics
            .lock()
            .map(|metrics| *metrics)
            .unwrap_or_default()
    }

    fn enqueue_batch(&mut self, batch: PendingLightStatusBatch, pending_statuses: usize) {
        let batch_statuses = batch.target_count();
        let enqueued_at = timing_start();
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.record_request(0);
            let pending_frames = metrics
                .request_frames
                .saturating_sub(metrics.inbound_frames);
            metrics.observe_pending_frames(pending_frames);
            if let Ok(mut mailbox_metrics) = self.mailbox_metrics.lock() {
                mailbox_metrics.record_enqueue(batch_statuses, pending_frames, pending_statuses);
            }
        }
        self.sender
            .send(LightStatusRequest::ComputeBatch(
                LightStatusComputeRequest {
                    batch,
                    enqueued_at,
                    target_count: batch_statuses,
                },
            ))
            .expect("native light status worker stopped before receiving job");
    }

    fn enqueue_unload(&mut self, positions: Vec<ChunkPos>) {
        if let Ok(mut control) = self.control.lock() {
            control.pending_unloads.extend(positions);
        }
    }

    fn cancel_token(&mut self, token: LightRequestToken) {
        if let Ok(mut control) = self.control.lock() {
            control.cancelled_tokens.insert(token);
        }
    }

    fn acknowledge_terminal(&mut self, token: LightRequestToken) {
        if let Ok(mut control) = self.control.lock() {
            control.cancelled_tokens.remove(&token);
        }
    }

    fn wait_for_light_idle(&mut self, timeout: Duration) -> bool {
        let (ack_sender, ack_receiver) = mpsc::channel();
        if self
            .sender
            .send(LightStatusRequest::Sync(ack_sender))
            .is_err()
        {
            return false;
        }
        ack_receiver.recv_timeout(timeout).is_ok()
    }

    fn drain_completed(&mut self) -> Vec<CompletedLightStatus> {
        let mut messages = self.completed.drain(..).collect::<Vec<_>>();
        while let Ok(message) = self.completion_receiver.try_recv() {
            messages.push(message);
        }
        let mut completed = Vec::with_capacity(messages.len());
        if let Ok(mut metrics) = self.mailbox_metrics.lock() {
            for message in messages {
                metrics.record_completion_drain_wait(timing_elapsed_us(message.completed_at));
                completed.push(message.completed);
            }
        } else {
            completed.extend(messages.into_iter().map(|message| message.completed));
        }
        completed
    }

    fn wait_for_completed(&mut self, timeout: Duration) -> bool {
        if !self.completed.is_empty() {
            return true;
        }

        match self.completion_receiver.recv_timeout(timeout) {
            Ok(job) => {
                self.completed.push_back(job);
                true
            }
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => false,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for LightStatusMailboxBackend {
    fn drop(&mut self) {
        let _ = self.sender.send(LightStatusRequest::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
enum LightStatusRequest {
    ComputeBatch(LightStatusComputeRequest),
    /// Round-trip barrier: the worker replies on the carried channel once it has
    /// drained every earlier request. Lets callers observe the post-unload
    /// retained gauge deterministically (unloads produce no completion to wait
    /// on).
    Sync(mpsc::Sender<()>),
    Shutdown,
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_pending_light_unloads(
    control: &Arc<Mutex<LightStatusControlState>>,
    light_state: &mut RetainedInitialLightState,
) {
    let positions = control
        .lock()
        .map(|mut control| std::mem::take(&mut control.pending_unloads))
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>();
    light_state.evict_chunks(&positions);
}

#[cfg(not(target_arch = "wasm32"))]
struct LightStatusComputeRequest {
    batch: PendingLightStatusBatch,
    enqueued_at: Option<TimingSample>,
    target_count: usize,
}

#[cfg(not(target_arch = "wasm32"))]
struct CompletedLightStatusMessage {
    completed: CompletedLightStatus,
    completed_at: Option<TimingSample>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::light_status::PendingLightStatus;
    use mclone_core::{ChunkRevision, ChunkStatus};

    fn test_batch(id: u64, pos: ChunkPos) -> PendingLightStatusBatch {
        let snapshot = ChunkSnapshot {
            pos,
            status: ChunkStatus::Features,
            revision: ChunkRevision(id),
            min_y: 0,
            height: 16,
            biomes: Vec::new(),
            sections: Vec::new(),
            light_correct: false,
            light_sections: Vec::new(),
        };
        PendingLightStatusBatch::new(vec![PendingLightStatus::from_parts_with_token(
            LightRequestToken::new(id, pos, snapshot.revision),
            snapshot,
            vec![0; 16 * 16 * 16],
            Vec::new(),
        )])
    }

    fn two_status_batch() -> PendingLightStatusBatch {
        let mut statuses = test_batch(1, ChunkPos::new(0, 0)).into_statuses();
        statuses.extend(test_batch(2, ChunkPos::new(1, 0)).into_statuses());
        PendingLightStatusBatch::new(statuses)
    }

    #[test]
    fn admission_reservation_remains_until_completion_is_drained() {
        let mut mailbox = LightStatusMailbox::with_limits(1, usize::MAX);
        mailbox
            .try_enqueue_batch(test_batch(1, ChunkPos::new(0, 0)))
            .unwrap();
        assert!(mailbox.wait_for_completed(Duration::from_secs(5)));

        let rejected = mailbox
            .try_enqueue_batch(test_batch(2, ChunkPos::new(1, 0)))
            .unwrap_err();
        assert_eq!(mailbox.pending_count(), 1);
        assert_eq!(mailbox.mailbox_metrics().admission_rejections, 1);

        assert_eq!(mailbox.drain_completed().len(), 1);
        assert_eq!(mailbox.pending_count(), 0);
        mailbox.try_enqueue_batch(rejected).unwrap();
    }

    #[test]
    fn one_oversize_status_cannot_admit_a_second_status() {
        let mut mailbox = LightStatusMailbox::with_limits(2, 1);
        mailbox
            .try_enqueue_batch(test_batch(1, ChunkPos::new(0, 0)))
            .unwrap();
        assert!(
            mailbox
                .try_enqueue_batch(test_batch(2, ChunkPos::new(1, 0)))
                .is_err()
        );

        let metrics = mailbox.mailbox_metrics();
        assert_eq!(metrics.admitted_statuses, 1);
        assert!(metrics.admitted_owned_bytes > 1);
        assert_eq!(metrics.oversize_admissions, 1);
        assert_eq!(metrics.admission_rejections, 1);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_worker_skips_a_cancelled_token_before_its_target() {
        let mut mailbox = LightStatusMailbox::with_limits(2, usize::MAX);
        let cancelled = LightRequestToken::new(2, ChunkPos::new(1, 0), ChunkRevision(2));
        mailbox
            .backend
            .control
            .lock()
            .unwrap()
            .cancelled_tokens
            .insert(cancelled);

        mailbox.try_enqueue_batch(two_status_batch()).unwrap();
        assert!(mailbox.wait_for_completed(Duration::from_secs(5)));
        let mut observed_cancelled = false;
        while mailbox.pending_count() > 0 {
            assert!(mailbox.wait_for_completed(Duration::from_secs(5)));
            let completed = mailbox.drain_completed();
            for status in completed {
                if status.token == cancelled {
                    assert!(status.cancelled);
                    observed_cancelled = true;
                }
            }
        }

        assert!(observed_cancelled);
        assert_eq!(mailbox.mailbox_metrics().cancelled_statuses, 1);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_worker_applies_pending_unload_before_next_target() {
        let mut mailbox = LightStatusMailbox::with_limits(1, usize::MAX);
        mailbox
            .try_enqueue_batch(test_batch(1, ChunkPos::new(0, 0)))
            .unwrap();
        assert!(mailbox.wait_for_completed(Duration::from_secs(5)));
        mailbox.drain_completed();

        mailbox.enqueue_unload(vec![ChunkPos::new(0, 0)]);
        mailbox
            .try_enqueue_batch(test_batch(2, ChunkPos::new(10, 0)))
            .unwrap();
        assert!(mailbox.wait_for_completed(Duration::from_secs(5)));
        mailbox.drain_completed();
        assert!(mailbox.wait_for_light_idle(Duration::from_secs(5)));

        assert_eq!(mailbox.mailbox_metrics().retained_light_chunk_count, 1);
    }
}
