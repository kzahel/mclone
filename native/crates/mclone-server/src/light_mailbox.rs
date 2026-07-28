//! Light-status mailbox ownership.
//!
//! Native desktop computes `ChunkStatus::Light` payloads off the foreground
//! scheduler path, mirroring the reference split where `ThreadedLevelLightEngine`
//! keeps propagation work outside `ServerChunkCache`'s immediate tick body. WASM
//! keeps an inline backend until worker plumbing exists there.

use std::collections::VecDeque;
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
};
use crate::level_light_bridge::LevelLightComputationTiming;
use crate::light_status::{LightRequestToken, PendingLightStatusBatch};
use crate::light_world::RetainedInitialLightState;
use crate::persistence::ScheduledTickRecord;
use crate::timing::{TimingSample, timing_elapsed_us, timing_start};
#[cfg(target_arch = "wasm32")]
use crate::wasm_job_worker::WasmJobWorker;
use crate::{LightStatusMailboxKind, LightStatusMailboxMetrics, WorkerFrameMetrics};

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
                }
            })
            .collect()
    }
}

pub(crate) struct LightStatusMailbox {
    backend: LightStatusMailboxBackend,
    pending_count: usize,
}

impl LightStatusMailbox {
    pub(crate) fn new() -> Self {
        Self {
            backend: LightStatusMailboxBackend::new(None),
            pending_count: 0,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn with_wasm_job_worker(config: WasmServerJobWorkerConfig) -> Self {
        Self {
            backend: LightStatusMailboxBackend::new(Some(config)),
            pending_count: 0,
        }
    }

    pub(crate) fn enqueue_batch(&mut self, batch: PendingLightStatusBatch) {
        if batch.is_empty() {
            return;
        }
        self.pending_count = self.pending_count.saturating_add(batch.target_count());
        self.backend.enqueue_batch(batch, self.pending_count);
    }

    pub(crate) fn drain_completed(&mut self) -> Vec<CompletedLightStatus> {
        let completed = self.backend.drain_completed();
        self.pending_count = self.pending_count.saturating_sub(completed.len());
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

    /// Block until the light worker has drained every request enqueued so far
    /// (including unloads), so the retained gauge reflects them. Returns `false`
    /// on timeout / dead worker.
    pub(crate) fn wait_for_light_idle(&mut self, timeout: Duration) -> bool {
        self.backend.wait_for_light_idle(timeout)
    }

    pub(crate) const fn pending_count(&self) -> usize {
        self.pending_count
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
        self.backend.mailbox_metrics()
    }
}

impl fmt::Debug for LightStatusMailbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LightStatusMailbox")
            .field("backend", &self.backend)
            .field("pending_count", &self.pending_count)
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
        if self.worker.is_some() {
            // The web worker rebuilds `RetainedInitialLightState` per job frame
            // (`compute_light_status_job_frame`), so it retains nothing across
            // batches and has nothing to evict.
            return;
        }
        self.light_state.evict_chunks(&positions);
        self.mailbox_metrics.retained_light_chunk_count = self.light_state.retained_chunk_count();
    }

    fn wait_for_light_idle(&mut self, _timeout: Duration) -> bool {
        // Inline compute + eviction are synchronous; the web worker retains no
        // cross-batch state, so there is never a pending eviction to await.
        true
    }

    fn drain_completed(&mut self) -> Vec<CompletedLightStatus> {
        if let Some(worker) = &mut self.worker {
            for frame in worker
                .drain_frames()
                .expect("wasm light-status worker failed while draining jobs")
            {
                self.completed.extend(
                    decode_light_status_response(&frame)
                        .expect("failed to decode wasm light-status job"),
                );
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
                            let completed =
                                CompletedLightStatus::from_batch(&mut light_state, request.batch);
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
                                metrics.retained_light_chunk_count =
                                    light_state.retained_chunk_count();
                            }
                        }
                        LightStatusRequest::Unload(positions) => {
                            light_state.evict_chunks(&positions);
                            if let Ok(mut metrics) = worker_mailbox_metrics.lock() {
                                metrics.retained_light_chunk_count =
                                    light_state.retained_chunk_count();
                            }
                        }
                        LightStatusRequest::Sync(ack) => {
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
        // Best-effort memory cleanup: if the worker has already shut down there is
        // nothing left to evict, so a failed send is not fatal.
        let _ = self.sender.send(LightStatusRequest::Unload(positions));
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
    /// Evict unloaded chunks from the retained light state (155 P0). Rides the
    /// same FIFO request channel as `ComputeBatch`, so an unload always applies
    /// after any earlier compute for the same chunk and before any later relight.
    Unload(Vec<ChunkPos>),
    /// Round-trip barrier: the worker replies on the carried channel once it has
    /// drained every earlier request. Lets callers observe the post-unload
    /// retained gauge deterministically (unloads produce no completion to wait
    /// on).
    Sync(mpsc::Sender<()>),
    Shutdown,
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
