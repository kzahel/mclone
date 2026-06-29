use anyhow::{Result, bail};
use mclone_render::chunk::ChunkDepthTarget;
use mclone_xr_graphics::vulkan::{self, VulkanEyeSwapchain, VulkanStereoSwapchain};
use openxr as xr;

pub(super) type AppGraphics = vulkan::AppGraphics;
pub(super) type VulkanGraphicsSession = vulkan::VulkanGraphicsSession;

pub(super) struct OpenXrEyeState {
    swapchain: VulkanEyeSwapchain,
    pub(super) depth: ChunkDepthTarget,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) struct OpenXrStereoState {
    swapchain: VulkanStereoSwapchain,
    pub(super) width: u32,
    pub(super) height: u32,
}

impl OpenXrEyeState {
    pub(super) fn texture_count(&self) -> usize {
        self.swapchain.textures().len()
    }
}

impl OpenXrStereoState {
    pub(super) fn texture_count(&self) -> usize {
        self.swapchain.textures().len()
    }

    pub(super) fn array_size(&self) -> u32 {
        self.swapchain.array_size()
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

impl mclone_xr_host::XrStereoSwapchain<AppGraphics> for OpenXrStereoState {
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

    fn array_size(&self) -> u32 {
        self.swapchain.array_size()
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
    if sample_count != 1 {
        bail!("Android XR eye depth targets require sample_count=1, got {sample_count}");
    }
    if depth_format != mclone_render::chunk::DEPTH_FORMAT {
        bail!(
            "Android XR eye depth targets require {:?}, got {depth_format:?}",
            mclone_render::chunk::DEPTH_FORMAT
        );
    }

    let swapchain = vulkan::create_eye_swapchain(
        device,
        session,
        eye_width,
        eye_height,
        color_format,
        sample_count,
        "mclone_android_xr_swapchain",
    )?;

    Ok(OpenXrEyeState {
        swapchain,
        depth: ChunkDepthTarget::new(device, eye_width, eye_height),
        width: eye_width,
        height: eye_height,
    })
}

pub(super) fn create_stereo(
    device: &wgpu::Device,
    session: &xr::Session<AppGraphics>,
    eye_width: u32,
    eye_height: u32,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
) -> Result<OpenXrStereoState> {
    if sample_count != 1 {
        bail!("Android XR stereo array targets require sample_count=1, got {sample_count}");
    }

    let swapchain = vulkan::create_stereo_swapchain(
        device,
        session,
        eye_width,
        eye_height,
        color_format,
        sample_count,
        "mclone_android_xr_stereo_array_swapchain",
    )?;

    Ok(OpenXrStereoState {
        swapchain,
        width: eye_width,
        height: eye_height,
    })
}
