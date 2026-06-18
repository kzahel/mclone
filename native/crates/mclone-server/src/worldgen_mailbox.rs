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
}

impl WorldgenMailbox {
    pub(crate) fn new() -> Self {
        Self {
            backend: WorldgenMailboxBackend::new(),
        }
    }

    pub(crate) fn enqueue_features(
        &mut self,
        job_id: ChunkJobId,
        seed: i64,
        targets: &[ChunkPos],
        dependencies: Vec<MutableChunkBlockBuffer>,
    ) {
        self.backend
            .enqueue_features(job_id, seed, targets, dependencies);
    }

    pub(crate) fn drain_completed(&mut self) -> Vec<WorldgenCompletedJob> {
        self.backend.drain_completed()
    }

    pub(crate) fn wait_for_completed(&mut self, timeout: Duration) -> bool {
        self.backend.wait_for_completed(timeout)
    }

    pub(crate) fn kind(&self) -> WorldgenMailboxKind {
        self.backend.kind()
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
    completed: VecDeque<WorldgenCompletedJob>,
}

#[cfg(target_arch = "wasm32")]
impl WorldgenMailboxBackend {
    fn new() -> Self {
        Self {
            completed: VecDeque::new(),
        }
    }

    fn kind(&self) -> WorldgenMailboxKind {
        WorldgenMailboxKind::Inline
    }

    fn enqueue_features(
        &mut self,
        job_id: ChunkJobId,
        seed: i64,
        targets: &[ChunkPos],
        dependencies: Vec<MutableChunkBlockBuffer>,
    ) {
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
        self.completed.drain(..).collect()
    }

    fn wait_for_completed(&mut self, _timeout: Duration) -> bool {
        !self.completed.is_empty()
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
    fn new() -> Self {
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
