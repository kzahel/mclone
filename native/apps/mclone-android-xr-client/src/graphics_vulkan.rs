use anyhow::Result;
use mclone_xr_graphics::vulkan::{self, VulkanEyeSwapchain};
use openxr as xr;

pub(super) type AppGraphics = vulkan::AppGraphics;
pub(super) type VulkanGraphicsSession = vulkan::VulkanGraphicsSession;

pub(super) struct OpenXrEyeState {
    swapchain: VulkanEyeSwapchain,
    pub(super) depth_view: wgpu::TextureView,
    pub(super) width: u32,
    pub(super) height: u32,
}

impl OpenXrEyeState {
    pub(super) fn texture_count(&self) -> usize {
        self.swapchain.textures().len()
    }
}

impl mclone_xr_host::XrEyeSwapchain<AppGraphics> for OpenXrEyeState {
    fn swapchain(&self) -> &xr::Swapchain<AppGraphics> {
        self.swapchain.swapchain()
    }

    fn swapchain_mut(&mut self) -> &mut xr::Swapchain<AppGraphics> {
        self.swapchain.swapchain_mut()
    }

    fn textures(&self) -> &[wgpu::Texture] {
        self.swapchain.textures()
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

pub(super) fn create_graphics_session(
    xr_instance: &xr::Instance,
    system: xr::SystemId,
) -> Result<VulkanGraphicsSession> {
    vulkan::create_graphics_session(xr_instance, system, "mclone_android_xr_vulkan_device")
}

pub(super) fn create_eye(
    device: &wgpu::Device,
    session: &xr::Session<AppGraphics>,
    eye_width: u32,
    eye_height: u32,
    color_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
    sample_count: u32,
) -> Result<OpenXrEyeState> {
    let swapchain = vulkan::create_eye_swapchain(
        device,
        session,
        eye_width,
        eye_height,
        color_format,
        sample_count,
        "mclone_android_xr_swapchain",
    )?;
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_android_xr_depth"),
        size: wgpu::Extent3d {
            width: eye_width,
            height: eye_height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: depth_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });

    Ok(OpenXrEyeState {
        swapchain,
        depth_view: depth_texture.create_view(&Default::default()),
        width: eye_width,
        height: eye_height,
    })
}
