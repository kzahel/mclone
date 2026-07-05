use mclone_diagnostics::GpuPassId;

use crate::gpu_timestamps::GpuTimestampFrameEncoder;

#[derive(Clone, Copy)]
pub struct RenderFrameTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth_view: Option<&'a wgpu::TextureView>,
    pub size: [u32; 2],
    pub gpu_timestamps: Option<&'a GpuTimestampFrameEncoder>,
}

impl<'a> RenderFrameTarget<'a> {
    pub fn color(color_view: &'a wgpu::TextureView, size: [u32; 2]) -> Self {
        Self {
            color_view,
            depth_view: None,
            size,
            gpu_timestamps: None,
        }
    }

    pub fn with_depth(self, depth_view: &'a wgpu::TextureView) -> Self {
        Self {
            depth_view: Some(depth_view),
            ..self
        }
    }

    pub fn with_gpu_timestamps(self, gpu_timestamps: &'a GpuTimestampFrameEncoder) -> Self {
        Self {
            gpu_timestamps: Some(gpu_timestamps),
            ..self
        }
    }

    pub fn gpu_timestamp_writes(
        self,
        pass: GpuPassId,
    ) -> Option<wgpu::RenderPassTimestampWrites<'a>> {
        self.gpu_timestamps
            .and_then(|timestamps| timestamps.render_pass_timestamp_writes(pass))
    }
}

pub struct RenderFrameContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub target: RenderFrameTarget<'a>,
}

impl<'a> RenderFrameContext<'a> {
    pub fn new(
        device: &'a wgpu::Device,
        queue: &'a wgpu::Queue,
        encoder: &'a mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'a>,
    ) -> Self {
        Self {
            device,
            queue,
            encoder,
            target,
        }
    }
}
