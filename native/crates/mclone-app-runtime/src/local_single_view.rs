use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_client::ClientRuntime;
use mclone_core::ChunkPos;
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh};
use mclone_render_session::RenderSectionCacheUpdate;
use mclone_server::{
    IntegratedServerRunner, NativeIntegratedServerRunner, NativeIntegratedServerRunnerConfig,
    ServerRunnerDiagnostics,
};

use crate::render_assets::{
    RenderSectionCompileWorker, TexturedMeshAssets, load_textured_mesh_assets,
};
use crate::{
    RuntimeUpdateApplyReport, SingleViewRuntime, chunk_tracking_radius_for_render_distance,
    elapsed_ms,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSingleViewSceneOptions {
    pub seed: i64,
    pub center: ChunkPos,
    pub render_distance: u32,
    pub day_time_override: Option<u64>,
    pub freeze_time: bool,
    pub lighting_enabled: bool,
}

impl LocalSingleViewSceneOptions {
    pub const fn new(seed: i64, center: ChunkPos, render_distance: u32) -> Self {
        Self {
            seed,
            center,
            render_distance,
            day_time_override: None,
            freeze_time: false,
            lighting_enabled: true,
        }
    }

    pub const fn with_day_time(mut self, day_time: Option<u64>) -> Self {
        self.day_time_override = day_time;
        self
    }

    pub const fn with_freeze_time(mut self, freeze_time: bool) -> Self {
        self.freeze_time = freeze_time;
        self
    }

    pub const fn with_lighting_enabled(mut self, lighting_enabled: bool) -> Self {
        self.lighting_enabled = lighting_enabled;
        self
    }

    pub fn chunk_tracking_radius(&self) -> u32 {
        chunk_tracking_radius_for_render_distance(self.render_distance)
    }
}

#[derive(Debug)]
pub struct LocalSingleViewSceneRuntime {
    core: SingleViewRuntime,
    server_runner: NativeIntegratedServerRunner,
    mesh_assets: TexturedMeshAssets,
    render_compile_worker: RenderSectionCompileWorker,
}

impl LocalSingleViewSceneRuntime {
    pub fn new(options: LocalSingleViewSceneOptions) -> Result<Self> {
        let mesh_assets = load_textured_mesh_assets()?;
        Self::with_mesh_assets(options, mesh_assets)
    }

    pub fn with_mesh_assets(
        options: LocalSingleViewSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        let render_compile_worker = RenderSectionCompileWorker::new(mesh_assets.catalog.clone())?;
        let server_runner = NativeIntegratedServerRunner::new(native_runner_config(&options))
            .context("failed to start local single-view integrated server runner")?;
        let mut scene = Self {
            core: SingleViewRuntime::local_integrated(
                options.center,
                options.render_distance,
                options.chunk_tracking_radius(),
            ),
            server_runner,
            mesh_assets,
            render_compile_worker,
        };
        if let Some(day_time) = options.day_time_override {
            scene.core.force_day_time(day_time);
        }
        scene.set_chunk_view(
            options.center,
            options.render_distance,
            options.chunk_tracking_radius(),
        )?;
        Ok(scene)
    }

    pub const fn core(&self) -> &SingleViewRuntime {
        &self.core
    }

    pub const fn core_mut(&mut self) -> &mut SingleViewRuntime {
        &mut self.core
    }

    pub const fn client(&self) -> &ClientRuntime {
        self.core.client()
    }

    pub const fn mesh_assets(&self) -> &TexturedMeshAssets {
        &self.mesh_assets
    }

    pub fn render_distance(&self) -> u32 {
        self.core.render_distance()
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.core.client().loaded_chunk_count()
    }

    pub fn set_chunk_view(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Result<bool> {
        let Some(command) =
            self.core
                .set_chunk_view_command(center, render_distance, chunk_tracking_radius)
        else {
            return Ok(false);
        };
        self.server_runner
            .send_command(command)
            .context("failed to send local single-view chunk view command")?;
        let _ = self.drain_runner_updates_report()?;
        Ok(true)
    }

    pub fn poll(&mut self) -> Result<bool> {
        let flush_start = Instant::now();
        let apply_report = self.drain_runner_updates_report()?;
        let changed = apply_report.changed;
        let runner_diagnostics = self
            .server_runner
            .poll_diagnostics()
            .context("failed to poll local single-view integrated server diagnostics")?;
        self.core.finish_poll_diagnostics(
            elapsed_ms(flush_start.elapsed()),
            apply_report,
            Some(&runner_diagnostics),
        );
        Ok(changed)
    }

    pub fn poll_until_idle(&mut self) -> Result<(usize, f64)> {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut polls = 0_usize;
        let mut poll_ms = 0.0_f64;
        loop {
            let poll_start = Instant::now();
            self.poll()?;
            poll_ms += elapsed_ms(poll_start.elapsed());
            polls += 1;
            let diagnostics = self.server_runner_diagnostics()?;
            if runner_idle(&diagnostics) {
                return Ok((polls, poll_ms));
            }
            if Instant::now() >= deadline {
                bail!("timed out waiting for local single-view worldgen jobs");
            }
            if diagnostics.update_queue_depth == 0 {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    pub fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        let render_compile_worker = &mut self.render_compile_worker;
        self.core.sync_render_sections(
            render_compile_worker,
            camera_position,
            |client, _compiler| client.chunk_snapshots().cloned().collect(),
        )
    }

    pub fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut combined = RenderSectionCacheUpdate::default();
        loop {
            let update = self.core.sync_render_sections_with_budget(
                &mut self.render_compile_worker,
                camera_position,
                usize::MAX,
                |client, _compiler| client.chunk_snapshots().cloned().collect(),
            )?;
            let progressed = update.rebuilt_section_count() > 0
                || update.removed_section_count() > 0
                || update.submitted_compile_section_count > 0
                || update.completed_compile_section_count > 0
                || update.stale_compile_section_count > 0;
            combined.merge(update);
            if self.render_compile_worker.pending_job_count() == 0
                && !self.core.has_ready_pending_render_work(camera_position)
            {
                combined.pending_compile_jobs = 0;
                return Ok(combined);
            }
            if Instant::now() >= deadline {
                bail!("timed out waiting for local single-view render section compile queue");
            }
            if !progressed {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    pub fn cached_sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.core.cached_sections()
    }

    pub fn traversal_ready_render_section_keys(
        &self,
        camera_position: Vec3,
    ) -> BTreeSet<RenderSectionKey> {
        self.core
            .traversal_ready_render_section_keys(camera_position)
    }

    pub fn sky_clear_color(&self) -> wgpu::Color {
        mclone_render::sky::overworld_clear_color(self.core.time_of_day())
    }

    pub fn time_of_day(&self) -> f32 {
        self.core.time_of_day()
    }

    pub fn sun_angle(&self) -> f32 {
        self.core.sun_angle()
    }

    fn server_runner_diagnostics(&self) -> Result<ServerRunnerDiagnostics> {
        self.server_runner
            .poll_diagnostics()
            .context("failed to poll local single-view integrated server diagnostics")
    }

    fn drain_runner_updates_report(&mut self) -> Result<RuntimeUpdateApplyReport> {
        let updates = self
            .server_runner
            .drain_updates()
            .context("failed to drain local single-view integrated server updates")?;
        if updates.is_empty() {
            return Ok(RuntimeUpdateApplyReport::default());
        }
        Ok(self.core.apply_server_updates_report(updates))
    }
}

fn native_runner_config(
    options: &LocalSingleViewSceneOptions,
) -> NativeIntegratedServerRunnerConfig {
    NativeIntegratedServerRunnerConfig::new(options.seed)
        .with_lighting_enabled(options.lighting_enabled)
        .with_day_time(options.day_time_override)
        .with_day_time_frozen(options.freeze_time)
}

fn runner_idle(diagnostics: &ServerRunnerDiagnostics) -> bool {
    diagnostics.command_queue_depth == 0
        && diagnostics.update_queue_depth == 0
        && !diagnostics.awaiting_tick
        && diagnostics.pending_jobs == 0
        && diagnostics.pending_publications == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_assets::extracted_asset_root;

    #[test]
    fn local_single_view_options_apply_java_tracking_radius() {
        let options = LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), 2);

        assert_eq!(options.chunk_tracking_radius(), 3);
    }

    #[test]
    fn local_single_view_runtime_loads_center_chunk() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = LocalSingleViewSceneRuntime::new(LocalSingleViewSceneOptions::new(
            12345,
            ChunkPos::new(0, 0),
            0,
        ))
        .unwrap();
        runtime.poll_until_idle().unwrap();

        assert_eq!(runtime.loaded_chunk_count(), 1);
        assert!(
            runtime
                .client()
                .chunk_snapshot(ChunkPos::new(0, 0))
                .is_some()
        );
    }
}
