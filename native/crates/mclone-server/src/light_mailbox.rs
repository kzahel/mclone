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
use std::{sync::mpsc, thread};

use mclone_core::{ChunkPos, ChunkSnapshot, PackedLightSection};

#[cfg(target_arch = "wasm32")]
use crate::WasmServerJobWorkerConfig;
#[cfg(target_arch = "wasm32")]
use crate::job_codec::{decode_light_status_response, encode_light_status_request};
use crate::level_light_bridge::LevelLightComputationTiming;
use crate::light_status::PendingLightStatusBatch;
use crate::light_world::RetainedInitialLightState;
use crate::persistence::ScheduledTickRecord;
use crate::timing::{timing_elapsed_us, timing_start};
#[cfg(target_arch = "wasm32")]
use crate::wasm_job_worker::WasmJobWorker;
use crate::{LightStatusMailboxKind, WorkerFrameMetrics};

#[derive(Debug)]
pub(crate) struct CompletedLightStatus {
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
        self.backend.enqueue_batch(batch);
    }

    pub(crate) fn drain_completed(&mut self) -> Vec<CompletedLightStatus> {
        let completed = self.backend.drain_completed();
        self.pending_count = self.pending_count.saturating_sub(completed.len());
        completed
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
}

#[cfg(target_arch = "wasm32")]
impl LightStatusMailboxBackend {
    fn new(config: Option<WasmServerJobWorkerConfig>) -> Self {
        let worker = config.map(|config| {
            WasmJobWorker::new("mclone-light-status", "light-status", &config)
                .expect("failed to spawn wasm light-status worker")
        });
        Self {
            worker,
            light_state: RetainedInitialLightState::new(),
            completed: VecDeque::new(),
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

    fn enqueue_batch(&mut self, batch: PendingLightStatusBatch) {
        if let Some(worker) = &mut self.worker {
            let frame = encode_light_status_request(batch)
                .expect("failed to encode wasm light-status worker request");
            worker
                .post_frame(frame)
                .expect("wasm light-status worker stopped before receiving job");
            return;
        }

        self.completed.extend(CompletedLightStatus::from_batch(
            &mut self.light_state,
            batch,
        ));
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
    completion_receiver: mpsc::Receiver<CompletedLightStatus>,
    completed: VecDeque<CompletedLightStatus>,
    worker: Option<thread::JoinHandle<()>>,
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
        let (completion_sender, completion_receiver) = mpsc::channel::<CompletedLightStatus>();
        let worker = thread::Builder::new()
            .name("mclone-light-status".to_owned())
            .spawn(move || {
                let mut light_state = RetainedInitialLightState::new();
                while let Ok(request) = receiver.recv() {
                    match request {
                        LightStatusRequest::ComputeBatch(batch) => {
                            for completed in
                                CompletedLightStatus::from_batch(&mut light_state, batch)
                            {
                                if completion_sender.send(completed).is_err() {
                                    return;
                                }
                            }
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
        }
    }

    fn kind(&self) -> LightStatusMailboxKind {
        LightStatusMailboxKind::NativeThread
    }

    fn frame_metrics(&self) -> WorkerFrameMetrics {
        WorkerFrameMetrics::default()
    }

    fn enqueue_batch(&mut self, batch: PendingLightStatusBatch) {
        self.sender
            .send(LightStatusRequest::ComputeBatch(batch))
            .expect("native light status worker stopped before receiving job");
    }

    fn drain_completed(&mut self) -> Vec<CompletedLightStatus> {
        let mut completed = self.completed.drain(..).collect::<Vec<_>>();
        while let Ok(job) = self.completion_receiver.try_recv() {
            completed.push(job);
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
    ComputeBatch(PendingLightStatusBatch),
    Shutdown,
}
