use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::frame_render::FullFrameRenderSummary;
use mclone_client::{ActorPresentationId, ClientHost, ClientRuntime};
use mclone_core::{AIR_BLOCK_STATE_ID, BlockPos, BlockStateId};
use mclone_input::{FlatInputAction, FlatInputFrame, FlatInputIntent};
use mclone_render::headless::{HeadlessFrameLoopOptions, run_headless_capture_loop, save_rgba_png};
use mclone_scene::{MonoUiPresentation, MonoWorldActionStatus};
use mclone_server::initial_spawn_center_for_seed;
use mclone_ui::{GameTravelAssistMode, Point};

use crate::camera::SpectatorCamera;
use crate::cli::{
    HeadlessScreenshotOptions, HeadlessScreenshotUi, SceneOptions, StartupWaitPolicy,
    WarmWorldSwapSmokeOptions,
};
use crate::offscreen_scene_host::OffscreenDriver;
use crate::render_cache::load_asset_source;
use crate::scene_runtime::WindowSceneAssets;

const OFFSCREEN_BLINK_DEBUG_PREVIEW_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OffscreenFlatClientScreenshotReport {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) summary: FullFrameRenderSummary,
    pub(crate) remote_player_count: usize,
    pub(crate) remote_actor_figures: Vec<mclone_assets::ActorFigureId>,
    pub(crate) remote_actor_walk_animation_distances: Vec<f32>,
    pub(crate) entity_count: usize,
    pub(crate) underwater: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OffscreenFlatClientFrameOptions {
    pub(crate) hud: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OffscreenScript {
    steps: Vec<OffscreenScriptStep>,
}

impl OffscreenScript {
    pub(crate) fn from_steps(steps: impl IntoIterator<Item = OffscreenScriptStep>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }

    #[cfg(test)]
    fn steps(&self) -> &[OffscreenScriptStep] {
        &self.steps
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum OffscreenScriptStep {
    SetCameraLookAt {
        eye: Vec3,
        target: Vec3,
    },
    InputFrame {
        frame: FlatInputFrame,
        require_changed_action: Option<FlatInputAction>,
    },
    #[allow(dead_code)]
    UiPointerClick {
        point: Point,
        require_action: Option<mclone_ui::GameUiAction>,
    },
    AdvanceFrame {
        checkpoint: OffscreenScriptCheckpoint,
    },
    SwapWarmWorldStandby,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OffscreenScriptCheckpoint {
    SourceBefore,
    DestinationFirst,
    DestinationSteady,
    SourceReturn,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OffscreenScriptReport {
    pub(crate) input_frame_count: usize,
    pub(crate) world_action_count: usize,
    pub(crate) ui_pointer_click_count: usize,
    pub(crate) ui_action_count: usize,
    pub(crate) advance_frame_count: usize,
    pub(crate) warm_world_swap_count: usize,
}

struct OffscreenScriptRunner {
    script: OffscreenScript,
    cursor: usize,
    report: OffscreenScriptReport,
    switch_reports: Vec<mclone_scene::WarmWorldSwitchReport>,
}

impl OffscreenScriptRunner {
    fn new(script: OffscreenScript) -> Self {
        Self {
            script,
            cursor: 0,
            report: OffscreenScriptReport::default(),
            switch_reports: Vec::new(),
        }
    }

    fn prepare_next_frame(
        &mut self,
        host: &mut OffscreenFlatClientHost,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Option<OffscreenScriptCheckpoint>> {
        while let Some(step) = self.script.steps.get(self.cursor).copied() {
            self.cursor += 1;
            match step {
                OffscreenScriptStep::AdvanceFrame { checkpoint } => {
                    self.report.advance_frame_count += 1;
                    return Ok(Some(checkpoint));
                }
                OffscreenScriptStep::SwapWarmWorldStandby => {
                    self.report.warm_world_swap_count += 1;
                    let report = host.driver.host_mut().apply_warm_world_selection_command(
                        mclone_scene::WarmWorldSelectionCommand::SwapWithStandby,
                    )?;
                    self.switch_reports.push(report);
                }
                immediate => {
                    let step_report =
                        host.run_script(&OffscreenScript::from_steps([immediate]), device, queue)?;
                    self.report.input_frame_count += step_report.input_frame_count;
                    self.report.world_action_count += step_report.world_action_count;
                    self.report.ui_pointer_click_count += step_report.ui_pointer_click_count;
                    self.report.ui_action_count += step_report.ui_action_count;
                }
            }
        }
        Ok(None)
    }

    fn observe_rendered_frame(&mut self, host: &OffscreenFlatClientHost) {
        let Some(pending) = self.switch_reports.last_mut() else {
            return;
        };
        if pending.first_drawable_destination_frame.is_some() {
            return;
        }
        let Some(updated) = host.driver.host().last_warm_world_switch_report() else {
            return;
        };
        if updated.sequence == pending.sequence {
            *pending = updated;
        }
    }

    fn complete(&self) -> bool {
        self.cursor == self.script.steps.len()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WarmWorldSwapSmokeFrameReport {
    pub(crate) checkpoint: OffscreenScriptCheckpoint,
    pub(crate) path: PathBuf,
    pub(crate) instance_id: mclone_scene::WorldInstanceId,
    pub(crate) seed: i64,
    pub(crate) drawn_section_count: usize,
    pub(crate) byte_len: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct WarmWorldSwapSmokeReport {
    pub(crate) directory: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) source_seed: i64,
    pub(crate) destination_seed: i64,
    pub(crate) frames: Vec<WarmWorldSwapSmokeFrameReport>,
    pub(crate) switches: Vec<mclone_scene::WarmWorldSwitchReport>,
    pub(crate) source_destination_difference_ratio: f64,
    pub(crate) destination_steady_difference_ratio: f64,
}

struct WarmWorldSwapSmokeState {
    host: OffscreenFlatClientHost,
    script: OffscreenScriptRunner,
    frames: Vec<(
        OffscreenScriptCheckpoint,
        mclone_scene::WorldInstanceId,
        i64,
        usize,
    )>,
}

pub(crate) struct OffscreenFlatClientHost {
    driver: OffscreenDriver,
    camera: SpectatorCamera,
    asset_replacement_smoke: Option<AssetReplacementSmoke>,
    asset_pack_ui_smoke: Option<AssetPackUiSmoke>,
}

struct AssetPackUiSmoke {
    phase: AssetPackUiSmokePhase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AssetPackUiSmokePhase {
    Start,
    WaitingOriginal,
    WaitingHybrid,
    WaitingFallback,
    WaitingVanilla,
    Complete,
}

struct AssetReplacementSmoke {
    authored: PathBuf,
    fallback: PathBuf,
    restore: Option<mclone_app_runtime::prepared_assets::PreparedSceneAssets>,
    phase: AssetReplacementSmokePhase,
    first_party_frame: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AssetReplacementSmokePhase {
    Baseline,
    FirstParty,
    Restore,
    Complete,
}

impl OffscreenFlatClientHost {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        target_size: [u32; 2],
        scene: &SceneOptions,
        render_options: mclone_render::chunk::TexturedSectionRenderOptions,
        assets: &WindowSceneAssets,
        asset_source: &impl mclone_assets::AssetSource,
        startup_camera: SpectatorCamera,
    ) -> Result<Self> {
        let driver = OffscreenDriver::new(
            device,
            queue,
            format,
            target_size,
            scene,
            render_options,
            assets,
            asset_source,
            Some(&startup_camera),
        )?;
        Ok(Self {
            driver,
            camera: startup_camera,
            asset_replacement_smoke: None,
            asset_pack_ui_smoke: None,
        })
    }

    fn enable_asset_pack_ui_smoke(&mut self) {
        self.driver
            .host_mut()
            .set_mono_ui_screen(Some(mclone_ui::GameScreen::AssetPacks {
                parent: mclone_ui::GameOptionsParent::Pause,
            }));
        self.asset_pack_ui_smoke = Some(AssetPackUiSmoke {
            phase: AssetPackUiSmokePhase::Start,
        });
    }

    fn asset_pack_ui_row_id(&self, pack_id: &str) -> Result<mclone_ui::AssetPackUiId> {
        self.driver
            .host()
            .asset_pack_ui_state()
            .rows
            .iter()
            .flatten()
            .find(|row| row.pack_id.as_str() == pack_id)
            .map(|row| row.ui_id)
            .with_context(|| format!("asset pack UI row {pack_id} is missing"))
    }

    fn advance_asset_pack_ui_smoke(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let Some(phase) = self.asset_pack_ui_smoke.as_ref().map(|smoke| smoke.phase) else {
            return Ok(());
        };
        if let mclone_scene::AssetReplacementStatus::Failed {
            active_epoch,
            message,
        } = self.driver.host().asset_replacement_status()
        {
            bail!(
                "asset pack UI smoke failed with active epoch {active_epoch} retained: {message}"
            );
        }
        let active_epoch = self.driver.host().active_asset_epoch();
        let authored = self.asset_pack_ui_row_id(
            mclone_app_runtime::prepared_assets::AUTHORED_FIRST_PARTY_PACK_ID,
        )?;
        let reference = self.asset_pack_ui_row_id(
            mclone_app_runtime::prepared_assets::MINECRAFT_REFERENCE_PACK_ID,
        )?;
        let actions: Option<(AssetPackUiSmokePhase, Vec<mclone_ui::GameUiAction>)> = match phase {
            AssetPackUiSmokePhase::Start if active_epoch == 0 => Some((
                AssetPackUiSmokePhase::WaitingOriginal,
                vec![
                    mclone_ui::GameUiAction::ToggleAssetPack(authored),
                    mclone_ui::GameUiAction::ToggleAssetPack(reference),
                    mclone_ui::GameUiAction::ApplyAssetPacks,
                ],
            )),
            AssetPackUiSmokePhase::WaitingOriginal if active_epoch == 1 => Some((
                AssetPackUiSmokePhase::WaitingHybrid,
                vec![
                    mclone_ui::GameUiAction::ToggleAssetPack(reference),
                    mclone_ui::GameUiAction::ApplyAssetPacks,
                ],
            )),
            AssetPackUiSmokePhase::WaitingHybrid if active_epoch == 2 => Some((
                AssetPackUiSmokePhase::WaitingFallback,
                vec![
                    mclone_ui::GameUiAction::ToggleAssetPack(authored),
                    mclone_ui::GameUiAction::ToggleAssetPack(reference),
                    mclone_ui::GameUiAction::ApplyAssetPacks,
                ],
            )),
            AssetPackUiSmokePhase::WaitingFallback if active_epoch == 3 => Some((
                AssetPackUiSmokePhase::WaitingVanilla,
                vec![
                    mclone_ui::GameUiAction::ToggleAssetPack(reference),
                    mclone_ui::GameUiAction::ApplyAssetPacks,
                ],
            )),
            AssetPackUiSmokePhase::WaitingVanilla if active_epoch == 4 => Some((
                AssetPackUiSmokePhase::Complete,
                vec![mclone_ui::GameUiAction::ToggleAssetPack(authored)],
            )),
            _ => None,
        };
        if let Some((next, actions)) = actions {
            for action in actions {
                self.driver.apply_ui_action(action, device, queue)?;
            }
            self.asset_pack_ui_smoke
                .as_mut()
                .expect("asset pack UI smoke remains configured")
                .phase = next;
        }
        Ok(())
    }

    fn enable_asset_replacement_smoke(&mut self, authored: PathBuf, fallback: PathBuf) {
        self.asset_replacement_smoke = Some(AssetReplacementSmoke {
            authored,
            fallback,
            restore: None,
            phase: AssetReplacementSmokePhase::Baseline,
            first_party_frame: None,
        });
    }

    fn advance_asset_replacement_smoke(&mut self, frame_index: usize) -> Result<()> {
        let Some(smoke) = self.asset_replacement_smoke.as_mut() else {
            return Ok(());
        };
        match self.driver.host().asset_replacement_status() {
            mclone_scene::AssetReplacementStatus::Failed {
                active_epoch,
                message,
            } => {
                bail!(
                    "asset replacement smoke failed with active epoch {active_epoch} retained: {message}"
                );
            }
            mclone_scene::AssetReplacementStatus::Active { epoch: 1 }
                if smoke.phase == AssetReplacementSmokePhase::FirstParty =>
            {
                smoke.first_party_frame = Some(frame_index);
                let restore = smoke
                    .restore
                    .take()
                    .context("asset replacement smoke lost restore snapshot")?;
                self.driver
                    .host_mut()
                    .begin_prepared_asset_replacement(restore)?;
                smoke.phase = AssetReplacementSmokePhase::Restore;
            }
            mclone_scene::AssetReplacementStatus::Active { epoch: 2 }
                if smoke.phase == AssetReplacementSmokePhase::Restore =>
            {
                smoke.phase = AssetReplacementSmokePhase::Complete;
            }
            _ if smoke.phase == AssetReplacementSmokePhase::Baseline => {
                smoke.restore = Some(self.driver.host().active_asset_snapshot_for_epoch(2));
                let request = mclone_app_runtime::prepared_assets::PreparedSceneAssetsRequest::first_party_from_files(
                    1,
                    smoke.authored.clone(),
                    smoke.fallback.clone(),
                )?;
                self.driver.host_mut().begin_asset_replacement(request)?;
                smoke.phase = AssetReplacementSmokePhase::FirstParty;
            }
            _ => {}
        }
        Ok(())
    }

    fn asset_replacement_smoke_result(&self) -> Option<(bool, usize)> {
        self.asset_replacement_smoke.as_ref().map(|smoke| {
            (
                smoke.phase == AssetReplacementSmokePhase::Complete,
                smoke.first_party_frame.unwrap_or_default(),
            )
        })
    }

    pub(crate) fn start_scene_with_wait_policy(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        startup_wait: StartupWaitPolicy,
    ) -> Result<()> {
        let _ = self.start_scene_with_wait_policy_report(device, queue, startup_wait)?;
        Ok(())
    }

    pub(crate) fn start_scene_with_wait_policy_report(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        startup_wait: StartupWaitPolicy,
    ) -> Result<crate::offscreen_scene_host::OffscreenWarmupReport> {
        let report = self
            .driver
            .drive_to_wait_policy(device, queue, startup_wait)?;
        if self.driver.host().warm_world_standby_snapshot().is_some() {
            let standby = self
                .driver
                .drive_until_warm_world_standby_ready(device, queue)?;
            eprintln!(
                "warm_world_standby id={} seed={} phase={} elapsed_ms={:.3} shell_ms={:.3} multiview_ms={:.3} polls={} loaded_chunks={} seed_sections={} drawable_sections={} seed_bytes={} initial_uploads={}/{} initial_releases={} gpu_advances={}/{} gpu_ms={:.3} gpu_sections={} gpu_indices={} queue={} queue_bytes={} entry_resident={} topology_ready={} worst_advance_ms={:.3} worst_startup_step_ms={:.3} worst_runtime_poll_ms={:.3} worst_gpu_ms={:.3} endpoint_ms={:.3} skipped_no_slack={}",
                standby.instance_id.get(),
                standby.seed,
                standby.phase.label(),
                standby.elapsed_ms,
                standby.renderer_shell_create_ms,
                standby.renderer_multiview_create_ms,
                standby.poll_count,
                standby.loaded_chunks,
                standby.startup_seed_sections,
                standby.startup_seed_drawable_sections,
                standby.startup_seed_owned_bytes,
                standby.initial_upload_applied_lifecycle_items,
                standby.initial_upload_lifecycle_items,
                standby.initial_upload_released_compile_jobs,
                standby.gpu_advance_count,
                standby.gpu_ready_advance_count,
                standby.gpu_warm_ms,
                standby.gpu_section_count,
                standby.gpu_index_count,
                standby.queued_upload_lifecycle_items,
                standby.queued_upload_mesh_owned_bytes,
                standby.readiness.entry_section_gpu_resident
                    && standby.readiness.entry_section_traversal_ready,
                standby.readiness.renderer_topology_ready,
                standby.worst_advance_ms,
                standby.worst_startup_step_ms,
                standby.worst_runtime_poll_ms,
                standby.worst_gpu_advance_ms,
                standby.endpoint_resolution_ms,
                standby.gpu_skipped_no_slack_count,
            );
        }
        if matches!(
            startup_wait,
            StartupWaitPolicy::Playable | StartupWaitPolicy::Idle
        ) && self.driver.host().render_stats().section_count == 0
        {
            bail!("offscreen flat client reached readiness without render sections");
        }
        Ok(report)
    }

    pub(crate) fn drive_until_streamed(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<crate::offscreen_scene_host::OffscreenWarmupReport> {
        self.driver.drive_until_streamed(device, queue)
    }

    pub(crate) fn drive_until_streamed_at_output_size(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<crate::offscreen_scene_host::OffscreenWarmupReport> {
        self.driver
            .drive_until_streamed_at_output_size(device, queue)
    }

    pub(crate) fn far_lod_settle_snapshot(&self) -> Result<mclone_scene::FarLodSettleSnapshot> {
        self.driver.far_lod_settle_snapshot()
    }

    pub(crate) fn depth_target(&self) -> &mclone_render::chunk::ChunkDepthTarget {
        self.driver.depth_target()
    }

    pub(crate) fn render_view(
        &self,
        size: [u32; 2],
    ) -> Result<mclone_render::chunk::ChunkRenderView> {
        self.driver.host().mono_render_view(size)
    }

    pub(crate) fn apply_ui_action(
        &mut self,
        action: mclone_ui::GameUiAction,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        self.driver.apply_ui_action(action, device, queue)
    }

    pub(crate) fn far_lod_config(&self) -> mclone_app_runtime::far_lod::FarTerrainLodConfig {
        self.driver.host().scene_options().far_lod
    }

    pub(crate) fn render_distance(&self) -> u32 {
        self.driver.host().current_render_distance()
    }

    pub(crate) fn set_frame_clock(&mut self, frame_ms: f64, target_frame_ms: Option<f64>) {
        self.driver
            .set_clock(crate::offscreen_scene_host::OffscreenFrameClock {
                frame_ms,
                target_frame_ms,
            });
    }

    pub(crate) fn pending_stream_work(&self) -> usize {
        self.driver.host().pending_stream_work(self.camera.position)
    }

    pub(crate) fn latest_budget_decision_panel(
        &self,
    ) -> mclone_diagnostics::BudgetDecisionPanelReport {
        self.driver.latest_budget_decision_panel()
    }

    pub(crate) fn lod_coverage_counters(
        &self,
    ) -> mclone_app_runtime::lod_coverage::LodReplacementCounters {
        self.driver.host().lod_coverage_counters()
    }

    pub(crate) fn far_lod_stats(&self) -> mclone_app_runtime::far_lod::FarTerrainLodProducerStats {
        self.driver.host().far_lod_stats()
    }

    pub(crate) fn render_frame(
        &mut self,
        frame: mclone_render::target::RenderFrameContext<'_>,
        options: OffscreenFlatClientFrameOptions,
    ) -> Result<FullFrameRenderSummary> {
        let summary = if self.asset_replacement_smoke.is_some() {
            self.driver
                .render_frozen(frame, MonoUiPresentation::ScreenSpaceHud, options.hud)?
        } else {
            self.driver
                .render(frame, MonoUiPresentation::ScreenSpaceHud, options.hud)?
        };
        Ok(summary.render)
    }

    pub(crate) fn run_script(
        &mut self,
        script: &OffscreenScript,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<OffscreenScriptReport> {
        let mut report = OffscreenScriptReport::default();
        for step in &script.steps {
            match *step {
                OffscreenScriptStep::SetCameraLookAt { eye, target } => {
                    self.set_camera_look_at(eye, target);
                    self.driver.commit_camera()?;
                }
                OffscreenScriptStep::InputFrame {
                    frame,
                    require_changed_action,
                } => {
                    report.input_frame_count += 1;
                    let statuses = self.driver.apply_input_frame(frame)?;
                    report.world_action_count += statuses.len();
                    if let Some(action) = require_changed_action {
                        require_world_action_changed(
                            &statuses,
                            action,
                            "offscreen script input frame",
                        )?;
                    }
                }
                OffscreenScriptStep::UiPointerClick {
                    point,
                    require_action,
                } => {
                    report.ui_pointer_click_count += 1;
                    let action = self.driver.apply_ui_pointer_click(point, device, queue)?;
                    report.ui_action_count += usize::from(action.is_some());
                    if let Some(required) = require_action
                        && action != Some(required)
                    {
                        bail!(
                            "offscreen script UI pointer click at ({:.1}, {:.1}) emitted {:?}, expected {:?}",
                            point.x,
                            point.y,
                            action,
                            required
                        );
                    }
                }
                OffscreenScriptStep::SwapWarmWorldStandby => {
                    report.warm_world_swap_count += 1;
                    self.driver.host_mut().apply_warm_world_selection_command(
                        mclone_scene::WarmWorldSelectionCommand::SwapWithStandby,
                    )?;
                }
                OffscreenScriptStep::AdvanceFrame { checkpoint } => {
                    bail!(
                        "offscreen script checkpoint {checkpoint:?} requires the frame-advancing runner"
                    );
                }
            }
        }
        Ok(report)
    }

    fn force_day_time(&mut self, day_time: u64) {
        self.driver.host_mut().force_mono_day_time(day_time);
    }

    fn settle_remote_session(&mut self, remote_settle_ms: u64) -> Result<()> {
        if remote_settle_ms == 0
            || !self
                .driver
                .host()
                .mono_client()
                .is_some_and(|client| client.host() == ClientHost::RemoteDedicated)
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(remote_settle_ms));
        self.driver.commit_camera()?;
        Ok(())
    }

    fn apply_scripted_interaction(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let target = self.scripted_interaction_target()?;
        let report = self.run_script(&scripted_interaction_script(target), device, queue)?;
        if report.input_frame_count != 2 || report.world_action_count != 1 {
            bail!(
                "scripted interaction expected 2 input frames and 1 world action, got {} input frames and {} world actions",
                report.input_frame_count,
                report.world_action_count
            );
        }
        Ok(())
    }

    fn scripted_interaction_target(&self) -> Result<ScriptedInteractionTarget> {
        self.driver
            .host()
            .mono_client()
            .context("offscreen scripted interaction requires an active runtime")?;
        let (base_x, base_z) = self.camera.block_column();
        if let Some(target) = clear_scripted_interaction_target(self.driver.host(), base_x, base_z)
        {
            return Ok(target);
        }
        let fallback_x = base_x;
        let fallback_z = base_z + 4;
        let surface_y = self
            .driver
            .host()
            .mono_highest_non_air_block_y_at_world(fallback_x, fallback_z)
            .with_context(|| {
                format!(
                    "no loaded surface for scripted interaction at ({fallback_x}, {fallback_z})"
                )
            })?;
        Ok(ScriptedInteractionTarget {
            x: fallback_x,
            y: surface_y,
            z: fallback_z,
        })
    }

    fn frame_first_actor(&mut self) {
        let Some(client) = self.driver.host().mono_client() else {
            return;
        };
        frame_first_actor(client, &mut self.camera);
        self.driver.set_camera(&self.camera);
    }

    fn set_eye_override(&mut self, eye: [f32; 3]) {
        self.camera.position = Vec3::from_array(eye);
        self.driver.set_camera(&self.camera);
    }

    pub(crate) fn set_camera_look_at(&mut self, eye: Vec3, target: Vec3) {
        aim_spectator_at(&mut self.camera, eye, target);
        self.driver.set_camera(&self.camera);
    }

    pub(crate) fn commit_camera(&mut self) -> Result<bool> {
        self.driver.commit_camera()
    }

    pub(crate) fn place_camera_above_loaded_surface(&mut self) -> Result<()> {
        let (world_x, world_z) = self.camera.block_column();
        let surface_y = self
            .driver
            .host()
            .mono_highest_non_air_block_y_at_world(world_x, world_z)
            .with_context(|| format!("no loaded spawn surface at ({world_x}, {world_z})"))?;
        self.camera.place_above_surface(surface_y);
        self.driver.set_camera(&self.camera);
        self.driver.commit_camera()?;
        Ok(())
    }

    fn frame_warm_world_source_gate_approach(&mut self) -> Result<()> {
        let endpoint = self
            .driver
            .host()
            .warm_world_standby_snapshot()
            .and_then(|snapshot| snapshot.source_endpoint)
            .context("warm-world source endpoint is unavailable for smoke framing")?;
        let feet = endpoint.feet_position.add(endpoint.normal.scale(1.25));
        let eye = Vec3::new(
            feet.x as f32,
            (feet.y + mclone_client::LOCAL_PLAYER_STANDING_EYE_HEIGHT) as f32,
            feet.z as f32,
        );
        let target = Vec3::new(
            endpoint.center.x as f32,
            endpoint.center.y as f32,
            endpoint.center.z as f32,
        );
        self.set_camera_look_at(eye, target);
        self.commit_camera()?;
        Ok(())
    }
}

fn warm_world_swap_script() -> OffscreenScript {
    OffscreenScript::from_steps([
        OffscreenScriptStep::AdvanceFrame {
            checkpoint: OffscreenScriptCheckpoint::SourceBefore,
        },
        OffscreenScriptStep::SwapWarmWorldStandby,
        OffscreenScriptStep::AdvanceFrame {
            checkpoint: OffscreenScriptCheckpoint::DestinationFirst,
        },
        OffscreenScriptStep::AdvanceFrame {
            checkpoint: OffscreenScriptCheckpoint::DestinationSteady,
        },
        OffscreenScriptStep::SwapWarmWorldStandby,
        OffscreenScriptStep::AdvanceFrame {
            checkpoint: OffscreenScriptCheckpoint::SourceReturn,
        },
    ])
}

fn warm_world_checkpoint_label(checkpoint: OffscreenScriptCheckpoint) -> &'static str {
    match checkpoint {
        OffscreenScriptCheckpoint::SourceBefore => "a-before",
        OffscreenScriptCheckpoint::DestinationFirst => "b-first",
        OffscreenScriptCheckpoint::DestinationSteady => "b-steady",
        OffscreenScriptCheckpoint::SourceReturn => "a-return",
    }
}

fn rgba_pixel_difference_ratio(left: &[u8], right: &[u8]) -> Result<f64> {
    if left.len() != right.len() || left.len() % 4 != 0 {
        bail!(
            "cannot compare RGBA captures with lengths {} and {}",
            left.len(),
            right.len()
        );
    }
    let differing = left
        .chunks_exact(4)
        .zip(right.chunks_exact(4))
        .filter(|(left, right)| left[..3] != right[..3])
        .count();
    Ok(differing as f64 / (left.len() / 4).max(1) as f64)
}

fn validate_warm_world_switch_report(report: &mclone_scene::WarmWorldSwitchReport) -> Result<()> {
    // One admitted render-compile request is one vertical chunk column in the
    // current 1.17.1 world shape, hence at most 16 submitted sections.
    const MAX_FIRST_FRAME_COMPILE_SECTIONS: usize = 16;
    if report.switch_uploaded_section_count != 0
        || report.switch_submitted_compile_section_count != 0
        || report.switch_accepted_compile_result_count != 0
        || report.switch_materialized_renderer
    {
        bail!(
            "warm-world switch {} performed forbidden reconstruction/upload work",
            report.sequence
        );
    }
    if report.first_drawable_destination_frame != Some(1) || report.first_drawn_section_count == 0 {
        bail!(
            "warm-world switch {} was not drawable on its next frame: frame={:?} drawn={}",
            report.sequence,
            report.first_drawable_destination_frame,
            report.first_drawn_section_count,
        );
    }
    if report.first_frame_uploaded_section_count > 1
        || report.first_frame_submitted_compile_section_count > MAX_FIRST_FRAME_COMPILE_SECTIONS
        || report.first_frame_accepted_compile_result_count > 1
    {
        bail!(
            "warm-world switch {} caused a first-frame work burst: uploaded={} submitted={} accepted={}",
            report.sequence,
            report.first_frame_uploaded_section_count,
            report.first_frame_submitted_compile_section_count,
            report.first_frame_accepted_compile_result_count,
        );
    }
    Ok(())
}

pub(crate) fn run_offscreen_warm_world_swap_smoke(
    options: &WarmWorldSwapSmokeOptions,
) -> Result<WarmWorldSwapSmokeReport> {
    let destination_seed = options
        .scene
        .warm_world_standby_seed
        .context("warm-world swap smoke requires a standby seed")?;
    std::fs::create_dir_all(&options.directory).with_context(|| {
        format!(
            "create warm-world swap output directory {}",
            options.directory.display()
        )
    })?;

    let assets = WindowSceneAssets::load()?;
    let asset_source = mclone_assets::SharedAssetSource::new(load_asset_source()?);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let startup_camera = screenshot_startup_camera(&scene, StartupWaitPolicy::Idle);
    let (loop_report, frame_pixels, state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: 4,
            pace_frame_duration: None,
        },
        move |device, queue, format, size| {
            let mut host = OffscreenFlatClientHost::new(
                device,
                queue,
                format,
                size,
                &scene,
                render_options,
                &assets,
                &asset_source,
                startup_camera,
            )?;
            host.start_scene_with_wait_policy(device, queue, StartupWaitPolicy::Idle)?;
            host.frame_warm_world_source_gate_approach()?;
            host.drive_until_streamed_at_output_size(device, queue)?;
            Ok(WarmWorldSwapSmokeState {
                host,
                script: OffscreenScriptRunner::new(warm_world_swap_script()),
                frames: Vec::with_capacity(4),
            })
        },
        |index, frame, state| {
            let checkpoint = state
                .script
                .prepare_next_frame(&mut state.host, frame.device, frame.queue)?
                .with_context(|| format!("warm-world script ended before capture frame {index}"))?;
            let summary = state
                .host
                .render_frame(frame, OffscreenFlatClientFrameOptions::default())?;
            state.script.observe_rendered_frame(&state.host);
            state.frames.push((
                checkpoint,
                state.host.driver.host().active_world_instance_id(),
                state.host.driver.host().active_world_seed(),
                summary.drawn_section_count,
            ));
            Ok(())
        },
    )?;

    if !state.script.complete()
        || state.script.report.advance_frame_count != 4
        || state.script.report.warm_world_swap_count != 2
    {
        bail!(
            "warm-world script did not complete its 4-frame/2-switch sequence: {:#?}",
            state.script.report
        );
    }
    let expected_checkpoints = [
        OffscreenScriptCheckpoint::SourceBefore,
        OffscreenScriptCheckpoint::DestinationFirst,
        OffscreenScriptCheckpoint::DestinationSteady,
        OffscreenScriptCheckpoint::SourceReturn,
    ];
    if state.frames.len() != expected_checkpoints.len()
        || !state
            .frames
            .iter()
            .zip(expected_checkpoints)
            .all(|((checkpoint, _, _, _), expected)| *checkpoint == expected)
    {
        bail!("warm-world script captured an unexpected checkpoint sequence");
    }
    if state.script.switch_reports.len() != 2 {
        bail!("warm-world script did not retain both switch reports");
    }
    for report in &state.script.switch_reports {
        validate_warm_world_switch_report(report)?;
    }

    let first = &state.script.switch_reports[0];
    let second = &state.script.switch_reports[1];
    let source_id = state.frames[0].1;
    let destination_id = state.frames[1].1;
    if source_id == destination_id
        || state.frames[2].1 != destination_id
        || state.frames[3].1 != source_id
        || first.source_instance_id != source_id
        || first.destination_instance_id != destination_id
        || second.source_instance_id != destination_id
        || second.destination_instance_id != source_id
    {
        bail!("warm-world A-to-B-to-A instance identity was not conserved");
    }
    if state.frames[0].2 != options.scene.seed
        || state.frames[1].2 != destination_seed
        || state.frames[2].2 != destination_seed
        || state.frames[3].2 != options.scene.seed
    {
        bail!("warm-world A-to-B-to-A seed selection was not conserved");
    }
    if second.source_runtime_command_count_before
        < first.destination_runtime_command_count_after_first_frame
        || second.source_runtime_update_count_before
            < first.destination_runtime_update_count_after_first_frame
        || second.destination_runtime_command_count_before
            < first.source_runtime_command_count_before
        || second.destination_runtime_update_count_before < first.source_runtime_update_count_before
    {
        bail!("warm-world runtime counters regressed across the round trip");
    }

    let source_destination_difference_ratio =
        rgba_pixel_difference_ratio(&frame_pixels[0], &frame_pixels[1])?;
    let destination_steady_difference_ratio =
        rgba_pixel_difference_ratio(&frame_pixels[1], &frame_pixels[2])?;
    if source_destination_difference_ratio < 0.02 {
        bail!(
            "warm-world destination is not visually distinct from source: {:.3}% differing pixels",
            source_destination_difference_ratio * 100.0
        );
    }

    let mut frames = Vec::with_capacity(4);
    for ((checkpoint, instance_id, seed, drawn_section_count), pixels) in
        state.frames.iter().copied().zip(&frame_pixels)
    {
        if drawn_section_count == 0 {
            bail!("warm-world checkpoint {checkpoint:?} rendered a blank terrain frame");
        }
        let path = options
            .directory
            .join(format!("{}.png", warm_world_checkpoint_label(checkpoint)));
        save_rgba_png(&path, loop_report.width, loop_report.height, pixels)?;
        frames.push(WarmWorldSwapSmokeFrameReport {
            checkpoint,
            path,
            instance_id,
            seed,
            drawn_section_count,
            byte_len: pixels.len(),
        });
    }

    let switch_json = |report: &mclone_scene::WarmWorldSwitchReport| {
        serde_json::json!({
            "sequence": report.sequence,
            "source_instance_id": report.source_instance_id.get(),
            "source_seed": report.source_seed,
            "destination_instance_id": report.destination_instance_id.get(),
            "destination_seed": report.destination_seed,
            "command_after_source_frame": report.command_after_source_frame,
            "switch_elapsed_ms": report.switch_elapsed_ms,
            "camera_commit_ms": report.camera_commit_ms,
            "source_camera_commit_changed": report.source_camera_commit_changed,
            "destination_camera_commit_changed": report.destination_camera_commit_changed,
            "source_camera_position_changed": report.source_camera_position_changed,
            "destination_camera_position_changed": report.destination_camera_position_changed,
            "source_cadence_changed": report.source_cadence_changed,
            "destination_cadence_changed": report.destination_cadence_changed,
            "source_queue_lifecycle_items_before": report.source_queue_lifecycle_items_before,
            "source_queue_mesh_owned_bytes_before": report.source_queue_mesh_owned_bytes_before,
            "destination_queue_lifecycle_items_before": report.destination_queue_lifecycle_items_before,
            "destination_queue_mesh_owned_bytes_before": report.destination_queue_mesh_owned_bytes_before,
            "source_pending_compile_jobs_before": report.source_pending_compile_jobs_before,
            "destination_pending_compile_jobs_before": report.destination_pending_compile_jobs_before,
            "switch_uploaded_section_count": report.switch_uploaded_section_count,
            "switch_submitted_compile_section_count": report.switch_submitted_compile_section_count,
            "switch_accepted_compile_result_count": report.switch_accepted_compile_result_count,
            "switch_materialized_renderer": report.switch_materialized_renderer,
            "first_drawable_destination_frame": report.first_drawable_destination_frame,
            "first_drawn_section_count": report.first_drawn_section_count,
            "first_frame_uploaded_section_count": report.first_frame_uploaded_section_count,
            "first_frame_submitted_compile_section_count": report.first_frame_submitted_compile_section_count,
            "first_frame_accepted_compile_result_count": report.first_frame_accepted_compile_result_count,
            "first_frame_queue_lifecycle_items": report.first_frame_queue_lifecycle_items,
            "first_frame_queue_mesh_owned_bytes": report.first_frame_queue_mesh_owned_bytes,
            "first_frame_pending_compile_jobs_after": report.first_frame_pending_compile_jobs_after,
            "source_runtime_command_count_before": report.source_runtime_command_count_before,
            "source_runtime_update_count_before": report.source_runtime_update_count_before,
            "destination_runtime_command_count_before": report.destination_runtime_command_count_before,
            "destination_runtime_update_count_before": report.destination_runtime_update_count_before,
            "destination_runtime_command_count_after_first_frame": report.destination_runtime_command_count_after_first_frame,
            "destination_runtime_update_count_after_first_frame": report.destination_runtime_update_count_after_first_frame,
        })
    };
    let report_path = options.directory.join("report.json");
    std::fs::write(
        &report_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": 1,
            "width": loop_report.width,
            "height": loop_report.height,
            "source_seed": options.scene.seed,
            "destination_seed": destination_seed,
            "source_destination_difference_ratio": source_destination_difference_ratio,
            "destination_steady_difference_ratio": destination_steady_difference_ratio,
            "loop_timing_ms": {
                "setup": loop_report.setup_ms,
                "total": loop_report.total_frame_ms,
                "average": loop_report.average_frame_ms,
                "min": loop_report.min_frame_ms,
                "max": loop_report.max_frame_ms,
            },
            "frames": frames.iter().map(|frame| serde_json::json!({
                "checkpoint": warm_world_checkpoint_label(frame.checkpoint),
                "path": frame.path,
                "instance_id": frame.instance_id.get(),
                "seed": frame.seed,
                "drawn_section_count": frame.drawn_section_count,
                "byte_len": frame.byte_len,
            })).collect::<Vec<_>>(),
            "switches": state.script.switch_reports.iter().map(switch_json).collect::<Vec<_>>(),
        }))?,
    )
    .with_context(|| format!("write warm-world swap report {}", report_path.display()))?;

    Ok(WarmWorldSwapSmokeReport {
        directory: options.directory.clone(),
        width: loop_report.width,
        height: loop_report.height,
        source_seed: options.scene.seed,
        destination_seed,
        frames,
        switches: state.script.switch_reports,
        source_destination_difference_ratio,
        destination_steady_difference_ratio,
    })
}

pub(crate) fn run_offscreen_flat_client_screenshot(
    options: &HeadlessScreenshotOptions,
) -> Result<OffscreenFlatClientScreenshotReport> {
    let asset_replacement_smoke = asset_replacement_smoke_paths()?;
    let asset_pack_ui_sources = asset_pack_ui_source_paths()?;
    let asset_pack_ui_smoke = asset_pack_ui_smoke_enabled();
    let asset_pack_preference_smoke = asset_pack_preference_smoke_enabled();
    let assets = WindowSceneAssets::load()?;
    let asset_source = mclone_assets::SharedAssetSource::new(load_asset_source()?);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let startup_wait = options.startup_wait;
    let startup_camera = screenshot_startup_camera(&scene, startup_wait);
    let mut frame_count = if options.frame_pipeline_overlay {
        startup_wait.offscreen_capture_frame_count().max(2)
    } else {
        startup_wait.offscreen_capture_frame_count()
    };
    if asset_replacement_smoke.is_some() {
        frame_count = frame_count.max(180);
    }
    if asset_pack_ui_smoke {
        frame_count = frame_count.max(480);
    }
    if asset_pack_preference_smoke {
        frame_count = frame_count.max(240);
    }
    let (loop_report, frame_pixels, mut host) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count,
            pace_frame_duration: None,
        },
        move |device, queue, format, size| {
            let mut host = OffscreenFlatClientHost::new(
                device,
                queue,
                format,
                size,
                &scene,
                render_options,
                &assets,
                &asset_source,
                startup_camera,
            )?;
            let mut asset_packs_configured = false;
            if let Some(registry) = mclone_app_runtime::prepared_assets::AssetPackSourceRegistry::discover_native_with_reference(asset_source.clone())? {
                host.driver.host_mut().configure_asset_pack_sources(
                    registry,
                    mclone_app_runtime::prepared_assets::reference_asset_pack_selection(),
                )?;
                asset_packs_configured = true;
            }
            if let Some((authored, fallback)) = &asset_pack_ui_sources {
                let registry = mclone_app_runtime::prepared_assets::AssetPackSourceRegistry::from_files_with_reference(
                    authored,
                    fallback,
                    asset_source.clone(),
                )?;
                host.driver.host_mut().configure_asset_pack_sources(
                    registry,
                    mclone_app_runtime::prepared_assets::reference_asset_pack_selection(),
                )?;
                asset_packs_configured = true;
            }
            if asset_packs_configured {
                if let Some(path) =
                    mclone_app_runtime::asset_pack_preferences::native_asset_pack_preference_path(
                        scene.world_root.as_deref(),
                    )
                {
                    let storage = mclone_app_runtime::asset_pack_preferences::FileAssetPackPreferenceStorage::new(path);
                    if asset_pack_preference_smoke {
                        mclone_app_runtime::asset_pack_preferences::AssetPackPreferenceStorage::store(
                            &storage,
                            &mclone_app_runtime::asset_pack_preferences::AssetPackPreference::new([
                                mclone_assets::AssetPackId::new(
                                    mclone_app_runtime::prepared_assets::AUTHORED_FIRST_PARTY_PACK_ID,
                                ),
                            ]),
                        )?;
                    }
                    host.driver
                        .host_mut()
                        .configure_asset_pack_preference_storage(Box::new(storage))?;
                }
            }
            host.start_scene_with_wait_policy(device, queue, startup_wait)?;
            configure_screenshot_scene(&mut host, options, device, queue)?;
            if let Some((authored, fallback)) = &asset_replacement_smoke {
                host.enable_asset_replacement_smoke(authored.clone(), fallback.clone());
            }
            if asset_pack_ui_smoke {
                host.enable_asset_pack_ui_smoke();
            }
            Ok(host)
        },
        |index, frame, host| {
            let device = frame.device;
            let queue = frame.queue;
            host.render_frame(frame, OffscreenFlatClientFrameOptions { hud: options.hud })?;
            host.advance_asset_replacement_smoke(index)?;
            host.advance_asset_pack_ui_smoke(device, queue)?;
            let diagnostic_needs_pacing = host.asset_replacement_smoke.is_some()
                || (asset_pack_preference_smoke && host.driver.host().active_asset_epoch() == 0)
                || host
                    .asset_pack_ui_smoke
                    .as_ref()
                    .is_some_and(|smoke| smoke.phase != AssetPackUiSmokePhase::Complete);
            if diagnostic_needs_pacing {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(())
        },
    )?;

    if let Some((complete, first_party_frame)) = host.asset_replacement_smoke_result() {
        if !complete {
            bail!(
                "asset replacement smoke did not complete within {frame_count} frames: phase={:?} status={:?}",
                host.asset_replacement_smoke
                    .as_ref()
                    .map(|smoke| smoke.phase),
                host.driver.host().asset_replacement_status()
            );
        }
        let commit = host
            .driver
            .host_mut()
            .take_last_asset_replacement_commit()
            .context("asset replacement smoke committed without a report")?;
        if !commit.session_preserved
            || !commit.camera_preserved
            || !commit.command_count_unchanged
            || !commit.update_count_unchanged
        {
            bail!("asset replacement smoke mutated session/camera/runtime facts");
        }
        let first_party_pixels = frame_pixels
            .get(first_party_frame)
            .context("asset replacement smoke did not retain its first-party frame")?;
        let first_party_path = options.path.with_file_name(format!(
            "{}-first-party.png",
            options
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("mclone-asset-replacement")
        ));
        save_rgba_png(
            &first_party_path,
            loop_report.width,
            loop_report.height,
            first_party_pixels,
        )?;
        let baseline = frame_pixels
            .first()
            .context("asset replacement smoke produced no baseline frame")?;
        let restored = frame_pixels
            .last()
            .context("asset replacement smoke produced no restored frame")?;
        let differing_pixels = baseline
            .chunks_exact(4)
            .zip(restored.chunks_exact(4))
            .filter(|(left, right)| left[..3] != right[..3])
            .count();
        let total_pixels = baseline.len() / 4;
        let difference_ratio = differing_pixels as f64 / total_pixels.max(1) as f64;
        let baseline_path = options.path.with_file_name(format!(
            "{}-baseline.png",
            options
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("mclone-asset-replacement")
        ));
        let restored_path = options.path.with_file_name(format!(
            "{}-restored.png",
            options
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("mclone-asset-replacement")
        ));
        save_rgba_png(
            &baseline_path,
            loop_report.width,
            loop_report.height,
            baseline,
        )?;
        save_rgba_png(
            &restored_path,
            loop_report.width,
            loop_report.height,
            restored,
        )?;
        if difference_ratio > 0.02 {
            bail!(
                "restored vanilla frame differs from baseline in {:.3}% of pixels",
                difference_ratio * 100.0
            );
        }
        println!(
            "asset replacement smoke completed epochs 0 -> 1 -> 2; restored pixel difference {:.3}%; first-party frame {}",
            difference_ratio * 100.0,
            first_party_path.display()
        );
    }

    if let Some(smoke) = &host.asset_pack_ui_smoke {
        if smoke.phase != AssetPackUiSmokePhase::Complete {
            bail!(
                "asset pack UI smoke did not complete within {frame_count} frames: phase={:?} status={:?}",
                smoke.phase,
                host.driver.host().asset_replacement_status()
            );
        }
        let state = host.driver.host().asset_pack_ui_state();
        if host.driver.host().active_asset_epoch() != 4
            || state.effective_label.as_str() != "Hybrid Authoring"
            || !state.dirty
        {
            bail!("asset pack UI smoke finished with an unexpected active/staged selection");
        }
        println!(
            "asset pack UI smoke applied Original, Hybrid, Fallback, and Vanilla at epochs 1..4; final staged label={} rows={}",
            state.effective_label.as_str(),
            state.row_count()
        );
        let diagnostics = host.driver.host().asset_pack_runtime_diagnostics();
        let commit = diagnostics
            .last_commit
            .context("asset pack UI smoke completed without commit diagnostics")?;
        let diagnostics_path =
            std::path::Path::new("/tmp/mclone-asset-pack-runtime-diagnostics.json");
        std::fs::write(
            diagnostics_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "schema": 1,
                "epoch": diagnostics.epoch,
                "active_ids": diagnostics.active_ids,
                "preferred_ids": diagnostics.preferred_ids,
                "proprietary_free": diagnostics.proprietary_free,
                "preference_error": diagnostics.preference_error,
                "provenance": {
                    "first_party": diagnostics.provenance.first_party,
                    "generated": diagnostics.provenance.generated,
                    "minecraft_reference": diagnostics.provenance.minecraft_reference,
                    "unknown": diagnostics.provenance.unknown,
                    "suppressed": diagnostics.provenance.suppressed,
                    "missing": diagnostics.provenance.missing,
                },
                "reload": {
                    "preparation_ms": commit.preparation_ms,
                    "compile_ms": commit.compile_ms,
                    "upload_ms": commit.upload_ms,
                    "total_ms": commit.total_ms,
                    "peak_retained_cpu_bytes": commit.peak_retained_cpu_bytes,
                    "peak_retained_gpu_bytes": commit.peak_retained_gpu_bytes,
                },
            }))?,
        )?;
        println!(
            "asset pack runtime diagnostics wrote {} total_ms={:.3} peak_cpu={} peak_gpu={}",
            diagnostics_path.display(),
            commit.total_ms,
            commit.peak_retained_cpu_bytes,
            commit.peak_retained_gpu_bytes,
        );
    }

    if asset_pack_preference_smoke {
        let diagnostics = host.driver.host().asset_pack_runtime_diagnostics();
        if diagnostics.epoch != 1
            || diagnostics.active_ids
                != [mclone_app_runtime::prepared_assets::AUTHORED_FIRST_PARTY_PACK_ID.to_owned()]
            || diagnostics.preferred_ids != diagnostics.active_ids
            || !diagnostics.proprietary_free
            || diagnostics.preference_error.is_some()
        {
            bail!("asset pack preference smoke did not restore Original: {diagnostics:?}");
        }
        println!(
            "asset pack preference smoke restored {} at epoch {} with no reference/unknown provenance",
            diagnostics.active_ids.join(","),
            diagnostics.epoch,
        );
    }

    let pixels = frame_pixels
        .last()
        .context("offscreen flat client screenshot produced no captured frame")?;
    save_rgba_png(&options.path, loop_report.width, loop_report.height, pixels)?;
    let summary = host
        .driver
        .last_summary()
        .context("offscreen flat client screenshot rendered no frame summary")?
        .render;
    let client = host.driver.host().mono_client();
    let remote_player_count = client.map_or(0, ClientRuntime::remote_player_count);
    let remote_actor_presentations =
        client.map_or_else(Vec::new, ClientRuntime::actor_presentations);
    let remote_actor_figures = remote_actor_presentations
        .iter()
        .filter_map(|actor| {
            matches!(actor.id, ActorPresentationId::RemotePlayer(_))
                .then_some(actor.appearance.figure)
                .flatten()
        })
        .collect();
    let remote_actor_walk_animation_distances = remote_actor_presentations
        .iter()
        .filter_map(|actor| {
            matches!(actor.id, ActorPresentationId::RemotePlayer(_))
                .then_some(actor.walk_animation_distance)
        })
        .collect();
    let entity_count = client.map_or(0, ClientRuntime::entity_count);
    let underwater = host.driver.host().mono_underwater();

    Ok(OffscreenFlatClientScreenshotReport {
        path: options.path.clone(),
        width: loop_report.width,
        height: loop_report.height,
        byte_len: pixels.len(),
        summary,
        remote_player_count,
        remote_actor_figures,
        remote_actor_walk_animation_distances,
        entity_count,
        underwater,
    })
}

fn asset_replacement_smoke_paths() -> Result<Option<(PathBuf, PathBuf)>> {
    let authored = std::env::var_os("MCLONE_ASSET_REPLACEMENT_AUTHORED").map(PathBuf::from);
    let fallback = std::env::var_os("MCLONE_ASSET_REPLACEMENT_FALLBACK").map(PathBuf::from);
    match (authored, fallback) {
        (None, None) => Ok(None),
        (Some(authored), Some(fallback)) => Ok(Some((authored, fallback))),
        _ => bail!(
            "MCLONE_ASSET_REPLACEMENT_AUTHORED and MCLONE_ASSET_REPLACEMENT_FALLBACK must be set together"
        ),
    }
}

fn asset_pack_ui_source_paths() -> Result<Option<(PathBuf, PathBuf)>> {
    let authored = std::env::var_os("MCLONE_ASSET_PACK_UI_AUTHORED").map(PathBuf::from);
    let fallback = std::env::var_os("MCLONE_ASSET_PACK_UI_FALLBACK").map(PathBuf::from);
    match (authored, fallback) {
        (None, None) => Ok(None),
        (Some(authored), Some(fallback)) => Ok(Some((authored, fallback))),
        _ => bail!(
            "MCLONE_ASSET_PACK_UI_AUTHORED and MCLONE_ASSET_PACK_UI_FALLBACK must be set together"
        ),
    }
}

fn asset_pack_ui_smoke_enabled() -> bool {
    std::env::var("MCLONE_ASSET_PACK_UI_SMOKE")
        .is_ok_and(|value| matches!(value.trim(), "1" | "true" | "yes" | "on"))
}

fn asset_pack_preference_smoke_enabled() -> bool {
    std::env::var("MCLONE_ASSET_PACK_PREFERENCE_SMOKE").as_deref() == Ok("1")
}

fn screenshot_startup_camera(
    scene: &SceneOptions,
    startup_wait: StartupWaitPolicy,
) -> SpectatorCamera {
    if scene.remote_addr.is_some() || matches!(startup_wait, StartupWaitPolicy::Idle) {
        return SpectatorCamera::spawn_for_scene(scene);
    }
    let spawn = initial_spawn_center_for_seed(scene.seed);
    let mut startup_scene = scene.clone();
    startup_scene.chunk_x = spawn.x;
    startup_scene.chunk_z = spawn.z;
    SpectatorCamera::spawn_for_scene(&startup_scene)
}

fn configure_screenshot_scene(
    host: &mut OffscreenFlatClientHost,
    options: &HeadlessScreenshotOptions,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<()> {
    if matches!(
        options.ui,
        HeadlessScreenshotUi::NewWorld | HeadlessScreenshotUi::WorldCreate
    ) {
        host.driver
            .host_mut()
            .set_mono_new_world_seed(options.scene.seed);
    }
    host.driver
        .host_mut()
        .set_mono_ui_screen(options.ui.game_screen());
    if let Some(day_time) = options.scene.day_time_override {
        host.force_day_time(day_time);
    }
    host.settle_remote_session(options.remote_settle_ms)?;
    if options.scripted_interaction {
        host.apply_scripted_interaction(device, queue)?;
    } else {
        host.frame_first_actor();
    }
    if let Some(eye) = options.eye {
        host.set_eye_override(eye);
    }
    if let Some(target) = options.target {
        host.set_camera_look_at(host.camera.position, Vec3::from_array(target));
    }
    host.driver
        .host_mut()
        .set_mono_player_collision_box_visible(options.player_collision_box);
    host.driver
        .host_mut()
        .set_mono_frame_pipeline_overlay_visible(options.frame_pipeline_overlay);
    host.driver
        .host_mut()
        .set_mono_debug_diagnostics_visible(options.debug_pane);
    host.driver
        .host_mut()
        .set_mono_camera_view(options.camera_view);
    if options.blink_debug {
        host.driver
            .host_mut()
            .set_mono_travel_assist_mode(GameTravelAssistMode::Blink);
        if !host.driver.host_mut().begin_mono_blink_debug() {
            bail!("offscreen Blink debug preview requires an active runtime");
        }
        let start = std::time::Instant::now();
        while !host.driver.host().mono_blink_preview_ready() {
            host.driver.host_mut().update_mono_blink_debug();
            if start.elapsed() >= OFFSCREEN_BLINK_DEBUG_PREVIEW_TIMEOUT {
                bail!("offscreen Blink debug preview worker timed out");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    Ok(())
}

fn clear_scripted_interaction_target(
    host: &crate::desktop_scene_host::DesktopSceneHost,
    base_x: i32,
    base_z: i32,
) -> Option<ScriptedInteractionTarget> {
    let client = host.mono_client()?;
    const X_OFFSETS: [i32; 7] = [0, -1, 1, -2, 2, -3, 3];
    for dz in 3..=10 {
        for dx in X_OFFSETS {
            let x = base_x + dx;
            let z = base_z + dz;
            let Some(y) = host.mono_highest_non_air_block_y_at_world(x, z) else {
                continue;
            };
            if is_clear_torch_surface(client, x, y, z) {
                return Some(ScriptedInteractionTarget { x, y, z });
            }
        }
    }
    None
}

fn is_clear_torch_surface(client: &ClientRuntime, x: i32, y: i32, z: i32) -> bool {
    let pos = BlockPos::new(x, y, z);
    let above = pos.relative(mclone_core::Direction::Up);
    let headroom = above.relative(mclone_core::Direction::Up);
    let Some(surface) = client.block_state_at_block_pos(pos) else {
        return false;
    };
    is_screenshot_surface_support(surface)
        && client.block_state_at_block_pos(above) == Some(AIR_BLOCK_STATE_ID)
        && client.block_state_at_block_pos(headroom) == Some(AIR_BLOCK_STATE_ID)
}

fn is_screenshot_surface_support(state: BlockStateId) -> bool {
    matches!(
        state.0,
        1 | 4 | 5 | 6 | 7 | 13 | 14 | 15 | 33 | 34 | 38 | 40 | 52 | 53 | 88
    )
}

fn frame_first_actor(client: &ClientRuntime, spectator: &mut SpectatorCamera) {
    let Some(actor) = client.actor_presentations().first().copied() else {
        return;
    };
    let target = Vec3::new(
        actor.feet_position.x as f32,
        actor.feet_position.y as f32 + 1.0,
        actor.feet_position.z as f32,
    );
    let eye = target + Vec3::new(-2.2, 1.4, -4.8);
    aim_spectator_at(spectator, eye, target);
}

fn aim_spectator_at(spectator: &mut SpectatorCamera, eye: Vec3, target: Vec3) {
    let direction = target - eye;
    let Some(direction) = direction.try_normalize() else {
        return;
    };
    spectator.position = eye;
    spectator.yaw = direction.x.atan2(direction.z);
    spectator.pitch = direction.y.clamp(-1.0, 1.0).asin();
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScriptedInteractionTarget {
    x: i32,
    y: i32,
    z: i32,
}

fn action_input_frame(action: FlatInputAction) -> FlatInputFrame {
    let mut frame = FlatInputFrame::default();
    frame.apply_intent(FlatInputIntent::Action {
        action,
        pressed: true,
    });
    frame
}

fn hotbar_input_frame(slot: u8) -> FlatInputFrame {
    let mut frame = FlatInputFrame::default();
    frame.apply_intent(FlatInputIntent::SelectHotbarSlot(slot));
    frame
}

fn scripted_interaction_script(target: ScriptedInteractionTarget) -> OffscreenScript {
    let interaction_eye = Vec3::new(
        target.x as f32 + 0.5,
        target.y as f32 + 3.0,
        target.z as f32 - 1.5,
    );
    let interaction_target = Vec3::new(
        target.x as f32 + 0.5,
        target.y as f32 + 0.5,
        target.z as f32 + 0.5,
    );
    let final_position = Vec3::new(
        target.x as f32 + 0.5,
        target.y as f32 + 6.0,
        target.z as f32 - 2.0,
    );
    let final_target = Vec3::new(
        target.x as f32 + 0.5,
        target.y as f32 + 1.2,
        target.z as f32 + 0.5,
    );
    OffscreenScript::from_steps([
        OffscreenScriptStep::SetCameraLookAt {
            eye: interaction_eye,
            target: interaction_target,
        },
        OffscreenScriptStep::InputFrame {
            frame: hotbar_input_frame(8),
            require_changed_action: None,
        },
        OffscreenScriptStep::InputFrame {
            frame: action_input_frame(FlatInputAction::Use),
            require_changed_action: Some(FlatInputAction::Use),
        },
        OffscreenScriptStep::SetCameraLookAt {
            eye: final_position,
            target: final_target,
        },
    ])
}

fn require_world_action_changed(
    statuses: &[(FlatInputAction, MonoWorldActionStatus)],
    action: FlatInputAction,
    label: &str,
) -> Result<()> {
    let Some((_, status)) = statuses
        .iter()
        .find(|(status_action, _)| *status_action == action)
    else {
        bail!("{label} did not run");
    };
    match status {
        MonoWorldActionStatus::Sent { changed: true, .. } => Ok(()),
        MonoWorldActionStatus::Sent { changed: false, .. } => {
            bail!("{label} sent a command but reported no world change")
        }
        MonoWorldActionStatus::NoRuntime => bail!("{label} had no active runtime"),
        MonoWorldActionStatus::NoTarget => bail!("{label} found no interaction target"),
        MonoWorldActionStatus::NoCommand => bail!("{label} produced no gameplay command"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_interaction_script_has_camera_input_and_final_framing_steps() {
        let script = scripted_interaction_script(ScriptedInteractionTarget { x: 1, y: 64, z: 2 });

        assert_eq!(script.steps().len(), 4);
        assert!(matches!(
            script.steps()[0],
            OffscreenScriptStep::SetCameraLookAt { .. }
        ));
        assert!(matches!(
            script.steps()[1],
            OffscreenScriptStep::InputFrame {
                frame: FlatInputFrame {
                    selected_hotbar_slot: Some(8),
                    ..
                },
                require_changed_action: None,
            }
        ));
        assert!(matches!(
            script.steps()[2],
            OffscreenScriptStep::InputFrame {
                require_changed_action: Some(FlatInputAction::Use),
                ..
            }
        ));
        assert!(matches!(
            script.steps()[3],
            OffscreenScriptStep::SetCameraLookAt { .. }
        ));
    }

    #[test]
    fn offscreen_script_can_store_ui_pointer_click_steps() {
        let point = Point { x: 12.0, y: 34.0 };
        let script = OffscreenScript::from_steps([OffscreenScriptStep::UiPointerClick {
            point,
            require_action: Some(mclone_ui::GameUiAction::ToggleCrosshair),
        }]);

        assert_eq!(script.steps().len(), 1);
        assert!(matches!(
            script.steps()[0],
            OffscreenScriptStep::UiPointerClick {
                point: stored_point,
                require_action: Some(mclone_ui::GameUiAction::ToggleCrosshair),
            } if stored_point == point
        ));
    }

    #[test]
    fn warm_world_swap_script_advances_a_to_b_to_a_at_frame_boundaries() {
        let script = warm_world_swap_script();

        assert_eq!(script.steps().len(), 6);
        assert!(matches!(
            script.steps()[0],
            OffscreenScriptStep::AdvanceFrame {
                checkpoint: OffscreenScriptCheckpoint::SourceBefore,
            }
        ));
        assert_eq!(
            script
                .steps()
                .iter()
                .filter(|step| matches!(step, OffscreenScriptStep::SwapWarmWorldStandby))
                .count(),
            2
        );
        assert!(matches!(
            script.steps()[5],
            OffscreenScriptStep::AdvanceFrame {
                checkpoint: OffscreenScriptCheckpoint::SourceReturn,
            }
        ));
    }
}
