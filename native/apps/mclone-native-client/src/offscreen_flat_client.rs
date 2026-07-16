use std::path::PathBuf;
#[cfg(unix)]
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::frame_render::FullFrameRenderSummary;
use mclone_client::{ActorPresentationId, ClientHost, ClientRuntime};
use mclone_core::{AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, ChunkPos};
use mclone_input::{FlatInputAction, FlatInputFrame, FlatInputIntent};
use mclone_render::headless::{HeadlessFrameLoopOptions, run_headless_capture_loop, save_rgba_png};
use mclone_scene::{
    EmbeddedWorldPreviewSnapshot, MonoUiPresentation, MonoWorldActionStatus,
    WarmWorldStandbySnapshot,
};
use mclone_server::initial_spawn_center_for_seed;
use mclone_ui::{GameTravelAssistMode, Point};

use crate::camera::SpectatorCamera;
use crate::cli::{
    HeadlessScreenshotOptions, HeadlessScreenshotUi, LobbyScenarioSmokeOptions, SceneOptions,
    StartupWaitPolicy, WarmWorldSwapSmokeOptions,
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
    pub(crate) embedded_preview: Option<EmbeddedWorldPreviewSnapshot>,
    pub(crate) warm_world_standby: Option<WarmWorldStandbySnapshot>,
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
    FaceActiveWorldGate,
    WalkForwardUntilWorldSwitch {
        max_frames: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OffscreenScriptCheckpoint {
    SourceBefore,
    DestinationFirst,
    DestinationGate,
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
    walking: Option<OffscreenScriptWalkState>,
    last_observed_switch_sequence: u64,
}

#[derive(Clone, Copy, Debug)]
struct OffscreenScriptWalkState {
    source_world: mclone_scene::WorldInstanceId,
    frame_count: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OffscreenScriptFrameDirective {
    Advance(Option<OffscreenScriptCheckpoint>),
    Complete,
}

impl OffscreenScriptRunner {
    fn new(script: OffscreenScript) -> Self {
        Self {
            script,
            cursor: 0,
            report: OffscreenScriptReport::default(),
            switch_reports: Vec::new(),
            walking: None,
            last_observed_switch_sequence: 0,
        }
    }

    fn prepare_next_frame(
        &mut self,
        host: &mut OffscreenFlatClientHost,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<OffscreenScriptFrameDirective> {
        while let Some(step) = self.script.steps.get(self.cursor).copied() {
            match step {
                OffscreenScriptStep::AdvanceFrame { checkpoint } => {
                    self.cursor += 1;
                    self.report.advance_frame_count += 1;
                    return Ok(OffscreenScriptFrameDirective::Advance(Some(checkpoint)));
                }
                OffscreenScriptStep::FaceActiveWorldGate => {
                    self.cursor += 1;
                    host.face_active_world_gate()?;
                }
                OffscreenScriptStep::WalkForwardUntilWorldSwitch { max_frames } => {
                    let walking = self.walking.get_or_insert(OffscreenScriptWalkState {
                        source_world: host.driver.host().active_world_instance_id(),
                        frame_count: 0,
                    });
                    if host.driver.host().active_world_instance_id() != walking.source_world {
                        self.cursor += 1;
                        self.walking = None;
                        continue;
                    }
                    if walking.frame_count >= max_frames {
                        let camera = host.driver.host().camera_snapshot();
                        let gate = host.driver.host().world_gate_snapshot();
                        bail!(
                            "offscreen script did not cross the world gate within {max_frames} walking frames: eye={:?} gate={gate:?}",
                            camera.eye,
                        );
                    }
                    walking.frame_count += 1;
                    let step_report = host.run_script(
                        &OffscreenScript::from_steps([OffscreenScriptStep::InputFrame {
                            frame: FlatInputFrame {
                                forward: true,
                                ..FlatInputFrame::default()
                            },
                            require_changed_action: None,
                        }]),
                        device,
                        queue,
                    )?;
                    self.report.input_frame_count += step_report.input_frame_count;
                    self.report.world_action_count += step_report.world_action_count;
                    self.report.advance_frame_count += 1;
                    return Ok(OffscreenScriptFrameDirective::Advance(None));
                }
                immediate => {
                    self.cursor += 1;
                    let step_report =
                        host.run_script(&OffscreenScript::from_steps([immediate]), device, queue)?;
                    self.report.input_frame_count += step_report.input_frame_count;
                    self.report.world_action_count += step_report.world_action_count;
                    self.report.ui_pointer_click_count += step_report.ui_pointer_click_count;
                    self.report.ui_action_count += step_report.ui_action_count;
                }
            }
        }
        Ok(OffscreenScriptFrameDirective::Complete)
    }

    fn observe_rendered_frame(&mut self, host: &OffscreenFlatClientHost) {
        let Some(updated) = host.driver.host().last_warm_world_switch_report() else {
            return;
        };
        if updated.sequence > self.last_observed_switch_sequence {
            self.last_observed_switch_sequence = updated.sequence;
            self.report.warm_world_swap_count += 1;
            self.switch_reports.push(updated);
            return;
        }
        let Some(pending) = self.switch_reports.last_mut() else {
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
    pub(crate) destination_gate_difference_ratio: f64,
    pub(crate) process_cost: Option<WarmWorldProcessCostReport>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WarmWorldProcessIdleSample {
    pub(crate) duration_ms: u64,
    pub(crate) cpu_time_ms: Option<f64>,
    pub(crate) cpu_percent_of_one_core: Option<f64>,
    pub(crate) rss_kb: Option<u64>,
    pub(crate) thread_count: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WarmWorldProcessCostReport {
    pub(crate) one_world: WarmWorldProcessIdleSample,
    pub(crate) two_worlds: WarmWorldProcessIdleSample,
    pub(crate) retained_rss_delta_kb: Option<i64>,
    pub(crate) observed_thread_delta: Option<i64>,
}

struct WarmWorldSwapSmokeState {
    host: OffscreenFlatClientHost,
    script: OffscreenScriptRunner,
    frames: Vec<(
        Option<OffscreenScriptCheckpoint>,
        mclone_scene::WorldInstanceId,
        i64,
        usize,
    )>,
    initial_standby: mclone_scene::WarmWorldStandbySnapshot,
    process_cost: Option<WarmWorldProcessCostReport>,
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
    standby_before: Option<mclone_scene::WarmWorldStandbySnapshot>,
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

    pub(crate) fn scene_host(&self) -> &mclone_scene::McloneSceneHost {
        self.driver.host()
    }

    pub(crate) fn scene_host_mut(&mut self) -> &mut mclone_scene::McloneSceneHost {
        self.driver.host_mut()
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
            standby_before: self.driver.host().warm_world_standby_snapshot(),
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
                "warm_world_standby id={} seed={} phase={} elapsed_ms={:.3} shell_ms={:.3} multiview_ms={:.3} polls={} loaded_chunks={} seed_sections={} drawable_sections={} seed_bytes={} startup_advances={} startup_cpu_ms={:.3} initial_uploads={}/{} initial_releases={} gpu_advances={}/{} gpu_elapsed_ms={:.3} gpu_cpu_ms={:.3} gpu_sections={} gpu_indices={} gpu_bytes={} queue={} queue_bytes={} entry_resident={} topology_ready={} cadence={}/{}/{} cadence_applied={} worst_advance_ms={:.3} worst_startup_step_ms={:.3} worst_runtime_poll_ms={:.3} worst_gpu_ms={:.3} endpoint_ms={:.3} skipped_no_slack={}",
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
                standby.startup_advance_count,
                standby.startup_advance_total_ms,
                standby.initial_upload_applied_lifecycle_items,
                standby.initial_upload_lifecycle_items,
                standby.initial_upload_released_compile_jobs,
                standby.gpu_advance_count,
                standby.gpu_ready_advance_count,
                standby.gpu_warm_ms,
                standby.gpu_advance_total_ms,
                standby.gpu_section_count,
                standby.gpu_index_count,
                standby.estimated_gpu_terrain_bytes,
                standby.queued_upload_lifecycle_items,
                standby.queued_upload_mesh_owned_bytes,
                standby.readiness.entry_section_gpu_resident
                    && standby.readiness.entry_section_traversal_ready,
                standby.readiness.renderer_topology_ready,
                standby.standby_cadence.host_rate_hz,
                standby.standby_cadence.gameplay_rate_hz,
                standby.standby_cadence.physics_rate_hz,
                standby.standby_cadence_applied,
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

    pub(crate) fn drive_until_embedded_preview_idle(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<crate::offscreen_scene_host::OffscreenWarmupReport> {
        self.driver.drive_until_embedded_preview_idle(device, queue)
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

    pub(crate) fn apply_input_frame(
        &mut self,
        frame: mclone_input::FlatInputFrame,
    ) -> Result<Vec<(mclone_input::FlatInputAction, MonoWorldActionStatus)>> {
        self.driver.apply_input_frame(frame)
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
                OffscreenScriptStep::AdvanceFrame { checkpoint } => {
                    bail!(
                        "offscreen script checkpoint {checkpoint:?} requires the frame-advancing runner"
                    );
                }
                OffscreenScriptStep::FaceActiveWorldGate
                | OffscreenScriptStep::WalkForwardUntilWorldSwitch { .. } => {
                    bail!("offscreen world-gate walking requires the frame-advancing runner");
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

    fn face_active_world_gate(&mut self) -> Result<()> {
        let endpoint = self
            .driver
            .host()
            .world_gate_snapshot()
            .map(|snapshot| snapshot.active_endpoint)
            .context("active world gate is unavailable for scripted facing")?;
        let eye = self.driver.host().camera_snapshot().eye;
        self.set_camera_look_at(
            Vec3::new(eye.x as f32, eye.y as f32, eye.z as f32),
            Vec3::new(
                endpoint.center.x as f32,
                endpoint.center.y as f32,
                endpoint.center.z as f32,
            ),
        );
        self.commit_camera()?;
        Ok(())
    }
}

// Walking locomotion advances at the player's collision-resolved pace rather
// than the free-camera speed. Leave enough deterministic frames to clear the
// 1.25-block approach offset on both terrain shapes without turning this into
// an unbounded readiness wait.
const WARM_WORLD_GATE_MAX_WALK_FRAMES: u16 = 32;

fn warm_world_swap_script() -> OffscreenScript {
    OffscreenScript::from_steps([
        OffscreenScriptStep::AdvanceFrame {
            checkpoint: OffscreenScriptCheckpoint::SourceBefore,
        },
        OffscreenScriptStep::WalkForwardUntilWorldSwitch {
            max_frames: WARM_WORLD_GATE_MAX_WALK_FRAMES,
        },
        OffscreenScriptStep::FaceActiveWorldGate,
        OffscreenScriptStep::AdvanceFrame {
            checkpoint: OffscreenScriptCheckpoint::DestinationGate,
        },
        OffscreenScriptStep::WalkForwardUntilWorldSwitch {
            max_frames: WARM_WORLD_GATE_MAX_WALK_FRAMES,
        },
        OffscreenScriptStep::AdvanceFrame {
            checkpoint: OffscreenScriptCheckpoint::SourceReturn,
        },
    ])
}

fn warm_world_checkpoint_label(checkpoint: OffscreenScriptCheckpoint) -> &'static str {
    match checkpoint {
        OffscreenScriptCheckpoint::SourceBefore => "a-gate",
        OffscreenScriptCheckpoint::DestinationFirst => "b-first",
        OffscreenScriptCheckpoint::DestinationGate => "b-gate",
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

fn rgba_exact_pixel_ratio(pixels: &[u8], color: [u8; 4]) -> Result<f64> {
    if pixels.len() % 4 != 0 {
        bail!(
            "cannot inspect non-RGBA capture with {} bytes",
            pixels.len()
        );
    }
    let matches = pixels
        .chunks_exact(4)
        .filter(|pixel| **pixel == color)
        .count();
    Ok(matches as f64 / (pixels.len() / 4).max(1) as f64)
}

fn sample_idle_process(duration_ms: u64) -> WarmWorldProcessIdleSample {
    // Let worker completions and deferred drops triggered by the final
    // readiness frame drain before opening the measured idle interval.
    std::thread::sleep(Duration::from_millis(duration_ms.min(1_000)));
    let cpu_before = process_cpu_time_ms();
    if duration_ms > 0 {
        std::thread::sleep(Duration::from_millis(duration_ms));
    }
    let cpu_after = process_cpu_time_ms();
    let cpu_time_ms = cpu_before
        .zip(cpu_after)
        .map(|(before, after)| (after - before).max(0.0));
    WarmWorldProcessIdleSample {
        duration_ms,
        cpu_time_ms,
        cpu_percent_of_one_core: cpu_time_ms
            .filter(|_| duration_ms > 0)
            .map(|cpu_ms| cpu_ms / duration_ms as f64 * 100.0),
        rss_kb: process_rss_kb(),
        thread_count: process_thread_count(),
    }
}

#[cfg(unix)]
fn process_cpu_time_ms() -> Option<f64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage initializes the caller-owned rusage value for
    // RUSAGE_SELF and does not retain the pointer.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: a successful getrusage call initialized the complete value.
    let usage = unsafe { usage.assume_init() };
    let timeval_ms =
        |time: libc::timeval| time.tv_sec as f64 * 1_000.0 + time.tv_usec as f64 / 1_000.0;
    Some(timeval_ms(usage.ru_utime) + timeval_ms(usage.ru_stime))
}

#[cfg(not(unix))]
fn process_cpu_time_ms() -> Option<f64> {
    None
}

#[cfg(unix)]
fn process_rss_kb() -> Option<u64> {
    let pid = std::process::id().to_string();
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

#[cfg(not(unix))]
fn process_rss_kb() -> Option<u64> {
    None
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn process_thread_count() -> Option<usize> {
    std::fs::read_dir("/proc/self/task")
        .ok()
        .map(|entries| entries.count())
}

#[cfg(target_vendor = "apple")]
fn process_thread_count() -> Option<usize> {
    let pid = std::process::id().to_string();
    let output = Command::new("ps")
        .args(["-M", "-p", &pid])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let rows = String::from_utf8(output.stdout).ok()?.lines().count();
    rows.checked_sub(1)
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
fn process_thread_count() -> Option<usize> {
    None
}

fn process_cost_report(
    one_world: WarmWorldProcessIdleSample,
    two_worlds: WarmWorldProcessIdleSample,
) -> WarmWorldProcessCostReport {
    WarmWorldProcessCostReport {
        one_world,
        two_worlds,
        retained_rss_delta_kb: one_world
            .rss_kb
            .zip(two_worlds.rss_kb)
            .map(|(one, two)| two as i64 - one as i64),
        observed_thread_delta: one_world
            .thread_count
            .zip(two_worlds.thread_count)
            .map(|(one, two)| two as i64 - one as i64),
    }
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
    let cost_sample_ms = options.cost_sample_ms;
    let startup_camera = screenshot_startup_camera(&scene, StartupWaitPolicy::Idle);
    let script_frame_count = usize::from(WARM_WORLD_GATE_MAX_WALK_FRAMES) * 2 + 5;
    let (loop_report, frame_pixels, state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: script_frame_count,
            pace_frame_duration: None,
        },
        move |device, queue, format, size| {
            // Establish the one-world control inside the same process, with
            // the same assets/device/active runtime, before adding the
            // retained slot. This removes process-start and GPU-driver noise
            // from the RSS/thread delta.
            let mut active_scene = scene.clone();
            active_scene.warm_world_standby_seed = None;
            active_scene.warm_world_standby_cadence = None;
            let mut host = OffscreenFlatClientHost::new(
                device,
                queue,
                format,
                size,
                &active_scene,
                render_options,
                &assets,
                &asset_source,
                startup_camera,
            )?;
            if !host
                .scene_host()
                .debug_managed_scenario_auxiliary_player_script_enabled()
            {
                bail!("scene host lost the lobby auxiliary-player validation option");
            }
            host.start_scene_with_wait_policy(device, queue, StartupWaitPolicy::Idle)?;
            let one_world_process =
                (cost_sample_ms > 0).then(|| sample_idle_process(cost_sample_ms));

            let mut request = mclone_scene::WarmWorldStandbyRequest::new(
                destination_seed,
                ChunkPos::new(scene.chunk_x, scene.chunk_z),
            );
            if let Some(cadence) = scene.warm_world_standby_cadence {
                request = request.with_standby_cadence(cadence);
            }
            host.driver
                .host_mut()
                .begin_warm_world_standby(device, queue, request)?;
            host.start_scene_with_wait_policy(device, queue, StartupWaitPolicy::Idle)?;
            let initial_standby = host
                .driver
                .host()
                .warm_world_standby_snapshot()
                .context("warm-world cost probe lost its initial standby snapshot")?;
            let expected_standby_cadence = scene
                .warm_world_standby_cadence
                .unwrap_or(scene.simulation_cadence);
            if initial_standby.standby_cadence != expected_standby_cadence
                || !initial_standby.standby_cadence_applied
            {
                bail!(
                    "warm-world standby cadence was not applied before switchable readiness: expected={expected_standby_cadence:?} snapshot={initial_standby:?}"
                );
            }
            let process_cost = one_world_process.map(|one_world| {
                process_cost_report(one_world, sample_idle_process(cost_sample_ms))
            });
            host.frame_warm_world_source_gate_approach()?;
            host.drive_until_streamed_at_output_size(device, queue)?;
            Ok(WarmWorldSwapSmokeState {
                host,
                script: OffscreenScriptRunner::new(warm_world_swap_script()),
                frames: Vec::with_capacity(script_frame_count),
                initial_standby,
                process_cost,
            })
        },
        |_index, frame, state| {
            let checkpoint =
                match state
                    .script
                    .prepare_next_frame(&mut state.host, frame.device, frame.queue)?
                {
                    OffscreenScriptFrameDirective::Advance(checkpoint) => checkpoint,
                    OffscreenScriptFrameDirective::Complete => None,
                };
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

    if !state.script.complete() || state.script.report.warm_world_swap_count != 2 {
        bail!(
            "warm-world script did not complete its two-crossing sequence: {:#?}",
            state.script.report
        );
    }
    let expected_checkpoints = [
        OffscreenScriptCheckpoint::SourceBefore,
        OffscreenScriptCheckpoint::DestinationGate,
        OffscreenScriptCheckpoint::SourceReturn,
    ];
    let actual_checkpoints = state
        .frames
        .iter()
        .filter_map(|(checkpoint, _, _, _)| *checkpoint)
        .collect::<Vec<_>>();
    if actual_checkpoints != expected_checkpoints {
        bail!("warm-world script captured an unexpected checkpoint sequence");
    }
    if state.script.switch_reports.len() != 2 {
        bail!("warm-world script did not retain both switch reports");
    }
    for report in &state.script.switch_reports {
        validate_warm_world_switch_report(report)?;
    }
    if options
        .scene
        .warm_world_standby_cadence
        .is_some_and(|cadence| cadence != options.scene.simulation_cadence)
        && state
            .script
            .switch_reports
            .iter()
            .any(|report| !report.source_cadence_changed || !report.destination_cadence_changed)
    {
        bail!(
            "warm-world throttled cadence was not applied on demotion and restored on activation"
        );
    }

    let first = &state.script.switch_reports[0];
    let second = &state.script.switch_reports[1];
    let source_gate_index = state
        .frames
        .iter()
        .position(|(checkpoint, _, _, _)| {
            *checkpoint == Some(OffscreenScriptCheckpoint::SourceBefore)
        })
        .context("warm-world gate smoke did not capture the source gate")?;
    let source_id = state.frames[source_gate_index].1;
    let destination_first_index = state
        .frames
        .iter()
        .enumerate()
        .skip(source_gate_index + 1)
        .find_map(|(index, (_, id, _, _))| (*id != source_id).then_some(index))
        .context("warm-world walking never selected the destination")?;
    let destination_id = state.frames[destination_first_index].1;
    let destination_gate_index = state
        .frames
        .iter()
        .position(|(checkpoint, _, _, _)| {
            *checkpoint == Some(OffscreenScriptCheckpoint::DestinationGate)
        })
        .context("warm-world gate smoke did not capture the destination gate")?;
    let source_return_index = state
        .frames
        .iter()
        .enumerate()
        .skip(destination_gate_index + 1)
        .find_map(|(index, (_, id, _, _))| (*id == source_id).then_some(index))
        .context("warm-world walking never returned to the source")?;
    let selected_frames = [
        (OffscreenScriptCheckpoint::SourceBefore, source_gate_index),
        (
            OffscreenScriptCheckpoint::DestinationFirst,
            destination_first_index,
        ),
        (
            OffscreenScriptCheckpoint::DestinationGate,
            destination_gate_index,
        ),
        (OffscreenScriptCheckpoint::SourceReturn, source_return_index),
    ];
    if source_id == destination_id
        || state.frames[destination_gate_index].1 != destination_id
        || state.frames[source_return_index].1 != source_id
        || first.source_instance_id != source_id
        || first.destination_instance_id != destination_id
        || second.source_instance_id != destination_id
        || second.destination_instance_id != source_id
    {
        bail!("warm-world A-to-B-to-A instance identity was not conserved");
    }
    if state.frames[source_gate_index].2 != options.scene.seed
        || state.frames[destination_first_index].2 != destination_seed
        || state.frames[destination_gate_index].2 != destination_seed
        || state.frames[source_return_index].2 != options.scene.seed
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
    let gate = state
        .host
        .driver
        .host()
        .world_gate_snapshot()
        .context("warm-world walking lost its gate model")?;
    if gate.crossing_count != 2
        || gate.active_world_id != source_id
        || gate.destination_world_id != destination_id
        || gate.direction != mclone_scene::WorldGateSwitchDirection::PositiveToNegative
    {
        bail!("warm-world gate did not retain its paired A-to-B return state: {gate:#?}");
    }

    let source_destination_difference_ratio = rgba_pixel_difference_ratio(
        &frame_pixels[source_gate_index],
        &frame_pixels[destination_first_index],
    )?;
    let destination_gate_difference_ratio = rgba_pixel_difference_ratio(
        &frame_pixels[destination_first_index],
        &frame_pixels[destination_gate_index],
    )?;
    // The switchable diagnostic surface is written as exact opaque RGBA8.
    // Require substantial coverage on both endpoints while retaining some
    // surrounding terrain/sky pixels; the rendered captures are still
    // inspected for the concrete nearer-terrain occlusion arrangement.
    const SWITCHABLE_GATE_RGBA: [u8; 4] = [46, 61, 242, 255];
    let source_gate_pixel_ratio =
        rgba_exact_pixel_ratio(&frame_pixels[source_gate_index], SWITCHABLE_GATE_RGBA)?;
    let destination_gate_pixel_ratio =
        rgba_exact_pixel_ratio(&frame_pixels[destination_gate_index], SWITCHABLE_GATE_RGBA)?;
    for (label, ratio) in [
        ("source", source_gate_pixel_ratio),
        ("destination", destination_gate_pixel_ratio),
    ] {
        if !(0.2..0.995).contains(&ratio) {
            bail!(
                "warm-world {label} gate did not retain opaque partial-frame coverage: {:.3}%",
                ratio * 100.0,
            );
        }
    }
    if source_destination_difference_ratio < 0.02 {
        bail!(
            "warm-world destination is not visually distinct from source: {:.3}% differing pixels",
            source_destination_difference_ratio * 100.0
        );
    }

    let mut frames = Vec::with_capacity(4);
    for (checkpoint, index) in selected_frames {
        let (_, instance_id, seed, drawn_section_count) = state.frames[index];
        let pixels = &frame_pixels[index];
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
    let process_sample_json = |sample: WarmWorldProcessIdleSample| {
        serde_json::json!({
            "duration_ms": sample.duration_ms,
            "cpu_time_ms": sample.cpu_time_ms,
            "cpu_percent_of_one_core": sample.cpu_percent_of_one_core,
            "rss_kb": sample.rss_kb,
            "thread_count": sample.thread_count,
        })
    };
    let process_cost_json = state.process_cost.map(|cost| {
        serde_json::json!({
            "one_world": process_sample_json(cost.one_world),
            "two_worlds": process_sample_json(cost.two_worlds),
            "retained_rss_delta_kb": cost.retained_rss_delta_kb,
            "observed_thread_delta": cost.observed_thread_delta,
        })
    });
    let compile_workers = options.scene.render_compile_worker_count;
    let managed_roles_json = |worlds: usize| {
        serde_json::json!({
            "integrated_server": worlds,
            "worldgen": worlds,
            "light_status": worlds,
            "render_compile_dispatch": worlds,
            "render_compile_worker": worlds.saturating_mul(compile_workers),
            "chunk_drop": worlds,
            "total": worlds.saturating_mul(5 + compile_workers),
        })
    };
    let standby = &state.initial_standby;
    let report_path = options.directory.join("report.json");
    std::fs::write(
        &report_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": 3,
            "width": loop_report.width,
            "height": loop_report.height,
            "source_seed": options.scene.seed,
            "destination_seed": destination_seed,
            "source_destination_difference_ratio": source_destination_difference_ratio,
            "destination_gate_difference_ratio": destination_gate_difference_ratio,
            "source_gate_pixel_ratio": source_gate_pixel_ratio,
            "destination_gate_pixel_ratio": destination_gate_pixel_ratio,
            "gate": {
                "crossing_count": gate.crossing_count,
                "availability": gate.availability.label(),
                "direction": format!("{:?}", gate.direction),
                "active_world_id": gate.active_world_id.get(),
                "destination_world_id": gate.destination_world_id.get(),
            },
            "script": {
                "input_frame_count": state.script.report.input_frame_count,
                "advance_frame_count": state.script.report.advance_frame_count,
                "warm_world_swap_count": state.script.report.warm_world_swap_count,
            },
            "standby": {
                "elapsed_ms": standby.elapsed_ms,
                "renderer_shell_create_ms": standby.renderer_shell_create_ms,
                "renderer_multiview_create_ms": standby.renderer_multiview_create_ms,
                "startup_advance_count": standby.startup_advance_count,
                "startup_advance_total_ms": standby.startup_advance_total_ms,
                "worst_startup_advance_ms": standby.worst_advance_ms,
                "startup_poll_ms": standby.poll_ms,
                "gpu_warm_elapsed_ms": standby.gpu_warm_ms,
                "gpu_advance_count": standby.gpu_advance_count,
                "gpu_ready_advance_count": standby.gpu_ready_advance_count,
                "gpu_advance_total_ms": standby.gpu_advance_total_ms,
                "worst_gpu_advance_ms": standby.worst_gpu_advance_ms,
                "startup_seed_owned_bytes": standby.startup_seed_owned_bytes,
                "estimated_gpu_terrain_bytes": standby.estimated_gpu_terrain_bytes,
                "atlas_base_bytes": standby.atlas_base_bytes,
                "duplicated_atlas_base_bytes": standby.duplicated_atlas_base_bytes,
                "shared_terrain_resource_owner_count": standby.shared_terrain_resource_owner_count,
                "actor_state_materialized": standby.actor_state_materialized,
                "shared_actor_resource_owner_count": standby.shared_actor_resource_owner_count,
                "shared_actor_known_retained_bytes": standby.shared_actor_known_retained_bytes,
                "standby_actor_state_allocated_bytes": standby.standby_actor_state_allocated_bytes,
                "gpu_section_count": standby.gpu_section_count,
                "gpu_vertex_count": standby.gpu_vertex_count,
                "gpu_index_count": standby.gpu_index_count,
                "standby_cadence": {
                    "host_hz": standby.standby_cadence.host_rate_hz,
                    "gameplay_hz": standby.standby_cadence.gameplay_rate_hz,
                    "physics_hz": standby.standby_cadence.physics_rate_hz,
                    "applied": standby.standby_cadence_applied,
                },
            },
            "process_cost": process_cost_json,
            "managed_threads_by_role": {
                "one_world": managed_roles_json(1),
                "two_worlds": managed_roles_json(2),
            },
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
        destination_gate_difference_ratio,
        process_cost: state.process_cost,
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
            // A requested diagnostic camera is an override for the whole
            // capture, not only the setup frame. Runtime spawn reconciliation
            // may update the scene camera while warmup frames stream; reapply
            // the explicit eye/target immediately before every rendered frame.
            if let Some(eye) = options.eye {
                host.set_eye_override(eye);
            }
            if let Some(target) = options.target {
                host.set_camera_look_at(host.camera.position, Vec3::from_array(target));
            }
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
        let standby_was_cancelled = if let Some(before) = host
            .asset_replacement_smoke
            .as_ref()
            .and_then(|smoke| smoke.standby_before.as_ref())
        {
            let after = host
                .driver
                .host()
                .warm_world_standby_snapshot()
                .context("asset replacement discarded standby diagnostics")?;
            if after.instance_id != before.instance_id
                || after.asset_epoch != before.asset_epoch
                || after.phase != mclone_scene::WarmWorldStandbyPhase::Cancelled
                || after.readiness.switchable
                || after.failure.as_deref() != Some("asset replacement")
            {
                bail!(
                    "asset replacement did not cancel the old-epoch standby before commit: before={before:?} after={after:?}"
                );
            }
            let gate = host
                .driver
                .host()
                .world_gate_snapshot()
                .context("asset replacement standby lost its gate diagnostics")?;
            if gate.availability != mclone_scene::WorldGateAvailability::Failed {
                bail!("asset replacement left the cancelled standby gate open: {gate:?}");
            }
            true
        } else {
            false
        };
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
        // The ordinary replacement smoke returns to an exact vanilla frame.
        // When it also exercises standby invalidation, the baseline contains
        // the live blue gate and the restored frame intentionally does not.
        if !standby_was_cancelled && difference_ratio > 0.02 {
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
    let embedded_preview = host.driver.host().embedded_world_preview_snapshot();
    let warm_world_standby = host.driver.host().warm_world_standby_snapshot();

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
        embedded_preview,
        warm_world_standby,
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
    if scene.remote_addr.is_some()
        || matches!(startup_wait, StartupWaitPolicy::Idle)
        || scene.world_generation_profile != mclone_server::WorldGenerationProfile::Overworld
    {
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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LobbyScenarioSmokeReport {
    pub(crate) directory: PathBuf,
    pub(crate) capture_count: usize,
    pub(crate) switch_count: u64,
    pub(crate) cancelled_launch_count: u64,
    pub(crate) relaunch_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LobbyScenarioSmokePhase {
    Title,
    TitleAfterCancellation,
    WaitingForLobby,
    LobbyEmptyTable,
    WaitingForPreview,
    WaitingForPreviewMotion,
    ActivatingPreview,
    ActivatingIsland,
    IslandPreview,
    ActivatingReturn,
    ReturnedLobby,
    WaitingForRelaunchTitle,
    WaitingForRelaunchPreview,
    Complete,
}

struct LobbyScenarioSmokeState {
    host: OffscreenFlatClientHost,
    phase: LobbyScenarioSmokePhase,
    captures: Vec<(&'static str, usize)>,
    actor_receipts: Vec<(
        &'static str,
        mclone_scene::EmbeddedWorldPreviewRenderSnapshot,
    )>,
    motion_receipt: Option<mclone_scene::EmbeddedWorldPreviewRenderSnapshot>,
    remote_motion_receipt: Option<mclone_scene::EmbeddedWorldPreviewRenderSnapshot>,
    remote_motion_world: Option<mclone_scene::WorldInstanceId>,
    initial_remote_motion_sequence: u64,
    active_actor_counts_before_motion: Option<(usize, usize)>,
    activation_reports: Vec<mclone_scene::EmbeddedWorldActivationReport>,
    preview_region: Option<mclone_render::placement::EmbeddedChunkRegion>,
    catalog_acceptance: Option<NativeCatalogScenarioAcceptance>,
}

#[derive(Clone, Debug)]
struct NativeCatalogScenarioAcceptance {
    root: PathBuf,
    selected_id: mclone_app_runtime::world_catalog::LocalWorldId,
    selected_seed: i64,
    other_id: mclone_app_runtime::world_catalog::LocalWorldId,
    selected_before_warm: u64,
    other_before_warm: u64,
    selected_during_warm: Option<u64>,
    other_during_warm: Option<u64>,
    selected_after_activation: Option<u64>,
    other_after_activation: Option<u64>,
}

impl NativeCatalogScenarioAcceptance {
    fn current_recency(&self) -> Result<(u64, u64)> {
        let worlds = mclone_app_runtime::world_catalog::NativeWorldCatalog::new(&self.root)
            .list_worlds()
            .map_err(anyhow::Error::msg)?;
        let recency = |id: &mclone_app_runtime::world_catalog::LocalWorldId| {
            worlds
                .iter()
                .find(|world| &world.id == id)
                .and_then(|world| world.last_played_unix_millis)
                .with_context(|| format!("catalog acceptance world `{id}` has no recency"))
        };
        Ok((recency(&self.selected_id)?, recency(&self.other_id)?))
    }

    fn observe_warm_preview(&mut self) -> Result<()> {
        let (selected, other) = self.current_recency()?;
        if selected != self.selected_before_warm || other != self.other_before_warm {
            bail!(
                "catalog preview warmup changed recency: selected {} -> {}, other {} -> {}",
                self.selected_before_warm,
                selected,
                self.other_before_warm,
                other,
            );
        }
        self.selected_during_warm = Some(selected);
        self.other_during_warm = Some(other);
        Ok(())
    }

    fn observe_activation(&mut self) -> Result<()> {
        let (selected, other) = self.current_recency()?;
        if selected <= self.selected_before_warm || other != self.other_before_warm {
            bail!(
                "catalog activation did not update only the selected world's recency: selected {} -> {}, other {} -> {}",
                self.selected_before_warm,
                selected,
                self.other_before_warm,
                other,
            );
        }
        self.selected_after_activation = Some(selected);
        self.other_after_activation = Some(other);
        Ok(())
    }
}

fn prepare_native_catalog_scenario_acceptance(
    root: PathBuf,
) -> Result<NativeCatalogScenarioAcceptance> {
    use mclone_app_runtime::world_catalog::{
        LocalWorldCreateOptions, LocalWorldId, NativeWorldCatalog,
    };

    let catalog = NativeWorldCatalog::new(&root);
    let selected_id = LocalWorldId::new("recent-lobby-world").map_err(anyhow::Error::msg)?;
    let other_id = LocalWorldId::new("older-lobby-world").map_err(anyhow::Error::msg)?;
    let selected_seed = mclone_app_runtime::scenario_content::LOBBY_PREVIEW_FALLBACK_SEED;
    catalog
        .create_world(
            LocalWorldCreateOptions::new("Recent Lobby World", selected_seed)
                .map_err(anyhow::Error::msg)?
                .with_requested_id(selected_id.clone()),
        )
        .map_err(anyhow::Error::msg)?;
    catalog
        .create_world(
            LocalWorldCreateOptions::new("Older Lobby World", 67_890)
                .map_err(anyhow::Error::msg)?
                .with_requested_id(other_id.clone()),
        )
        .map_err(anyhow::Error::msg)?;
    std::thread::sleep(Duration::from_millis(2));
    catalog
        .record_world_played(&selected_id)
        .map_err(anyhow::Error::msg)?;
    let worlds = catalog.list_worlds().map_err(anyhow::Error::msg)?;
    let recency = |id: &LocalWorldId| {
        worlds
            .iter()
            .find(|world| &world.id == id)
            .and_then(|world| world.last_played_unix_millis)
            .with_context(|| format!("prepared catalog world `{id}` has no recency"))
    };
    let selected_before_warm = recency(&selected_id)?;
    let other_before_warm = recency(&other_id)?;
    if selected_before_warm <= other_before_warm {
        bail!("prepared catalog destination is not the most recent world");
    }
    Ok(NativeCatalogScenarioAcceptance {
        root,
        selected_id,
        selected_seed,
        other_id,
        selected_before_warm,
        other_before_warm,
        selected_during_warm: None,
        other_during_warm: None,
        selected_after_activation: None,
        other_after_activation: None,
    })
}

fn begin_lobby_scenario_for_smoke(
    host: &mut OffscreenFlatClientHost,
    options: &LobbyScenarioSmokeOptions,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<()> {
    if options.preview_chunk_span != 2 {
        let bounds = mclone_app_runtime::scenario::ScenarioPreviewBounds::square(
            options.preview_chunk_span,
        )?;
        host.scene_host_mut().begin_managed_scenario_launch(
            mclone_app_runtime::scenario::ScenarioLaunchIntent::lobby_preview()
                .with_preview_bounds(bounds)?,
        )?;
        host.scene_host_mut()
            .set_mono_ui_screen(Some(mclone_ui::GameScreen::PreparingLobby));
        return Ok(());
    }

    let widget = host
        .scene_host_mut()
        .mono_ui_debug_snapshot()
        .context("title frame has no UI debug snapshot")?
        .widgets
        .into_iter()
        .find(|widget| widget.label == "Enter Lobby")
        .context("title frame has no enabled Enter Lobby widget")?;
    host.run_script(
        &OffscreenScript::from_steps([OffscreenScriptStep::UiPointerClick {
            point: Point {
                x: widget.rect.center_x(),
                y: widget.rect.y + widget.rect.height * 0.5,
            },
            require_action: Some(mclone_ui::GameUiAction::EnterScenario(
                mclone_ui::GameScenarioId::LobbyPreview,
            )),
        }]),
        device,
        queue,
    )?;
    Ok(())
}

pub(crate) fn run_lobby_scenario_smoke(
    options: &LobbyScenarioSmokeOptions,
) -> Result<LobbyScenarioSmokeReport> {
    const FRAME_COUNT: usize = 320;
    if !options.scene.debug_auxiliary_player_script {
        bail!("lobby scenario smoke requires the shared auxiliary-player script");
    }
    if options.directory.exists() {
        std::fs::remove_dir_all(&options.directory).with_context(|| {
            format!(
                "clear lobby scenario smoke directory `{}`",
                options.directory.display()
            )
        })?;
    }
    std::fs::create_dir_all(&options.directory).with_context(|| {
        format!(
            "create lobby scenario smoke directory `{}`",
            options.directory.display()
        )
    })?;
    let catalog_acceptance = if options.catalog_destination {
        Some(prepare_native_catalog_scenario_acceptance(
            options
                .scene
                .world_root
                .clone()
                .context("catalog lobby smoke requires a native world root")?,
        )?)
    } else {
        None
    };
    let expected_destination_seed = catalog_acceptance.as_ref().map_or(
        mclone_app_runtime::scenario_content::LOBBY_PREVIEW_FALLBACK_SEED,
        |receipt| receipt.selected_seed,
    );
    let assets = WindowSceneAssets::load()?;
    let asset_source = mclone_assets::SharedAssetSource::new(load_asset_source()?);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let startup_camera = SpectatorCamera::spawn_for_scene(&scene);
    let (_, frame_pixels, state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: FRAME_COUNT,
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
            host.start_scene_with_wait_policy(device, queue, StartupWaitPolicy::Playable)?;
            host.scene_host_mut()
                .set_mono_ui_screen(Some(mclone_ui::GameScreen::Title));
            Ok(LobbyScenarioSmokeState {
                host,
                phase: LobbyScenarioSmokePhase::Title,
                captures: Vec::with_capacity(6),
                actor_receipts: Vec::with_capacity(3),
                motion_receipt: None,
                remote_motion_receipt: None,
                remote_motion_world: None,
                initial_remote_motion_sequence: 0,
                active_actor_counts_before_motion: None,
                activation_reports: Vec::with_capacity(2),
                preview_region: None,
                catalog_acceptance,
            })
        },
        |frame_index, frame, state| {
            let device = frame.device;
            let queue = frame.queue;
            if state.phase != LobbyScenarioSmokePhase::Title
                && state.phase != LobbyScenarioSmokePhase::Complete
            {
                state.host.apply_input_frame(FlatInputFrame::default())?;
            }
            state
                .host
                .render_frame(frame, OffscreenFlatClientFrameOptions { hud: false })?;

            match state.phase {
                LobbyScenarioSmokePhase::Title => {
                    state.captures.push(("title", frame_index));
                    begin_lobby_scenario_for_smoke(&mut state.host, options, device, queue)?;
                    state.host.apply_ui_action(
                        mclone_ui::GameUiAction::BackToTitle,
                        device,
                        queue,
                    )?;
                    state.phase = LobbyScenarioSmokePhase::TitleAfterCancellation;
                }
                LobbyScenarioSmokePhase::TitleAfterCancellation => {
                    if state.host.scene_host().active_world_seed() != options.scene.seed {
                        bail!("cancelled lobby launch replaced the active world");
                    }
                    begin_lobby_scenario_for_smoke(&mut state.host, options, device, queue)?;
                    state.phase = LobbyScenarioSmokePhase::WaitingForLobby;
                }
                LobbyScenarioSmokePhase::WaitingForLobby => {
                    if state.host.scene_host().active_world_seed()
                        == mclone_server::AuthoredWorldFixtureKind::Table.seed()
                        && state.host.scene_host().local_startup_complete()
                    {
                        if state.host.scene_host().active_world_behavior_profile()
                            != mclone_server::WorldBehaviorProfile::ProtectedLobby
                        {
                            bail!("menu-launched lobby did not retain protected authority");
                        }
                        if state
                            .host
                            .scene_host()
                            .embedded_world_preview_snapshot()
                            .is_some_and(|preview| {
                                preview.phase == mclone_scene::EmbeddedWorldPreviewPhase::Visible
                                    || preview.last_drawn_section_count > 0
                            })
                        {
                            bail!("lobby preview drew before the empty-table checkpoint");
                        }
                        let [x, y, z] =
                            mclone_server::AuthoredWorldFixtureKind::Table.preview_anchor();
                        let eye = state.host.scene_host().camera_snapshot().eye;
                        state.host.set_camera_look_at(
                            Vec3::new(eye.x as f32, eye.y as f32, eye.z as f32),
                            Vec3::new(x as f32, y as f32 + 0.25, z as f32),
                        );
                        state.host.commit_camera()?;
                        state.phase = LobbyScenarioSmokePhase::LobbyEmptyTable;
                    }
                }
                LobbyScenarioSmokePhase::LobbyEmptyTable => {
                    if state
                        .host
                        .scene_host()
                        .embedded_world_preview_snapshot()
                        .is_some_and(|preview| preview.last_drawn_section_count > 0)
                    {
                        bail!("lobby preview warmed before the empty-table frame was captured");
                    }
                    state.captures.push(("lobby-before-preview", frame_index));
                    state.phase = LobbyScenarioSmokePhase::WaitingForPreview;
                }
                LobbyScenarioSmokePhase::WaitingForPreview => {
                    if state
                        .host
                        .scene_host()
                        .embedded_world_preview_snapshot()
                        .is_some_and(|preview| {
                            preview.phase == mclone_scene::EmbeddedWorldPreviewPhase::Visible
                                && preview.last_drawn_section_count > 0
                                && preview.render.last_actor_entity_count >= 2
                                && preview.render.last_actor_remote_player_count <= 1
                                && preview.render.last_drawn_actor_count > 0
                                && preview.render.last_actor_source_local_player_count == 0
                                && preview.render.actor_observation_count
                                    == preview.render.last_actor_entity_count
                                && preview.render.remote_player_observation_count
                                    == preview.render.last_actor_remote_player_count
                        })
                    {
                        let preview = state
                            .host
                            .scene_host()
                            .embedded_world_preview_snapshot()
                            .expect("visible preview checked above");
                        if let Some(remote) = preview.render.first_remote_player_observation {
                            if remote.appearance.model
                                != mclone_protocol::PlayerModelKind::UprightBear
                                || remote.source_packed_light == 0
                            {
                                bail!(
                                    "live preview auxiliary player facts are incomplete: {remote:?}"
                                );
                            }
                        }
                        state.initial_remote_motion_sequence =
                            preview.render.remote_player_motion_sequence;
                        let active = state
                            .host
                            .driver
                            .last_summary()
                            .context("live preview initial frame has no render summary")?
                            .render;
                        state.active_actor_counts_before_motion =
                            Some((active.actor_count, active.drawn_actor_count));
                        state
                            .captures
                            .push(("lobby-with-preview-initial", frame_index));
                        state.phase = LobbyScenarioSmokePhase::WaitingForPreviewMotion;
                    }
                }
                LobbyScenarioSmokePhase::WaitingForPreviewMotion => {
                    let initial_frame = state
                        .captures
                        .last()
                        .map(|(_, frame)| *frame)
                        .unwrap_or(frame_index);
                    if frame_index >= initial_frame.saturating_add(4)
                        && state
                            .host
                            .scene_host()
                            .embedded_world_preview_snapshot()
                            .is_some_and(|preview| {
                                preview.phase == mclone_scene::EmbeddedWorldPreviewPhase::Visible
                                    && preview.last_drawn_section_count > 0
                            })
                    {
                        let preview = state
                            .host
                            .scene_host()
                            .embedded_world_preview_snapshot()
                            .expect("settled lobby preview checked above");
                        let active = state
                            .host
                            .driver
                            .last_summary()
                            .context("settled live preview has no render summary")?
                            .render;
                        if state.active_actor_counts_before_motion
                            != Some((active.actor_count, active.drawn_actor_count))
                        {
                            bail!(
                                "destination player motion changed active-world actor counts: before={:?} after={:?}",
                                state.active_actor_counts_before_motion,
                                (active.actor_count, active.drawn_actor_count)
                            );
                        }
                        state
                            .captures
                            .push(("lobby-with-preview-settled", frame_index));
                        state.actor_receipts.push(("lobby", preview.render));
                        state.motion_receipt = Some(preview.render);
                        if preview.region.chunk_width() != options.preview_chunk_span
                            || preview.region.chunk_depth() != options.preview_chunk_span
                        {
                            bail!(
                                "lobby preview resolved {}x{} chunks instead of {}x{}",
                                preview.region.chunk_width(),
                                preview.region.chunk_depth(),
                                options.preview_chunk_span,
                                options.preview_chunk_span,
                            );
                        }
                        state.preview_region = Some(preview.region);
                        if let Some(catalog) = state.catalog_acceptance.as_mut() {
                            catalog.observe_warm_preview()?;
                        }
                        if preview.render.last_actor_remote_player_count != 0 {
                            state.remote_motion_receipt = Some(preview.render);
                            state.remote_motion_world = Some(preview.source_world);
                        }
                        state.phase = LobbyScenarioSmokePhase::ActivatingPreview;
                    }
                }
                LobbyScenarioSmokePhase::ActivatingPreview => {
                    aim_current_eye_at_embedded_preview(&mut state.host)?;
                    crate::live_diorama_smoke::request_flat_embedded_world_activation(
                        &mut state.host,
                    )?;
                    state.phase = LobbyScenarioSmokePhase::ActivatingIsland;
                }
                LobbyScenarioSmokePhase::ActivatingIsland => {
                    let activation = state.host.scene_host().embedded_world_activation_snapshot();
                    if state.host.scene_host().active_world_seed() == expected_destination_seed
                        && activation.phase == mclone_scene::EmbeddedWorldActivationPhase::Idle
                        && activation.last_report.is_some()
                    {
                        if state.host.scene_host().active_world_behavior_profile()
                            != mclone_server::WorldBehaviorProfile::Mutable
                        {
                            bail!("menu-launched destination did not retain mutable authority");
                        }
                        let report = activation
                            .last_report
                            .as_ref()
                            .expect("completed outbound activation has a report");
                        validate_supported_activation_receipt(report)?;
                        state.activation_reports.push(report.clone());
                        if let Some(catalog) = state.catalog_acceptance.as_mut() {
                            catalog.observe_activation()?;
                        }
                        crate::live_diorama_smoke::aim_flat_host_at_embedded_preview(
                            &mut state.host,
                        )?;
                        state.phase = LobbyScenarioSmokePhase::IslandPreview;
                    }
                }
                LobbyScenarioSmokePhase::IslandPreview => {
                    if state
                        .host
                        .scene_host()
                        .embedded_world_preview_snapshot()
                        .is_some_and(|preview| preview.last_drawn_section_count > 0)
                    {
                        let active_client = state
                            .host
                            .scene_host()
                            .mono_client()
                            .context("active destination has no client replica")?;
                        if active_client.entity_count() < 2
                            || active_client.remote_player_count() > 1
                        {
                            bail!(
                                "activation did not preserve one ordinary remote peer beside destination entities"
                            );
                        }
                        state
                            .captures
                            .push(("destination-with-return-preview", frame_index));
                        let preview = state
                            .host
                            .scene_host()
                            .embedded_world_preview_snapshot()
                            .expect("visible destination return preview checked above");
                        state.actor_receipts.push(("destination", preview.render));
                        crate::live_diorama_smoke::request_flat_embedded_world_activation(
                            &mut state.host,
                        )?;
                        state.phase = LobbyScenarioSmokePhase::ActivatingReturn;
                    }
                }
                LobbyScenarioSmokePhase::ActivatingReturn => {
                    let activation = state.host.scene_host().embedded_world_activation_snapshot();
                    if state.host.scene_host().active_world_seed()
                        == mclone_server::AuthoredWorldFixtureKind::Table.seed()
                        && activation.phase == mclone_scene::EmbeddedWorldActivationPhase::Idle
                        && activation
                            .last_report
                            .as_ref()
                            .is_some_and(|report| report.sequence >= 2)
                    {
                        let report = activation
                            .last_report
                            .as_ref()
                            .expect("completed return activation has a report");
                        validate_supported_activation_receipt(report)?;
                        state.activation_reports.push(report.clone());
                        aim_current_eye_at_embedded_preview(&mut state.host)?;
                        state.phase = LobbyScenarioSmokePhase::ReturnedLobby;
                    }
                }
                LobbyScenarioSmokePhase::ReturnedLobby => {
                    if state
                        .host
                        .scene_host()
                        .embedded_world_preview_snapshot()
                        .is_some_and(|preview| {
                            preview.last_drawn_section_count > 0
                                && preview.render.last_actor_entity_count >= 2
                                && preview.render.last_actor_remote_player_count <= 1
                                && preview.render.last_drawn_actor_count > 0
                                && preview.render.last_actor_source_local_player_count == 0
                                && preview.render.actor_observation_count
                                    == preview.render.last_actor_entity_count
                                && preview.render.remote_player_observation_count
                                    == preview.render.last_actor_remote_player_count
                        })
                    {
                        state.captures.push(("returned-lobby", frame_index));
                        let preview = state
                            .host
                            .scene_host()
                            .embedded_world_preview_snapshot()
                            .expect("visible returned lobby preview checked above");
                        let moved = state
                            .motion_receipt
                            .and_then(|receipt| {
                                receipt
                                    .last_actor_motion_to
                                    .or(receipt.first_actor_observation)
                            })
                            .context("returned lobby has no retained actor witness")?;
                        if !preview_actor_observations(preview.render).any(|observation| {
                            observation.entity_id == moved.entity_id
                                && observation.kind == moved.kind
                                && observation.age_ticks >= moved.age_ticks
                        }) {
                            bail!(
                                "A-to-B-to-A return duplicated or reset the moving actor: {:?}",
                                preview.render
                            );
                        }
                        if let Some(moved_remote) = state
                            .remote_motion_receipt
                            .and_then(|receipt| receipt.last_remote_player_motion_to)
                        {
                            let returned_remote = preview
                                .render
                                .first_remote_player_observation
                                .context("returned lobby omitted its retained remote player")?;
                            if returned_remote.player_id != moved_remote.player_id
                                || returned_remote.appearance != moved_remote.appearance
                                || preview.render.last_actor_remote_player_count != 1
                                || preview.render.last_actor_source_local_player_count != 0
                            {
                                bail!(
                                    "A-to-B-to-A duplicated the preview observer as a local-player figure or collided remote players: {:?}",
                                    preview.render
                                );
                            }
                        }
                        state
                            .actor_receipts
                            .push(("returned-lobby", preview.render));
                        state.host.apply_ui_action(
                            mclone_ui::GameUiAction::QuitToTitle,
                            device,
                            queue,
                        )?;
                        state.phase = LobbyScenarioSmokePhase::WaitingForRelaunchTitle;
                    }
                }
                LobbyScenarioSmokePhase::WaitingForRelaunchTitle => {
                    begin_lobby_scenario_for_smoke(&mut state.host, options, device, queue)?;
                    state.phase = LobbyScenarioSmokePhase::WaitingForRelaunchPreview;
                }
                LobbyScenarioSmokePhase::WaitingForRelaunchPreview => {
                    if state.host.scene_host().active_world_seed()
                        == mclone_server::AuthoredWorldFixtureKind::Table.seed()
                        && state.host.scene_host().local_startup_complete()
                        && state
                            .host
                            .scene_host()
                            .warm_world_standby_snapshot()
                            .is_some_and(|standby| standby.seed == expected_destination_seed)
                        && state
                            .host
                            .scene_host()
                            .embedded_world_preview_snapshot()
                            .is_some_and(|preview| {
                                preview.phase == mclone_scene::EmbeddedWorldPreviewPhase::Visible
                                    && preview.last_drawn_section_count > 0
                                    && preview.render.last_actor_entity_count >= 2
                                    && preview.render.last_actor_remote_player_count <= 1
                                    && preview.render.last_actor_source_local_player_count == 0
                                    && preview.render.actor_observation_count
                                        == preview.render.last_actor_entity_count
                                    && preview.render.remote_player_observation_count
                                        == preview.render.last_actor_remote_player_count
                            })
                    {
                        if state.host.scene_host().active_world_behavior_profile()
                            != mclone_server::WorldBehaviorProfile::ProtectedLobby
                        {
                            bail!("reopened lobby did not retain protected authority");
                        }
                        let preview = state
                            .host
                            .scene_host()
                            .embedded_world_preview_snapshot()
                            .expect("reopened preview checked above");
                        let moved = state
                            .motion_receipt
                            .and_then(|receipt| {
                                receipt
                                    .last_actor_motion_to
                                    .or(receipt.first_actor_observation)
                            })
                            .context("reopened lobby has no retained actor witness")?;
                        if !preview_actor_observations(preview.render).any(|observation| {
                            observation.kind == moved.kind
                                && observation.age_ticks >= moved.age_ticks
                        }) {
                            bail!(
                                "reopened authored actor reset instead of loading persisted state: {:?}",
                                preview.render
                            );
                        }
                        state.phase = LobbyScenarioSmokePhase::Complete;
                    }
                }
                LobbyScenarioSmokePhase::Complete => {}
            }
            if state.phase != LobbyScenarioSmokePhase::Complete {
                std::thread::sleep(
                    if state.phase == LobbyScenarioSmokePhase::WaitingForPreviewMotion {
                        Duration::from_millis(50)
                    } else {
                        Duration::from_millis(4)
                    },
                );
            }
            Ok(())
        },
    )?;

    if state.phase != LobbyScenarioSmokePhase::Complete || state.captures.len() != 6 {
        bail!(
            "lobby scenario smoke did not complete in {FRAME_COUNT} frames: phase={:?} captures={:?} standby={:?} preview={:?}",
            state.phase,
            state.captures,
            state.host.scene_host().warm_world_standby_snapshot(),
            state.host.scene_host().embedded_world_preview_snapshot()
        );
    }
    if state.actor_receipts.len() != 3 {
        bail!(
            "lobby scenario smoke expected three composed actor receipts, got {:?}",
            state
                .actor_receipts
                .iter()
                .map(|(label, receipt)| (*label, receipt.last_drawn_actor_count))
                .collect::<Vec<_>>()
        );
    }
    for (label, receipt) in &state.actor_receipts {
        let expected_actor_count = receipt
            .last_actor_entity_count
            .saturating_add(receipt.last_actor_remote_player_count)
            .saturating_add(receipt.last_actor_source_local_player_count);
        if receipt.last_actor_source_local_player_count != 0
            || receipt.last_submitted_actor_count != expected_actor_count
            || receipt.last_drawn_actor_count != expected_actor_count
            || receipt.last_source_rejected_actor_count != 0
            || receipt.last_clip_rejected_actor_count != 0
            || receipt.last_frustum_rejected_actor_count != 0
        {
            bail!("{label} composed actor receipt lost or duplicated an actor: {receipt:?}");
        }
    }
    let initial_frame = state
        .captures
        .iter()
        .find_map(|(label, frame)| (*label == "lobby-with-preview-initial").then_some(*frame))
        .context("lobby smoke has no frozen initial actor frame")?;
    let settled_frame = state
        .captures
        .iter()
        .find_map(|(label, frame)| (*label == "lobby-with-preview-settled").then_some(*frame))
        .context("lobby smoke has no settled preview frame")?;
    let preview_settle_pixel_difference_ratio =
        rgba_pixel_difference_ratio(&frame_pixels[initial_frame], &frame_pixels[settled_frame])?;
    for (label, frame_index) in &state.captures {
        save_rgba_png(
            &options.directory.join(format!("{label}.png")),
            options.width,
            options.height,
            &frame_pixels[*frame_index],
        )?;
    }
    let switch_count = state
        .host
        .scene_host()
        .last_warm_world_switch_report()
        .map_or(0, |report| report.sequence);
    if switch_count != 2 {
        bail!("lobby scenario smoke expected two slot exchanges, got {switch_count}");
    }
    let scenario_cost = state
        .host
        .scene_host()
        .warm_world_standby_snapshot()
        .context("reopened lobby has no retained destination cost snapshot")?;
    if scenario_cost.phase != mclone_scene::WarmWorldStandbyPhase::Switchable {
        bail!("reopened lobby destination is not switchable: {scenario_cost:?}");
    }
    let preview_region = state
        .preview_region
        .context("lobby scenario smoke did not retain its resolved preview region")?;
    if let Some(catalog) = state.catalog_acceptance.as_ref()
        && (catalog.selected_during_warm.is_none()
            || catalog.other_during_warm.is_none()
            || catalog.selected_after_activation.is_none()
            || catalog.other_after_activation.is_none())
    {
        bail!("catalog lobby scenario did not complete every recency checkpoint");
    }
    let receipt = serde_json::json!({
        "schema": 2,
        "captures": state.captures.iter().map(|(label, frame)| {
            serde_json::json!({ "label": label, "frame": frame })
        }).collect::<Vec<_>>(),
        "switchCount": switch_count,
        "cancelledLaunchCount": 1,
        "relaunchCount": 1,
        "managedRoot": options.scene.world_root.as_ref().map(|root| root.parent().unwrap_or(root).join("scenarios")),
        "lobbyBehavior": "protectedLobby",
        "destinationBehavior": "mutable",
        "destinationSource": if options.catalog_destination { "catalog-recent-world" } else { "managed-overworld-fallback" },
        "previewBounds": {
            "minChunk": [preview_region.min_chunk().x, preview_region.min_chunk().z],
            "maxChunk": [preview_region.max_chunk().x, preview_region.max_chunk().z],
            "chunkWidth": preview_region.chunk_width(),
            "chunkDepth": preview_region.chunk_depth(),
            "minSectionY": preview_region.min_section_y(),
            "maxSectionY": preview_region.max_section_y(),
        },
        "catalogRecency": state.catalog_acceptance.as_ref().map(|catalog| serde_json::json!({
            "selectedId": catalog.selected_id.as_str(),
            "selectedSeed": catalog.selected_seed,
            "otherId": catalog.other_id.as_str(),
            "selectedBeforeWarm": catalog.selected_before_warm,
            "otherBeforeWarm": catalog.other_before_warm,
            "selectedDuringWarm": catalog.selected_during_warm,
            "otherDuringWarm": catalog.other_during_warm,
            "selectedAfterActivation": catalog.selected_after_activation,
            "otherAfterActivation": catalog.other_after_activation,
        })),
        "actorReceipts": state.actor_receipts.iter().map(|(label, receipt)| {
            serde_json::json!({
                "label": label,
                "entities": receipt.last_actor_entity_count,
                "remotePlayers": receipt.last_actor_remote_player_count,
                "sourceLocalPlayers": receipt.last_actor_source_local_player_count,
                "submitted": receipt.last_submitted_actor_count,
                "drawn": receipt.last_drawn_actor_count,
                "sourceRejected": receipt.last_source_rejected_actor_count,
                "clipRejected": receipt.last_clip_rejected_actor_count,
                "frustumRejected": receipt.last_frustum_rejected_actor_count,
                "meshRebuilds": receipt.actor_mesh_rebuild_count,
                "meshUploads": receipt.actor_mesh_upload_count,
                "gpuCapacityBytes": receipt.actor_gpu_capacity_bytes,
                "placedPipelines": receipt.placed_actor_pipeline_count,
                "placedMultiviewPipelines": receipt.placed_actor_multiview_pipeline_count,
            })
        }).collect::<Vec<_>>(),
        "actorMotion": state.motion_receipt.and_then(|receipt| {
            receipt.last_actor_motion_from.zip(receipt.last_actor_motion_to).map(|(from, to)| {
                serde_json::json!({
                    "sequence": receipt.actor_motion_sequence,
                    "entityId": to.entity_id.0.to_string(),
                    "kind": format!("{:?}", to.kind),
                    "fromAgeTicks": from.age_ticks.to_string(),
                    "toAgeTicks": to.age_ticks.to_string(),
                    "fromSource": [from.source_feet_position.x, from.source_feet_position.y, from.source_feet_position.z],
                    "toSource": [to.source_feet_position.x, to.source_feet_position.y, to.source_feet_position.z],
                    "fromComposition": [from.composition_feet_position.x, from.composition_feet_position.y, from.composition_feet_position.z],
                    "toComposition": [to.composition_feet_position.x, to.composition_feet_position.y, to.composition_feet_position.z],
                    "sourcePackedLight": to.source_packed_light,
                    "updateToVisibleMs": receipt.last_actor_update_to_visible_ms,
                    "updateToVisibleFrames": receipt.last_actor_update_to_visible_frame_count,
                    "pixelDifferenceRatio": preview_settle_pixel_difference_ratio,
                })
            })
        }),
        "remotePlayerMotion": state.remote_motion_receipt.and_then(|receipt| {
            receipt.last_remote_player_motion_from.zip(receipt.last_remote_player_motion_to).map(|(from, to)| {
                serde_json::json!({
                    "sequence": receipt.remote_player_motion_sequence,
                    "worldInstanceId": state.remote_motion_world.map(|world| world.get().to_string()),
                    "playerId": to.player_id.0.to_string(),
                    "model": format!("{:?}", to.appearance.model),
                    "fromSource": [from.source_feet_position.x, from.source_feet_position.y, from.source_feet_position.z],
                    "toSource": [to.source_feet_position.x, to.source_feet_position.y, to.source_feet_position.z],
                    "fromComposition": [from.composition_feet_position.x, from.composition_feet_position.y, from.composition_feet_position.z],
                    "toComposition": [to.composition_feet_position.x, to.composition_feet_position.y, to.composition_feet_position.z],
                    "fromWalkDistance": from.walk_animation_distance,
                    "toWalkDistance": to.walk_animation_distance,
                    "sourcePackedLight": to.source_packed_light,
                    "updateToVisibleMs": receipt.last_remote_player_update_to_visible_ms,
                    "updateToVisibleFrames": receipt.last_remote_player_update_to_visible_frame_count,
                    "activeActorCountsBeforeAndAfter": state.active_actor_counts_before_motion,
                    "pixelDifferenceRatio": preview_settle_pixel_difference_ratio,
                })
            })
        }),
        "previewSettlePixelDifferenceRatio": preview_settle_pixel_difference_ratio,
        "activationSupport": state.activation_reports.iter().map(|report| {
            let accepted = report.accepted_destination_entry_pose.expect("validated accepted entry");
            let post_swap = report.post_swap_entry.expect("validated post-swap entry");
            let first = report.first_uncovered_entry.expect("validated first-uncovered entry");
            let stability = report.stability_entry.expect("validated stability entry");
            serde_json::json!({
                "sequence": report.sequence,
                "acceptedEntry": [accepted.feet_position.x, accepted.feet_position.y, accepted.feet_position.z],
                "postSwapEntry": [post_swap.pose.feet_position.x, post_swap.pose.feet_position.y, post_swap.pose.feet_position.z],
                "postSwapOnGround": post_swap.on_ground,
                "postSwapSupport": {
                    "bodyLoaded": post_swap.support.body_loaded,
                    "bodyClear": post_swap.support.body_clear,
                    "supportLoaded": post_swap.support.support_loaded,
                    "solidSupport": post_swap.support.solid_support,
                },
                "firstUncoveredFrame": report.first_uncovered_activation_frame,
                "firstUncoveredEntry": [first.pose.feet_position.x, first.pose.feet_position.y, first.pose.feet_position.z],
                "firstUncoveredOnGround": first.on_ground,
                "firstUncoveredSupport": {
                    "bodyLoaded": first.support.body_loaded,
                    "bodyClear": first.support.body_clear,
                    "supportLoaded": first.support.support_loaded,
                    "solidSupport": first.support.solid_support,
                },
                "stabilityFrame": report.stability_activation_frame,
                "stabilityEntry": [stability.pose.feet_position.x, stability.pose.feet_position.y, stability.pose.feet_position.z],
                "stabilityOnGround": stability.on_ground,
                "stabilitySupport": {
                    "bodyLoaded": stability.support.body_loaded,
                    "bodyClear": stability.support.body_clear,
                    "supportLoaded": stability.support.support_loaded,
                    "solidSupport": stability.support.solid_support,
                },
            })
        }).collect::<Vec<_>>(),
        "scenarioCost": {
            "rendererShellCreateMs": scenario_cost.renderer_shell_create_ms,
            "rendererMultiviewCreateMs": scenario_cost.renderer_multiview_create_ms,
            "rendererMultiviewRequired": scenario_cost.renderer_multiview_required,
            "destinationElapsedMs": scenario_cost.elapsed_ms,
            "destinationPollMs": scenario_cost.poll_ms,
            "destinationGpuWarmMs": scenario_cost.gpu_warm_ms,
            "destinationStartupSeedBytes": scenario_cost.startup_seed_owned_bytes,
            "destinationEstimatedGpuTerrainBytes": scenario_cost.estimated_gpu_terrain_bytes,
            "sharedAtlasBaseBytes": scenario_cost.atlas_base_bytes,
            "duplicatedAtlasBaseBytes": scenario_cost.duplicated_atlas_base_bytes,
            "sharedTerrainResourceOwnerCount": scenario_cost.shared_terrain_resource_owner_count,
            "destinationLoadedChunks": scenario_cost.loaded_chunks,
            "destinationGpuSections": scenario_cost.gpu_section_count,
        },
    });
    std::fs::write(
        options.directory.join("report.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    Ok(LobbyScenarioSmokeReport {
        directory: options.directory.clone(),
        capture_count: state.captures.len(),
        switch_count,
        cancelled_launch_count: 1,
        relaunch_count: 1,
    })
}

fn validate_supported_activation_receipt(
    report: &mclone_scene::EmbeddedWorldActivationReport,
) -> Result<()> {
    if report.accepted_destination_entry_pose.is_none()
        || !report
            .post_swap_entry
            .is_some_and(|sample| sample.support.supported())
        || !report
            .first_uncovered_entry
            .is_some_and(|sample| sample.support.supported())
        || !report
            .stability_entry
            .is_some_and(|sample| sample.support.supported())
        || report.stability_activation_frame.is_none()
        || report.failure.is_some()
    {
        bail!("lobby activation omitted stable supported arrival facts: {report:?}");
    }
    Ok(())
}

fn preview_actor_observations(
    render: mclone_scene::EmbeddedWorldPreviewRenderSnapshot,
) -> impl Iterator<Item = mclone_scene::EmbeddedWorldPreviewActorObservation> {
    [
        render.first_actor_observation,
        render.second_actor_observation,
    ]
    .into_iter()
    .flatten()
}

fn aim_current_eye_at_embedded_preview(host: &mut OffscreenFlatClientHost) -> Result<()> {
    let preview = host
        .scene_host()
        .embedded_world_preview_snapshot()
        .context("lobby scenario has no embedded preview placement")?;
    let anchor = preview.placement.composition_anchor();
    let eye = host.scene_host().camera_snapshot().eye;
    host.set_camera_look_at(
        Vec3::new(eye.x as f32, eye.y as f32, eye.z as f32),
        Vec3::new(anchor.x as f32, anchor.y as f32 + 0.25, anchor.z as f32),
    );
    host.commit_camera()?;
    Ok(())
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
        MonoWorldActionStatus::DeniedByWorldBehavior => {
            bail!("{label} was denied by the active world behavior")
        }
        MonoWorldActionStatus::EmbeddedWorldActivationRequested => {
            bail!("{label} activated an embedded world instead of changing a block")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_screenshot_starts_at_the_configured_entry_center() {
        let mut scene = SceneOptions::default();
        scene.seed = 17_501;
        scene.chunk_x = 3;
        scene.chunk_z = -2;
        scene.world_generation_profile = mclone_server::WorldGenerationProfile::authored_only();
        let expected = SpectatorCamera::spawn_for_scene(&scene);

        let actual = screenshot_startup_camera(&scene, StartupWaitPolicy::Playable);

        assert_eq!(actual.position, expected.position);
    }

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
    fn warm_world_swap_script_walks_through_both_gate_directions() {
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
                .filter(|step| matches!(
                    step,
                    OffscreenScriptStep::WalkForwardUntilWorldSwitch { .. }
                ))
                .count(),
            2
        );
        assert!(matches!(
            script.steps()[2],
            OffscreenScriptStep::FaceActiveWorldGate
        ));
        assert!(matches!(
            script.steps()[5],
            OffscreenScriptStep::AdvanceFrame {
                checkpoint: OffscreenScriptCheckpoint::SourceReturn,
            }
        ));
    }

    #[test]
    fn warm_world_process_cost_uses_paired_residency_deltas() {
        let one_world = WarmWorldProcessIdleSample {
            duration_ms: 3_000,
            cpu_time_ms: Some(450.0),
            cpu_percent_of_one_core: Some(15.0),
            rss_kb: Some(160_000),
            thread_count: Some(10),
        };
        let two_worlds = WarmWorldProcessIdleSample {
            duration_ms: 3_000,
            cpu_time_ms: Some(630.0),
            cpu_percent_of_one_core: Some(21.0),
            rss_kb: Some(197_000),
            thread_count: Some(16),
        };

        let report = process_cost_report(one_world, two_worlds);

        assert_eq!(report.one_world, one_world);
        assert_eq!(report.two_worlds, two_worlds);
        assert_eq!(report.retained_rss_delta_kb, Some(37_000));
        assert_eq!(report.observed_thread_delta, Some(6));
    }
}
