//! Worldgen mailbox ownership: the off-thread (native) / inline (wasm) feature
//! generation backend and its completed-job plumbing.
//!
//! Move-only home for the worldgen mailbox, both cfg-gated backend variants, the
//! worker request enum, and the completed-job and pending-publication carriers.
//! `ChunkStatus::Light` is scheduler-owned and runs after completed feature
//! chunks are published to holders.

#[cfg(target_arch = "wasm32")]
use std::collections::BTreeSet;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Instant,
};

use mclone_core::ChunkPos;
use mclone_worldgen::levelgen::{
    GeneratedChunk, MutableChunkBlockBuffer, OverworldFeatureDependencyCache,
};

#[cfg(target_arch = "wasm32")]
use crate::WasmServerJobWorkerConfig;
use crate::job_codec::{OverworldGenerationDiagnostics, generate_chunks_with_dependencies};
#[cfg(target_arch = "wasm32")]
use crate::job_codec::{decode_worldgen_response, encode_worldgen_delta_request};
#[cfg(target_arch = "wasm32")]
use crate::wasm_job_worker::WasmJobWorker;
use crate::{ChunkJobId, WorkerFrameMetrics, WorldGenerationDescriptor, WorldgenMailboxKind};

#[derive(Debug)]
pub(crate) struct WorldgenCompletedJob {
    pub(crate) job_id: ChunkJobId,
    pub(crate) descriptor: WorldGenerationDescriptor,
    pub(crate) generated_chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    pub(crate) retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    pub(crate) overworld_diagnostics: Option<OverworldGenerationDiagnostics>,
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
        descriptor: WorldGenerationDescriptor,
        targets: &[ChunkPos],
        dependencies: Vec<MutableChunkBlockBuffer>,
    ) {
        self.pending_count = self.pending_count.saturating_add(1);
        self.backend
            .enqueue_features(job_id, descriptor, targets, dependencies);
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

    pub(crate) fn frame_metrics(&self) -> WorkerFrameMetrics {
        self.backend.frame_metrics()
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
    /// 069 Stage 1: main-side shadow of the dependency columns the resident web
    /// worker mirror holds. Each job ships only the columns the worker lacks (a
    /// delta) instead of re-serializing the whole 529-chunk dependency
    /// neighbourhood every job — the 067 Stage 4 resident-mirror + delta pattern.
    /// Unused on the inline (no-worker) path. Corrected authoritatively from each
    /// response's retained set in `drain_completed`; advanced on submit by the
    /// shipped upserts so a job enqueued before the prior job's response is drained
    /// still ships a minimal delta.
    worker_mirror_shadow: BTreeSet<ChunkPos>,
    /// Mirror epoch for the worker-side desync tripwire. Bumped on each full resync
    /// (reset). A non-reset delta carries this so the worker can reject a delta that
    /// does not match the mirror it actually holds.
    worker_mirror_generation: u64,
    /// Whether the resident worker mirror has been seeded with a reset yet; the
    /// first job (or a post-reset job) is a full resync.
    worker_mirror_initialized: bool,
    /// Descriptor held by the resident worker mirror. A profile or seed change
    /// forces a reset even if the scheduler instance is reused by a test host.
    worker_descriptor: Option<WorldGenerationDescriptor>,
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
            worker_mirror_shadow: BTreeSet::new(),
            worker_mirror_generation: 0,
            worker_mirror_initialized: false,
            worker_descriptor: None,
        }
    }

    fn kind(&self) -> WorldgenMailboxKind {
        if self.worker.is_some() {
            WorldgenMailboxKind::WebWorker
        } else {
            WorldgenMailboxKind::Inline
        }
    }

    fn frame_metrics(&self) -> WorkerFrameMetrics {
        self.worker
            .as_ref()
            .map_or_else(WorkerFrameMetrics::default, WasmJobWorker::frame_metrics)
    }

    fn enqueue_features(
        &mut self,
        job_id: ChunkJobId,
        descriptor: WorldGenerationDescriptor,
        targets: &[ChunkPos],
        dependencies: Vec<MutableChunkBlockBuffer>,
    ) {
        if self.worker.is_some() {
            // 069 Stage 1: ship a delta against the worker's resident dependency
            // mirror, not the whole seeded neighbourhood. The first job (or any job
            // before the mirror is seeded) is a full-resync reset under a fresh
            // generation; every later job ships only the columns the shadow says the
            // worker lacks. Dependency buffers are deterministic in `(seed, pos)` and
            // never feature-mutated, so the worker regenerates any column it is not
            // sent — byte-identical output (this only changes transport).
            let reset =
                !self.worker_mirror_initialized || self.worker_descriptor != Some(descriptor);
            if reset {
                self.worker_mirror_generation =
                    self.worker_mirror_generation.wrapping_add(1).max(1);
                self.worker_mirror_shadow.clear();
            }
            let generation = self.worker_mirror_generation;
            let upserts: Vec<MutableChunkBlockBuffer> = dependencies
                .into_iter()
                .filter(|dependency| {
                    reset
                        || !self
                            .worker_mirror_shadow
                            .contains(&ChunkPos::new(dependency.chunk_x, dependency.chunk_z))
                })
                .collect();
            let frame = encode_worldgen_delta_request(
                job_id, descriptor, generation, reset, targets, &upserts,
            )
            .expect("failed to encode wasm worldgen worker delta request");
            self.worker
                .as_mut()
                .expect("wasm worldgen worker present")
                .post_frame(frame)
                .expect("wasm worldgen worker stopped before receiving job");
            // Apply-on-submit: the worker will hold these columns once it processes
            // this job, so a job enqueued before this one's response is drained still
            // ships a minimal delta. `drain_completed` then corrects the shadow to the
            // worker's authoritative retained set.
            self.worker_mirror_shadow.extend(
                upserts
                    .iter()
                    .map(|dependency| ChunkPos::new(dependency.chunk_x, dependency.chunk_z)),
            );
            self.worker_mirror_initialized = true;
            self.worker_descriptor = Some(descriptor);
            return;
        }

        let mut dependency_cache = OverworldFeatureDependencyCache::new();
        let result = generate_chunks_with_dependencies(
            &mut dependency_cache,
            descriptor,
            targets,
            dependencies,
        )
        .expect("scheduler sent unsupported profile to inline worldgen worker");
        self.completed.push_back(WorldgenCompletedJob {
            job_id,
            descriptor,
            generated_chunks: result.chunks,
            retained_dependencies: result.retained_dependencies,
            overworld_diagnostics: result.overworld_diagnostics,
        });
    }

    fn drain_completed(&mut self) -> Vec<WorldgenCompletedJob> {
        let frames = match &mut self.worker {
            Some(worker) => worker
                .drain_frames()
                .expect("wasm worldgen worker failed while draining jobs"),
            None => Vec::new(),
        };
        for frame in frames {
            let decoded =
                decode_worldgen_response(&frame).expect("failed to decode wasm worldgen job");
            // 069 Stage 1/2: correct the shadow to the worker's authoritative retained
            // set — exactly the dependency columns its resident mirror holds after
            // this job's own retain step. Stage 2 ships only a *subset* of the
            // dependency buffers (new-to-worker + light ring), so the full retained
            // set comes from the positions-only `retained_dependency_positions` list.
            // Keeps the shadow in lockstep with the worker mirror across boundary
            // crossings (where the worker evicts the columns the new plan drops).
            self.worker_mirror_shadow = decoded
                .retained_dependency_positions
                .iter()
                .copied()
                .collect();
            self.completed.push_back(WorldgenCompletedJob {
                job_id: decoded.job_id,
                descriptor: decoded.descriptor,
                generated_chunks: decoded.generated_chunks,
                retained_dependencies: decoded.retained_dependencies,
                overworld_diagnostics: decoded.overworld_diagnostics,
            });
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
    metrics: Arc<Mutex<WorkerFrameMetrics>>,
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
        let metrics = Arc::new(Mutex::new(WorkerFrameMetrics::message_transfer()));
        let worker_metrics = Arc::clone(&metrics);
        let worker = thread::Builder::new()
            .name("mclone-worldgen".to_owned())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    match request {
                        WorldgenRequest::GenerateFeatures {
                            job_id,
                            descriptor,
                            targets,
                            dependencies,
                        } => {
                            if let Ok(mut metrics) = worker_metrics.lock() {
                                metrics.record_request(0);
                            }
                            let request_start = Instant::now();
                            let mut dependency_cache = OverworldFeatureDependencyCache::new();
                            let result = generate_chunks_with_dependencies(
                                &mut dependency_cache,
                                descriptor,
                                &targets,
                                dependencies,
                            )
                            .expect("scheduler sent unsupported profile to native worldgen worker");
                            let completed = WorldgenCompletedJob {
                                job_id,
                                descriptor,
                                generated_chunks: result.chunks,
                                retained_dependencies: result.retained_dependencies,
                                overworld_diagnostics: result.overworld_diagnostics,
                            };
                            if let Ok(mut metrics) = worker_metrics.lock() {
                                metrics.record_inbound(0);
                                metrics.record_request_time_us(request_start.elapsed().as_micros());
                            }
                            if completion_sender.send(completed).is_err() {
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
            metrics,
        }
    }

    fn kind(&self) -> WorldgenMailboxKind {
        WorldgenMailboxKind::NativeThread
    }

    fn frame_metrics(&self) -> WorkerFrameMetrics {
        self.metrics
            .lock()
            .map(|metrics| *metrics)
            .unwrap_or_default()
    }

    fn enqueue_features(
        &mut self,
        job_id: ChunkJobId,
        descriptor: WorldGenerationDescriptor,
        targets: &[ChunkPos],
        dependencies: Vec<MutableChunkBlockBuffer>,
    ) {
        self.sender
            .send(WorldgenRequest::GenerateFeatures {
                job_id,
                descriptor,
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
        descriptor: WorldGenerationDescriptor,
        targets: Vec<ChunkPos>,
        dependencies: Vec<MutableChunkBlockBuffer>,
    },
    Shutdown,
}
