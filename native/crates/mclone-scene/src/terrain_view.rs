use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use mclone_app_runtime::frame_render::{TerrainBackdropRenderContext, TerrainBackdropRenderer};
use mclone_app_runtime::host_mode::SingleViewHostMode;
use mclone_app_runtime::render_asset_data::TexturedMeshAssets;
use mclone_core::{BlockStateId, ChunkPos, HorizontalTopology};
use mclone_render::color_profile::RenderColorProfile;
use mclone_terrain_view::{
    ExactPaintedCoverageSnapshot, TERRAIN_PREVIEW_MATERIAL_UV_COUNT, TerrainClipmapConfig,
    TerrainCompositionSourceIdentity, TerrainExactCoverageMode, TerrainHorizonFrameStats,
    TerrainHorizonPresentation, TerrainHorizonRenderTarget, TerrainPreparedExactFrame,
    TerrainPreviewCamera, TerrainPreviewMaterialAtlas, TerrainPreviewView,
    TerrainVegetationExecutor, TerrainViewEngine, TerrainViewEngineConfig,
    TerrainViewSourceIdentity,
};
use mclone_worldgen::terrain_preview::{TerrainPreviewContentStage, TerrainPreviewProfile};

use crate::{
    GameTerrainPresentation, McloneSceneHost, WorldInstanceId, engine_terrain_presentation,
};

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
    pub source_generation: u64,
    pub coverage_generation: u64,
    pub exact_column_count: u32,
    pub last_frame_revision: u64,
    pub ready_slots: u32,
    pub drawn_levels: u32,
    pub drawn_tiles: u32,
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
        seed: i64,
        topology: HorizontalTopology,
        vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
    ) -> Result<Self> {
        let source = live_source(world, seed, topology)?;
        let coverage_generation = 1;
        let coverage = ExactPaintedCoverageSnapshot::new(
            source.composition_source().map_err(anyhow::Error::msg)?,
            coverage_generation,
            [],
        )
        .map_err(anyhow::Error::msg)?;
        let exact = TerrainPreparedExactFrame::new(source, coverage).map_err(anyhow::Error::msg)?;
        let mut material_uvs = [[0.0_f32, 0.0, 1.0, 1.0]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT];
        for (raw_id, target) in material_uvs.iter_mut().enumerate() {
            if let Some(sprite) = mesh_assets.catalog.gui_icon_uv(BlockStateId(raw_id as u32)) {
                *target = [sprite.u0, sprite.v0, sprite.u1, sprite.v1];
            }
        }
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
                clipmap: scene_terrain_clipmap_config(),
                render_cell_stride: scene_terrain_render_cell_stride(),
                vegetation_enabled,
                color_profile,
            },
            TerrainPreviewMaterialAtlas {
                width: atlas.width,
                height: atlas.height,
                rgba: atlas.rgba,
                material_uvs: &material_uvs,
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
            diagnostics: SceneTerrainViewDiagnostics {
                source_generation: source.generation(),
                coverage_generation,
                ..Default::default()
            },
        })
    }

    pub(crate) fn prepare(
        &mut self,
        world: WorldInstanceId,
        seed: i64,
        topology: HorizontalTopology,
        focus: [f64; 3],
        ready_columns: BTreeSet<ChunkPos>,
    ) -> Result<()> {
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
            self.source = live_source(world, seed, topology)?;
            self.engine
                .replace_source(self.source)
                .map_err(anyhow::Error::msg)?;
            self.ready_columns.clear();
            self.coverage_generation = 1;
        }
        if ready_columns != self.ready_columns {
            self.ready_columns = ready_columns;
            self.coverage_generation = self.coverage_generation.saturating_add(1).max(1);
        }
        let coverage = ExactPaintedCoverageSnapshot::new(
            self.source
                .composition_source()
                .map_err(anyhow::Error::msg)?,
            self.coverage_generation,
            self.ready_columns.iter().copied(),
        )
        .map_err(anyhow::Error::msg)
        .context("pack live exact-painted terrain coverage")?;
        self.exact =
            TerrainPreparedExactFrame::new(self.source, coverage).map_err(anyhow::Error::msg)?;
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
        Ok(())
    }

    pub(crate) const fn diagnostics(&self) -> SceneTerrainViewDiagnostics {
        self.diagnostics
    }

    pub(crate) fn shutdown(&mut self) {
        self.engine.shutdown();
    }
}

pub(crate) fn scene_terrain_projection_far_distance(
    mode: mclone_app_runtime::startup_args::TerrainPresentationMode,
    ordinary_far_distance: f32,
) -> f32 {
    if mode == mclone_app_runtime::startup_args::TerrainPresentationMode::Composed {
        ordinary_far_distance
            .max(scene_terrain_clipmap_config().conservative_view_distance_blocks())
    } else {
        ordinary_far_distance
    }
}

impl McloneSceneHost {
    pub const fn terrain_presentation_preference(&self) -> GameTerrainPresentation {
        self.terrain_presentation_preference
    }

    pub fn terrain_presentation_supported(&self) -> bool {
        self.active_world.scene.world_generation_profile
            == mclone_server::WorldGenerationProfile::McloneOverworldV1
            && self.active_world.runtime.as_ref().map_or_else(
                || self.active_world.scene.startup.remote_addr.is_none(),
                |runtime| runtime.host_mode() == SingleViewHostMode::LocalIntegrated,
            )
    }

    pub(crate) fn effective_terrain_presentation_mode(
        &self,
    ) -> mclone_app_runtime::startup_args::TerrainPresentationMode {
        if self.terrain_presentation_supported() {
            engine_terrain_presentation(self.terrain_presentation_preference)
        } else {
            mclone_app_runtime::startup_args::TerrainPresentationMode::ExactOnly
        }
    }

    pub(crate) fn terrain_projection_far_distance(&self, ordinary_far_distance: f32) -> f32 {
        scene_terrain_projection_far_distance(
            self.effective_terrain_presentation_mode(),
            ordinary_far_distance,
        )
    }

    pub fn request_terrain_presentation(
        &mut self,
        presentation: GameTerrainPresentation,
    ) -> Result<()> {
        if presentation == self.terrain_presentation_preference {
            return Ok(());
        }
        self.terrain_presentation_preference = presentation;
        self.reset_terrain_view();
        self.persist_graphics_preferences();
        log::info!(
            "distant terrain preference set to {}; active={}",
            presentation.label(),
            self.effective_terrain_presentation_mode().label(),
        );
        Ok(())
    }

    pub(crate) fn prepare_terrain_view_for_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        focus: [f64; 3],
    ) -> Result<bool> {
        if self.effective_terrain_presentation_mode()
            != mclone_app_runtime::startup_args::TerrainPresentationMode::Composed
        {
            self.reset_terrain_view();
            return Ok(false);
        }
        let world = self.active_world.id;
        let seed = self.active_world.scene.seed;
        let topology = self
            .active_world
            .runtime
            .as_ref()
            .map_or(self.active_world.scene.world_topology, |runtime| {
                runtime.client().topology()
            });
        let ready_columns = self.active_world.draw.traversal_ready_columns_snapshot();
        if self.terrain_view.is_none() {
            let vegetation_executor = self
                .terrain_vegetation_executor_factory
                .as_ref()
                .map(|factory| factory())
                .transpose()
                .map_err(anyhow::Error::msg)
                .context("construct scene terrain vegetation executor")?;
            self.terrain_view = Some(SceneTerrainViewState::new(
                device,
                queue,
                self.color_format,
                self.render_options.color_profile,
                &self.mesh_assets,
                world,
                seed,
                topology,
                vegetation_executor,
            )?);
        }
        self.terrain_view
            .as_mut()
            .expect("composed terrain view was initialized")
            .prepare(world, seed, topology, focus, ready_columns)?;
        Ok(true)
    }

    pub fn terrain_view_diagnostics(&self) -> Option<SceneTerrainViewDiagnostics> {
        self.terrain_view
            .as_ref()
            .map(SceneTerrainViewState::diagnostics)
    }

    pub(crate) fn reset_terrain_view(&mut self) {
        if let Some(mut terrain_view) = self.terrain_view.take() {
            terrain_view.shutdown();
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
        .with_fog(context.fog);
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
        self.diagnostics.drawn_levels = stats.drawn_levels;
        self.diagnostics.drawn_tiles = stats.drawn_tiles;
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

const fn scene_terrain_render_cell_stride() -> u32 {
    1
}

fn scene_terrain_clipmap_config() -> TerrainClipmapConfig {
    TerrainClipmapConfig::default()
}

fn live_source(
    world: WorldInstanceId,
    seed: i64,
    topology: HorizontalTopology,
) -> Result<TerrainViewSourceIdentity> {
    if world.get() == 0 {
        bail!("live terrain-view world generation must be non-zero");
    }
    TerrainViewSourceIdentity::live(
        TerrainCompositionSourceIdentity::new(TerrainPreviewProfile::McloneOverworldV1, seed),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_source_uses_world_identity_as_generation() {
        let source = live_source(
            WorldInstanceId::new(17),
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
    fn only_composed_terrain_extends_the_shared_projection_reach() {
        use mclone_app_runtime::startup_args::TerrainPresentationMode;

        assert_eq!(
            scene_terrain_projection_far_distance(TerrainPresentationMode::ExactOnly, 1_084.0),
            1_084.0
        );
        assert!(
            scene_terrain_projection_far_distance(TerrainPresentationMode::Composed, 1_084.0)
                > 139_000.0
        );
    }
}
