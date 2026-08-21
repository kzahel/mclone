use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use mclone_app_runtime::frame_render::{TerrainBackdropRenderContext, TerrainBackdropRenderer};
use mclone_app_runtime::host_mode::SingleViewHostMode;
use mclone_app_runtime::render_asset_data::TexturedMeshAssets;
use mclone_blocks::{BlockFluidKind, block_fluid_kind};
use mclone_core::{ChunkPos, HorizontalTopology, TerrainLodPreset};
use mclone_render::color_profile::RenderColorProfile;
use mclone_terrain_view::{
    ExactPaintedCoverageSnapshot, TERRAIN_LOD_HIGH_LEVEL_COUNT, TerrainCompositionSourceIdentity,
    TerrainExactBoundaryColumn, TerrainExactBoundaryProfile, TerrainExactCoverageMode,
    TerrainFrontierAdmissionReceipt, TerrainFrontierPlanReceipt, TerrainFrontierTopologyReceipt,
    TerrainHorizonDiagnostic, TerrainHorizonFrameStats, TerrainHorizonPresentation,
    TerrainHorizonRenderTarget, TerrainLodPresetDescriptor, TerrainPreparedExactFrame,
    TerrainPreviewCamera, TerrainPreviewMaterialAtlas, TerrainPreviewMaterialTable,
    TerrainPreviewView, TerrainVegetationExecutor, TerrainViewEngine, TerrainViewEngineConfig,
    TerrainViewSourceIdentity, terrain_exact_exposed_boundary_blocks,
    terrain_exact_player_connected_chunks,
};
use mclone_worldgen::terrain_preview::{TerrainPreviewContentStage, TerrainPreviewProfile};

use crate::{McloneSceneHost, WorldInstanceId, engine_terrain_lod_preset};

pub(crate) type SceneTerrainVegetationExecutorFactory =
    Box<dyn Fn() -> Result<Box<dyn TerrainVegetationExecutor>, String>>;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn default_scene_terrain_vegetation_executor_factory()
-> Option<SceneTerrainVegetationExecutorFactory> {
    Some(Box::new(|| {
        Ok(Box::new(
            mclone_terrain_view::NativeTerrainVegetationExecutor::new(),
        ))
    }))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn default_scene_terrain_vegetation_executor_factory()
-> Option<SceneTerrainVegetationExecutorFactory> {
    None
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SceneTerrainViewDiagnostics {
    pub lod_preset: TerrainLodPreset,
    pub lod_level_count: u32,
    pub vegetation_max_sample_spacing: u32,
    pub source_generation: u64,
    pub coverage_generation: u64,
    pub exact_column_count: u32,
    pub exact_center_ready: bool,
    pub last_frame_revision: u64,
    pub ready_slots: u32,
    pub cpu_compile_workers: u32,
    pub cpu_compile_in_flight: u32,
    pub cpu_compile_submitted_total: u64,
    pub cpu_compile_completed_total: u64,
    pub cpu_compile_micros_total: u64,
    pub cpu_compile_stale_results_total: u64,
    pub drawn_levels: u32,
    pub drawn_tiles: u32,
    pub drawn_tiles_by_level: [u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize],
    pub vertex_count: u32,
    pub fixed_resident_bytes: u64,
    pub resident_bytes: u64,
    pub exact_connector_segments: u32,
    pub exact_connector_vertex_count: u32,
    pub exact_connector_bytes: u64,
    pub frontier_support_allocated_tiles: u32,
    pub frontier_support_ready_tiles: u32,
    pub frontier_support_pending_tiles: u32,
    pub frontier_support_drawn_tiles: u32,
    pub frontier_support_dispatches: u32,
    pub frontier_support_dispatches_total: u64,
    pub frontier_support_resource_bytes: u64,
    pub frontier_support_vertex_count: u32,
    pub frontier_connector_segments: u32,
    pub frontier_connector_vertex_count: u32,
    pub frontier_connector_bytes: u64,
    pub exact_transition_preparation_micros: u64,
    pub exact_transition_payload_bytes: u64,
    pub exact_boundary_preparation_micros: u64,
    pub exact_boundary_columns: u32,
    pub exact_boundary_payload_bytes: u64,
    pub frontier: TerrainFrontierPlanReceipt,
    pub frontier_plan_failures: u64,
    pub frontier_topology: TerrainFrontierTopologyReceipt,
    pub frontier_topology_failures: u64,
    pub frontier_admission: TerrainFrontierAdmissionReceipt,
    pub inner_hole_culled_tiles: u32,
    pub frustum_culled_tiles: u32,
    pub far_culled_tiles: u32,
    pub drawn_tree_tiles: u32,
    pub drawn_tree_instances: u32,
    pub drawn_tree_tiles_by_level: [u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize],
    pub drawn_tree_instances_by_level: [u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize],
    pub drawn_canopy_tiles: u32,
    pub drawn_canopy_cells: u32,
    pub drawn_canopy_vertices: u32,
    pub drawn_canopy_tiles_by_level: [u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize],
    pub drawn_canopy_cells_by_level: [u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize],
    pub target_ready: bool,
    pub tree_instance_count: u32,
    pub pending_vegetation_tiles: u32,
    pub vegetation_enabled: bool,
    pub vegetation_resident_tiles: u32,
    pub vegetation_submitted_jobs: u64,
    pub vegetation_completed_jobs: u64,
    pub vegetation_transport_failures: u64,
    pub vegetation_job_failures: u64,
}

pub(crate) struct SceneTerrainViewState {
    engine: TerrainViewEngine,
    world: WorldInstanceId,
    source: TerrainViewSourceIdentity,
    exact: TerrainPreparedExactFrame,
    ready_columns: BTreeSet<ChunkPos>,
    coverage_generation: u64,
    anchor: [f64; 2],
    diagnostic: TerrainHorizonDiagnostic,
    diagnostics: SceneTerrainViewDiagnostics,
}

impl SceneTerrainViewState {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        color_profile: RenderColorProfile,
        mesh_assets: &TexturedMeshAssets,
        world: WorldInstanceId,
        profile: mclone_server::WorldGenerationProfile,
        seed: i64,
        topology: HorizontalTopology,
        lod: TerrainLodPresetDescriptor,
        vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
    ) -> Result<Self> {
        let lod = lod.validated().map_err(anyhow::Error::msg)?;
        let clipmap = lod
            .clipmap
            .context("cannot create a terrain-view engine for LOD Off")?;
        let source = live_source(world, profile, seed, topology)?;
        let coverage_generation = 1;
        let coverage = ExactPaintedCoverageSnapshot::new(
            source.composition_source().map_err(anyhow::Error::msg)?,
            coverage_generation,
            [],
        )
        .map_err(anyhow::Error::msg)?;
        let exact = TerrainPreparedExactFrame::new(source, coverage).map_err(anyhow::Error::msg)?;
        let material_table = TerrainPreviewMaterialTable::from_catalog(&mesh_assets.catalog);
        let atlas = mesh_assets.atlas.as_upload();
        let vegetation_enabled = vegetation_executor.is_some();
        let mut engine = TerrainViewEngine::new(
            device,
            queue,
            color_format,
            TerrainViewEngineConfig {
                width: 1,
                height: 1,
                source,
                clipmap,
                render_cell_stride: lod.render_cell_stride,
                vegetation_max_sample_spacing: lod
                    .vegetation_max_sample_spacing
                    .context("enabled terrain LOD has no vegetation bound")?,
                vegetation_enabled,
                color_profile,
            },
            TerrainPreviewMaterialAtlas {
                width: atlas.width,
                height: atlas.height,
                rgba: atlas.rgba,
                material_table: &material_table,
            },
            vegetation_executor,
        )
        .map_err(anyhow::Error::msg)
        .context("create shared live terrain-view engine")?;
        engine.set_authoritative_tree_ownership(true);
        Ok(Self {
            engine,
            world,
            source,
            exact,
            ready_columns: BTreeSet::new(),
            coverage_generation,
            anchor: [0.0, 0.0],
            diagnostic: TerrainHorizonDiagnostic::Natural,
            diagnostics: SceneTerrainViewDiagnostics {
                lod_preset: lod.preset,
                lod_level_count: clipmap.level_count,
                vegetation_max_sample_spacing: lod
                    .vegetation_max_sample_spacing
                    .expect("validated enabled LOD retains a vegetation bound"),
                source_generation: source.generation(),
                coverage_generation,
                ..Default::default()
            },
        })
    }

    pub(crate) fn prepare(
        &mut self,
        world: WorldInstanceId,
        profile: mclone_server::WorldGenerationProfile,
        seed: i64,
        topology: HorizontalTopology,
        focus: [f64; 3],
        ready_columns: BTreeSet<ChunkPos>,
    ) -> Result<(BTreeSet<ChunkPos>, bool)> {
        let mut source_changed = false;
        if self.world != world
            || self
                .source
                .composition_source()
                .map_err(anyhow::Error::msg)?
                .seed
                != seed
            || self.source.topology() != topology
        {
            self.world = world;
            self.source = live_source(world, profile, seed, topology)?;
            self.engine
                .replace_source(self.source)
                .map_err(anyhow::Error::msg)?;
            self.ready_columns.clear();
            self.coverage_generation = 1;
            source_changed = true;
        }
        let focus_chunk =
            ChunkPos::from_block_coords(floor_f64_to_i32(focus[0]), floor_f64_to_i32(focus[2]));
        let admitted_columns = terrain_exact_player_connected_chunks(
            &ready_columns,
            &self.ready_columns,
            focus_chunk,
            topology,
        );
        let coverage_changed = admitted_columns != self.ready_columns;
        if coverage_changed {
            self.ready_columns = admitted_columns;
            self.coverage_generation = self.coverage_generation.saturating_add(1).max(1);
        }
        if source_changed || coverage_changed {
            let coverage = ExactPaintedCoverageSnapshot::new(
                self.source
                    .composition_source()
                    .map_err(anyhow::Error::msg)?,
                self.coverage_generation,
                self.ready_columns.iter().copied(),
            )
            .map_err(anyhow::Error::msg)
            .context("pack live exact-painted terrain coverage")?;
            self.exact = TerrainPreparedExactFrame::new(self.source, coverage)
                .map_err(anyhow::Error::msg)?;
        }
        self.anchor = [focus[0], focus[2]];
        self.engine.set_residency(
            floor_f64_to_i32(focus[0]),
            floor_f64_to_i32(focus[2]),
            TerrainPreviewContentStage::Cover,
        );
        self.diagnostics.source_generation = self.source.generation();
        self.diagnostics.coverage_generation = self.coverage_generation;
        self.diagnostics.exact_column_count =
            u32::try_from(self.ready_columns.len()).unwrap_or(u32::MAX);
        self.diagnostics.exact_center_ready =
            terrain_exact_center_ready(&self.ready_columns, focus);
        Ok((
            self.ready_columns.clone(),
            source_changed || coverage_changed,
        ))
    }

    pub(crate) fn reconfigure_lod(
        &mut self,
        device: &wgpu::Device,
        descriptor: TerrainLodPresetDescriptor,
    ) -> Result<bool> {
        let changed = self
            .engine
            .reconfigure_lod(device, descriptor)
            .map_err(anyhow::Error::msg)?;
        if changed {
            let descriptor = descriptor.validated().map_err(anyhow::Error::msg)?;
            self.diagnostics.target_ready = false;
            self.diagnostics.lod_preset = descriptor.preset;
            self.diagnostics.lod_level_count = descriptor
                .clipmap
                .expect("enabled LOD descriptor remains enabled")
                .level_count;
            self.diagnostics.vegetation_max_sample_spacing = descriptor
                .vegetation_max_sample_spacing
                .expect("enabled LOD descriptor retains vegetation");
        }
        Ok(changed)
    }

    pub(crate) fn exact_coverage(&self) -> &ExactPaintedCoverageSnapshot {
        self.exact.coverage()
    }

    pub(crate) fn set_exact_boundary_profile(
        &mut self,
        boundary: TerrainExactBoundaryProfile,
        preparation_micros: u64,
    ) -> Result<()> {
        self.diagnostics.exact_boundary_preparation_micros = preparation_micros;
        self.diagnostics.exact_boundary_columns = boundary.valid_columns();
        self.diagnostics.exact_boundary_payload_bytes = boundary.payload_bytes();
        self.exact = self
            .exact
            .clone()
            .with_boundary_profile(boundary)
            .map_err(anyhow::Error::msg)?;
        Ok(())
    }

    pub(crate) const fn diagnostics(&self) -> SceneTerrainViewDiagnostics {
        self.diagnostics
    }

    pub(crate) fn set_diagnostic(&mut self, diagnostic: TerrainHorizonDiagnostic) {
        self.diagnostic = diagnostic;
    }

    pub(crate) fn shutdown(&mut self) {
        self.engine.shutdown();
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        size: [u32; 2],
        render_views: [mclone_render::chunk::ChunkRenderView; 2],
        sky_darken: f32,
        fog: mclone_render::fog::RenderFog,
    ) -> Result<()> {
        self.engine.resize(device, size[0], size[1]);
        let presentation = TerrainHorizonPresentation::new(
            self.anchor[0],
            self.anchor[1],
            1_048_576.0,
            1_048_576.0,
            TerrainPreviewView::ThreeDimensional,
            TerrainPreviewCamera::default(),
        )
        .and_then(|presentation| presentation.with_multiview_render_views(render_views))
        .map_err(anyhow::Error::msg)?
        .with_sky_darken(sky_darken)
        .with_fog(fog)
        .with_diagnostic(self.diagnostic);
        let stats = self
            .engine
            .encode_multiview_to_target(
                device,
                queue,
                encoder,
                TerrainHorizonRenderTarget {
                    color_view,
                    depth_view,
                    color_load: wgpu::LoadOp::Load,
                    color_store: wgpu::StoreOp::Store,
                    depth_load: wgpu::LoadOp::Load,
                    depth_store: wgpu::StoreOp::Store,
                },
                presentation,
                Some((&self.exact, TerrainExactCoverageMode::DiscardPainted)),
                None,
            )
            .map_err(anyhow::Error::msg)
            .context("render shared live terrain backdrop multiview")?;
        self.record_stats(stats);
        Ok(())
    }
}

pub(crate) fn scene_terrain_projection_far_distance(
    preset: TerrainLodPreset,
    ordinary_far_distance: f32,
) -> f32 {
    if preset.horizon_enabled() {
        ordinary_far_distance
            .max(TerrainLodPresetDescriptor::for_preset(preset).terrain_visibility_distance())
    } else {
        ordinary_far_distance
    }
}

impl McloneSceneHost {
    pub const fn terrain_lod_preset_preference(&self) -> TerrainLodPreset {
        self.terrain_lod_preset_preference
    }

    pub fn terrain_lod_supported(&self) -> bool {
        matches!(
            self.active_world.scene.world_generation_profile,
            mclone_server::WorldGenerationProfile::McloneOverworldV1
                | mclone_server::WorldGenerationProfile::McloneOverworldV2
        ) && self.active_world.runtime.as_ref().map_or_else(
            || self.active_world.scene.startup.remote_addr.is_none(),
            |runtime| runtime.host_mode() == SingleViewHostMode::LocalIntegrated,
        )
    }

    pub(crate) fn effective_terrain_lod_preset(&self) -> TerrainLodPreset {
        if self.terrain_lod_supported() {
            engine_terrain_lod_preset(self.terrain_lod_preset_preference)
        } else {
            TerrainLodPreset::Off
        }
    }

    pub fn applied_terrain_lod_preset(&self) -> TerrainLodPreset {
        if !self.terrain_lod_supported() {
            TerrainLodPreset::Off
        } else {
            self.terrain_lod_applied_preset
        }
    }

    pub fn terrain_lod_applying(&self) -> bool {
        self.terrain_lod_supported()
            && self.terrain_lod_applied_preset != self.terrain_lod_preset_preference
    }

    pub fn terrain_lod_apply_error(&self) -> Option<&str> {
        self.terrain_lod_apply_error.as_deref()
    }

    pub(crate) fn terrain_projection_far_distance(&self, ordinary_far_distance: f32) -> f32 {
        scene_terrain_projection_far_distance(
            self.effective_terrain_lod_preset(),
            ordinary_far_distance,
        )
    }

    pub fn request_terrain_lod_preset(&mut self, presentation: TerrainLodPreset) -> Result<()> {
        if presentation == self.terrain_lod_preset_preference {
            return Ok(());
        }
        self.terrain_lod_apply_error = None;
        self.terrain_lod_preset_preference = presentation;
        if !presentation.horizon_enabled() {
            self.reset_terrain_view();
            self.terrain_lod_persisted_preference = Some(presentation);
            self.terrain_lod_pending_persistence = None;
            self.persist_graphics_preferences();
        } else if !self.terrain_lod_supported() {
            self.terrain_lod_persisted_preference = Some(presentation);
            self.terrain_lod_pending_persistence = None;
            self.persist_graphics_preferences();
        } else {
            self.terrain_lod_pending_persistence = Some(presentation);
        }
        self.sync_terrain_lod_settings_controller_state();
        log::info!(
            "terrain horizon preference set to {}; active={}",
            presentation.label(),
            self.effective_terrain_lod_preset().label(),
        );
        Ok(())
    }

    pub(crate) fn prepare_terrain_view_for_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        focus: [f64; 3],
    ) -> Result<bool> {
        let effective_preset = self.effective_terrain_lod_preset();
        self.accept_ready_terrain_lod_preset(effective_preset);
        if !effective_preset.horizon_enabled() {
            self.reset_terrain_view();
            return Ok(false);
        }
        let world = self.active_world.id;
        let profile = self.active_world.scene.world_generation_profile;
        let seed = self.active_world.scene.seed;
        let topology = self
            .active_world
            .runtime
            .as_ref()
            .map_or(self.active_world.scene.world_topology, |runtime| {
                runtime.client().topology()
            });
        let ready_columns = self
            .active_world
            .traversal_ready_sections
            .ready_columns()
            .clone();
        if self.terrain_view.is_none() {
            let terrain_view = (|| -> Result<SceneTerrainViewState> {
                let vegetation_executor = self
                    .terrain_vegetation_executor_factory
                    .as_ref()
                    .map(|factory| factory())
                    .transpose()
                    .map_err(anyhow::Error::msg)
                    .context("construct scene terrain vegetation executor")?;
                SceneTerrainViewState::new(
                    device,
                    queue,
                    self.color_format,
                    self.render_options.color_profile,
                    &self.mesh_assets,
                    world,
                    profile,
                    seed,
                    topology,
                    TerrainLodPresetDescriptor::for_preset(effective_preset),
                    vegetation_executor,
                )
            })();
            match terrain_view {
                Ok(terrain_view) => self.terrain_view = Some(terrain_view),
                Err(error) => {
                    self.reject_pending_terrain_lod_preset(error);
                    return Ok(false);
                }
            }
        }
        if let Err(error) = self
            .terrain_view
            .as_mut()
            .expect("enabled terrain view was initialized")
            .reconfigure_lod(
                device,
                TerrainLodPresetDescriptor::for_preset(effective_preset),
            )
        {
            let fallback_enabled = self.terrain_lod_applied_preset.horizon_enabled();
            self.reject_pending_terrain_lod_preset(error);
            return Ok(fallback_enabled);
        }
        self.terrain_view
            .as_mut()
            .expect("composed terrain view was initialized")
            .set_diagnostic(self.terrain_horizon_diagnostic);
        let (admitted_columns, coverage_changed) = self
            .terrain_view
            .as_mut()
            .expect("composed terrain view was initialized")
            .prepare(world, profile, seed, topology, focus, ready_columns)?;
        if coverage_changed {
            let coverage = self
                .terrain_view
                .as_ref()
                .expect("composed terrain view was initialized")
                .exact_coverage()
                .clone();
            let boundary_started = boundary_timing_now();
            let boundary = live_exact_boundary_profile(
                self.active_world.runtime.as_ref(),
                &self.mesh_assets.catalog,
                &coverage,
                topology,
            )?;
            let boundary_preparation_micros = boundary_timing_elapsed_micros(boundary_started);
            self.terrain_view
                .as_mut()
                .expect("composed terrain view was initialized")
                .set_exact_boundary_profile(boundary, boundary_preparation_micros)?;
        }
        self.active_world
            .draw
            .set_traversal_ready_columns_with_context(&admitted_columns, false);
        Ok(true)
    }

    fn accept_ready_terrain_lod_preset(&mut self, effective_preset: TerrainLodPreset) {
        let ready = self.terrain_view.as_ref().is_some_and(|state| {
            terrain_lod_ready_for_acceptance(state.diagnostics(), effective_preset)
        });
        if !ready {
            return;
        }
        self.terrain_lod_applied_preset = effective_preset;
        self.commit_pending_terrain_lod_preference(effective_preset);
        self.sync_terrain_lod_settings_controller_state();
    }

    fn reject_pending_terrain_lod_preset(&mut self, error: anyhow::Error) {
        let rejected = self.terrain_lod_preset_preference;
        let fallback = self.terrain_lod_applied_preset;
        let message = format!(
            "Could not apply {} distant terrain detail; kept {}: {error:#}",
            rejected.label(),
            fallback.label(),
        );
        log::error!("{message}");
        self.terrain_lod_preset_preference = fallback;
        self.terrain_lod_pending_persistence = None;
        self.terrain_lod_apply_error = Some(message);
        if !fallback.horizon_enabled() {
            self.reset_terrain_view();
        }
        self.sync_terrain_lod_settings_controller_state();
    }

    fn commit_pending_terrain_lod_preference(&mut self, accepted: TerrainLodPreset) {
        let Some(preset) = self.terrain_lod_pending_persistence else {
            return;
        };
        if preset != accepted {
            return;
        }
        self.terrain_lod_pending_persistence = None;
        self.terrain_lod_persisted_preference = Some(preset);
        self.persist_graphics_preferences();
    }

    pub(crate) fn reset_terrain_lod_preference_to_platform_default(&mut self) {
        self.terrain_lod_persisted_preference = None;
        self.terrain_lod_pending_persistence = None;
        self.terrain_lod_apply_error = None;
        if self.active_world.scene.startup.terrain_lod_preset_explicit {
            return;
        }
        self.terrain_lod_preset_preference = self
            .active_world
            .scene
            .startup
            .graphics_platform_profile
            .default_terrain_lod_preset();
        if !self.terrain_lod_preset_preference.horizon_enabled() {
            self.reset_terrain_view();
        }
        self.sync_terrain_lod_settings_controller_state();
    }

    fn sync_terrain_lod_settings_controller_state(&mut self) {
        let desired = self.terrain_lod_preset_preference;
        let effective = self.applied_terrain_lod_preset();
        let applying = self.terrain_lod_applying();
        let available = self.terrain_lod_supported();
        let failed = self.terrain_lod_apply_error.is_some();
        let settings = self.client_experience.settings_mut();
        let mut state = settings.state();
        state.terrain_lod_preset = desired;
        state.terrain_lod_effective_preset = effective;
        state.terrain_lod_applying = applying;
        state.terrain_lod_available = available;
        state.terrain_lod_apply_failed = failed;
        settings.set_state(state);
    }

    pub fn terrain_view_diagnostics(&self) -> Option<SceneTerrainViewDiagnostics> {
        self.terrain_view
            .as_ref()
            .map(SceneTerrainViewState::diagnostics)
    }

    pub fn set_terrain_horizon_diagnostic(&mut self, diagnostic: TerrainHorizonDiagnostic) {
        self.terrain_horizon_diagnostic = diagnostic;
        if let Some(terrain_view) = self.terrain_view.as_mut() {
            terrain_view.set_diagnostic(diagnostic);
        }
    }

    pub(crate) fn reset_terrain_view(&mut self) {
        self.terrain_lod_applied_preset = TerrainLodPreset::Off;
        if let Some(mut terrain_view) = self.terrain_view.take() {
            terrain_view.shutdown();
            let ready_columns = self
                .active_world
                .traversal_ready_sections
                .ready_columns()
                .clone();
            self.active_world
                .draw
                .set_traversal_ready_columns_with_context(&ready_columns, false);
        }
    }

    pub fn configure_terrain_vegetation_executor_factory(
        &mut self,
        factory: impl Fn() -> Result<Box<dyn TerrainVegetationExecutor>, String> + 'static,
    ) {
        self.reset_terrain_view();
        self.terrain_vegetation_executor_factory = Some(Box::new(factory));
    }
}

impl TerrainBackdropRenderer for SceneTerrainViewState {
    fn render(&mut self, context: TerrainBackdropRenderContext<'_>) -> Result<()> {
        self.engine
            .resize(context.device, context.size[0], context.size[1]);
        let presentation = TerrainHorizonPresentation::new(
            self.anchor[0],
            self.anchor[1],
            1_048_576.0,
            1_048_576.0,
            TerrainPreviewView::ThreeDimensional,
            TerrainPreviewCamera::default(),
        )
        .and_then(|presentation| presentation.with_render_view(context.render_view))
        .map_err(anyhow::Error::msg)?
        .with_sky_darken(context.sky_darken)
        .with_fog(context.fog)
        .with_diagnostic(self.diagnostic);
        let stats = self
            .engine
            .encode_to_target(
                context.device,
                context.queue,
                context.encoder,
                TerrainHorizonRenderTarget {
                    color_view: context.color_view,
                    depth_view: context.depth_view,
                    color_load: wgpu::LoadOp::Load,
                    color_store: wgpu::StoreOp::Store,
                    // Exact opaque/cutout terrain has prepared the frame's
                    // reversed-Z depth. The shared backdrop loads it so the
                    // representations participate in one depth comparison.
                    depth_load: wgpu::LoadOp::Load,
                    depth_store: wgpu::StoreOp::Store,
                },
                presentation,
                Some((&self.exact, TerrainExactCoverageMode::DiscardPainted)),
                None,
            )
            .map_err(anyhow::Error::msg)
            .context("render shared live terrain backdrop")?;
        self.record_stats(stats);
        Ok(())
    }
}

impl SceneTerrainViewState {
    fn record_stats(&mut self, stats: TerrainHorizonFrameStats) {
        self.diagnostics.last_frame_revision = stats.revision;
        self.diagnostics.ready_slots = stats.ready_slots;
        self.diagnostics.cpu_compile_workers = stats.cpu_compile_workers;
        self.diagnostics.cpu_compile_in_flight = stats.cpu_compile_in_flight;
        self.diagnostics.cpu_compile_submitted_total = stats.cpu_compile_submitted_total;
        self.diagnostics.cpu_compile_completed_total = stats.cpu_compile_completed_total;
        self.diagnostics.cpu_compile_micros_total = stats.cpu_compile_micros_total;
        self.diagnostics.cpu_compile_stale_results_total = stats.cpu_compile_stale_results_total;
        self.diagnostics.drawn_levels = stats.drawn_levels;
        self.diagnostics.drawn_tiles = stats.drawn_tiles;
        self.diagnostics.drawn_tiles_by_level = stats.drawn_tiles_by_level;
        self.diagnostics.vertex_count = stats.vertex_count;
        self.diagnostics.fixed_resident_bytes = stats.fixed_resident_bytes;
        self.diagnostics.resident_bytes = stats.resident_bytes;
        self.diagnostics.exact_connector_segments = stats.exact_connector_segments;
        self.diagnostics.exact_connector_vertex_count = stats.exact_connector_vertex_count;
        self.diagnostics.exact_connector_bytes = stats.exact_connector_bytes;
        self.diagnostics.frontier_support_allocated_tiles = stats.frontier_support_allocated_tiles;
        self.diagnostics.frontier_support_ready_tiles = stats.frontier_support_ready_tiles;
        self.diagnostics.frontier_support_pending_tiles = stats.frontier_support_pending_tiles;
        self.diagnostics.frontier_support_drawn_tiles = stats.frontier_support_drawn_tiles;
        self.diagnostics.frontier_support_dispatches = stats.frontier_support_dispatches;
        self.diagnostics.frontier_support_dispatches_total =
            stats.frontier_support_dispatches_total;
        self.diagnostics.frontier_support_resource_bytes = stats.frontier_support_resource_bytes;
        self.diagnostics.frontier_support_vertex_count = stats.frontier_support_vertex_count;
        self.diagnostics.frontier_connector_segments = stats.frontier_connector_segments;
        self.diagnostics.frontier_connector_vertex_count = stats.frontier_connector_vertex_count;
        self.diagnostics.frontier_connector_bytes = stats.frontier_connector_bytes;
        self.diagnostics.exact_transition_preparation_micros =
            stats.exact_transition_preparation_micros;
        self.diagnostics.exact_transition_payload_bytes = stats.exact_transition_payload_bytes;
        self.diagnostics.exact_boundary_columns = stats.exact_boundary_columns;
        self.diagnostics.exact_boundary_payload_bytes = stats.exact_boundary_payload_bytes;
        self.diagnostics.frontier = stats.frontier;
        self.diagnostics.frontier_plan_failures = stats.frontier_plan_failures;
        self.diagnostics.frontier_topology = stats.frontier_topology;
        self.diagnostics.frontier_topology_failures = stats.frontier_topology_failures;
        self.diagnostics.frontier_admission = stats.frontier_admission;
        self.diagnostics.inner_hole_culled_tiles = stats.inner_hole_culled_tiles;
        self.diagnostics.frustum_culled_tiles = stats.frustum_culled_tiles;
        self.diagnostics.far_culled_tiles = stats.far_culled_tiles;
        self.diagnostics.drawn_tree_tiles = stats.drawn_tree_tiles;
        self.diagnostics.drawn_tree_instances = stats.drawn_tree_instances;
        self.diagnostics.drawn_tree_tiles_by_level = stats.drawn_tree_tiles_by_level;
        self.diagnostics.drawn_tree_instances_by_level = stats.drawn_tree_instances_by_level;
        self.diagnostics.drawn_canopy_tiles = stats.drawn_canopy_tiles;
        self.diagnostics.drawn_canopy_cells = stats.drawn_canopy_cells;
        self.diagnostics.drawn_canopy_vertices = stats.drawn_canopy_vertices;
        self.diagnostics.drawn_canopy_tiles_by_level = stats.drawn_canopy_tiles_by_level;
        self.diagnostics.drawn_canopy_cells_by_level = stats.drawn_canopy_cells_by_level;
        self.diagnostics.target_ready = stats.target_ready;
        self.diagnostics.tree_instance_count = stats.tree_instance_count;
        self.diagnostics.pending_vegetation_tiles = stats.pending_vegetation_tiles;
        self.diagnostics.vegetation_enabled = stats.vegetation_service.enabled;
        self.diagnostics.vegetation_resident_tiles = stats.vegetation_service.resident_tiles;
        self.diagnostics.vegetation_submitted_jobs = stats.vegetation_service.submitted_jobs;
        self.diagnostics.vegetation_completed_jobs = stats.vegetation_service.completed_jobs;
        self.diagnostics.vegetation_transport_failures =
            stats.vegetation_service.transport_failures;
        self.diagnostics.vegetation_job_failures = stats.vegetation_service.job_failures;
    }
}

fn live_source(
    world: WorldInstanceId,
    profile: mclone_server::WorldGenerationProfile,
    seed: i64,
    topology: HorizontalTopology,
) -> Result<TerrainViewSourceIdentity> {
    if world.get() == 0 {
        bail!("live terrain-view world generation must be non-zero");
    }
    let preview_profile = match profile {
        mclone_server::WorldGenerationProfile::McloneOverworldV1 => {
            TerrainPreviewProfile::McloneOverworldV1
        }
        mclone_server::WorldGenerationProfile::McloneOverworldV2 => {
            TerrainPreviewProfile::McloneOverworldV2
        }
        _ => bail!(
            "world profile {} has no live distant-terrain source",
            profile.label()
        ),
    };
    TerrainViewSourceIdentity::live(
        TerrainCompositionSourceIdentity::new(preview_profile, seed),
        topology,
        world.get(),
        1,
    )
    .map_err(anyhow::Error::msg)
}

fn floor_f64_to_i32(value: f64) -> i32 {
    if value.is_nan() {
        0
    } else if value <= f64::from(i32::MIN) {
        i32::MIN
    } else if value >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        value.floor() as i32
    }
}

fn terrain_lod_ready_for_acceptance(
    diagnostics: SceneTerrainViewDiagnostics,
    requested: TerrainLodPreset,
) -> bool {
    requested.horizon_enabled() && diagnostics.target_ready && diagnostics.lod_preset == requested
}

fn terrain_exact_center_ready(ready_columns: &BTreeSet<ChunkPos>, focus: [f64; 3]) -> bool {
    let center =
        ChunkPos::from_block_coords(floor_f64_to_i32(focus[0]), floor_f64_to_i32(focus[2]));
    ready_columns.contains(&center)
}

#[cfg(target_arch = "wasm32")]
fn boundary_timing_now() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn boundary_timing_now() -> std::time::Instant {
    std::time::Instant::now()
}

#[cfg(target_arch = "wasm32")]
fn boundary_timing_elapsed_micros(started: f64) -> u64 {
    ((js_sys::Date::now() - started).max(0.0) * 1_000.0) as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn boundary_timing_elapsed_micros(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn live_exact_boundary_profile(
    runtime: Option<&mclone_app_runtime::scene_session_runtime::SceneSessionRuntime>,
    catalog: &mclone_mesh::TexturedMeshCatalog,
    coverage: &ExactPaintedCoverageSnapshot,
    topology: HorizontalTopology,
) -> Result<TerrainExactBoundaryProfile> {
    let Some(runtime) = runtime else {
        return TerrainExactBoundaryProfile::empty(coverage).map_err(anyhow::Error::msg);
    };
    let mut columns = Vec::new();
    for [world_x, world_z] in
        terrain_exact_exposed_boundary_blocks(coverage, topology).map_err(anyhow::Error::msg)?
    {
        let Some(highest_y) = runtime.highest_non_air_block_y_at_world(world_x, world_z) else {
            continue;
        };
        let mut water = false;
        let minimum_y = highest_y.saturating_sub(1_024);
        for world_y in (minimum_y..=highest_y).rev() {
            let Some(state) = runtime.block_state_at_world(world_x, world_y, world_z) else {
                continue;
            };
            if block_fluid_kind(state) == BlockFluidKind::Water {
                water = true;
                continue;
            }
            if !catalog.occludes(state) || exact_boundary_natural_feature_block(state.0) {
                continue;
            }
            let side_material = u8::try_from(state.0)
                .ok()
                .filter(|_| catalog.terrain_surface_material(state).is_some());
            columns.push(TerrainExactBoundaryColumn {
                world_x,
                world_z,
                solid_top_y: world_y.saturating_add(1),
                side_material,
                water,
            });
            break;
        }
    }
    TerrainExactBoundaryProfile::from_columns(coverage, columns).map_err(anyhow::Error::msg)
}

fn exact_boundary_natural_feature_block(raw: u32) -> bool {
    use mclone_worldgen::block::{
        ACACIA_LOG, BIRCH_LOG, BIRCH_LOG_X, BIRCH_LOG_Z, DARK_OAK_LOG, JUNGLE_LOG, MUSHROOM_STEM,
        OAK_LOG, OAK_LOG_X, OAK_LOG_Z, SPRUCE_LOG, SPRUCE_LOG_X, SPRUCE_LOG_Z,
    };
    [
        OAK_LOG,
        BIRCH_LOG,
        SPRUCE_LOG,
        OAK_LOG_X,
        OAK_LOG_Z,
        BIRCH_LOG_X,
        BIRCH_LOG_Z,
        SPRUCE_LOG_X,
        SPRUCE_LOG_Z,
        DARK_OAK_LOG,
        MUSHROOM_STEM,
        ACACIA_LOG,
        JUNGLE_LOG,
    ]
    .into_iter()
    .any(|feature| raw == u32::from(feature))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_source_uses_world_identity_as_generation() {
        let source = live_source(
            WorldInstanceId::new(17),
            mclone_server::WorldGenerationProfile::McloneOverworldV2,
            -98_765,
            HorizontalTopology::UNBOUNDED,
        )
        .unwrap();
        assert_eq!(source.generation(), 17);
        assert_eq!(source.source_revision(), 1);
        assert_eq!(
            source.truth_role(),
            mclone_terrain_view::TerrainViewTruthRole::LiveAuthoritative
        );
        assert_eq!(source.composition_source().unwrap().seed, -98_765);
        assert_eq!(
            source.composition_source().unwrap().profile,
            TerrainPreviewProfile::McloneOverworldV2
        );
    }

    #[test]
    fn terrain_focus_conversion_is_saturating_and_flooring() {
        assert_eq!(floor_f64_to_i32(15.99), 15);
        assert_eq!(floor_f64_to_i32(-0.01), -1);
        assert_eq!(floor_f64_to_i32(f64::INFINITY), i32::MAX);
        assert_eq!(floor_f64_to_i32(f64::NEG_INFINITY), i32::MIN);
        assert_eq!(floor_f64_to_i32(f64::NAN), 0);
    }

    #[test]
    fn exact_center_readiness_tracks_the_focus_chunk() {
        let ready = BTreeSet::from([ChunkPos::new(-1, 2), ChunkPos::new(0, 2)]);
        assert!(terrain_exact_center_ready(&ready, [-0.01, 90.0, 47.99]));
        assert!(!terrain_exact_center_ready(&ready, [16.0, 90.0, 47.99]));
    }

    #[test]
    fn lod_presets_extend_projection_to_their_shared_reach() {
        assert_eq!(
            scene_terrain_projection_far_distance(TerrainLodPreset::Off, 1_084.0),
            1_084.0
        );
        let low = scene_terrain_projection_far_distance(TerrainLodPreset::Low, 1_084.0);
        let medium = scene_terrain_projection_far_distance(TerrainLodPreset::Medium, 1_084.0);
        let high = scene_terrain_projection_far_distance(TerrainLodPreset::High, 1_084.0);
        assert!(low > 1_084.0);
        assert!(medium > low);
        assert!(high > medium);
    }

    #[test]
    fn lod_acceptance_requires_the_requested_preset_to_be_fully_ready() {
        let ready_low = SceneTerrainViewDiagnostics {
            lod_preset: TerrainLodPreset::Low,
            target_ready: true,
            ..Default::default()
        };

        assert!(terrain_lod_ready_for_acceptance(
            ready_low,
            TerrainLodPreset::Low
        ));
        assert!(!terrain_lod_ready_for_acceptance(
            ready_low,
            TerrainLodPreset::Medium
        ));
        assert!(!terrain_lod_ready_for_acceptance(
            SceneTerrainViewDiagnostics {
                target_ready: false,
                ..ready_low
            },
            TerrainLodPreset::Low
        ));
        assert!(!terrain_lod_ready_for_acceptance(
            ready_low,
            TerrainLodPreset::Off
        ));
    }
}
