use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mclone_worldgen::terrain_preview::{
    TerrainPreviewRequest, TerrainPreviewVegetationProduct,
    terrain_preview_requests_tree_records_for_profile,
};
use mclone_worldgen::terrain_vegetation::{
    TerrainVegetationProductReceipt, TerrainVegetationSourceIdentity,
    terrain_vegetation_product_receipt,
};

use crate::TerrainViewportTileId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerrainVegetationExecutorKind {
    InlineTest,
    NativeThread,
    BrowserWorker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainVegetationExecutorActor {
    pub executor_generation: u32,
    pub source_epoch: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TerrainVegetationSlotToken {
    pub physical_slot: u32,
    pub slot_generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainVegetationDesiredTile {
    pub tile: TerrainViewportTileId,
    pub slot: TerrainVegetationSlotToken,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainVegetationJobIdentity {
    pub actor: TerrainVegetationExecutorActor,
    pub request_id: u32,
    pub tile: TerrainViewportTileId,
    pub slot: TerrainVegetationSlotToken,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainVegetationExecutorJob {
    pub identity: TerrainVegetationJobIdentity,
    pub source: TerrainVegetationSourceIdentity,
    pub request: TerrainPreviewRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerrainVegetationExecutorEvent {
    Ready {
        actor: TerrainVegetationExecutorActor,
        source: TerrainVegetationSourceIdentity,
    },
    Completed {
        identity: TerrainVegetationJobIdentity,
        source: TerrainVegetationSourceIdentity,
        product: TerrainPreviewVegetationProduct,
        compile_micros: u64,
    },
    JobFailed {
        identity: TerrainVegetationJobIdentity,
        error: String,
    },
    TransportFailed {
        actor: TerrainVegetationExecutorActor,
        error: String,
    },
    ShutdownComplete {
        actor: TerrainVegetationExecutorActor,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerrainVegetationSubmitError {
    Full,
    Failed(String),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainVegetationExecutorDiagnostics {
    pub submitted_jobs: u64,
    pub completed_jobs: u64,
    pub transport_failures: u64,
    pub restarts: u64,
    pub result_capacity_bytes: u64,
    pub result_high_water_bytes: u64,
    pub result_overflows: u64,
    pub copied_result_bytes: u64,
    pub main_decode_micros: u64,
}

pub trait TerrainVegetationExecutor {
    fn kind(&self) -> TerrainVegetationExecutorKind;

    fn try_submit(
        &mut self,
        job: &TerrainVegetationExecutorJob,
    ) -> Result<(), TerrainVegetationSubmitError>;

    fn drain_events(&mut self) -> Vec<TerrainVegetationExecutorEvent>;

    fn restart(
        &mut self,
        actor: TerrainVegetationExecutorActor,
        source: TerrainVegetationSourceIdentity,
    ) -> Result<(), String>;

    fn request_shutdown(&mut self, actor: TerrainVegetationExecutorActor);

    fn is_terminated(&self) -> bool;

    fn diagnostics(&self) -> TerrainVegetationExecutorDiagnostics;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerrainVegetationCoordinatorState {
    Starting,
    Running,
    Failed,
    ShuttingDown,
    Terminated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainVegetationAdmission {
    pub identity: TerrainVegetationJobIdentity,
    pub source: TerrainVegetationSourceIdentity,
    pub product: TerrainPreviewVegetationProduct,
    pub receipt: TerrainVegetationProductReceipt,
    pub compile_micros: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainVegetationCoordinatorDiagnostics {
    pub state: TerrainVegetationCoordinatorState,
    pub executor_kind: TerrainVegetationExecutorKind,
    pub executor_generation: u32,
    pub source_epoch: u32,
    pub coverage_revision: u64,
    pub desired_tiles: u32,
    pub queued_tiles: u32,
    pub resident_tiles: u32,
    pub in_flight: bool,
    pub submitted_jobs: u64,
    pub completed_jobs: u64,
    pub admitted_products: u64,
    pub source_resets: u64,
    pub transport_failures: u64,
    pub executor_restarts: u64,
    pub job_failures: u64,
    pub stale_generation_completions: u64,
    pub stale_source_completions: u64,
    pub stale_request_completions: u64,
    pub stale_slot_completions: u64,
    pub superseded_completions: u64,
    pub submit_full_count: u64,
    pub compile_micros: u64,
    pub cache_cell_requests: u64,
    pub cache_cell_hits: u64,
    pub cache_cell_misses: u64,
    pub cache_retained_cells: u64,
    pub cache_retained_preliminary_candidates: u64,
    pub last_error: Option<String>,
    pub executor: TerrainVegetationExecutorDiagnostics,
}

pub struct TerrainVegetationCoordinator {
    executor: Box<dyn TerrainVegetationExecutor>,
    source: TerrainVegetationSourceIdentity,
    actor: TerrainVegetationExecutorActor,
    state: TerrainVegetationCoordinatorState,
    maximum_desired_tiles: usize,
    desired: BTreeMap<TerrainViewportTileId, TerrainVegetationSlotToken>,
    queue: VecDeque<TerrainVegetationDesiredTile>,
    resident: BTreeMap<TerrainViewportTileId, TerrainVegetationSlotToken>,
    in_flight: Option<TerrainVegetationExecutorJob>,
    focus_x: i32,
    focus_z: i32,
    coverage_revision: u64,
    next_request_id: u32,
    submitted_jobs: u64,
    completed_jobs: u64,
    admitted_products: u64,
    source_resets: u64,
    transport_failures: u64,
    executor_restarts: u64,
    job_failures: u64,
    stale_generation_completions: u64,
    stale_source_completions: u64,
    stale_request_completions: u64,
    stale_slot_completions: u64,
    superseded_completions: u64,
    submit_full_count: u64,
    compile_micros: u64,
    cache_cell_requests: u64,
    cache_cell_hits: u64,
    cache_cell_misses: u64,
    cache_retained_cells: u64,
    cache_retained_preliminary_candidates: u64,
    consecutive_transport_failures: u8,
    last_error: Option<String>,
}

impl TerrainVegetationCoordinator {
    pub fn new(
        mut executor: Box<dyn TerrainVegetationExecutor>,
        source: TerrainVegetationSourceIdentity,
        maximum_desired_tiles: usize,
    ) -> Result<Self, String> {
        source.validate()?;
        if maximum_desired_tiles == 0 {
            return Err("terrain vegetation desired-tile bound must be non-zero".to_owned());
        }
        let actor = TerrainVegetationExecutorActor {
            executor_generation: 1,
            source_epoch: 1,
        };
        executor.restart(actor, source)?;
        Ok(Self {
            executor,
            source,
            actor,
            state: TerrainVegetationCoordinatorState::Starting,
            maximum_desired_tiles,
            desired: BTreeMap::new(),
            queue: VecDeque::with_capacity(maximum_desired_tiles),
            resident: BTreeMap::new(),
            in_flight: None,
            focus_x: 0,
            focus_z: 0,
            coverage_revision: 0,
            next_request_id: 1,
            submitted_jobs: 0,
            completed_jobs: 0,
            admitted_products: 0,
            source_resets: 0,
            transport_failures: 0,
            executor_restarts: 0,
            job_failures: 0,
            stale_generation_completions: 0,
            stale_source_completions: 0,
            stale_request_completions: 0,
            stale_slot_completions: 0,
            superseded_completions: 0,
            submit_full_count: 0,
            compile_micros: 0,
            cache_cell_requests: 0,
            cache_cell_hits: 0,
            cache_cell_misses: 0,
            cache_retained_cells: 0,
            cache_retained_preliminary_candidates: 0,
            consecutive_transport_failures: 0,
            last_error: None,
        })
    }

    pub const fn source(&self) -> TerrainVegetationSourceIdentity {
        self.source
    }

    pub const fn state(&self) -> TerrainVegetationCoordinatorState {
        self.state
    }

    /// Change the validation bound used by the next semantic desired-set update.
    ///
    /// The caller owns selecting that next set, so lowering the bound does not
    /// arbitrarily evict currently desired tiles here.
    pub fn reconfigure_maximum_desired_tiles(
        &mut self,
        maximum_desired_tiles: usize,
    ) -> Result<bool, String> {
        if maximum_desired_tiles == 0 {
            return Err("terrain vegetation desired-tile bound must be non-zero".to_owned());
        }
        if maximum_desired_tiles == self.maximum_desired_tiles {
            return Ok(false);
        }
        if maximum_desired_tiles > self.maximum_desired_tiles {
            self.queue
                .reserve(maximum_desired_tiles.saturating_sub(self.queue.len()));
        }
        self.maximum_desired_tiles = maximum_desired_tiles;
        Ok(true)
    }

    pub fn update_desired(
        &mut self,
        source: TerrainVegetationSourceIdentity,
        focus_x: i32,
        focus_z: i32,
        tiles: impl IntoIterator<Item = TerrainVegetationDesiredTile>,
    ) -> Result<(), String> {
        if matches!(
            self.state,
            TerrainVegetationCoordinatorState::ShuttingDown
                | TerrainVegetationCoordinatorState::Terminated
        ) {
            return Err("terrain vegetation coordinator is shutting down".to_owned());
        }
        source.validate()?;

        let mut desired = BTreeMap::new();
        let mut slots = BTreeSet::new();
        for desired_tile in tiles {
            if desired.len() >= self.maximum_desired_tiles {
                return Err(format!(
                    "terrain vegetation desired set exceeds {} tiles",
                    self.maximum_desired_tiles
                ));
            }
            let request = request_for(desired_tile.tile, source);
            source.validate_request(request)?;
            if !terrain_preview_requests_tree_records_for_profile(
                source.profile,
                desired_tile.tile.sample_spacing,
            ) {
                return Err(format!(
                    "terrain vegetation tile spacing {} does not request records",
                    desired_tile.tile.sample_spacing
                ));
            }
            if desired
                .insert(desired_tile.tile, desired_tile.slot)
                .is_some()
            {
                return Err(format!(
                    "terrain vegetation desired tile {:?} is duplicated",
                    desired_tile.tile
                ));
            }
            if !slots.insert(desired_tile.slot) {
                return Err(format!(
                    "terrain vegetation physical slot {:?} is duplicated",
                    desired_tile.slot
                ));
            }
        }

        if source != self.source {
            self.reset_source(source)?;
        }
        self.focus_x = focus_x;
        self.focus_z = focus_z;
        self.coverage_revision = self.coverage_revision.saturating_add(1);
        self.desired = desired;
        self.resident
            .retain(|tile, slot| self.desired.get(tile) == Some(slot));
        self.rebuild_queue();
        Ok(())
    }

    pub fn pump(&mut self) -> Option<TerrainVegetationAdmission> {
        if self.state == TerrainVegetationCoordinatorState::Terminated {
            return None;
        }
        let events = self.executor.drain_events();
        let mut admission = None;
        for event in events {
            match event {
                TerrainVegetationExecutorEvent::Ready { actor, source } => {
                    if actor == self.actor
                        && source == self.source
                        && self.state == TerrainVegetationCoordinatorState::Starting
                    {
                        self.state = TerrainVegetationCoordinatorState::Running;
                    } else if actor.executor_generation != self.actor.executor_generation {
                        self.stale_generation_completions =
                            self.stale_generation_completions.saturating_add(1);
                    } else {
                        self.stale_source_completions =
                            self.stale_source_completions.saturating_add(1);
                    }
                }
                TerrainVegetationExecutorEvent::Completed {
                    identity,
                    source,
                    product,
                    compile_micros,
                } => {
                    let accepted =
                        self.handle_completion(identity, source, product, compile_micros);
                    if admission.is_none() {
                        admission = accepted;
                    } else if accepted.is_some() {
                        self.fail_current("executor produced more than one admissible completion");
                    }
                }
                TerrainVegetationExecutorEvent::JobFailed { identity, error } => {
                    if identity.actor == self.actor
                        && self.in_flight.is_some_and(|job| job.identity == identity)
                    {
                        self.in_flight = None;
                        self.job_failures = self.job_failures.saturating_add(1);
                        self.fail_current(&error);
                    } else {
                        self.note_stale_identity(identity);
                    }
                }
                TerrainVegetationExecutorEvent::TransportFailed { actor, error } => {
                    if actor == self.actor {
                        self.handle_transport_failure(error);
                    } else {
                        self.stale_generation_completions =
                            self.stale_generation_completions.saturating_add(1);
                    }
                }
                TerrainVegetationExecutorEvent::ShutdownComplete { actor } => {
                    if actor == self.actor
                        && self.state == TerrainVegetationCoordinatorState::ShuttingDown
                    {
                        self.state = TerrainVegetationCoordinatorState::Terminated;
                    } else if actor != self.actor {
                        self.stale_generation_completions =
                            self.stale_generation_completions.saturating_add(1);
                    }
                }
            }
        }

        if self.state == TerrainVegetationCoordinatorState::ShuttingDown
            && self.executor.is_terminated()
        {
            self.state = TerrainVegetationCoordinatorState::Terminated;
        }
        if self.state == TerrainVegetationCoordinatorState::Running && self.in_flight.is_none() {
            self.submit_next();
        }
        admission
    }

    pub fn shutdown(&mut self) {
        if matches!(
            self.state,
            TerrainVegetationCoordinatorState::ShuttingDown
                | TerrainVegetationCoordinatorState::Terminated
        ) {
            return;
        }
        self.desired.clear();
        self.queue.clear();
        self.resident.clear();
        self.state = TerrainVegetationCoordinatorState::ShuttingDown;
        self.executor.request_shutdown(self.actor);
        if self.executor.is_terminated() {
            self.state = TerrainVegetationCoordinatorState::Terminated;
        }
    }

    pub fn diagnostics(&self) -> TerrainVegetationCoordinatorDiagnostics {
        TerrainVegetationCoordinatorDiagnostics {
            state: self.state,
            executor_kind: self.executor.kind(),
            executor_generation: self.actor.executor_generation,
            source_epoch: self.actor.source_epoch,
            coverage_revision: self.coverage_revision,
            desired_tiles: u32::try_from(self.desired.len()).unwrap_or(u32::MAX),
            queued_tiles: u32::try_from(self.queue.len()).unwrap_or(u32::MAX),
            resident_tiles: u32::try_from(self.resident.len()).unwrap_or(u32::MAX),
            in_flight: self.in_flight.is_some(),
            submitted_jobs: self.submitted_jobs,
            completed_jobs: self.completed_jobs,
            admitted_products: self.admitted_products,
            source_resets: self.source_resets,
            transport_failures: self.transport_failures,
            executor_restarts: self.executor_restarts,
            job_failures: self.job_failures,
            stale_generation_completions: self.stale_generation_completions,
            stale_source_completions: self.stale_source_completions,
            stale_request_completions: self.stale_request_completions,
            stale_slot_completions: self.stale_slot_completions,
            superseded_completions: self.superseded_completions,
            submit_full_count: self.submit_full_count,
            compile_micros: self.compile_micros,
            cache_cell_requests: self.cache_cell_requests,
            cache_cell_hits: self.cache_cell_hits,
            cache_cell_misses: self.cache_cell_misses,
            cache_retained_cells: self.cache_retained_cells,
            cache_retained_preliminary_candidates: self.cache_retained_preliminary_candidates,
            last_error: self.last_error.clone(),
            executor: self.executor.diagnostics(),
        }
    }

    fn reset_source(&mut self, source: TerrainVegetationSourceIdentity) -> Result<(), String> {
        self.actor.executor_generation = self
            .actor
            .executor_generation
            .checked_add(1)
            .ok_or("terrain vegetation executor generation exhausted")?;
        self.actor.source_epoch = self
            .actor
            .source_epoch
            .checked_add(1)
            .ok_or("terrain vegetation source epoch exhausted")?;
        self.source = source;
        self.desired.clear();
        self.queue.clear();
        self.resident.clear();
        self.in_flight = None;
        self.consecutive_transport_failures = 0;
        self.cache_cell_requests = 0;
        self.cache_cell_hits = 0;
        self.cache_cell_misses = 0;
        self.cache_retained_cells = 0;
        self.cache_retained_preliminary_candidates = 0;
        self.last_error = None;
        self.source_resets = self.source_resets.saturating_add(1);
        self.state = TerrainVegetationCoordinatorState::Starting;
        self.executor.restart(self.actor, source).map_err(|error| {
            self.fail_current(&error);
            error
        })
    }

    fn rebuild_queue(&mut self) {
        let in_flight = self.in_flight.map(|job| job.identity);
        let mut queued = self
            .desired
            .iter()
            .filter_map(|(tile, slot)| {
                if self.resident.get(tile) == Some(slot)
                    || in_flight
                        .is_some_and(|identity| identity.tile == *tile && identity.slot == *slot)
                {
                    None
                } else {
                    Some(TerrainVegetationDesiredTile {
                        tile: *tile,
                        slot: *slot,
                    })
                }
            })
            .collect::<Vec<_>>();
        queued.sort_by_key(|desired| {
            (
                Reverse(desired.tile.sample_spacing),
                tile_focus_distance(
                    desired.tile,
                    self.focus_x,
                    self.focus_z,
                    desired.tile.sample_spacing,
                ),
                desired.tile.tile_z,
                desired.tile.tile_x,
            )
        });
        self.queue = queued.into();
    }

    fn submit_next(&mut self) {
        while let Some(desired) = self.queue.pop_front() {
            if self.desired.get(&desired.tile) != Some(&desired.slot)
                || self.resident.get(&desired.tile) == Some(&desired.slot)
            {
                continue;
            }
            let request_id = self.next_request_id;
            let Some(next_request_id) = request_id.checked_add(1) else {
                self.fail_current("terrain vegetation request ID exhausted");
                return;
            };
            let job = TerrainVegetationExecutorJob {
                identity: TerrainVegetationJobIdentity {
                    actor: self.actor,
                    request_id,
                    tile: desired.tile,
                    slot: desired.slot,
                },
                source: self.source,
                request: request_for(desired.tile, self.source),
            };
            match self.executor.try_submit(&job) {
                Ok(()) => {
                    self.next_request_id = next_request_id;
                    self.in_flight = Some(job);
                    self.submitted_jobs = self.submitted_jobs.saturating_add(1);
                }
                Err(TerrainVegetationSubmitError::Full) => {
                    self.queue.push_front(desired);
                    self.submit_full_count = self.submit_full_count.saturating_add(1);
                }
                Err(TerrainVegetationSubmitError::Failed(error)) => {
                    self.queue.push_front(desired);
                    self.handle_transport_failure(error);
                }
            }
            return;
        }
    }

    fn handle_completion(
        &mut self,
        identity: TerrainVegetationJobIdentity,
        source: TerrainVegetationSourceIdentity,
        product: TerrainPreviewVegetationProduct,
        compile_micros: u64,
    ) -> Option<TerrainVegetationAdmission> {
        if identity.actor.executor_generation != self.actor.executor_generation {
            self.stale_generation_completions = self.stale_generation_completions.saturating_add(1);
            return None;
        }
        if identity.actor.source_epoch != self.actor.source_epoch || source != self.source {
            self.stale_source_completions = self.stale_source_completions.saturating_add(1);
            return None;
        }
        let Some(in_flight) = self.in_flight else {
            self.stale_request_completions = self.stale_request_completions.saturating_add(1);
            return None;
        };
        if in_flight.identity != identity {
            self.stale_request_completions = self.stale_request_completions.saturating_add(1);
            return None;
        }
        self.in_flight = None;
        self.completed_jobs = self.completed_jobs.saturating_add(1);
        self.compile_micros = self.compile_micros.saturating_add(compile_micros);
        self.consecutive_transport_failures = 0;

        if product.request().request() != in_flight.request {
            self.handle_transport_failure(
                "terrain vegetation completion product request does not match its job".to_owned(),
            );
            return None;
        }
        match self.desired.get(&identity.tile) {
            Some(slot) if *slot == identity.slot => {}
            Some(_) => {
                self.stale_slot_completions = self.stale_slot_completions.saturating_add(1);
                self.rebuild_queue();
                return None;
            }
            None => {
                self.superseded_completions = self.superseded_completions.saturating_add(1);
                self.rebuild_queue();
                return None;
            }
        }
        let receipt = terrain_vegetation_product_receipt(source, &product);
        let cache = product.cache_report();
        self.cache_cell_requests = self.cache_cell_requests.saturating_add(cache.cell_requests);
        self.cache_cell_hits = self.cache_cell_hits.saturating_add(cache.cell_hits);
        self.cache_cell_misses = self.cache_cell_misses.saturating_add(cache.cell_misses);
        self.cache_retained_cells = u64::try_from(cache.retained_cells).unwrap_or(u64::MAX);
        self.cache_retained_preliminary_candidates =
            u64::try_from(cache.retained_preliminary_candidates).unwrap_or(u64::MAX);
        self.resident.insert(identity.tile, identity.slot);
        self.admitted_products = self.admitted_products.saturating_add(1);
        self.rebuild_queue();
        Some(TerrainVegetationAdmission {
            identity,
            source,
            product,
            receipt,
            compile_micros,
        })
    }

    fn note_stale_identity(&mut self, identity: TerrainVegetationJobIdentity) {
        if identity.actor.executor_generation != self.actor.executor_generation {
            self.stale_generation_completions = self.stale_generation_completions.saturating_add(1);
        } else if identity.actor.source_epoch != self.actor.source_epoch {
            self.stale_source_completions = self.stale_source_completions.saturating_add(1);
        } else {
            self.stale_request_completions = self.stale_request_completions.saturating_add(1);
        }
    }

    fn handle_transport_failure(&mut self, error: String) {
        if matches!(
            self.state,
            TerrainVegetationCoordinatorState::ShuttingDown
                | TerrainVegetationCoordinatorState::Terminated
        ) {
            return;
        }
        self.transport_failures = self.transport_failures.saturating_add(1);
        self.consecutive_transport_failures = self.consecutive_transport_failures.saturating_add(1);
        self.last_error = Some(error.clone());
        self.in_flight = None;
        if self.consecutive_transport_failures > 1 {
            self.fail_current(&error);
            return;
        }
        let Some(generation) = self.actor.executor_generation.checked_add(1) else {
            self.fail_current("terrain vegetation executor generation exhausted");
            return;
        };
        self.actor.executor_generation = generation;
        self.executor_restarts = self.executor_restarts.saturating_add(1);
        self.state = TerrainVegetationCoordinatorState::Starting;
        if let Err(restart_error) = self.executor.restart(self.actor, self.source) {
            self.transport_failures = self.transport_failures.saturating_add(1);
            self.consecutive_transport_failures =
                self.consecutive_transport_failures.saturating_add(1);
            self.fail_current(&restart_error);
        }
        self.rebuild_queue();
    }

    fn fail_current(&mut self, error: impl Into<String>) {
        self.state = TerrainVegetationCoordinatorState::Failed;
        self.last_error = Some(error.into());
        self.queue.clear();
    }
}

impl Drop for TerrainVegetationCoordinator {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn request_for(
    tile: TerrainViewportTileId,
    source: TerrainVegetationSourceIdentity,
) -> TerrainPreviewRequest {
    let mut request = tile.preview_request();
    request.topology = source.topology;
    request
}

fn tile_focus_distance(
    tile: TerrainViewportTileId,
    focus_x: i32,
    focus_z: i32,
    spacing: u32,
) -> u128 {
    let footprint = i64::from(tile.footprint_blocks());
    let spacing = i64::from(spacing);
    let center_x = i64::from(tile.min_x()) + footprint / 2;
    let center_z = i64::from(tile.min_z()) + footprint / 2;
    let tile_x = center_x.div_euclid(spacing);
    let tile_z = center_z.div_euclid(spacing);
    let focus_x = i64::from(focus_x).div_euclid(spacing);
    let focus_z = i64::from(focus_z).div_euclid(spacing);
    let dx = tile_x.abs_diff(focus_x) as u128;
    let dz = tile_z.abs_diff(focus_z) as u128;
    dx * dx + dz * dz
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use mclone_worldgen::terrain_preview::{
        TerrainPreviewContentStage, TerrainPreviewProfile, TerrainPreviewSurfaceQuality,
    };
    use mclone_worldgen::terrain_vegetation::TerrainVegetationCompilerSession;

    use super::*;

    #[derive(Default)]
    struct FakeControl {
        submitted: Vec<TerrainVegetationExecutorJob>,
        events: VecDeque<TerrainVegetationExecutorEvent>,
        restarts: Vec<(
            TerrainVegetationExecutorActor,
            TerrainVegetationSourceIdentity,
        )>,
        shutdown_requests: Vec<TerrainVegetationExecutorActor>,
        terminated: bool,
        full: bool,
        diagnostics: TerrainVegetationExecutorDiagnostics,
    }

    struct FakeExecutor {
        control: Rc<RefCell<FakeControl>>,
    }

    impl TerrainVegetationExecutor for FakeExecutor {
        fn kind(&self) -> TerrainVegetationExecutorKind {
            TerrainVegetationExecutorKind::InlineTest
        }

        fn try_submit(
            &mut self,
            job: &TerrainVegetationExecutorJob,
        ) -> Result<(), TerrainVegetationSubmitError> {
            let mut control = self.control.borrow_mut();
            if control.full {
                return Err(TerrainVegetationSubmitError::Full);
            }
            control.submitted.push(*job);
            control.diagnostics.submitted_jobs =
                control.diagnostics.submitted_jobs.saturating_add(1);
            Ok(())
        }

        fn drain_events(&mut self) -> Vec<TerrainVegetationExecutorEvent> {
            self.control.borrow_mut().events.drain(..).collect()
        }

        fn restart(
            &mut self,
            actor: TerrainVegetationExecutorActor,
            source: TerrainVegetationSourceIdentity,
        ) -> Result<(), String> {
            let mut control = self.control.borrow_mut();
            control.restarts.push((actor, source));
            control.diagnostics.restarts = control.diagnostics.restarts.saturating_add(1);
            control
                .events
                .push_back(TerrainVegetationExecutorEvent::Ready { actor, source });
            control.terminated = false;
            Ok(())
        }

        fn request_shutdown(&mut self, actor: TerrainVegetationExecutorActor) {
            let mut control = self.control.borrow_mut();
            control.shutdown_requests.push(actor);
            control
                .events
                .push_back(TerrainVegetationExecutorEvent::ShutdownComplete { actor });
            control.terminated = true;
        }

        fn is_terminated(&self) -> bool {
            self.control.borrow().terminated
        }

        fn diagnostics(&self) -> TerrainVegetationExecutorDiagnostics {
            self.control.borrow().diagnostics
        }
    }

    fn source(seed: i64) -> TerrainVegetationSourceIdentity {
        TerrainVegetationSourceIdentity::for_request(tile(seed, 4, 0, 0).preview_request()).unwrap()
    }

    fn tile(seed: i64, spacing: u32, tile_x: i32, tile_z: i32) -> TerrainViewportTileId {
        TerrainViewportTileId {
            profile: TerrainPreviewProfile::McloneOverworldV1,
            seed,
            tile_x,
            tile_z,
            sample_spacing: spacing,
            content_stage: TerrainPreviewContentStage::Cover,
            surface_quality: TerrainPreviewSurfaceQuality::Inferred,
        }
    }

    fn desired(
        tile: TerrainViewportTileId,
        physical_slot: u32,
        slot_generation: u32,
    ) -> TerrainVegetationDesiredTile {
        TerrainVegetationDesiredTile {
            tile,
            slot: TerrainVegetationSlotToken {
                physical_slot,
                slot_generation,
            },
        }
    }

    fn coordinator() -> (TerrainVegetationCoordinator, Rc<RefCell<FakeControl>>) {
        let control = Rc::new(RefCell::new(FakeControl::default()));
        let coordinator = TerrainVegetationCoordinator::new(
            Box::new(FakeExecutor {
                control: Rc::clone(&control),
            }),
            source(12_345),
            48,
        )
        .unwrap();
        (coordinator, control)
    }

    fn complete(
        control: &Rc<RefCell<FakeControl>>,
        job: TerrainVegetationExecutorJob,
        compile_micros: u64,
    ) {
        let product = TerrainVegetationCompilerSession::default()
            .compile(job.source, job.request)
            .unwrap();
        let mut control = control.borrow_mut();
        control.diagnostics.completed_jobs = control.diagnostics.completed_jobs.saturating_add(1);
        control
            .events
            .push_back(TerrainVegetationExecutorEvent::Completed {
                identity: job.identity,
                source: job.source,
                product,
                compile_micros,
            });
    }

    #[test]
    fn priority_is_coarse_to_fine_distance_then_stable_coordinates() {
        let (mut coordinator, control) = coordinator();
        let source = source(12_345);
        coordinator
            .update_desired(
                source,
                0,
                0,
                [
                    desired(tile(12_345, 1, 0, 0), 0, 1),
                    desired(tile(12_345, 4, 4, 0), 1, 1),
                    desired(tile(12_345, 4, -1, 0), 2, 1),
                    desired(tile(12_345, 2, 0, 0), 3, 1),
                ],
            )
            .unwrap();

        assert!(coordinator.pump().is_none());
        let first = control.borrow().submitted[0];
        assert_eq!(
            (
                first.identity.tile.sample_spacing,
                first.identity.tile.tile_x
            ),
            (4, -1)
        );
        assert_eq!(coordinator.diagnostics().submitted_jobs, 1);
        assert!(coordinator.diagnostics().in_flight);

        complete(&control, first, 9);
        let admission = coordinator.pump().expect("one completion is admitted");
        assert_eq!(admission.identity, first.identity);
        assert_eq!(coordinator.diagnostics().admitted_products, 1);
        assert_eq!(coordinator.diagnostics().compile_micros, 9);
        assert_eq!(control.borrow().submitted.len(), 2);
        assert_eq!(
            control.borrow().submitted[1].identity.tile.sample_spacing,
            4
        );
    }

    #[test]
    fn retained_tiles_accept_old_coverage_but_reassigned_slots_reject() {
        let (mut coordinator, control) = coordinator();
        let source = source(12_345);
        let tile_id = tile(12_345, 4, 0, 0);
        coordinator
            .update_desired(source, 0, 0, [desired(tile_id, 7, 1)])
            .unwrap();
        coordinator.pump();
        let retained_job = control.borrow().submitted[0];
        coordinator
            .update_desired(source, 16, 0, [desired(tile_id, 7, 1)])
            .unwrap();
        complete(&control, retained_job, 1);
        assert!(coordinator.pump().is_some());

        let moved_tile = tile(12_345, 4, 1, 0);
        coordinator
            .update_desired(source, 64, 0, [desired(moved_tile, 7, 2)])
            .unwrap();
        coordinator.pump();
        let reassigned_job = *control.borrow().submitted.last().unwrap();
        coordinator
            .update_desired(source, 64, 0, [desired(moved_tile, 7, 3)])
            .unwrap();
        complete(&control, reassigned_job, 1);
        assert!(coordinator.pump().is_none());
        assert_eq!(coordinator.diagnostics().stale_slot_completions, 1);
    }

    #[test]
    fn source_reset_restarts_and_rejects_retired_generation() {
        let (mut coordinator, control) = coordinator();
        let old_source = source(12_345);
        coordinator
            .update_desired(old_source, 0, 0, [desired(tile(12_345, 4, 0, 0), 0, 1)])
            .unwrap();
        coordinator.pump();
        let old_job = control.borrow().submitted[0];

        let new_source = source(-98_765);
        coordinator
            .update_desired(new_source, 0, 0, [desired(tile(-98_765, 4, 0, 0), 0, 2)])
            .unwrap();
        complete(&control, old_job, 1);
        assert!(coordinator.pump().is_none());
        let diagnostics = coordinator.diagnostics();
        assert_eq!(diagnostics.source_resets, 1);
        assert_eq!(diagnostics.executor_generation, 2);
        assert_eq!(diagnostics.source_epoch, 2);
        assert_eq!(diagnostics.stale_generation_completions, 1);
        assert_eq!(control.borrow().restarts.len(), 2);
    }

    #[test]
    fn one_restart_is_allowed_and_success_clears_the_failure_streak() {
        let (mut coordinator, control) = coordinator();
        let source = source(12_345);
        coordinator
            .update_desired(source, 0, 0, [desired(tile(12_345, 4, 0, 0), 0, 1)])
            .unwrap();
        coordinator.pump();
        let actor = coordinator.actor;
        control
            .borrow_mut()
            .events
            .push_back(TerrainVegetationExecutorEvent::TransportFailed {
                actor,
                error: "first".to_owned(),
            });
        coordinator.pump();
        assert_eq!(
            coordinator.state(),
            TerrainVegetationCoordinatorState::Starting
        );
        coordinator.pump();
        let recovered_job = *control.borrow().submitted.last().unwrap();
        complete(&control, recovered_job, 1);
        assert!(coordinator.pump().is_some());
        assert_eq!(
            coordinator.state(),
            TerrainVegetationCoordinatorState::Running
        );

        coordinator
            .update_desired(source, 512, 0, [desired(tile(12_345, 4, 2, 0), 1, 1)])
            .unwrap();
        coordinator.pump();
        let actor = coordinator.actor;
        control
            .borrow_mut()
            .events
            .push_back(TerrainVegetationExecutorEvent::TransportFailed {
                actor,
                error: "after success".to_owned(),
            });
        coordinator.pump();
        assert_eq!(
            coordinator.state(),
            TerrainVegetationCoordinatorState::Starting
        );
        assert_eq!(coordinator.diagnostics().executor_restarts, 2);
    }

    #[test]
    fn consecutive_transport_failure_enters_explicit_failed_state() {
        let (mut coordinator, control) = coordinator();
        let source = source(12_345);
        coordinator
            .update_desired(source, 0, 0, [desired(tile(12_345, 4, 0, 0), 0, 1)])
            .unwrap();
        coordinator.pump();
        let actor = coordinator.actor;
        control
            .borrow_mut()
            .events
            .push_back(TerrainVegetationExecutorEvent::TransportFailed {
                actor,
                error: "transport".to_owned(),
            });
        coordinator.pump();
        assert_eq!(
            coordinator.state(),
            TerrainVegetationCoordinatorState::Starting
        );
        coordinator.pump();
        let actor = coordinator.actor;
        control
            .borrow_mut()
            .events
            .push_back(TerrainVegetationExecutorEvent::TransportFailed {
                actor,
                error: "transport".to_owned(),
            });
        coordinator.pump();
        assert_eq!(
            coordinator.state(),
            TerrainVegetationCoordinatorState::Failed
        );
        assert_eq!(coordinator.diagnostics().transport_failures, 2);
        assert!(coordinator.diagnostics().last_error.is_some());
    }

    #[test]
    fn deterministic_job_failure_does_not_restart() {
        let (mut coordinator, control) = coordinator();
        let source = source(12_345);
        coordinator
            .update_desired(source, 0, 0, [desired(tile(12_345, 4, 0, 0), 0, 1)])
            .unwrap();
        coordinator.pump();
        let job = control.borrow().submitted[0];
        control
            .borrow_mut()
            .events
            .push_back(TerrainVegetationExecutorEvent::JobFailed {
                identity: job.identity,
                error: "deterministic".to_owned(),
            });
        coordinator.pump();
        assert_eq!(
            coordinator.state(),
            TerrainVegetationCoordinatorState::Failed
        );
        assert_eq!(coordinator.diagnostics().job_failures, 1);
        assert_eq!(coordinator.diagnostics().executor_restarts, 0);
    }

    #[test]
    fn graceful_shutdown_is_idempotent() {
        let (mut coordinator, control) = coordinator();
        coordinator.shutdown();
        coordinator.shutdown();
        assert_eq!(control.borrow().shutdown_requests.len(), 1);
        coordinator.pump();
        assert_eq!(
            coordinator.state(),
            TerrainVegetationCoordinatorState::Terminated
        );
        coordinator.shutdown();
        assert_eq!(control.borrow().shutdown_requests.len(), 1);
    }

    #[test]
    fn desired_sets_reject_duplicates_bounds_and_coarse_tiles() {
        let (mut coordinator, _) = coordinator();
        let source = source(12_345);
        let tile_id = tile(12_345, 4, 0, 0);
        assert!(
            coordinator
                .update_desired(
                    source,
                    0,
                    0,
                    [desired(tile_id, 0, 1), desired(tile_id, 1, 1)],
                )
                .is_err()
        );
        assert!(
            coordinator
                .update_desired(
                    source,
                    0,
                    0,
                    [desired(tile_id, 0, 1), desired(tile(12_345, 4, 1, 0), 0, 1),],
                )
                .is_err()
        );
        assert!(
            coordinator
                .update_desired(source, 0, 0, [desired(tile(12_345, 8, 0, 0), 0, 1)])
                .is_err()
        );
    }

    #[test]
    fn desired_tile_bound_tracks_live_lod_reconfiguration() {
        let control = Rc::new(RefCell::new(FakeControl::default()));
        let mut coordinator = TerrainVegetationCoordinator::new(
            Box::new(FakeExecutor {
                control: Rc::clone(&control),
            }),
            source(12_345),
            1,
        )
        .unwrap();
        let first = desired(tile(12_345, 1, 0, 0), 0, 1);
        let second = desired(tile(12_345, 1, 1, 0), 1, 1);

        assert!(
            coordinator
                .update_desired(source(12_345), 0, 0, [first, second])
                .is_err()
        );
        assert!(coordinator.reconfigure_maximum_desired_tiles(2).unwrap());
        coordinator
            .update_desired(source(12_345), 0, 0, [first, second])
            .unwrap();
        assert_eq!(coordinator.diagnostics().desired_tiles, 2);

        assert!(coordinator.reconfigure_maximum_desired_tiles(1).unwrap());
        coordinator
            .update_desired(source(12_345), 0, 0, [first])
            .unwrap();
        assert_eq!(coordinator.diagnostics().desired_tiles, 1);
        assert!(!coordinator.reconfigure_maximum_desired_tiles(1).unwrap());
        assert!(coordinator.reconfigure_maximum_desired_tiles(0).is_err());
    }

    #[test]
    fn full_executor_never_blocks_or_loses_the_front_job() {
        let (mut coordinator, control) = coordinator();
        let source = source(12_345);
        coordinator
            .update_desired(source, 0, 0, [desired(tile(12_345, 4, 0, 0), 0, 1)])
            .unwrap();
        control.borrow_mut().full = true;
        coordinator.pump();
        assert_eq!(coordinator.diagnostics().submit_full_count, 1);
        assert_eq!(coordinator.diagnostics().queued_tiles, 1);
        assert!(!coordinator.diagnostics().in_flight);
        control.borrow_mut().full = false;
        coordinator.pump();
        assert_eq!(control.borrow().submitted.len(), 1);
    }
}
