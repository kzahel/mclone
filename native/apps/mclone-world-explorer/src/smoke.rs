use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use mclone_terrain_view::TerrainHorizonFrameStats;
use mclone_view_control::{
    ViewPoint, ViewportMetrics, WorldViewIntent, WorldViewMode, WorldViewState,
};
use serde_json::{Value, json};

use crate::capture::{DepthStats, PixelStats};
use crate::options::ExplorerOptions;
use crate::terrain::ExplorerTerrain;

const MOVEMENT_FRAMES: u32 = 8;
const ZOOM_FRAMES: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SmokeStage {
    Initial3d,
    MoveX,
    MoveZ,
    MoveDiagonal,
    Zoom,
    Map,
    Orbit,
}

const SMOKE_STAGES: [SmokeStage; 7] = [
    SmokeStage::Initial3d,
    SmokeStage::MoveX,
    SmokeStage::MoveZ,
    SmokeStage::MoveDiagonal,
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
}

impl SmokeStage {
    const fn input_frame_count(self) -> u32 {
        match self {
            Self::Initial3d => 0,
            Self::MoveX | Self::MoveZ | Self::MoveDiagonal => MOVEMENT_FRAMES,
            Self::Zoom => ZOOM_FRAMES,
            Self::Map | Self::Orbit => 1,
        }
    }

    const fn capture_label(self) -> Option<&'static str> {
        match self {
            Self::Initial3d => Some("3d"),
            Self::MoveDiagonal => Some("movement"),
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
            captures: Vec::new(),
        })
    }

    pub fn note_frame(
        &mut self,
        frame_time: Duration,
        input_applied: bool,
        stats: TerrainHorizonFrameStats,
    ) {
        self.frame_count = self.frame_count.saturating_add(1);
        let frame_ms = frame_time.as_secs_f64() * 1_000.0;
        if input_applied {
            self.movement_frame_ms.push(frame_ms);
        } else {
            self.settling_frame_ms.push(frame_ms);
        }
        self.peak_resident_bytes = self.peak_resident_bytes.max(stats.resident_bytes);
        self.peak_pending_work = self.peak_pending_work.max(stats.pending_refills);
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
            pixels,
            depth,
        } = checkpoint;
        self.checkpoint_frame_ms
            .push(frame_time.as_secs_f64() * 1_000.0);
        self.captures.push(json!({
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
            "allocation_slots": stats.allocation_slots,
            "ready_slots": stats.ready_slots,
            "pending_refills": stats.pending_refills,
            "total_refills": stats.residency.total_refills,
            "total_rebases": stats.residency.total_rebases,
            "rgb_min": pixels.min_rgb,
            "rgb_max": pixels.max_rgb,
            "opaque_pixels": pixels.opaque_pixels,
            "depth_min": depth.min_depth,
            "depth_max": depth.max_depth,
            "depth_covered_pixels": depth.covered_pixels,
            "depth_clear_pixels": depth.clear_pixels,
        }));
        Ok(())
    }

    pub fn write(&self, terrain: &ExplorerTerrain, options: &ExplorerOptions) -> Result<PathBuf> {
        let final_stats = terrain
            .last_stats()
            .context("World Explorer smoke completed without frame statistics")?;
        let path = self.root.join("receipt.json");
        let receipt = json!({
            "schema": "mclone-world-explorer-smoke-v2",
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

fn duration_ms(duration: Option<Duration>) -> Option<f64> {
    duration.map(|duration| duration.as_secs_f64() * 1_000.0)
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
