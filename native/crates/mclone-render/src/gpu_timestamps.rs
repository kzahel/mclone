use std::cell::{Cell, RefCell};
use std::sync::mpsc;

use anyhow::{Context, Result, bail};
use mclone_diagnostics::{GpuPassId, GpuTimestampPanelReport, GpuTimestampPassReport};

const DEFAULT_FRAMES_IN_FLIGHT: usize = 4;
const DEFAULT_QUERIES_PER_FRAME: u32 = 64;

#[derive(Clone, Debug)]
pub struct GpuTimestampConfig {
    pub frames_in_flight: usize,
    pub queries_per_frame: u32,
}

impl Default for GpuTimestampConfig {
    fn default() -> Self {
        Self {
            frames_in_flight: DEFAULT_FRAMES_IN_FLIGHT,
            queries_per_frame: DEFAULT_QUERIES_PER_FRAME,
        }
    }
}

#[derive(Clone, Debug)]
struct PassAllocation {
    pass: GpuPassId,
    begin_query: u32,
    end_query: u32,
}

#[derive(Debug)]
pub struct GpuTimestampFrameEncoder {
    slot_index: usize,
    query_set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    query_capacity: u32,
    next_query: Cell<u32>,
    passes: RefCell<Vec<PassAllocation>>,
}

impl GpuTimestampFrameEncoder {
    pub fn render_pass_timestamp_writes(
        &self,
        pass: GpuPassId,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let begin_query = self.next_query.get();
        let end_query = begin_query.checked_add(1)?;
        if end_query >= self.query_capacity {
            return None;
        }
        self.next_query.set(end_query + 1);
        self.passes.borrow_mut().push(PassAllocation {
            pass,
            begin_query,
            end_query,
        });
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(begin_query),
            end_of_pass_write_index: Some(end_query),
        })
    }

    fn used_query_count(&self) -> u32 {
        self.next_query.get()
    }

    fn pass_allocations(&self) -> Vec<PassAllocation> {
        self.passes.borrow().clone()
    }
}

#[derive(Debug)]
pub struct EncodedGpuTimestampFrame {
    slot_index: usize,
    readback_buffer: wgpu::Buffer,
    query_count: u32,
    passes: Vec<PassAllocation>,
}

#[derive(Debug)]
struct PendingGpuTimestampFrame {
    slot_index: usize,
    readback_buffer: wgpu::Buffer,
    byte_len: wgpu::BufferAddress,
    passes: Vec<PassAllocation>,
    receiver: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SlotState {
    Free,
    Reserved,
    Pending,
}

#[derive(Debug)]
struct GpuTimestampSlot {
    query_set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    state: SlotState,
}

#[derive(Debug)]
pub struct GpuTimestampProfiler {
    timestamp_period_ns: f64,
    query_capacity: u32,
    slots: Vec<GpuTimestampSlot>,
    pending: Vec<PendingGpuTimestampFrame>,
    frames_submitted: u64,
    frames_resolved: u64,
    dropped_frames: u64,
    latest_passes: Vec<GpuTimestampPassReport>,
}

#[derive(Clone, Debug)]
pub struct GpuTimestampCalibrationReport {
    pub supported: bool,
    pub timestamp_period_ns: Option<f64>,
    pub light_pass_ms: Option<f64>,
    pub light_pass_valid: bool,
    pub heavy_pass_ms: Option<f64>,
    pub heavy_pass_valid: bool,
    pub heavy_quad_count: u32,
    pub scale_ratio: Option<f64>,
    pub panel: GpuTimestampPanelReport,
}

impl GpuTimestampProfiler {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        Self::with_config(device, queue, GpuTimestampConfig::default())
    }

    pub fn with_config(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: GpuTimestampConfig,
    ) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let frames_in_flight = config.frames_in_flight.max(1);
        let query_capacity = config.queries_per_frame.max(2);
        let resolve_size = aligned_query_buffer_size(query_capacity);
        let mut slots = Vec::with_capacity(frames_in_flight);
        for index in 0..frames_in_flight {
            let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some(&format!("mclone_gpu_timestamp_query_set_{index}")),
                ty: wgpu::QueryType::Timestamp,
                count: query_capacity,
            });
            let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("mclone_gpu_timestamp_resolve_buffer_{index}")),
                size: resolve_size,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("mclone_gpu_timestamp_readback_buffer_{index}")),
                size: resolve_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            slots.push(GpuTimestampSlot {
                query_set,
                resolve_buffer,
                readback_buffer,
                state: SlotState::Free,
            });
        }
        Some(Self {
            timestamp_period_ns: queue.get_timestamp_period() as f64,
            query_capacity,
            slots,
            pending: Vec::new(),
            frames_submitted: 0,
            frames_resolved: 0,
            dropped_frames: 0,
            latest_passes: Vec::new(),
        })
    }

    pub fn begin_frame(&mut self) -> Option<GpuTimestampFrameEncoder> {
        let Some((slot_index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.state == SlotState::Free)
        else {
            self.dropped_frames = self.dropped_frames.saturating_add(1);
            return None;
        };
        slot.state = SlotState::Reserved;
        Some(GpuTimestampFrameEncoder {
            slot_index,
            query_set: slot.query_set.clone(),
            resolve_buffer: slot.resolve_buffer.clone(),
            readback_buffer: slot.readback_buffer.clone(),
            query_capacity: self.query_capacity,
            next_query: Cell::new(0),
            passes: RefCell::new(Vec::new()),
        })
    }

    pub fn resolve_frame(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: GpuTimestampFrameEncoder,
    ) -> Option<EncodedGpuTimestampFrame> {
        let query_count = frame.used_query_count();
        let slot_index = frame.slot_index;
        if query_count == 0 {
            self.release_reserved_slot(slot_index);
            return None;
        }
        let byte_len = query_byte_len(query_count);
        encoder.resolve_query_set(&frame.query_set, 0..query_count, &frame.resolve_buffer, 0);
        encoder.copy_buffer_to_buffer(
            &frame.resolve_buffer,
            0,
            &frame.readback_buffer,
            0,
            byte_len,
        );
        let passes = frame.pass_allocations();
        Some(EncodedGpuTimestampFrame {
            slot_index,
            readback_buffer: frame.readback_buffer,
            query_count,
            passes,
        })
    }

    pub fn discard_frame(&mut self, frame: GpuTimestampFrameEncoder) {
        self.release_reserved_slot(frame.slot_index);
    }

    pub fn mark_submitted(&mut self, frame: EncodedGpuTimestampFrame) {
        let byte_len = query_byte_len(frame.query_count);
        let (sender, receiver) = mpsc::channel();
        frame
            .readback_buffer
            .slice(0..byte_len)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        if let Some(slot) = self.slots.get_mut(frame.slot_index) {
            slot.state = SlotState::Pending;
        }
        self.frames_submitted = self.frames_submitted.saturating_add(1);
        self.pending.push(PendingGpuTimestampFrame {
            slot_index: frame.slot_index,
            readback_buffer: frame.readback_buffer,
            byte_len,
            passes: frame.passes,
            receiver,
        });
    }

    pub fn poll_completed(&mut self, device: &wgpu::Device) -> Vec<Vec<GpuTimestampPassReport>> {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut completed_reports = Vec::new();
        let mut index = 0;
        while index < self.pending.len() {
            match self.pending[index].receiver.try_recv() {
                Ok(Ok(())) => {
                    let pending = self.pending.remove(index);
                    let reports = self.read_pending_frame(&pending);
                    pending.readback_buffer.unmap();
                    self.release_pending_slot(pending.slot_index);
                    self.frames_resolved = self.frames_resolved.saturating_add(1);
                    self.latest_passes = reports.clone();
                    completed_reports.push(reports);
                }
                Ok(Err(_)) => {
                    let pending = self.pending.remove(index);
                    self.release_pending_slot(pending.slot_index);
                    self.dropped_frames = self.dropped_frames.saturating_add(1);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    index += 1;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let pending = self.pending.remove(index);
                    self.release_pending_slot(pending.slot_index);
                    self.dropped_frames = self.dropped_frames.saturating_add(1);
                }
            }
        }
        completed_reports
    }

    pub fn wait_for_all(&mut self, device: &wgpu::Device) -> Vec<Vec<GpuTimestampPassReport>> {
        let mut reports = Vec::new();
        while !self.pending.is_empty() {
            let _ = device.poll(wgpu::PollType::Wait);
            reports.extend(self.poll_completed(device));
        }
        reports
    }

    pub fn panel_report(&self) -> GpuTimestampPanelReport {
        GpuTimestampPanelReport::supported(self.timestamp_period_ns)
            .with_counters(
                self.frames_submitted,
                self.frames_resolved,
                self.pending.len() as u64,
                self.dropped_frames,
            )
            .with_latest_passes(self.latest_passes.clone())
    }

    fn read_pending_frame(
        &self,
        pending: &PendingGpuTimestampFrame,
    ) -> Vec<GpuTimestampPassReport> {
        let mapped = pending
            .readback_buffer
            .slice(0..pending.byte_len)
            .get_mapped_range();
        let mut ticks = Vec::with_capacity(mapped.len() / wgpu::QUERY_SIZE as usize);
        for chunk in mapped.chunks_exact(wgpu::QUERY_SIZE as usize) {
            let mut bytes = [0_u8; 8];
            bytes.copy_from_slice(chunk);
            ticks.push(u64::from_le_bytes(bytes));
        }
        drop(mapped);
        pending
            .passes
            .iter()
            .filter_map(|allocation| {
                let begin_tick = *ticks.get(allocation.begin_query as usize)?;
                let end_tick = *ticks.get(allocation.end_query as usize)?;
                let valid = end_tick != 0 && end_tick >= begin_tick;
                let elapsed_ms = if valid {
                    ((end_tick - begin_tick) as f64 * self.timestamp_period_ns) / 1_000_000.0
                } else {
                    0.0
                };
                Some(GpuTimestampPassReport::new(
                    allocation.pass.clone(),
                    elapsed_ms,
                    begin_tick,
                    end_tick,
                ))
            })
            .collect()
    }

    fn release_reserved_slot(&mut self, slot_index: usize) {
        if let Some(slot) = self.slots.get_mut(slot_index)
            && slot.state == SlotState::Reserved
        {
            slot.state = SlotState::Free;
        }
    }

    fn release_pending_slot(&mut self, slot_index: usize) {
        if let Some(slot) = self.slots.get_mut(slot_index)
            && slot.state == SlotState::Pending
        {
            slot.state = SlotState::Free;
        }
    }
}

pub fn run_gpu_timestamp_calibration(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    heavy_quad_count: u32,
) -> Result<GpuTimestampCalibrationReport> {
    let heavy_quad_count = heavy_quad_count.max(2);
    let Some(mut profiler) = GpuTimestampProfiler::new(device, queue) else {
        return Ok(GpuTimestampCalibrationReport {
            supported: false,
            timestamp_period_ns: None,
            light_pass_ms: None,
            light_pass_valid: false,
            heavy_pass_ms: None,
            heavy_pass_valid: false,
            heavy_quad_count,
            scale_ratio: None,
            panel: GpuTimestampPanelReport::unsupported(),
        });
    };
    let target = CalibrationTarget::new(device, 512, 512);
    let renderer = CalibrationRenderer::new(device, target.format);
    let frame = profiler
        .begin_frame()
        .context("GPU timestamp calibration could not reserve a query frame")?;
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_gpu_timestamp_calibration_encoder"),
    });
    renderer.render_pass(
        &mut encoder,
        &target.view,
        1,
        frame.render_pass_timestamp_writes(GpuPassId::Calibration),
        "mclone_gpu_timestamp_calibration_light_pass",
    );
    renderer.render_pass(
        &mut encoder,
        &target.view,
        heavy_quad_count,
        frame.render_pass_timestamp_writes(GpuPassId::Custom("calibration-heavy".to_owned())),
        "mclone_gpu_timestamp_calibration_heavy_pass",
    );
    renderer.render_pass(
        &mut encoder,
        &target.view,
        1,
        frame.render_pass_timestamp_writes(GpuPassId::Custom("calibration-sentinel".to_owned())),
        "mclone_gpu_timestamp_calibration_sentinel_pass",
    );
    let encoded = profiler
        .resolve_frame(&mut encoder, frame)
        .context("GPU timestamp calibration produced no timestamp queries")?;
    queue.submit(Some(encoder.finish()));
    profiler.mark_submitted(encoded);
    let completed = profiler.wait_for_all(device);
    let passes = completed
        .last()
        .context("GPU timestamp calibration produced no readback")?;
    let light_pass = passes
        .iter()
        .find(|pass| pass.pass == GpuPassId::Calibration)
        .context("GPU timestamp calibration missing light pass")?;
    let heavy_pass = passes
        .iter()
        .find(|pass| pass.pass == GpuPassId::Custom("calibration-heavy".to_owned()))
        .context("GPU timestamp calibration missing heavy pass")?;
    let light_pass_ms = light_pass.elapsed_ms;
    let heavy_pass_ms = heavy_pass.elapsed_ms;
    if !light_pass.valid && !heavy_pass.valid {
        bail!(
            "GPU timestamp calibration produced no positive timings: light={light_pass_ms:.6} heavy={heavy_pass_ms:.6}"
        );
    }
    if light_pass.valid && heavy_pass.valid && heavy_pass_ms < light_pass_ms * 0.5 {
        log::warn!(
            "GPU timestamp calibration heavy pass did not scale plausibly: light={light_pass_ms:.6} heavy={heavy_pass_ms:.6}"
        );
    }
    let scale_ratio =
        (light_pass.valid && heavy_pass.valid).then_some(heavy_pass_ms / light_pass_ms);
    Ok(GpuTimestampCalibrationReport {
        supported: true,
        timestamp_period_ns: Some(queue.get_timestamp_period() as f64),
        light_pass_ms: Some(light_pass_ms),
        light_pass_valid: light_pass.valid,
        heavy_pass_ms: Some(heavy_pass_ms),
        heavy_pass_valid: heavy_pass.valid,
        heavy_quad_count,
        scale_ratio,
        panel: profiler
            .panel_report()
            .with_latest_passes(vec![(*light_pass).clone(), (*heavy_pass).clone()]),
    })
}

struct CalibrationTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
}

impl CalibrationTarget {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_gpu_timestamp_calibration_target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            _texture: texture,
            view,
            format,
        }
    }
}

struct CalibrationRenderer {
    pipeline: wgpu::RenderPipeline,
}

impl CalibrationRenderer {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_gpu_timestamp_calibration_shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );
    let pos = positions[vertex_index];
    var out: VsOut;
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = pos * 0.5 + vec2<f32>(0.5, 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    var value = in.uv.x * 0.37 + in.uv.y * 0.63;
    for (var i: u32 = 0u; i < 8u; i = i + 1u) {
        value = fract(value * 1.6180339 + f32(i) * 0.013);
    }
    let facing = select(0.75, 1.0, front_facing);
    return vec4<f32>(value, value * facing, 1.0 - value, 1.0);
}
"#
                .into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_gpu_timestamp_calibration_pipeline_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_gpu_timestamp_calibration_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Self { pipeline }
    }

    fn render_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        quad_count: u32,
        timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
        label: &'static str,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes,
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.draw(0..6, 0..quad_count);
    }
}

fn query_byte_len(query_count: u32) -> wgpu::BufferAddress {
    query_count as wgpu::BufferAddress * wgpu::QUERY_SIZE as wgpu::BufferAddress
}

fn aligned_query_buffer_size(query_count: u32) -> wgpu::BufferAddress {
    align_to(
        query_byte_len(query_count),
        wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT,
    )
}

fn align_to(value: wgpu::BufferAddress, alignment: wgpu::BufferAddress) -> wgpu::BufferAddress {
    let remainder = value % alignment;
    if remainder == 0 {
        value
    } else {
        value + (alignment - remainder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_buffer_size_is_resolve_aligned() {
        let size = aligned_query_buffer_size(3);
        assert_eq!(size % wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT, 0);
        assert!(size >= 3 * wgpu::QUERY_SIZE as u64);
    }
}
