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

use crate::level_light_bridge::LevelLightComputationTiming;
use crate::light_status::PendingLightStatusBatch;
use crate::light_world::RetainedInitialLightState;
use crate::timing::{timing_elapsed_us, timing_start};

#[derive(Debug)]
pub(crate) struct CompletedLightStatus {
    pub(crate) pos: ChunkPos,
    pub(crate) feature_snapshot: ChunkSnapshot,
    pub(crate) light_sections: Vec<PackedLightSection>,
    pub(crate) batch_compute_leader: bool,
    pub(crate) compute_us: u128,
    pub(crate) timing: LevelLightComputationTiming,
}

impl CompletedLightStatus {
    fn from_batch(
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
            backend: LightStatusMailboxBackend::new(),
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
    light_state: RetainedInitialLightState,
    completed: VecDeque<CompletedLightStatus>,
}

#[cfg(target_arch = "wasm32")]
impl LightStatusMailboxBackend {
    fn new() -> Self {
        Self {
            light_state: RetainedInitialLightState::new(),
            completed: VecDeque::new(),
        }
    }

    fn enqueue_batch(&mut self, batch: PendingLightStatusBatch) {
        self.completed.extend(CompletedLightStatus::from_batch(
            &mut self.light_state,
            batch,
        ));
    }

    fn drain_completed(&mut self) -> Vec<CompletedLightStatus> {
        self.completed.drain(..).collect()
    }

    fn wait_for_completed(&mut self, _timeout: Duration) -> bool {
        !self.completed.is_empty()
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
    fn new() -> Self {
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
