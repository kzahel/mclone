//! Worldgen mailbox ownership: the off-thread (native) / inline (wasm) feature
//! generation backend and its completed-job plumbing.
//!
//! Move-only home for the worldgen mailbox, both cfg-gated backend variants, the
//! worker request enum, and the completed-job and pending-publication carriers.
//! `ChunkStatus::Light` is scheduler-owned and runs after completed feature
//! chunks are published to holders.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use std::{sync::mpsc, thread};

use mclone_core::ChunkPos;
use mclone_worldgen::levelgen::{
    GeneratedChunk, MutableChunkBlockBuffer, OverworldFeatureBatchTiming,
    OverworldFeatureDependencyCache, OverworldFeatureDependencyCacheReport,
};

#[cfg(target_arch = "wasm32")]
use crate::WasmServerJobWorkerConfig;
#[cfg(target_arch = "wasm32")]
use crate::job_codec::{decode_worldgen_response, encode_worldgen_request};
#[cfg(target_arch = "wasm32")]
use crate::wasm_job_worker::WasmJobWorker;
use crate::{ChunkJobId, WorldgenMailboxKind};

#[derive(Debug)]
pub(crate) struct WorldgenCompletedJob {
    pub(crate) job_id: ChunkJobId,
    pub(crate) generated_chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    pub(crate) retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    pub(crate) cache_report: OverworldFeatureDependencyCacheReport,
    pub(crate) timing: OverworldFeatureBatchTiming,
}

#[derive(Debug)]
pub(crate) struct PendingWorldgenPublication {
    pub(crate) completed: WorldgenCompletedJob,
    pub(crate) next_target_index: usize,
}

impl PendingWorldgenPublication {
    pub(crate) fn new(completed: WorldgenCompletedJob) -> Self {
        Self {
            completed,
            next_target_index: 0,
        }
    }
}

pub(crate) struct WorldgenMailbox {
    backend: WorldgenMailboxBackend,
    pending_count: usize,
}

impl WorldgenMailbox {
    pub(crate) fn new() -> Self {
        Self {
            backend: WorldgenMailboxBackend::new(None),
            pending_count: 0,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn with_wasm_job_worker(config: WasmServerJobWorkerConfig) -> Self {
        Self {
            backend: WorldgenMailboxBackend::new(Some(config)),
            pending_count: 0,
        }
    }

    pub(crate) fn enqueue_features(
        &mut self,
        job_id: ChunkJobId,
        seed: i64,
        targets: &[ChunkPos],
        dependencies: Vec<MutableChunkBlockBuffer>,
    ) {
        self.pending_count = self.pending_count.saturating_add(1);
        self.backend
            .enqueue_features(job_id, seed, targets, dependencies);
    }

    pub(crate) fn drain_completed(&mut self) -> Vec<WorldgenCompletedJob> {
        let completed = self.backend.drain_completed();
        self.pending_count = self.pending_count.saturating_sub(completed.len());
        completed
    }

    pub(crate) fn wait_for_completed(&mut self, timeout: Duration) -> bool {
        self.backend.wait_for_completed(timeout)
    }

    pub(crate) fn kind(&self) -> WorldgenMailboxKind {
        self.backend.kind()
    }

    pub(crate) const fn pending_count(&self) -> usize {
        self.pending_count
    }
}

impl fmt::Debug for WorldgenMailbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorldgenMailbox")
            .field("backend", &self.backend)
            .finish()
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
struct WorldgenMailboxBackend {
    worker: Option<WasmJobWorker>,
    completed: VecDeque<WorldgenCompletedJob>,
}

#[cfg(target_arch = "wasm32")]
impl WorldgenMailboxBackend {
    fn new(config: Option<WasmServerJobWorkerConfig>) -> Self {
        let worker = config.map(|config| {
            WasmJobWorker::new("mclone-worldgen", "worldgen", &config)
                .expect("failed to spawn wasm worldgen worker")
        });
        Self {
            worker,
            completed: VecDeque::new(),
        }
    }

    fn kind(&self) -> WorldgenMailboxKind {
        if self.worker.is_some() {
            WorldgenMailboxKind::WebWorker
        } else {
            WorldgenMailboxKind::Inline
        }
    }

    fn enqueue_features(
        &mut self,
        job_id: ChunkJobId,
        seed: i64,
        targets: &[ChunkPos],
        dependencies: Vec<MutableChunkBlockBuffer>,
    ) {
        if let Some(worker) = &mut self.worker {
            let frame = encode_worldgen_request(job_id, seed, targets, dependencies)
                .expect("failed to encode wasm worldgen worker request");
            worker
                .post_frame(frame)
                .expect("wasm worldgen worker stopped before receiving job");
            return;
        }

        let mut dependency_cache = OverworldFeatureDependencyCache::new();
        let result = dependency_cache.generate_features_chunks_with_dependencies(
            seed,
            targets.iter().copied(),
            dependencies,
        );
        self.completed.push_back(WorldgenCompletedJob {
            job_id,
            generated_chunks: result.chunks,
            retained_dependencies: result.retained_dependencies,
            cache_report: result.cache_report,
            timing: result.timing,
        });
    }

    fn drain_completed(&mut self) -> Vec<WorldgenCompletedJob> {
        if let Some(worker) = &mut self.worker {
            for frame in worker
                .drain_frames()
                .expect("wasm worldgen worker failed while draining jobs")
            {
                let decoded =
                    decode_worldgen_response(&frame).expect("failed to decode wasm worldgen job");
                self.completed.push_back(WorldgenCompletedJob {
                    job_id: decoded.job_id,
                    generated_chunks: decoded.generated_chunks,
                    retained_dependencies: decoded.retained_dependencies,
                    cache_report: decoded.cache_report,
                    timing: decoded.timing,
                });
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
struct WorldgenMailboxBackend {
    sender: mpsc::Sender<WorldgenRequest>,
    completion_receiver: mpsc::Receiver<WorldgenCompletedJob>,
    completed: VecDeque<WorldgenCompletedJob>,
    worker: Option<thread::JoinHandle<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl fmt::Debug for WorldgenMailboxBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorldgenMailboxBackend")
            .field("kind", &"native-thread")
            .field("worker_running", &self.worker.is_some())
            .finish()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl WorldgenMailboxBackend {
    fn new(_config: Option<()>) -> Self {
        let (sender, receiver) = mpsc::channel::<WorldgenRequest>();
        let (completion_sender, completion_receiver) = mpsc::channel::<WorldgenCompletedJob>();
        let worker = thread::Builder::new()
            .name("mclone-worldgen".to_owned())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    match request {
                        WorldgenRequest::GenerateFeatures {
                            job_id,
                            seed,
                            targets,
                            dependencies,
                        } => {
                            let mut dependency_cache = OverworldFeatureDependencyCache::new();
                            let result = dependency_cache
                                .generate_features_chunks_with_dependencies(
                                    seed,
                                    targets.iter().copied(),
                                    dependencies,
                                );
                            if completion_sender
                                .send(WorldgenCompletedJob {
                                    job_id,
                                    generated_chunks: result.chunks,
                                    retained_dependencies: result.retained_dependencies,
                                    cache_report: result.cache_report,
                                    timing: result.timing,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        WorldgenRequest::Shutdown => break,
                    }
                }
            })
            .expect("failed to spawn native worldgen worker");

        Self {
            sender,
            completion_receiver,
            completed: VecDeque::new(),
            worker: Some(worker),
        }
    }

    fn kind(&self) -> WorldgenMailboxKind {
        WorldgenMailboxKind::NativeThread
    }

    fn enqueue_features(
        &mut self,
        job_id: ChunkJobId,
        seed: i64,
        targets: &[ChunkPos],
        dependencies: Vec<MutableChunkBlockBuffer>,
    ) {
        self.sender
            .send(WorldgenRequest::GenerateFeatures {
                job_id,
                seed,
                targets: targets.to_vec(),
                dependencies,
            })
            .expect("native worldgen worker stopped before receiving job");
    }

    fn drain_completed(&mut self) -> Vec<WorldgenCompletedJob> {
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
impl Drop for WorldgenMailboxBackend {
    fn drop(&mut self) {
        let _ = self.sender.send(WorldgenRequest::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
enum WorldgenRequest {
    GenerateFeatures {
        job_id: ChunkJobId,
        seed: i64,
        targets: Vec<ChunkPos>,
        dependencies: Vec<MutableChunkBlockBuffer>,
    },
    Shutdown,
}
