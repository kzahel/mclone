use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use mclone_terrain_view::{
    TerrainExactCoverageMode, TerrainHorizonFrameStats, TerrainHorizonVegetationServiceStats,
    TerrainVegetationCoordinatorState, TerrainVegetationExecutorKind,
};
use mclone_view_control::{
    ViewPoint, ViewportMetrics, WorldViewIntent, WorldViewMode, WorldViewState,
};
use serde_json::{Map, Value, json};

use crate::capture::{DepthStats, PixelStats};
use crate::exact::ExplorerExactStats;
use crate::options::ExplorerOptions;
use crate::terrain::ExplorerTerrain;
use mclone_world_explorer::WorldExplorerCompositionMode;

const MOVEMENT_FRAMES: u32 = 8;
const ZOOM_FRAMES: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SmokeStage {
    Initial3d,
    MoveX,
    MoveZ,
    MoveDiagonal,
    NegativeCoordinates,
    Teleport,
    Zoom,
    Map,
    Orbit,
}

const SMOKE_STAGES: [SmokeStage; 9] = [
    SmokeStage::Initial3d,
    SmokeStage::MoveX,
    SmokeStage::MoveZ,
    SmokeStage::MoveDiagonal,
    SmokeStage::NegativeCoordinates,
    SmokeStage::Teleport,
    SmokeStage::Zoom,
    SmokeStage::Map,
    SmokeStage::Orbit,
];

#[derive(Default)]
pub struct SmokeSequence {
    stage_index: usize,
    input_frames_applied: u32,
}

impl SmokeSequence {
    pub fn prepare_frame(
        &mut self,
        terrain: &mut ExplorerTerrain,
        viewport: ViewportMetrics,
    ) -> Result<bool> {
        let Some(stage) = SMOKE_STAGES.get(self.stage_index).copied() else {
            return Ok(false);
        };
        let frame_count = stage.input_frame_count();
        if self.input_frames_applied >= frame_count {
            return Ok(false);
        }
        for intent in stage.intents(terrain.view_state(), viewport) {
            terrain.apply_intent(intent)?;
        }
        self.input_frames_applied = self.input_frames_applied.saturating_add(1);
        Ok(true)
    }

    pub fn after_frame(&mut self, stats: TerrainHorizonFrameStats) -> SmokeFrameOutcome {
        let Some(stage) = SMOKE_STAGES.get(self.stage_index).copied() else {
            return SmokeFrameOutcome::Complete;
        };
        if self.input_frames_applied < stage.input_frame_count()
            || !stats.target_ready
            || stats.needs_redraw
        {
            return SmokeFrameOutcome::Continue;
        }
        self.stage_index += 1;
        self.input_frames_applied = 0;
        let complete_after_capture = self.stage_index == SMOKE_STAGES.len();
        match stage.capture_label() {
            Some(label) => SmokeFrameOutcome::Capture {
                label,
                complete_after_capture,
            },
            None if complete_after_capture => SmokeFrameOutcome::Complete,
            None => SmokeFrameOutcome::Advanced,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.stage_index >= SMOKE_STAGES.len()
    }

    pub fn requires_continuous_coverage(&self) -> bool {
        SMOKE_STAGES.get(self.stage_index).is_some_and(|stage| {
            matches!(
                stage,
                SmokeStage::MoveX
                    | SmokeStage::MoveZ
                    | SmokeStage::MoveDiagonal
                    | SmokeStage::Zoom
                    | SmokeStage::Map
                    | SmokeStage::Orbit
            )
        })
    }
}

impl SmokeStage {
    const fn input_frame_count(self) -> u32 {
        match self {
            Self::Initial3d => 0,
            Self::MoveX | Self::MoveZ | Self::MoveDiagonal => MOVEMENT_FRAMES,
            Self::Zoom => ZOOM_FRAMES,
            Self::NegativeCoordinates | Self::Teleport | Self::Map | Self::Orbit => 1,
        }
    }

    const fn capture_label(self) -> Option<&'static str> {
        match self {
            Self::Initial3d => Some("3d"),
            Self::MoveDiagonal => Some("movement"),
            Self::NegativeCoordinates => Some("negative"),
            Self::Teleport => Some("teleport"),
            Self::Map => Some("map"),
            Self::Orbit => Some("orbit"),
            Self::MoveX | Self::MoveZ | Self::Zoom => None,
        }
    }

    fn intents(self, state: WorldViewState, viewport: ViewportMetrics) -> Vec<WorldViewIntent> {
        match self {
            Self::Initial3d => Vec::new(),
            Self::MoveX => vec![WorldViewIntent::PanWorld {
                delta_x: state.blocks_across * 0.025,
                delta_z: 0.0,
            }],
            Self::MoveZ => vec![WorldViewIntent::PanWorld {
                delta_x: 0.0,
                delta_z: -state.blocks_across * 0.02,
            }],
            Self::MoveDiagonal => vec![WorldViewIntent::PanWorld {
                delta_x: state.blocks_across * 0.015,
                delta_z: state.blocks_across * 0.015,
            }],
            Self::NegativeCoordinates => vec![WorldViewIntent::FocusAt {
                world_x: -8_193.0,
                world_z: -4_097.0,
            }],
            Self::Teleport => vec![WorldViewIntent::FocusAt {
                world_x: 1_000_000.0,
                world_z: -1_000_000.0,
            }],
            Self::Zoom => vec![WorldViewIntent::AnchoredZoom {
                log_delta: 0.5_f64.ln() / f64::from(ZOOM_FRAMES),
                normalized_anchor: ViewPoint::new(0.2, -0.15),
                viewport,
            }],
            Self::Map => vec![WorldViewIntent::SetMode(WorldViewMode::Map)],
            Self::Orbit => vec![
                WorldViewIntent::SetMode(WorldViewMode::Orbit),
                WorldViewIntent::Orbit {
                    delta_pixels: ViewPoint::new(
                        viewport.width_pixels * 0.16,
                        -viewport.height_pixels * 0.08,
                    ),
                    viewport,
                },
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmokeFrameOutcome {
    Continue,
    Advanced,
    Capture {
        label: &'static str,
        complete_after_capture: bool,
    },
    Complete,
}

pub struct SmokeCheckpoint {
    pub state: WorldViewState,
    pub stats: TerrainHorizonFrameStats,
    pub composition: WorldExplorerCompositionMode,
    pub exact: ExplorerExactStats,
    pub pixels: PixelStats,
    pub depth: DepthStats,
}

pub struct SmokeRecorder {
    target: &'static str,
    root: PathBuf,
    adapter_name: String,
    adapter_backend: String,
    adapter_device_type: String,
    format: String,
    frame_count: u64,
    movement_frame_ms: Vec<f64>,
    settling_frame_ms: Vec<f64>,
    checkpoint_frame_ms: Vec<f64>,
    peak_resident_bytes: u64,
    peak_pending_work: u32,
    allocation_slots: Option<u32>,
    fixed_resident_bytes: Option<u64>,
    captures: Vec<Value>,
}

impl SmokeRecorder {
    pub fn new(
        target: &'static str,
        root: &Path,
        adapter: &wgpu::AdapterInfo,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        std::fs::create_dir_all(root)
            .with_context(|| format!("failed to create smoke directory {}", root.display()))?;
        Ok(Self {
            target,
            root: root.to_owned(),
            adapter_name: adapter.name.clone(),
            adapter_backend: format!("{:?}", adapter.backend),
            adapter_device_type: format!("{:?}", adapter.device_type),
            format: format!("{format:?}"),
            frame_count: 0,
            movement_frame_ms: Vec::new(),
            settling_frame_ms: Vec::new(),
            checkpoint_frame_ms: Vec::new(),
            peak_resident_bytes: 0,
            peak_pending_work: 0,
            allocation_slots: None,
            fixed_resident_bytes: None,
            captures: Vec::new(),
        })
    }

    pub fn note_frame(
        &mut self,
        frame_time: Duration,
        input_applied: bool,
        continuous_coverage_required: bool,
        stats: TerrainHorizonFrameStats,
    ) -> Result<()> {
        self.frame_count = self.frame_count.saturating_add(1);
        let frame_ms = frame_time.as_secs_f64() * 1_000.0;
        if input_applied {
            self.movement_frame_ms.push(frame_ms);
        } else {
            self.settling_frame_ms.push(frame_ms);
        }
        self.peak_resident_bytes = self.peak_resident_bytes.max(stats.resident_bytes);
        self.peak_pending_work = self.peak_pending_work.max(stats.pending_refills);
        ensure!(
            !continuous_coverage_required
                || (stats.ready_slots == stats.allocation_slots
                    && stats.drawn_levels == 10
                    && stats.committed_levels == 10
                    && stats.vegetation_committed_levels == 3),
            "World Explorer exposed incomplete committed coverage during a bounded transition: \
             {stats:?}"
        );
        match self.allocation_slots {
            Some(expected) if expected != stats.allocation_slots => {
                anyhow::bail!(
                    "World Explorer allocation changed from {expected} to {}",
                    stats.allocation_slots
                );
            }
            None => self.allocation_slots = Some(stats.allocation_slots),
            _ => {}
        }
        match self.fixed_resident_bytes {
            Some(expected) if expected != stats.fixed_resident_bytes => {
                anyhow::bail!(
                    "World Explorer fixed terrain memory changed from {expected} to {}",
                    stats.fixed_resident_bytes
                );
            }
            None => self.fixed_resident_bytes = Some(stats.fixed_resident_bytes),
            _ => {}
        }
        Ok(())
    }

    pub fn checkpoint_path(&self, label: &str) -> PathBuf {
        self.root.join(format!("{label}.png"))
    }

    pub fn note_checkpoint(
        &mut self,
        label: &str,
        frame_time: Duration,
        path: &Path,
        checkpoint: SmokeCheckpoint,
    ) -> Result<()> {
        let SmokeCheckpoint {
            state,
            stats,
            composition,
            exact,
            pixels,
            depth,
        } = checkpoint;
        let service = stats.vegetation_service;
        ensure!(
            stats.staging_slots == 70
                && stats.normal_halo_radius == 2
                && stats.normal_halo_samples_per_tile == 536
                && stats.normal_halo_fixed_bytes == 493_120
                && stats.normal_height_fixed_bytes == 4_380_120
                && stats.staged_levels == 0
                && stats.committed_levels == 10
                && stats.vegetation_committed_levels == 3
                && service.coordinator_state == Some(TerrainVegetationCoordinatorState::Running)
                && service.executor_kind == Some(TerrainVegetationExecutorKind::NativeThread)
                && service.desired_tiles == 48
                && service.queued_tiles == 0
                && service.resident_tiles == 48
                && !service.in_flight
                && service.product_count == 48
                && service.record_count == stats.tree_instance_count
                && service.family_counts.iter().sum::<u32>() == stats.tree_instance_count
                && service.source_fingerprint != 0
                && service.record_hash != 0
                && service.compile_micros > 0,
            "World Explorer {label} checkpoint has incomplete vegetation diagnostics: \
             {service:?}"
        );
        if composition != WorldExplorerCompositionMode::Horizon {
            ensure!(
                exact.complete
                    && exact.desired_chunks > 0
                    && exact.painted_chunks == exact.desired_chunks
                    && !exact.in_flight
                    && exact.queued_chunks == 0
                    && exact.pending_admissions == 0,
                "World Explorer {label} checkpoint has incomplete exact coverage: {exact:?}"
            );
        }
        let expected_coverage_mode = match composition {
            WorldExplorerCompositionMode::Composed => {
                Some(TerrainExactCoverageMode::DiscardPainted)
            }
            WorldExplorerCompositionMode::Coverage => {
                Some(TerrainExactCoverageMode::VisualizePainted)
            }
            WorldExplorerCompositionMode::Horizon | WorldExplorerCompositionMode::Exact => None,
        };
        if let Some(expected) = expected_coverage_mode {
            ensure!(
                stats.exact_coverage_mode == expected
                    && stats.exact_painted_chunks == exact.painted_chunks
                    && stats.exact_coverage_generation == exact.coverage_generation,
                "World Explorer {label} checkpoint mask does not match exact-painted coverage: \
                 horizon={stats:?} exact={exact:?}"
            );
        }
        self.checkpoint_frame_ms
            .push(frame_time.as_secs_f64() * 1_000.0);
        let mut capture = json!({
            "label": label,
            "path": path.display().to_string(),
            "file_bytes": std::fs::metadata(path)
                .with_context(|| format!("failed to inspect {}", path.display()))?
                .len(),
            "view": match state.mode {
                WorldViewMode::Map => "map",
                WorldViewMode::Orbit => "3d",
            },
            "center_x": state.center_x_i32(),
            "center_z": state.center_z_i32(),
            "blocks_across": state.blocks_across_u32(),
            "yaw_radians": state.yaw_radians,
            "pitch_radians": state.pitch_radians,
            "finest_spacing": stats.finest_sample_spacing,
            "resident_bytes": stats.resident_bytes,
            "fixed_resident_bytes": stats.fixed_resident_bytes,
            "vegetation_bytes": stats.vegetation_bytes,
            "allocation_slots": stats.allocation_slots,
            "staging_slots": stats.staging_slots,
            "normal_halo_radius": stats.normal_halo_radius,
            "normal_halo_samples_per_tile": stats.normal_halo_samples_per_tile,
            "normal_halo_fixed_bytes": stats.normal_halo_fixed_bytes,
            "normal_height_fixed_bytes": stats.normal_height_fixed_bytes,
            "ready_slots": stats.ready_slots,
            "requested_levels": stats.requested_levels,
            "staged_levels": stats.staged_levels,
            "committed_levels": stats.committed_levels,
            "vegetation_committed_levels": stats.vegetation_committed_levels,
            "atomic_level_commits": stats.atomic_level_commits,
            "deferred_transition_attempts": stats.deferred_transition_attempts,
            "pending_refills": stats.pending_refills,
            "vegetation_ready_tiles": stats.vegetation_ready_tiles,
            "pending_vegetation_tiles": stats.pending_vegetation_tiles,
            "tree_instance_count": stats.tree_instance_count,
            "tree_proxy_vertex_count": stats.tree_proxy_vertex_count,
            "vegetation_service": vegetation_service_json(stats.vegetation_service),
            "total_refills": stats.residency.total_refills,
            "total_rebases": stats.residency.total_rebases,
            "rgb_min": pixels.min_rgb,
            "rgb_max": pixels.max_rgb,
            "opaque_pixels": pixels.opaque_pixels,
            "depth_min": depth.min_depth,
            "depth_max": depth.max_depth,
            "depth_covered_pixels": depth.covered_pixels,
            "depth_clear_pixels": depth.clear_pixels,
        });
        let fields = capture
            .as_object_mut()
            .expect("a JSON object literal produces an object");
        fields.insert("composition".to_owned(), json!(composition.label()));
        fields.insert("exact".to_owned(), exact_stats_json(exact));
        fields.insert("frontier".to_owned(), json!("procedural-collar-1.5-blocks"));
        self.captures.push(capture);
        Ok(())
    }

    pub fn write(&self, terrain: &ExplorerTerrain, options: &ExplorerOptions) -> Result<PathBuf> {
        let final_stats = terrain
            .last_stats()
            .context("World Explorer smoke completed without frame statistics")?;
        let path = self.root.join("receipt.json");
        let receipt = json!({
            "schema": "mclone-world-explorer-smoke-v3",
            "target": self.target,
            "adapter": {
                "name": self.adapter_name,
                "backend": self.adapter_backend,
                "device_type": self.adapter_device_type,
                "format": self.format,
            },
            "seed": options.seed,
            "asset_profile": options.asset_profile.label(),
            "asset_bytes": options.asset_bytes()?,
            "composition": terrain.composition_mode().label(),
            "exact": exact_stats_json(terrain.exact_stats()),
            "frontier": "procedural-collar-1.5-blocks",
            "viewport": {
                "width": options.width,
                "height": options.height,
            },
            "process_to_first_coarse_ms": duration_ms(
                terrain.process_first_coarse_ready_at()
            ),
            "process_to_first_target_ms": duration_ms(
                terrain.process_first_target_ready_at()
            ),
            "frame_count": self.frame_count,
            "movement_frames": frame_summary(&self.movement_frame_ms),
            "settling_frames": frame_summary(&self.settling_frame_ms),
            "checkpoint_frames": frame_summary(&self.checkpoint_frame_ms),
            "peak_resident_bytes": self.peak_resident_bytes,
            "peak_pending_work": self.peak_pending_work,
            "final_resident_bytes": final_stats.resident_bytes,
            "fixed_resident_bytes": final_stats.fixed_resident_bytes,
            "normal_halo_fixed_bytes": final_stats.normal_halo_fixed_bytes,
            "normal_height_fixed_bytes": final_stats.normal_height_fixed_bytes,
            "final_vegetation_bytes": final_stats.vegetation_bytes,
            "allocation_slots": final_stats.allocation_slots,
            "final_ready_slots": final_stats.ready_slots,
            "final_pending_work": final_stats.pending_refills,
            "total_refills": final_stats.residency.total_refills,
            "total_rebases": final_stats.residency.total_rebases,
            "captures": self.captures,
            "sequence": [
                "initial 3D",
                "continuous +X",
                "continuous -Z",
                "continuous diagonal",
                "negative-coordinate rebase",
                "large teleport",
                "anchored zoom",
                "map",
                "orbit",
            ],
        });
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&receipt)
                .context("failed to serialize World Explorer smoke receipt")?,
        )
        .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(path)
    }
}

fn exact_stats_json(stats: ExplorerExactStats) -> Value {
    json!({
        "desired_chunks": stats.desired_chunks,
        "painted_chunks": stats.painted_chunks,
        "queued_chunks": stats.queued_chunks,
        "pending_admissions": stats.pending_admissions,
        "in_flight": stats.in_flight,
        "coverage_generation": stats.coverage_generation,
        "admitted_chunks_total": stats.admitted_chunks_total,
        "stale_chunks_total": stats.stale_chunks_total,
        "generation_ms": stats.generation_ms,
        "presentation_ms": stats.presentation_ms,
        "mesh_ms": stats.mesh_ms,
        "pack_ms": stats.pack_ms,
        "resident_mesh_bytes": stats.resident_mesh_bytes,
        "vertex_count": stats.vertex_count,
        "index_count": stats.index_count,
        "drawn_sections": stats.drawn_sections,
        "drawn_indices": stats.drawn_indices,
        "complete": stats.complete,
    })
}

fn duration_ms(duration: Option<Duration>) -> Option<f64> {
    duration.map(|duration| duration.as_secs_f64() * 1_000.0)
}

fn stable_hex(value: u64) -> String {
    format!("{value:016x}")
}

fn vegetation_service_json(service: TerrainHorizonVegetationServiceStats) -> Value {
    let mut fields = Map::new();
    macro_rules! insert {
        ($name:literal, $value:expr) => {
            fields.insert($name.to_owned(), json!($value));
        };
    }
    insert!(
        "coordinator_state",
        coordinator_state_label(service.coordinator_state)
    );
    insert!("executor_kind", executor_kind_label(service.executor_kind));
    insert!("source_fingerprint", stable_hex(service.source_fingerprint));
    insert!(
        "terrain_source_revision",
        stable_hex(service.terrain_source_revision)
    );
    insert!(
        "compiler_source_revision",
        stable_hex(service.compiler_source_revision)
    );
    insert!(
        "vegetation_plan_revision",
        stable_hex(service.vegetation_plan_revision)
    );
    insert!("product_revision", service.product_revision);
    insert!(
        "mchv_wire_version",
        mclone_worldgen::terrain_vegetation::MCHV_WIRE_VERSION
    );
    insert!("record_hash", stable_hex(service.record_hash));
    insert!("family_counts", service.family_counts);
    insert!("record_count", service.record_count);
    insert!("product_count", service.product_count);
    insert!("executor_generation", service.executor_generation);
    insert!("source_epoch", service.source_epoch);
    insert!("coverage_revision", service.coverage_revision);
    insert!("desired_tiles", service.desired_tiles);
    insert!("queued_tiles", service.queued_tiles);
    insert!("resident_tiles", service.resident_tiles);
    insert!("in_flight", service.in_flight);
    insert!("submitted_jobs", service.submitted_jobs);
    insert!("completed_jobs", service.completed_jobs);
    insert!("admitted_products", service.admitted_products);
    insert!("source_resets", service.source_resets);
    insert!("transport_failures", service.transport_failures);
    insert!("executor_restarts", service.executor_restarts);
    insert!("job_failures", service.job_failures);
    insert!("stale_completions", service.stale_completions);
    insert!("superseded_completions", service.superseded_completions);
    insert!("submit_full_count", service.submit_full_count);
    insert!("compile_micros", service.compile_micros);
    insert!("cache_cell_requests", service.cache_cell_requests);
    insert!("cache_cell_hits", service.cache_cell_hits);
    insert!("cache_cell_misses", service.cache_cell_misses);
    insert!("cache_retained_cells", service.cache_retained_cells);
    insert!(
        "cache_retained_preliminary_candidates",
        service.cache_retained_preliminary_candidates
    );
    insert!("worker_submitted_jobs", service.executor_submitted_jobs);
    insert!("worker_completed_jobs", service.executor_completed_jobs);
    insert!(
        "worker_transport_failures",
        service.executor_transport_failures
    );
    insert!("worker_restart_count", service.executor_restart_count);
    insert!("result_capacity_bytes", service.result_capacity_bytes);
    insert!("result_high_water_bytes", service.result_high_water_bytes);
    insert!("result_overflow_count", service.result_overflow_count);
    insert!("copied_result_bytes", service.copied_result_bytes);
    insert!("main_decode_micros", service.main_decode_micros);
    Value::Object(fields)
}

const fn coordinator_state_label(state: Option<TerrainVegetationCoordinatorState>) -> &'static str {
    match state {
        None => "disabled",
        Some(TerrainVegetationCoordinatorState::Starting) => "starting",
        Some(TerrainVegetationCoordinatorState::Running) => "running",
        Some(TerrainVegetationCoordinatorState::Failed) => "failed",
        Some(TerrainVegetationCoordinatorState::ShuttingDown) => "shutting-down",
        Some(TerrainVegetationCoordinatorState::Terminated) => "terminated",
    }
}

const fn executor_kind_label(kind: Option<TerrainVegetationExecutorKind>) -> &'static str {
    match kind {
        None => "disabled",
        Some(TerrainVegetationExecutorKind::InlineTest) => "inline-test",
        Some(TerrainVegetationExecutorKind::NativeThread) => "native-thread",
        Some(TerrainVegetationExecutorKind::BrowserWorker) => "browser-worker",
    }
}

fn frame_summary(samples: &[f64]) -> Value {
    if samples.is_empty() {
        return json!({
            "count": 0,
            "mean_ms": 0.0,
            "p95_ms": 0.0,
            "max_ms": 0.0,
        });
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let p95_index = ((sorted.len() - 1) as f64 * 0.95).round() as usize;
    json!({
        "count": sorted.len(),
        "mean_ms": sorted.iter().sum::<f64>() / sorted.len() as f64,
        "p95_ms": sorted[p95_index],
        "max_ms": sorted[sorted.len() - 1],
    })
}
