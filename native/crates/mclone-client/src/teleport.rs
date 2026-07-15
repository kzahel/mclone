use std::collections::BTreeMap;
use std::fmt;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Condvar, Mutex, mpsc};
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use mclone_core::{Aabb, BlockPos, BlockStateId, Vec3d};
use mclone_path::{
    DEFAULT_PATH_HEURISTIC_WEIGHT, PathNeighbor, PathSearch, PathSearchDiagnostics,
    PathSearchLimits, PathSearchQuery,
};

use crate::{ClientRuntime, LOCAL_PLAYER_STANDING_HEIGHT, LOCAL_PLAYER_STANDING_WIDTH};

const TELEPORT_EPSILON: f64 = 1.0e-7;
const DEFAULT_MAX_DISTANCE: f64 = 8.0;
const DEFAULT_ARC_HEIGHT: f64 = 1.25;
const DEFAULT_ARC_SAMPLES: usize = 24;
const DEFAULT_CANDIDATE_RADIUS: f64 = 1.25;
const DEFAULT_CANDIDATE_SPACING: f64 = 0.5;
const DEFAULT_MIN_PROGRESS: f64 = 0.75;
const DEFAULT_MAX_STEP_UP: i32 = 1;
const DEFAULT_MAX_DROP: i32 = 3;
const DEFAULT_MAX_VISITED_NODES: usize = 512;
const DEFAULT_MARKER_HEIGHT: f64 = 1.45;
const SUPPORT_PROBE_DEPTH: f64 = 0.08;
const PROJECTILE_MIN_HAND_HEIGHT: f64 = 0.75;
const PROJECTILE_MAX_AIM_SLOPE: f64 = 2.0;

pub trait TeleportCollisionWorld {
    fn block_state_at(&self, pos: BlockPos) -> Option<BlockStateId>;
}

impl TeleportCollisionWorld for ClientRuntime {
    fn block_state_at(&self, pos: BlockPos) -> Option<BlockStateId> {
        self.block_state_at_block_pos(pos)
    }
}

impl<F> TeleportCollisionWorld for F
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    fn block_state_at(&self, pos: BlockPos) -> Option<BlockStateId> {
        self(pos)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeleportCollisionSnapshot {
    bounds: Option<TeleportCollisionSnapshotBounds>,
    blocks: BTreeMap<BlockPos, BlockStateId>,
}

impl TeleportCollisionSnapshot {
    pub fn capture<W>(world: &W, intent: TeleportIntent, config: TeleportConfig) -> Self
    where
        W: TeleportCollisionWorld + ?Sized,
    {
        let Some(bounds) = teleport_collision_snapshot_bounds(intent, config) else {
            return Self::empty();
        };
        let mut blocks = BTreeMap::new();
        for y in bounds.min.y..=bounds.max.y {
            for z in bounds.min.z..=bounds.max.z {
                for x in bounds.min.x..=bounds.max.x {
                    let pos = BlockPos::new(x, y, z);
                    if let Some(state) = world.block_state_at(pos) {
                        blocks.insert(pos, state);
                    }
                }
            }
        }
        Self {
            bounds: Some(bounds),
            blocks,
        }
    }

    pub fn empty() -> Self {
        Self {
            bounds: None,
            blocks: BTreeMap::new(),
        }
    }

    pub const fn bounds(&self) -> Option<TeleportCollisionSnapshotBounds> {
        self.bounds
    }

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

impl Default for TeleportCollisionSnapshot {
    fn default() -> Self {
        Self::empty()
    }
}

impl TeleportCollisionWorld for TeleportCollisionSnapshot {
    fn block_state_at(&self, pos: BlockPos) -> Option<BlockStateId> {
        if self.bounds.is_some_and(|bounds| !bounds.contains(pos)) {
            return None;
        }
        self.blocks.get(&pos).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TeleportCollisionSnapshotBounds {
    pub min: BlockPos,
    pub max: BlockPos,
}

impl TeleportCollisionSnapshotBounds {
    pub const fn new(min: BlockPos, max: BlockPos) -> Self {
        Self { min, max }
    }

    pub const fn contains(self, pos: BlockPos) -> bool {
        self.min.x <= pos.x
            && pos.x <= self.max.x
            && self.min.y <= pos.y
            && pos.y <= self.max.y
            && self.min.z <= pos.z
            && pos.z <= self.max.z
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeleportIntent {
    pub start_feet: Vec3d,
    pub aim_origin: Vec3d,
    pub aim_direction: Vec3d,
    pub current_yaw_degrees: f64,
}

impl TeleportIntent {
    pub const fn new(
        start_feet: Vec3d,
        aim_origin: Vec3d,
        aim_direction: Vec3d,
        current_yaw_degrees: f64,
    ) -> Self {
        Self {
            start_feet,
            aim_origin,
            aim_direction,
            current_yaw_degrees,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeleportConfig {
    pub max_distance: f64,
    pub arc_height: f64,
    pub arc_samples: usize,
    pub candidate_radius: f64,
    pub candidate_spacing: f64,
    pub min_candidate_progress: f64,
    pub body_width: f64,
    pub body_height: f64,
    pub max_step_up_blocks: i32,
    pub max_drop_blocks: i32,
    pub max_visited_nodes: usize,
    pub marker_height: f64,
}

impl Default for TeleportConfig {
    fn default() -> Self {
        Self {
            max_distance: DEFAULT_MAX_DISTANCE,
            arc_height: DEFAULT_ARC_HEIGHT,
            arc_samples: DEFAULT_ARC_SAMPLES,
            candidate_radius: DEFAULT_CANDIDATE_RADIUS,
            candidate_spacing: DEFAULT_CANDIDATE_SPACING,
            min_candidate_progress: DEFAULT_MIN_PROGRESS,
            body_width: LOCAL_PLAYER_STANDING_WIDTH,
            body_height: LOCAL_PLAYER_STANDING_HEIGHT,
            max_step_up_blocks: DEFAULT_MAX_STEP_UP,
            max_drop_blocks: DEFAULT_MAX_DROP,
            max_visited_nodes: DEFAULT_MAX_VISITED_NODES,
            marker_height: DEFAULT_MARKER_HEIGHT,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeleportValidityReason {
    Valid,
    InvalidIntent,
    StartUnloaded,
    StartBlocked,
    StartUnsupported,
    NoCandidate,
    NoReachableCandidate,
}

/// Loaded collision facts for admitting an already-selected standing pose.
///
/// Scene transitions reuse the same body/support probes as teleport instead
/// of maintaining a second platform- or feature-specific collision test.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StandingPoseFacts {
    pub body_loaded: bool,
    pub body_clear: bool,
    pub support_loaded: bool,
    pub solid_support: bool,
}

impl StandingPoseFacts {
    pub const fn supported(self) -> bool {
        self.body_loaded && self.body_clear && self.support_loaded && self.solid_support
    }
}

pub fn standing_pose_facts<W>(world: &W, feet: Vec3d) -> StandingPoseFacts
where
    W: TeleportCollisionWorld + ?Sized,
{
    standing_pose_facts_with_config(world, feet, TeleportConfig::default())
}

fn standing_pose_facts_with_config<W>(
    world: &W,
    feet: Vec3d,
    config: TeleportConfig,
) -> StandingPoseFacts
where
    W: TeleportCollisionWorld + ?Sized,
{
    if !feet.is_finite() {
        return StandingPoseFacts::default();
    }
    let body = player_body_aabb(feet, config);
    let body_loaded = area_loaded(world, body);
    let body_clear = body_loaded
        && mclone_blocks::solid_block_aabbs_in(|pos| world.block_state_at(pos), body).is_empty();
    let support = support_probe(body, feet.y);
    let support_loaded = area_loaded(world, support);
    let solid_support = support_loaded
        && !mclone_blocks::solid_block_aabbs_in(|pos| world.block_state_at(pos), support)
            .is_empty();
    StandingPoseFacts {
        body_loaded,
        body_clear,
        support_loaded,
        solid_support,
    }
}

impl TeleportValidityReason {
    pub const fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeleportResolverDiagnostics {
    pub candidate_count: usize,
    pub searched_candidates: usize,
    pub path: Option<PathSearchDiagnostics>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeleportPreview {
    pub validity: TeleportValidityReason,
    pub target_feet: Option<Vec3d>,
    pub target_yaw_degrees: f64,
    pub marker_dot: Option<Vec3d>,
    pub coarse_target: Option<BlockPos>,
    pub path: Vec<BlockPos>,
    pub arc_points: Vec<Vec3d>,
    pub diagnostics: TeleportResolverDiagnostics,
}

impl TeleportPreview {
    fn invalid(
        validity: TeleportValidityReason,
        target_yaw_degrees: f64,
        arc_points: Vec<Vec3d>,
        diagnostics: TeleportResolverDiagnostics,
    ) -> Self {
        Self {
            validity,
            target_feet: None,
            target_yaw_degrees,
            marker_dot: None,
            coarse_target: None,
            path: Vec::new(),
            arc_points,
            diagnostics,
        }
    }

    pub const fn is_valid(&self) -> bool {
        self.validity.is_valid()
    }
}

pub type TeleportPreviewRequestId = u64;

#[derive(Clone, Debug, PartialEq)]
pub struct TeleportPreviewRequest {
    pub id: TeleportPreviewRequestId,
    pub intent: TeleportIntent,
    pub config: TeleportConfig,
    pub collision: TeleportCollisionSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeleportPreviewResult {
    pub id: TeleportPreviewRequestId,
    pub preview: TeleportPreview,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TeleportPreviewServiceError {
    Unavailable,
    Failed(String),
}

impl fmt::Display for TeleportPreviewServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("teleport preview capability is unavailable"),
            Self::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for TeleportPreviewServiceError {}

/// Target-neutral asynchronous preview service. Native hosts attach the OS
/// worker adapter; browser hosts may attach a worker-backed compiler service or
/// leave the capability explicitly unavailable.
pub trait TeleportPreviewService: Send {
    fn submit_snapshot(
        &mut self,
        intent: TeleportIntent,
        config: TeleportConfig,
        collision: TeleportCollisionSnapshot,
    ) -> Result<TeleportPreviewRequestId, TeleportPreviewServiceError>;

    fn try_recv_latest(
        &mut self,
    ) -> Result<Option<TeleportPreviewResult>, TeleportPreviewServiceError>;
}

type TeleportPreviewServiceFactory =
    dyn Fn() -> Result<Box<dyn TeleportPreviewService>, TeleportPreviewServiceError> + Send + Sync;

/// Optional host capability retained by shared scene orchestration.
pub enum TeleportPreviewCapability {
    Unavailable,
    Available {
        factory: Box<TeleportPreviewServiceFactory>,
        service: Option<Box<dyn TeleportPreviewService>>,
    },
}

impl TeleportPreviewCapability {
    pub fn available(
        factory: impl Fn() -> Result<Box<dyn TeleportPreviewService>, TeleportPreviewServiceError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self::Available {
            factory: Box::new(factory),
            service: None,
        }
    }

    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available { .. })
    }

    pub fn ensure_started(&mut self) -> Result<bool, TeleportPreviewServiceError> {
        match self {
            Self::Unavailable => Ok(false),
            Self::Available { factory, service } => {
                if service.is_none() {
                    *service = Some(factory()?);
                }
                Ok(true)
            }
        }
    }

    pub fn submit_snapshot(
        &mut self,
        intent: TeleportIntent,
        config: TeleportConfig,
        collision: TeleportCollisionSnapshot,
    ) -> Result<Option<TeleportPreviewRequestId>, TeleportPreviewServiceError> {
        if !self.ensure_started()? {
            return Ok(None);
        }
        let Self::Available {
            service: Some(service),
            ..
        } = self
        else {
            return Err(TeleportPreviewServiceError::Unavailable);
        };
        service.submit_snapshot(intent, config, collision).map(Some)
    }

    pub fn try_recv_latest(
        &mut self,
    ) -> Result<Option<TeleportPreviewResult>, TeleportPreviewServiceError> {
        let Self::Available {
            service: Some(service),
            ..
        } = self
        else {
            return Ok(None);
        };
        service.try_recv_latest()
    }

    pub fn reset_service(&mut self) {
        if let Self::Available { service, .. } = self {
            *service = None;
        }
    }
}

impl Default for TeleportPreviewCapability {
    fn default() -> Self {
        Self::Unavailable
    }
}

impl fmt::Debug for TeleportPreviewCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TeleportPreviewCapability")
            .field("available", &self.is_available())
            .finish()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub enum TeleportPreviewWorkerError {
    Spawn(std::io::Error),
    Disconnected,
    QueuePoisoned,
    Shutdown,
}

#[cfg(not(target_arch = "wasm32"))]
impl fmt::Display for TeleportPreviewWorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) => write!(f, "failed to spawn teleport preview worker: {error}"),
            Self::Disconnected => write!(f, "teleport preview worker disconnected"),
            Self::QueuePoisoned => write!(f, "teleport preview worker queue was poisoned"),
            Self::Shutdown => write!(f, "teleport preview worker has shut down"),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::error::Error for TeleportPreviewWorkerError {}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct NativeTeleportPreviewWorker {
    queue: Arc<NativeTeleportPreviewQueue>,
    receiver: mpsc::Receiver<TeleportPreviewResult>,
    handle: Option<thread::JoinHandle<()>>,
    next_request_id: TeleportPreviewRequestId,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeTeleportPreviewWorker {
    pub fn new() -> Result<Self, TeleportPreviewWorkerError> {
        let queue = Arc::new(NativeTeleportPreviewQueue::new());
        let (sender, receiver) = mpsc::channel();
        let handle = {
            let queue = Arc::clone(&queue);
            thread::Builder::new()
                .name("mclone-teleport-preview".to_owned())
                .spawn(move || run_native_teleport_preview_worker(queue, sender))
                .map_err(TeleportPreviewWorkerError::Spawn)?
        };
        Ok(Self {
            queue,
            receiver,
            handle: Some(handle),
            next_request_id: 0,
        })
    }

    pub fn submit_from_world<W>(
        &mut self,
        world: &W,
        intent: TeleportIntent,
        config: TeleportConfig,
    ) -> Result<TeleportPreviewRequestId, TeleportPreviewWorkerError>
    where
        W: TeleportCollisionWorld + ?Sized,
    {
        let collision = TeleportCollisionSnapshot::capture(world, intent, config);
        self.submit_snapshot(intent, config, collision)
    }

    pub fn submit_snapshot(
        &mut self,
        intent: TeleportIntent,
        config: TeleportConfig,
        collision: TeleportCollisionSnapshot,
    ) -> Result<TeleportPreviewRequestId, TeleportPreviewWorkerError> {
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        let id = self.next_request_id;
        self.queue.submit(TeleportPreviewRequest {
            id,
            intent,
            config,
            collision,
        })?;
        Ok(id)
    }

    pub fn try_recv_latest(
        &mut self,
    ) -> Result<Option<TeleportPreviewResult>, TeleportPreviewWorkerError> {
        let mut latest = None;
        loop {
            match self.receiver.try_recv() {
                Ok(result) => latest = Some(result),
                Err(mpsc::TryRecvError::Empty) => return Ok(latest),
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(TeleportPreviewWorkerError::Disconnected);
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl TeleportPreviewService for NativeTeleportPreviewWorker {
    fn submit_snapshot(
        &mut self,
        intent: TeleportIntent,
        config: TeleportConfig,
        collision: TeleportCollisionSnapshot,
    ) -> Result<TeleportPreviewRequestId, TeleportPreviewServiceError> {
        NativeTeleportPreviewWorker::submit_snapshot(self, intent, config, collision)
            .map_err(|error| TeleportPreviewServiceError::Failed(error.to_string()))
    }

    fn try_recv_latest(
        &mut self,
    ) -> Result<Option<TeleportPreviewResult>, TeleportPreviewServiceError> {
        NativeTeleportPreviewWorker::try_recv_latest(self)
            .map_err(|error| TeleportPreviewServiceError::Failed(error.to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn native_teleport_preview_capability() -> TeleportPreviewCapability {
    TeleportPreviewCapability::available(|| {
        NativeTeleportPreviewWorker::new()
            .map(|worker| Box::new(worker) as Box<dyn TeleportPreviewService>)
            .map_err(|error| TeleportPreviewServiceError::Failed(error.to_string()))
    })
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for NativeTeleportPreviewWorker {
    fn drop(&mut self) {
        self.queue.shutdown();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct NativeTeleportPreviewQueue {
    state: Mutex<NativeTeleportPreviewQueueState>,
    request_available: Condvar,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct NativeTeleportPreviewQueueState {
    latest: Option<TeleportPreviewRequest>,
    shutdown: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeTeleportPreviewQueue {
    fn new() -> Self {
        Self {
            state: Mutex::new(NativeTeleportPreviewQueueState {
                latest: None,
                shutdown: false,
            }),
            request_available: Condvar::new(),
        }
    }

    fn submit(&self, request: TeleportPreviewRequest) -> Result<(), TeleportPreviewWorkerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| TeleportPreviewWorkerError::QueuePoisoned)?;
        if state.shutdown {
            return Err(TeleportPreviewWorkerError::Shutdown);
        }
        state.latest = Some(request);
        self.request_available.notify_one();
        Ok(())
    }

    fn take_next_blocking(&self) -> Option<TeleportPreviewRequest> {
        let mut state = self.state.lock().ok()?;
        loop {
            if state.shutdown {
                return None;
            }
            if state.latest.is_some() {
                return state.latest.take();
            }
            state = self.request_available.wait(state).ok()?;
        }
    }

    fn shutdown(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.shutdown = true;
            state.latest = None;
            self.request_available.notify_all();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_native_teleport_preview_worker(
    queue: Arc<NativeTeleportPreviewQueue>,
    sender: mpsc::Sender<TeleportPreviewResult>,
) {
    while let Some(request) = queue.take_next_blocking() {
        let preview = resolve_teleport_preview(&request.collision, request.intent, request.config);
        if sender
            .send(TeleportPreviewResult {
                id: request.id,
                preview,
            })
            .is_err()
        {
            break;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SearchBasis {
    start_feet: Vec3d,
    origin: Vec3d,
    forward: Vec3d,
    right: Vec3d,
    aim_slope: f64,
    intent_distance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Candidate {
    node: BlockPos,
    feet: Vec3d,
    progress: f64,
    lateral_distance: f64,
    vertical_delta: i32,
    score: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StandFailure {
    Unloaded,
    Blocked,
    Unsupported,
}

pub fn resolve_teleport_preview<W>(
    world: &W,
    intent: TeleportIntent,
    config: TeleportConfig,
) -> TeleportPreview
where
    W: TeleportCollisionWorld + ?Sized,
{
    let config = sanitize_config(config);
    let Some(basis) = SearchBasis::new(intent, config) else {
        return TeleportPreview::invalid(
            TeleportValidityReason::InvalidIntent,
            intent.current_yaw_degrees,
            Vec::new(),
            TeleportResolverDiagnostics {
                candidate_count: 0,
                searched_candidates: 0,
                path: None,
            },
        );
    };

    let arc_points = build_intent_arc(basis, config);
    let target_yaw_degrees = intent.current_yaw_degrees;
    let start_node = BlockPos::containing(intent.start_feet);
    if let Err(reason) = validate_standing_pose(world, intent.start_feet, config) {
        return TeleportPreview::invalid(
            start_failure_reason(reason),
            target_yaw_degrees,
            arc_points,
            TeleportResolverDiagnostics {
                candidate_count: 0,
                searched_candidates: 0,
                path: None,
            },
        );
    }

    let candidates = collect_candidates(world, basis, config);
    if candidates.is_empty() {
        return TeleportPreview::invalid(
            TeleportValidityReason::NoCandidate,
            target_yaw_degrees,
            arc_points,
            TeleportResolverDiagnostics {
                candidate_count: 0,
                searched_candidates: 0,
                path: None,
            },
        );
    }

    let search = PathSearch::new(PathSearchLimits {
        max_visited_nodes: config.max_visited_nodes,
    });
    let mut diagnostics = TeleportResolverDiagnostics {
        candidate_count: candidates.len(),
        searched_candidates: 0,
        path: None,
    };

    for candidate in candidates.iter().rev() {
        diagnostics.searched_candidates += 1;
        let path = search.find_path(
            PathSearchQuery::new(start_node, candidate.node)
                .with_follow_range(config.max_distance as f32 + config.max_drop_blocks as f32 + 2.0)
                .with_heuristic_weight(DEFAULT_PATH_HEURISTIC_WEIGHT),
            |node| path_neighbors(world, basis, config, node),
            |pos, target| pos == target,
        );
        diagnostics.path = Some(path.diagnostics());

        let Some(&resolved_node) = path.nodes().last() else {
            continue;
        };
        let resolved_progress = node_progress(basis, resolved_node);
        if resolved_progress < config.min_candidate_progress {
            continue;
        }
        let Some(resolved_feet) = refine_feet_in_node(world, resolved_node, candidate.feet, config)
        else {
            continue;
        };

        let marker_dot = marker_dot_for(resolved_feet, resolved_progress, basis, config);
        return TeleportPreview {
            validity: TeleportValidityReason::Valid,
            target_feet: Some(resolved_feet),
            target_yaw_degrees,
            marker_dot: Some(marker_dot),
            coarse_target: Some(resolved_node),
            path: path.into_nodes(),
            arc_points,
            diagnostics,
        };
    }

    TeleportPreview::invalid(
        TeleportValidityReason::NoReachableCandidate,
        target_yaw_degrees,
        arc_points,
        diagnostics,
    )
}

impl SearchBasis {
    fn new(intent: TeleportIntent, config: TeleportConfig) -> Option<Self> {
        if !intent.start_feet.is_finite()
            || !intent.aim_origin.is_finite()
            || !intent.aim_direction.is_finite()
        {
            return None;
        }
        let aim = normalize_or_zero(intent.aim_direction);
        let horizontal = Vec3d::new(aim.x, 0.0, aim.z);
        let forward = normalize_or_zero(horizontal);
        if forward == Vec3d::ZERO {
            return None;
        }
        let horizontal_len = horizontal.length_sqr().sqrt();
        if horizontal_len <= TELEPORT_EPSILON {
            return None;
        }
        let aim_slope =
            (aim.y / horizontal_len).clamp(-PROJECTILE_MAX_AIM_SLOPE, PROJECTILE_MAX_AIM_SLOPE);
        let intent_distance =
            projectile_intent_distance(intent.start_feet.y, intent.aim_origin.y, aim_slope, config);
        let right = Vec3d::new(forward.z, 0.0, -forward.x);
        Some(Self {
            start_feet: intent.start_feet,
            origin: intent.aim_origin,
            forward,
            right,
            aim_slope,
            intent_distance,
        })
    }
}

fn teleport_collision_snapshot_bounds(
    intent: TeleportIntent,
    config: TeleportConfig,
) -> Option<TeleportCollisionSnapshotBounds> {
    let config = sanitize_config(config);
    let basis = SearchBasis::new(intent, config)?;
    let start_node = BlockPos::containing(basis.start_feet);
    let xz_margin = config.body_width * 0.5 + 1.0;
    let lateral_extent = config.candidate_radius + xz_margin + 0.75;
    let progress_min = -1.0;
    let progress_max = config.max_distance + 1.0;
    let mut min_x = f64::INFINITY;
    let mut min_z = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_z = f64::NEG_INFINITY;

    include_snapshot_xz_point(
        basis.start_feet,
        xz_margin,
        &mut min_x,
        &mut min_z,
        &mut max_x,
        &mut max_z,
    );
    include_snapshot_xz_point(
        basis.origin,
        xz_margin,
        &mut min_x,
        &mut min_z,
        &mut max_x,
        &mut max_z,
    );
    for progress in [progress_min, progress_max] {
        for lateral in [-lateral_extent, lateral_extent] {
            let point = basis
                .origin
                .add(basis.forward.scale(progress))
                .add(basis.right.scale(lateral));
            include_snapshot_xz_point(
                point, xz_margin, &mut min_x, &mut min_z, &mut max_x, &mut max_z,
            );
        }
    }

    Some(TeleportCollisionSnapshotBounds::new(
        BlockPos::new(
            min_x.floor() as i32,
            start_node.y - config.max_drop_blocks - 1,
            min_z.floor() as i32,
        ),
        BlockPos::new(
            max_x.floor() as i32,
            start_node.y + config.max_step_up_blocks + config.body_height.ceil() as i32 + 1,
            max_z.floor() as i32,
        ),
    ))
}

fn include_snapshot_xz_point(
    point: Vec3d,
    margin: f64,
    min_x: &mut f64,
    min_z: &mut f64,
    max_x: &mut f64,
    max_z: &mut f64,
) {
    *min_x = (*min_x).min(point.x - margin);
    *min_z = (*min_z).min(point.z - margin);
    *max_x = (*max_x).max(point.x + margin);
    *max_z = (*max_z).max(point.z + margin);
}

fn sanitize_config(mut config: TeleportConfig) -> TeleportConfig {
    if !config.max_distance.is_finite() || config.max_distance <= 0.0 {
        config.max_distance = DEFAULT_MAX_DISTANCE;
    }
    if !config.arc_height.is_finite() || config.arc_height < 0.0 {
        config.arc_height = DEFAULT_ARC_HEIGHT;
    }
    config.arc_samples = config.arc_samples.max(2);
    if !config.candidate_radius.is_finite() || config.candidate_radius < 0.0 {
        config.candidate_radius = DEFAULT_CANDIDATE_RADIUS;
    }
    if !config.candidate_spacing.is_finite() || config.candidate_spacing <= 0.0 {
        config.candidate_spacing = DEFAULT_CANDIDATE_SPACING;
    }
    if !config.min_candidate_progress.is_finite() || config.min_candidate_progress < 0.0 {
        config.min_candidate_progress = DEFAULT_MIN_PROGRESS;
    }
    if !config.body_width.is_finite() || config.body_width <= 0.0 {
        config.body_width = LOCAL_PLAYER_STANDING_WIDTH;
    }
    if !config.body_height.is_finite() || config.body_height <= 0.0 {
        config.body_height = LOCAL_PLAYER_STANDING_HEIGHT;
    }
    config.max_step_up_blocks = config.max_step_up_blocks.max(0);
    config.max_drop_blocks = config.max_drop_blocks.max(0);
    config.max_visited_nodes = config.max_visited_nodes.max(1);
    if !config.marker_height.is_finite() || config.marker_height <= 0.0 {
        config.marker_height = DEFAULT_MARKER_HEIGHT;
    }
    config
}

fn build_intent_arc(basis: SearchBasis, config: TeleportConfig) -> Vec<Vec3d> {
    let mut points = Vec::with_capacity(config.arc_samples + 1);
    for index in 0..=config.arc_samples {
        let t = index as f64 / config.arc_samples as f64;
        points.push(arc_point_at_progress(
            basis,
            config,
            basis.intent_distance * t,
        ));
    }
    points
}

fn arc_point_at_progress(basis: SearchBasis, config: TeleportConfig, progress: f64) -> Vec3d {
    let gravity = projectile_gravity_per_block(basis, config);
    basis
        .origin
        .add(basis.forward.scale(progress))
        .add(Vec3d::new(
            0.0,
            basis.aim_slope * progress - gravity * progress * progress,
            0.0,
        ))
}

fn projectile_intent_distance(
    start_feet_y: f64,
    aim_origin_y: f64,
    aim_slope: f64,
    config: TeleportConfig,
) -> f64 {
    let hand_height = projectile_hand_height(start_feet_y, aim_origin_y, config);
    let gravity = hand_height / (config.max_distance * config.max_distance);
    if gravity <= TELEPORT_EPSILON {
        return config.max_distance;
    }
    let discriminant = aim_slope * aim_slope + 4.0 * gravity * hand_height;
    if !discriminant.is_finite() || discriminant < 0.0 {
        return config.max_distance;
    }
    let distance = (aim_slope + discriminant.sqrt()) / (2.0 * gravity);
    if !distance.is_finite() {
        return config.max_distance;
    }
    distance.clamp(config.min_candidate_progress, config.max_distance)
}

fn projectile_hand_height(start_feet_y: f64, aim_origin_y: f64, config: TeleportConfig) -> f64 {
    (aim_origin_y - start_feet_y)
        .max(config.arc_height)
        .max(PROJECTILE_MIN_HAND_HEIGHT)
}

fn projectile_gravity_per_block(basis: SearchBasis, config: TeleportConfig) -> f64 {
    projectile_hand_height(basis.start_feet.y, basis.origin.y, config)
        / (config.max_distance * config.max_distance)
}

fn collect_candidates<W>(world: &W, basis: SearchBasis, config: TeleportConfig) -> Vec<Candidate>
where
    W: TeleportCollisionWorld + ?Sized,
{
    let mut by_node: BTreeMap<BlockPos, Candidate> = BTreeMap::new();
    let lateral_steps = (config.candidate_radius / config.candidate_spacing).ceil() as i32;
    let start_y = BlockPos::containing(basis.start_feet).y;

    for sample in 1..=config.arc_samples {
        let t = sample as f64 / config.arc_samples as f64;
        let progress = basis.intent_distance * t;
        let arc = arc_point_at_progress(basis, config, progress);
        if progress < config.min_candidate_progress {
            continue;
        }
        for lateral_index in -lateral_steps..=lateral_steps {
            let lateral_offset = lateral_index as f64 * config.candidate_spacing;
            if lateral_offset.abs() > config.candidate_radius + TELEPORT_EPSILON {
                continue;
            }
            let preferred = arc.add(basis.right.scale(lateral_offset));
            for foot_y in (start_y - config.max_drop_blocks)..=(start_y + config.max_step_up_blocks)
            {
                let preferred_feet = Vec3d::new(preferred.x, foot_y as f64, preferred.z);
                let node = BlockPos::containing(preferred_feet);
                if !node_inside_search_window(basis, config, node) {
                    continue;
                }
                let Some(feet) = refine_feet_in_node(world, node, preferred_feet, config) else {
                    continue;
                };
                let progress = progress_for_feet(basis, feet);
                if progress < config.min_candidate_progress {
                    continue;
                }
                let lateral_distance = lateral_distance_for_feet(basis, feet);
                let vertical_delta = node.y - start_y;
                let score = candidate_score(basis, progress, lateral_distance, vertical_delta);
                let candidate = Candidate {
                    node,
                    feet,
                    progress,
                    lateral_distance,
                    vertical_delta,
                    score,
                };
                by_node
                    .entry(node)
                    .and_modify(|existing| {
                        if candidate.score > existing.score {
                            *existing = candidate;
                        }
                    })
                    .or_insert(candidate);
            }
        }
    }

    let mut candidates = by_node.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.score
            .total_cmp(&right.score)
            .then_with(|| left.progress.total_cmp(&right.progress))
            .then_with(|| right.lateral_distance.total_cmp(&left.lateral_distance))
            .then_with(|| right.vertical_delta.cmp(&left.vertical_delta))
            .then_with(|| right.node.cmp(&left.node))
    });
    candidates
}

fn path_neighbors<W>(
    world: &W,
    basis: SearchBasis,
    config: TeleportConfig,
    node: BlockPos,
) -> Vec<PathNeighbor>
where
    W: TeleportCollisionWorld + ?Sized,
{
    let mut neighbors = Vec::with_capacity(12);
    for (dx, dz) in [(1, 0), (0, -1), (0, 1), (-1, 0)] {
        let x = node.x + dx;
        let z = node.z + dz;
        for y in (node.y - config.max_drop_blocks)..=(node.y + config.max_step_up_blocks) {
            let next = BlockPos::new(x, y, z);
            if !node_inside_search_window(basis, config, next) {
                continue;
            }
            let preferred = node_center(next);
            if validate_standing_pose(world, preferred, config).is_err() {
                continue;
            }
            let vertical_cost = (next.y - node.y).abs() as f32 * 0.5;
            neighbors.push(PathNeighbor::new(next, vertical_cost));
        }
    }
    neighbors
}

fn node_inside_search_window(basis: SearchBasis, config: TeleportConfig, node: BlockPos) -> bool {
    let progress = node_progress(basis, node);
    let lateral = node_lateral_distance(basis, node);
    progress >= -0.5
        && progress <= basis.intent_distance + 0.75
        && lateral <= config.candidate_radius + 0.75
        && node.y >= BlockPos::containing(basis.start_feet).y - config.max_drop_blocks
        && node.y <= BlockPos::containing(basis.start_feet).y + config.max_step_up_blocks
}

fn validate_standing_pose<W>(
    world: &W,
    feet: Vec3d,
    config: TeleportConfig,
) -> Result<(), StandFailure>
where
    W: TeleportCollisionWorld + ?Sized,
{
    let facts = standing_pose_facts_with_config(world, feet, config);
    if !facts.body_loaded {
        return Err(StandFailure::Unloaded);
    }
    if !facts.body_clear {
        return Err(StandFailure::Blocked);
    }
    if !facts.support_loaded {
        return Err(StandFailure::Unloaded);
    }
    if !facts.solid_support {
        return Err(StandFailure::Unsupported);
    }

    Ok(())
}

fn refine_feet_in_node<W>(
    world: &W,
    node: BlockPos,
    preferred_feet: Vec3d,
    config: TeleportConfig,
) -> Option<Vec3d>
where
    W: TeleportCollisionWorld + ?Sized,
{
    let preferred_x = preferred_feet.x.clamp(
        node.x as f64 + TELEPORT_EPSILON,
        node.x as f64 + 1.0 - TELEPORT_EPSILON,
    );
    let preferred_z = preferred_feet.z.clamp(
        node.z as f64 + TELEPORT_EPSILON,
        node.z as f64 + 1.0 - TELEPORT_EPSILON,
    );
    let center = node_center(node);
    let mut probes = vec![Vec3d::new(preferred_x, node.y as f64, preferred_z), center];
    for radius in [0.15, 0.3] {
        for (dx, dz) in [
            (radius, 0.0),
            (-radius, 0.0),
            (0.0, radius),
            (0.0, -radius),
            (radius, radius),
            (radius, -radius),
            (-radius, radius),
            (-radius, -radius),
        ] {
            probes.push(Vec3d::new(
                (preferred_x + dx).clamp(
                    node.x as f64 + TELEPORT_EPSILON,
                    node.x as f64 + 1.0 - TELEPORT_EPSILON,
                ),
                node.y as f64,
                (preferred_z + dz).clamp(
                    node.z as f64 + TELEPORT_EPSILON,
                    node.z as f64 + 1.0 - TELEPORT_EPSILON,
                ),
            ));
        }
    }

    probes.sort_by(|left, right| {
        left.distance_to_sqr(preferred_feet)
            .total_cmp(&right.distance_to_sqr(preferred_feet))
    });
    probes.dedup_by(|left, right| left.distance_to_sqr(*right) < 1.0e-12);
    probes
        .into_iter()
        .find(|feet| validate_standing_pose(world, *feet, config).is_ok())
}

fn area_loaded<W>(world: &W, area: Aabb) -> bool
where
    W: TeleportCollisionWorld + ?Sized,
{
    if !area.is_finite() {
        return false;
    }
    let min_x = area.min_x.floor() as i32;
    let min_y = area.min_y.floor() as i32;
    let min_z = area.min_z.floor() as i32;
    let max_x = area.max_x.floor() as i32;
    let max_y = area.max_y.floor() as i32;
    let max_z = area.max_z.floor() as i32;
    for y in min_y..=max_y {
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                if world.block_state_at(BlockPos::new(x, y, z)).is_none() {
                    return false;
                }
            }
        }
    }
    true
}

fn player_body_aabb(feet: Vec3d, config: TeleportConfig) -> Aabb {
    mclone_blocks::collision_aabb_for_feet_position(feet, config.body_width, config.body_height)
}

fn support_probe(body: Aabb, feet_y: f64) -> Aabb {
    Aabb::new(
        body.min_x + TELEPORT_EPSILON,
        feet_y - SUPPORT_PROBE_DEPTH,
        body.min_z + TELEPORT_EPSILON,
        body.max_x - TELEPORT_EPSILON,
        feet_y,
        body.max_z - TELEPORT_EPSILON,
    )
}

fn candidate_score(
    basis: SearchBasis,
    progress: f64,
    lateral_distance: f64,
    vertical_delta: i32,
) -> f64 {
    let intent_error = (progress - basis.intent_distance).abs();
    -intent_error * 10.0 - lateral_distance * 2.0 - (vertical_delta.abs() as f64 * 0.75)
        + progress * 0.05
}

fn marker_dot_for(feet: Vec3d, progress: f64, basis: SearchBasis, config: TeleportConfig) -> Vec3d {
    let arc_y = arc_point_at_progress(basis, config, progress.clamp(0.0, basis.intent_distance)).y;
    Vec3d::new(feet.x, arc_y.max(feet.y + config.marker_height), feet.z)
}

fn progress_for_feet(basis: SearchBasis, feet: Vec3d) -> f64 {
    let delta = feet.subtract(basis.start_feet);
    dot_xz(delta, basis.forward)
}

fn lateral_distance_for_feet(basis: SearchBasis, feet: Vec3d) -> f64 {
    dot_xz(feet.subtract(basis.origin), basis.right).abs()
}

fn node_progress(basis: SearchBasis, node: BlockPos) -> f64 {
    progress_for_feet(basis, node_center(node))
}

fn node_lateral_distance(basis: SearchBasis, node: BlockPos) -> f64 {
    lateral_distance_for_feet(basis, node_center(node))
}

fn node_center(node: BlockPos) -> Vec3d {
    Vec3d::new(node.x as f64 + 0.5, node.y as f64, node.z as f64 + 0.5)
}

fn dot_xz(left: Vec3d, right: Vec3d) -> f64 {
    left.x * right.x + left.z * right.z
}

fn start_failure_reason(reason: StandFailure) -> TeleportValidityReason {
    match reason {
        StandFailure::Unloaded => TeleportValidityReason::StartUnloaded,
        StandFailure::Blocked => TeleportValidityReason::StartBlocked,
        StandFailure::Unsupported => TeleportValidityReason::StartUnsupported,
    }
}

fn normalize_or_zero(value: Vec3d) -> Vec3d {
    let len_sqr = value.length_sqr();
    if len_sqr <= TELEPORT_EPSILON * TELEPORT_EPSILON {
        Vec3d::ZERO
    } else {
        value.scale(1.0 / len_sqr.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::AIR_BLOCK_STATE_ID;

    const SOLID: BlockStateId = BlockStateId(7);

    #[derive(Clone, Debug)]
    struct TestWorld {
        min: BlockPos,
        max: BlockPos,
        blocks: BTreeMap<BlockPos, BlockStateId>,
    }

    impl TestWorld {
        fn loaded(min: BlockPos, max: BlockPos) -> Self {
            Self {
                min,
                max,
                blocks: BTreeMap::new(),
            }
        }

        fn with_flat_floor(
            x: std::ops::RangeInclusive<i32>,
            z: std::ops::RangeInclusive<i32>,
        ) -> Self {
            let min = BlockPos::new(*x.start(), 0, *z.start());
            let max = BlockPos::new(*x.end(), 5, *z.end());
            let mut world = Self::loaded(min, max);
            for x in x {
                for z in z.clone() {
                    world.set_solid(BlockPos::new(x, 0, z));
                }
            }
            world
        }

        fn set_solid(&mut self, pos: BlockPos) {
            self.blocks.insert(pos, SOLID);
        }

        fn fill_wall(
            &mut self,
            x: i32,
            z: std::ops::RangeInclusive<i32>,
            y: std::ops::RangeInclusive<i32>,
        ) {
            for z in z {
                for y in y.clone() {
                    self.set_solid(BlockPos::new(x, y, z));
                }
            }
        }
    }

    impl TeleportCollisionWorld for TestWorld {
        fn block_state_at(&self, pos: BlockPos) -> Option<BlockStateId> {
            if pos.x < self.min.x
                || pos.x > self.max.x
                || pos.y < self.min.y
                || pos.y > self.max.y
                || pos.z < self.min.z
                || pos.z > self.max.z
            {
                return None;
            }
            Some(*self.blocks.get(&pos).unwrap_or(&AIR_BLOCK_STATE_ID))
        }
    }

    fn config(max_distance: f64) -> TeleportConfig {
        TeleportConfig {
            max_distance,
            arc_height: 1.0,
            arc_samples: 20,
            candidate_radius: 1.25,
            candidate_spacing: 0.5,
            min_candidate_progress: 0.75,
            max_visited_nodes: 256,
            ..Default::default()
        }
    }

    fn forward_intent(max_distance: f64) -> (TeleportIntent, TeleportConfig) {
        (
            TeleportIntent::new(
                Vec3d::new(0.5, 1.0, 0.5),
                Vec3d::new(0.5, 2.2, 0.5),
                Vec3d::new(1.0, 0.0, 0.0),
                -90.0,
            ),
            config(max_distance),
        )
    }

    fn assert_approx_eq(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1.0e-9,
            "expected {actual} to be approximately {expected}"
        );
    }

    #[test]
    fn wall_aim_lands_before_obstruction() {
        let mut world = TestWorld::with_flat_floor(-2..=8, -3..=3);
        world.fill_wall(3, -2..=2, 1..=3);
        let (intent, config) = forward_intent(6.0);

        let preview = resolve_teleport_preview(&world, intent, config);

        assert_eq!(preview.validity, TeleportValidityReason::Valid);
        let feet = preview.target_feet.expect("target feet");
        assert!(feet.x <= 2.7, "feet should stop before the wall: {feet:?}");
        assert_eq!(preview.coarse_target, Some(BlockPos::new(2, 1, 0)));
        let marker = preview.marker_dot.expect("marker dot");
        assert_approx_eq(marker.x, feet.x);
        assert_approx_eq(marker.z, feet.z);
    }

    #[test]
    fn low_opening_is_treated_as_blocked_for_player_body() {
        let mut world = TestWorld::with_flat_floor(-2..=8, -3..=3);
        world.fill_wall(3, -2..=2, 1..=3);
        world.blocks.remove(&BlockPos::new(3, 1, 0));
        let (intent, config) = forward_intent(6.0);

        let preview = resolve_teleport_preview(&world, intent, config);

        assert_eq!(preview.validity, TeleportValidityReason::Valid);
        let feet = preview.target_feet.expect("target feet");
        assert!(
            feet.x <= 2.7,
            "feet should not pass through a one-block-high opening: {feet:?}"
        );
        assert_eq!(preview.coarse_target, Some(BlockPos::new(2, 1, 0)));
    }

    #[test]
    fn one_block_up_landing_is_valid_with_support_and_headroom() {
        let mut world = TestWorld::with_flat_floor(-1..=3, -2..=2);
        world.set_solid(BlockPos::new(2, 1, 0));
        let (intent, config) = forward_intent(2.2);

        let preview = resolve_teleport_preview(&world, intent, config);

        assert_eq!(preview.validity, TeleportValidityReason::Valid);
        let feet = preview.target_feet.expect("target feet");
        assert_eq!(preview.coarse_target, Some(BlockPos::new(2, 2, 0)));
        assert_approx_eq(feet.y, 2.0);
    }

    #[test]
    fn unloaded_window_without_forward_candidate_is_invalid() {
        let world = TestWorld::with_flat_floor(0..=0, 0..=0);
        let (intent, config) = forward_intent(5.0);

        let preview = resolve_teleport_preview(&world, intent, config);

        assert_eq!(preview.validity, TeleportValidityReason::NoCandidate);
        assert!(!preview.is_valid());
        assert_eq!(preview.target_feet, None);
    }

    #[test]
    fn refined_feet_pose_can_be_off_block_center() {
        let world = TestWorld::with_flat_floor(-1..=5, -2..=2);
        let intent = TeleportIntent::new(
            Vec3d::new(0.5, 1.0, 0.5),
            Vec3d::new(0.5, 2.2, 0.2),
            Vec3d::new(1.0, 0.0, -0.05),
            -90.0,
        );

        let preview = resolve_teleport_preview(&world, intent, config(3.0));

        assert_eq!(preview.validity, TeleportValidityReason::Valid);
        let feet = preview.target_feet.expect("target feet");
        let block_center_z = feet.z.floor() + 0.5;
        assert!(
            (feet.z - block_center_z).abs() > 0.1,
            "target feet should keep the continuous preferred placement: {feet:?}"
        );
    }

    #[test]
    fn downward_aim_selects_a_nearer_target_than_horizontal_aim() {
        let world = TestWorld::with_flat_floor(-1..=8, -2..=2);
        let (horizontal_intent, config) = forward_intent(6.0);
        let downward_intent = TeleportIntent::new(
            horizontal_intent.start_feet,
            horizontal_intent.aim_origin,
            Vec3d::new(1.0, -0.6, 0.0),
            horizontal_intent.current_yaw_degrees,
        );

        let horizontal = resolve_teleport_preview(&world, horizontal_intent, config);
        let downward = resolve_teleport_preview(&world, downward_intent, config);

        assert_eq!(horizontal.validity, TeleportValidityReason::Valid);
        assert_eq!(downward.validity, TeleportValidityReason::Valid);
        let horizontal_feet = horizontal.target_feet.expect("horizontal feet");
        let downward_feet = downward.target_feet.expect("downward feet");
        assert!(
            downward_feet.x < horizontal_feet.x - 1.5,
            "downward aim should shorten Blink distance: horizontal={horizontal_feet:?} downward={downward_feet:?}"
        );
        assert!(
            downward_feet.x < 3.5,
            "downward aim should allow near Blink targets: {downward_feet:?}"
        );
    }

    #[test]
    fn resolver_preserves_requested_landing_yaw() {
        let world = TestWorld::with_flat_floor(-1..=5, -2..=2);
        let intent = TeleportIntent::new(
            Vec3d::new(0.5, 1.0, 0.5),
            Vec3d::new(0.5, 2.2, 0.5),
            Vec3d::new(1.0, 0.0, 0.0),
            37.0,
        );

        let preview = resolve_teleport_preview(&world, intent, config(3.0));

        assert_eq!(preview.validity, TeleportValidityReason::Valid);
        assert_approx_eq(preview.target_yaw_degrees, 37.0);
    }

    #[test]
    fn collision_snapshot_preserves_loaded_air_and_solid_blocks() {
        let world = TestWorld::with_flat_floor(-1..=5, -2..=2);
        let (intent, config) = forward_intent(3.0);

        let snapshot = TeleportCollisionSnapshot::capture(&world, intent, config);

        assert!(snapshot.block_count() > 0);
        assert_eq!(snapshot.block_state_at(BlockPos::new(0, 0, 0)), Some(SOLID));
        assert_eq!(
            snapshot.block_state_at(BlockPos::new(0, 1, 0)),
            Some(AIR_BLOCK_STATE_ID)
        );
        assert_eq!(snapshot.block_state_at(BlockPos::new(32, 1, 0)), None);
    }

    #[test]
    fn unavailable_preview_capability_is_explicit_and_does_not_submit() {
        let mut capability = TeleportPreviewCapability::Unavailable;
        let (intent, config) = forward_intent(3.0);

        assert!(!capability.is_available());
        assert!(!capability.ensure_started().unwrap());
        assert_eq!(
            capability
                .submit_snapshot(intent, config, TeleportCollisionSnapshot::empty())
                .unwrap(),
            None
        );
        assert_eq!(capability.try_recv_latest().unwrap(), None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_preview_queue_replaces_pending_request_with_latest() {
        let queue = NativeTeleportPreviewQueue::new();
        let (intent, config) = forward_intent(2.0);
        let collision = TeleportCollisionSnapshot::empty();

        queue
            .submit(TeleportPreviewRequest {
                id: 1,
                intent,
                config,
                collision: collision.clone(),
            })
            .expect("submit first");
        queue
            .submit(TeleportPreviewRequest {
                id: 2,
                intent,
                config,
                collision,
            })
            .expect("submit second");

        let request = queue.take_next_blocking().expect("pending request");
        assert_eq!(request.id, 2);
        queue.shutdown();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_preview_worker_resolves_submitted_snapshot() {
        let world = TestWorld::with_flat_floor(-1..=5, -2..=2);
        let (intent, config) = forward_intent(3.0);
        let mut worker = NativeTeleportPreviewWorker::new().expect("worker");

        let id = worker
            .submit_from_world(&world, intent, config)
            .expect("submit preview request");
        let start = std::time::Instant::now();
        loop {
            if let Some(result) = worker.try_recv_latest().expect("worker result") {
                assert_eq!(result.id, id);
                assert_eq!(result.preview.validity, TeleportValidityReason::Valid);
                assert!(result.preview.target_feet.is_some());
                return;
            }
            assert!(
                start.elapsed() < std::time::Duration::from_secs(2),
                "timed out waiting for native teleport preview worker"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
