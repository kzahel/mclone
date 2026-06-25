use std::{ffi::c_void, ptr};

use anyhow::{Context, Result, bail};
use metal::foreign_types::ForeignTypeRef;
use openxr as xr;
use wgpu::hal::api::Metal as HalMetal;
use xr::Graphics;
use xr::sys::{
    GraphicsBindingMetalKHR, GraphicsRequirementsMetalKHR, Handle as _, Result as XrSysResult,
    Session as XrSession, SessionCreateInfo as XrSessionCreateInfo, SwapchainImageMetalKHR,
};

pub(super) enum AppGraphics {}

#[derive(Debug, Copy, Clone)]
pub(super) struct Requirements {
    metal_device: *mut c_void,
}

#[derive(Copy, Clone)]
pub(super) struct SessionCreateInfo {
    command_queue: *mut c_void,
}

pub(super) struct MetalGraphicsSession {
    pub(super) session: xr::Session<AppGraphics>,
    pub(super) adapter_name: String,
    pub(super) required_device_name: String,
    _device: wgpu::Device,
    _queue: wgpu::Queue,
    _frame_wait: xr::FrameWaiter,
    _frame_stream: xr::FrameStream<AppGraphics>,
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
        session,
        adapter_name,
        required_device_name,
        _device: device,
        _queue: queue,
        _frame_wait: frame_wait,
        _frame_stream: frame_stream,
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
        match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("mclone_xr_metal_device"),
            required_features: wgpu::Features::empty(),
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

fn cvt(result: XrSysResult) -> xr::Result<XrSysResult> {
    if result.into_raw() >= 0 {
        Ok(result)
    } else {
        Err(result)
    }
}
