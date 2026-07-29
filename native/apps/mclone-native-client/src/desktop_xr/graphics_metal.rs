use std::{ffi::c_void, ptr};

use anyhow::{Context, Result, bail};
use mclone_render::chunk::{ChunkDepthTarget, ChunkMultiviewDepthTarget};
use metal::foreign_types::{ForeignType, ForeignTypeRef};
use metal::{MTLPixelFormat, MTLTextureType};
use openxr as xr;
use wgpu::hal::api::Metal as HalMetal;
use xr::Graphics;
use xr::sys::{
    GraphicsBindingMetalKHR, GraphicsRequirementsMetalKHR, Handle as _, Result as XrSysResult,
    Session as XrSession, SessionCreateInfo as XrSessionCreateInfo, SwapchainImageMetalKHR,
};

pub(super) enum AppGraphics {}
pub(super) type GraphicsSession = MetalGraphicsSession;

#[derive(Debug, Copy, Clone)]
pub(super) struct Requirements {
    metal_device: *mut c_void,
}

#[derive(Copy, Clone)]
pub(super) struct SessionCreateInfo {
    command_queue: *mut c_void,
}

pub(super) struct MetalGraphicsSession {
    pub(super) frame_wait: xr::FrameWaiter,
    pub(super) frame_stream: xr::FrameStream<AppGraphics>,
    pub(super) session: xr::Session<AppGraphics>,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) adapter_name: String,
    pub(super) required_device_name: String,
}

pub(super) struct OpenXrEyeState {
    pub(super) depth: ChunkDepthTarget,
    pub(super) textures: Vec<wgpu::Texture>,
    pub(super) swapchain: xr::Swapchain<AppGraphics>,
    pub(super) width: u32,
    pub(super) height: u32,
    outstanding_images: u32,
}

pub(super) struct OpenXrStereoState {
    pub(super) depth: ChunkMultiviewDepthTarget,
    pub(super) textures: Vec<wgpu::Texture>,
    pub(super) swapchain: xr::Swapchain<AppGraphics>,
    pub(super) width: u32,
    pub(super) height: u32,
    outstanding_images: u32,
}

impl OpenXrStereoState {
    pub(super) fn texture_count(&self) -> usize {
        self.textures.len()
    }

    pub(super) const fn array_size(&self) -> u32 {
        2
    }
}

impl OpenXrEyeState {
    pub(super) fn texture_count(&self) -> usize {
        self.textures.len()
    }
}

impl mclone_xr_host::XrEyeSwapchain<AppGraphics> for OpenXrEyeState {
    fn swapchain(&self) -> &xr::Swapchain<AppGraphics> {
        &self.swapchain
    }

    fn swapchain_mut(&mut self) -> &mut xr::Swapchain<AppGraphics> {
        &mut self.swapchain
    }

    fn textures(&self) -> &[wgpu::Texture] {
        &self.textures
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn outstanding_image_count(&self) -> u32 {
        self.outstanding_images
    }

    fn record_image_acquired(&mut self) {
        self.outstanding_images = self.outstanding_images.saturating_add(1);
    }

    fn record_image_released(&mut self) {
        self.outstanding_images = self.outstanding_images.saturating_sub(1);
    }
}

impl mclone_xr_host::XrStereoSwapchain<AppGraphics> for OpenXrStereoState {
    fn swapchain(&self) -> &xr::Swapchain<AppGraphics> {
        &self.swapchain
    }

    fn swapchain_mut(&mut self) -> &mut xr::Swapchain<AppGraphics> {
        &mut self.swapchain
    }

    fn textures(&self) -> &[wgpu::Texture] {
        &self.textures
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn array_size(&self) -> u32 {
        self.array_size()
    }

    fn outstanding_image_count(&self) -> u32 {
        self.outstanding_images
    }

    fn record_image_acquired(&mut self) {
        self.outstanding_images = self.outstanding_images.saturating_add(1);
    }

    fn record_image_released(&mut self) {
        self.outstanding_images = self.outstanding_images.saturating_sub(1);
    }
}

impl Graphics for AppGraphics {
    type Requirements = Requirements;
    type SessionCreateInfo = SessionCreateInfo;
    type Format = i64;
    type SwapchainImage = *mut c_void;

    fn raise_format(format: i64) -> Self::Format {
        format
    }

    fn lower_format(format: Self::Format) -> i64 {
        format
    }

    fn requirements(
        instance: &xr::Instance,
        system: xr::SystemId,
    ) -> xr::Result<Self::Requirements> {
        let ext = instance
            .exts()
            .khr_metal_enable
            .as_ref()
            .expect("XR_KHR_metal_enable not loaded");
        let out = unsafe {
            let mut requirements = GraphicsRequirementsMetalKHR::out(ptr::null_mut());
            cvt((ext.get_metal_graphics_requirements)(
                instance.as_raw(),
                system,
                requirements.as_mut_ptr(),
            ))?;
            requirements.assume_init()
        };
        Ok(Requirements {
            metal_device: out.metal_device,
        })
    }

    unsafe fn create_session(
        instance: &xr::Instance,
        system: xr::SystemId,
        info: &Self::SessionCreateInfo,
    ) -> xr::Result<XrSession> {
        let binding = GraphicsBindingMetalKHR {
            ty: GraphicsBindingMetalKHR::TYPE,
            next: ptr::null(),
            command_queue: info.command_queue,
        };
        let create_info = XrSessionCreateInfo {
            ty: XrSessionCreateInfo::TYPE,
            next: &binding as *const _ as *const _,
            create_flags: Default::default(),
            system_id: system,
        };
        let mut out = XrSession::NULL;
        unsafe {
            cvt((instance.fp().create_session)(
                instance.as_raw(),
                &create_info,
                &mut out,
            ))?;
        }
        Ok(out)
    }

    fn enumerate_swapchain_images(
        swapchain: &xr::Swapchain<Self>,
    ) -> xr::Result<Vec<Self::SwapchainImage>> {
        let mut count = 0;
        cvt(unsafe {
            (swapchain.instance().fp().enumerate_swapchain_images)(
                swapchain.as_raw(),
                0,
                &mut count,
                ptr::null_mut(),
            )
        })?;
        let mut images = vec![
            SwapchainImageMetalKHR {
                ty: SwapchainImageMetalKHR::TYPE,
                next: ptr::null_mut(),
                texture: ptr::null_mut(),
            };
            count as usize
        ];
        cvt(unsafe {
            (swapchain.instance().fp().enumerate_swapchain_images)(
                swapchain.as_raw(),
                count,
                &mut count,
                images.as_mut_ptr() as *mut _,
            )
        })?;
        Ok(images.into_iter().map(|image| image.texture).collect())
    }
}

pub(super) fn create_graphics_session(
    xr_instance: &xr::Instance,
    system: xr::SystemId,
) -> Result<MetalGraphicsSession> {
    let requirements = xr_instance
        .graphics_requirements::<AppGraphics>(system)
        .context("query OpenXR Metal graphics requirements")?;
    let (device, queue, adapter_name, required_device_name) =
        create_matching_metal_device_and_queue(requirements.metal_device)?;
    let command_queue = metal_command_queue_ptr(&queue);

    let (session, frame_wait, frame_stream) = unsafe {
        xr_instance.create_session::<AppGraphics>(system, &SessionCreateInfo { command_queue })
    }
    .context("create OpenXR Metal session")?;

    Ok(MetalGraphicsSession {
        frame_wait,
        frame_stream,
        session,
        device,
        queue,
        adapter_name,
        required_device_name,
    })
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
    let metal_format = wgpu_format_to_metal_pixel_format(color_format)
        .ok_or_else(|| anyhow::anyhow!("unsupported XR color format {color_format:?}"))?;
    let xr_format = metal_format as i64;
    let supported_formats = session
        .enumerate_swapchain_formats()
        .context("enumerate OpenXR Metal swapchain formats")?;
    if !supported_formats.contains(&xr_format) {
        bail!("OpenXR runtime does not advertise {metal_format:?} swapchain images");
    }

    let swapchain = session
        .create_swapchain(&xr::SwapchainCreateInfo {
            create_flags: xr::SwapchainCreateFlags::EMPTY,
            usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
                | xr::SwapchainUsageFlags::SAMPLED
                | xr::SwapchainUsageFlags::TRANSFER_SRC,
            format: xr_format,
            sample_count,
            width: eye_width,
            height: eye_height,
            face_count: 1,
            array_size: 1,
            mip_count: 1,
        })
        .context("create OpenXR Metal eye swapchain")?;

    let textures = swapchain
        .enumerate_images()
        .context("enumerate OpenXR Metal swapchain images")?
        .into_iter()
        .map(|texture| {
            create_metal_swapchain_texture(device, texture, eye_width, eye_height, 1, color_format)
        })
        .collect();

    if sample_count != 1 {
        bail!("OpenXR mclone eye depth targets require sample_count=1, got {sample_count}");
    }
    if depth_format != mclone_render::chunk::DEPTH_FORMAT {
        bail!(
            "OpenXR mclone eye depth targets require {:?}, got {depth_format:?}",
            mclone_render::chunk::DEPTH_FORMAT
        );
    }

    Ok(OpenXrEyeState {
        swapchain,
        textures,
        depth: ChunkDepthTarget::new(device, eye_width, eye_height),
        width: eye_width,
        height: eye_height,
        outstanding_images: 0,
    })
}

pub(super) fn create_stereo(
    device: &wgpu::Device,
    session: &xr::Session<AppGraphics>,
    eye_width: u32,
    eye_height: u32,
    color_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
    sample_count: u32,
) -> Result<OpenXrStereoState> {
    if sample_count != 1 {
        bail!("OpenXR stereo-array depth targets require sample_count=1, got {sample_count}");
    }
    if depth_format != mclone_render::chunk::DEPTH_FORMAT {
        bail!(
            "OpenXR stereo-array depth targets require {:?}, got {depth_format:?}",
            mclone_render::chunk::DEPTH_FORMAT
        );
    }
    let metal_format = wgpu_format_to_metal_pixel_format(color_format)
        .ok_or_else(|| anyhow::anyhow!("unsupported XR color format {color_format:?}"))?;
    let xr_format = metal_format as i64;
    let supported_formats = session
        .enumerate_swapchain_formats()
        .context("enumerate OpenXR Metal swapchain formats")?;
    if !supported_formats.contains(&xr_format) {
        bail!("OpenXR runtime does not advertise {metal_format:?} swapchain images");
    }
    let array_size = 2;
    let swapchain = session
        .create_swapchain(&xr::SwapchainCreateInfo {
            create_flags: xr::SwapchainCreateFlags::EMPTY,
            usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
                | xr::SwapchainUsageFlags::SAMPLED
                | xr::SwapchainUsageFlags::TRANSFER_SRC,
            format: xr_format,
            sample_count,
            width: eye_width,
            height: eye_height,
            face_count: 1,
            array_size,
            mip_count: 1,
        })
        .context("create OpenXR Metal stereo-array swapchain")?;
    let textures = swapchain
        .enumerate_images()
        .context("enumerate OpenXR Metal stereo-array swapchain images")?
        .into_iter()
        .map(|texture| {
            create_metal_swapchain_texture(
                device,
                texture,
                eye_width,
                eye_height,
                array_size,
                color_format,
            )
        })
        .collect();
    Ok(OpenXrStereoState {
        swapchain,
        textures,
        depth: ChunkMultiviewDepthTarget::new(device, eye_width, eye_height),
        width: eye_width,
        height: eye_height,
        outstanding_images: 0,
    })
}

fn create_matching_metal_device_and_queue(
    required_device: *mut c_void,
) -> Result<(wgpu::Device, wgpu::Queue, String, String)> {
    let required_device_ref = unsafe { metal::DeviceRef::from_ptr(required_device.cast()) };
    let required_device_name = required_device_ref.name().to_string();
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..Default::default()
    });

    let mut last_error = None;
    for adapter in instance.enumerate_adapters(wgpu::Backends::METAL) {
        let info = adapter.get_info();
        let adapter_name = info.name.clone();
        let required_features = adapter.features() & wgpu::Features::MULTIVIEW;
        match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("mclone_xr_metal_device"),
            required_features,
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        })) {
            Ok((device, queue)) => {
                let matches = unsafe {
                    device.as_hal::<HalMetal, _, _>(|device| {
                        device.map(|device| device.raw_device().lock().as_ptr().cast::<c_void>())
                            == Some(required_device)
                    })
                };
                if matches {
                    return Ok((device, queue, adapter_name, required_device_name));
                }
                device.destroy();
            }
            Err(err) => {
                last_error = Some(format!("{adapter_name}: {err}"));
            }
        }
    }

    bail!(
        "{}",
        last_error.unwrap_or_else(|| {
            format!("no Metal adapter matched runtime-required device '{required_device_name}'")
        })
    )
}

fn metal_command_queue_ptr(queue: &wgpu::Queue) -> *mut c_void {
    unsafe {
        queue.as_hal::<HalMetal, _, _>(|queue| {
            queue
                .expect("wgpu queue is not backed by Metal")
                .as_raw()
                .lock()
                .as_ptr()
                .cast::<c_void>()
        })
    }
}

fn create_metal_swapchain_texture(
    device: &wgpu::Device,
    texture_ptr: *mut c_void,
    width: u32,
    height: u32,
    array_size: u32,
    color_format: wgpu::TextureFormat,
) -> wgpu::Texture {
    let texture_ref = unsafe { metal::TextureRef::from_ptr(texture_ptr.cast()) };
    let texture = texture_ref.to_owned();
    let hal_texture = unsafe {
        wgpu::hal::metal::Device::texture_from_raw(
            texture,
            color_format,
            if array_size > 1 {
                MTLTextureType::D2Array
            } else {
                MTLTextureType::D2
            },
            array_size,
            1,
            wgpu::hal::CopyExtent {
                width,
                height,
                depth: array_size,
            },
        )
    };
    unsafe {
        device.create_texture_from_hal::<HalMetal>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("mclone_xr_swapchain"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: array_size,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: color_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
        )
    }
}

fn wgpu_format_to_metal_pixel_format(format: wgpu::TextureFormat) -> Option<MTLPixelFormat> {
    match format {
        wgpu::TextureFormat::Rgba8UnormSrgb => Some(MTLPixelFormat::RGBA8Unorm_sRGB),
        wgpu::TextureFormat::Rgba8Unorm => Some(MTLPixelFormat::RGBA8Unorm),
        wgpu::TextureFormat::Bgra8UnormSrgb => Some(MTLPixelFormat::BGRA8Unorm_sRGB),
        wgpu::TextureFormat::Bgra8Unorm => Some(MTLPixelFormat::BGRA8Unorm),
        _ => None,
    }
}

fn cvt(result: XrSysResult) -> xr::Result<XrSysResult> {
    if result.into_raw() >= 0 {
        Ok(result)
    } else {
        Err(result)
    }
}
