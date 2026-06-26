#![allow(unsafe_code)]

use anyhow::{Context, Result, anyhow, bail};
use ash::vk;
use ash::vk::Handle as _;
use openxr as xr;
use wgpu::hal::api::Vulkan as HalVulkan;

pub(super) type AppGraphics = xr::Vulkan;

pub(super) struct VulkanGraphicsSession {
    pub(super) frame_wait: xr::FrameWaiter,
    pub(super) frame_stream: xr::FrameStream<AppGraphics>,
    pub(super) session: xr::Session<AppGraphics>,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) physical_device_name: String,
    pub(super) physical_device_api_version: String,
    pub(super) queue_family_index: u32,
}

pub(super) struct OpenXrEyeState {
    pub(super) swapchain: xr::Swapchain<AppGraphics>,
    pub(super) textures: Vec<wgpu::Texture>,
    #[allow(dead_code)]
    pub(super) depth_view: wgpu::TextureView,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) fn create_graphics_session(
    xr_instance: &xr::Instance,
    system: xr::SystemId,
) -> Result<VulkanGraphicsSession> {
    let vk_target_version = vk::make_api_version(0, 1, 1, 0);
    let vk_target_version_xr = xr::Version::new(1, 1, 0);
    let requirements = xr_instance
        .graphics_requirements::<AppGraphics>(system)
        .context("query OpenXR Vulkan graphics requirements")?;
    if vk_target_version_xr < requirements.min_api_version_supported
        || vk_target_version_xr.major() > requirements.max_api_version_supported.major()
    {
        bail!(
            "OpenXR runtime requires Vulkan version >= {}, < {}.0.0",
            requirements.min_api_version_supported,
            requirements.max_api_version_supported.major() + 1
        );
    }

    let vk_entry = unsafe { ash::Entry::load().context("load Vulkan loader")? };
    let vk_app_info = vk::ApplicationInfo::default()
        .application_name(c"mclone")
        .application_version(1)
        .engine_name(c"mclone")
        .engine_version(1)
        .api_version(vk_target_version);
    let vk_instance_info = vk::InstanceCreateInfo::default().application_info(&vk_app_info);
    let vk_instance = {
        let get_instance_proc_addr =
            unsafe { std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr) };
        let raw = unsafe {
            xr_instance.create_vulkan_instance(
                system,
                get_instance_proc_addr,
                &vk_instance_info as *const _ as *const _,
            )
        }
        .context("XR error creating Vulkan instance")?
        .map_err(vk::Result::from_raw)
        .map_err(|err| anyhow!("Vulkan error creating Vulkan instance: {err:?}"))?;
        unsafe { ash::Instance::load(vk_entry.static_fn(), vk::Instance::from_raw(raw as _)) }
    };

    let vk_physical_device = vk::PhysicalDevice::from_raw(unsafe {
        xr_instance
            .vulkan_graphics_device(system, vk_instance.handle().as_raw() as _)
            .context("query OpenXR Vulkan physical device")?
    } as _);
    let physical_device_properties =
        unsafe { vk_instance.get_physical_device_properties(vk_physical_device) };
    if physical_device_properties.api_version < vk_target_version {
        bail!(
            "OpenXR Vulkan physical device '{}' supports {}, below required {}",
            physical_device_name(&physical_device_properties),
            format_vulkan_api_version(physical_device_properties.api_version),
            format_vulkan_api_version(vk_target_version)
        );
    }
    let physical_device_name = physical_device_name(&physical_device_properties);
    let physical_device_api_version =
        format_vulkan_api_version(physical_device_properties.api_version);
    let queue_family_index =
        unsafe { vk_instance.get_physical_device_queue_family_properties(vk_physical_device) }
            .into_iter()
            .enumerate()
            .find_map(|(index, info)| {
                info.queue_flags
                    .contains(vk::QueueFlags::GRAPHICS)
                    .then_some(index as u32)
            })
            .ok_or_else(|| anyhow!("OpenXR Vulkan physical device has no graphics queue family"))?;

    let queue_priorities = [1.0];
    let queue_infos = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&queue_priorities)];
    let vk_device_info = vk::DeviceCreateInfo::default().queue_create_infos(&queue_infos);
    let vk_device = {
        let get_instance_proc_addr =
            unsafe { std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr) };
        let raw = unsafe {
            xr_instance.create_vulkan_device(
                system,
                get_instance_proc_addr,
                vk_physical_device.as_raw() as _,
                &vk_device_info as *const _ as *const _,
            )
        }
        .context("XR error creating Vulkan device")?
        .map_err(vk::Result::from_raw)
        .map_err(|err| anyhow!("Vulkan error creating Vulkan device: {err:?}"))?;
        unsafe { ash::Device::load(vk_instance.fp_v1_0(), vk::Device::from_raw(raw as _)) }
    };

    let hal_instance = unsafe {
        wgpu::hal::vulkan::Instance::from_raw(
            vk_entry.clone(),
            vk_instance.clone(),
            vk_target_version,
            0,
            None,
            vec![],
            wgpu::InstanceFlags::empty(),
            false,
            Some(Box::new(|| {})),
        )
    }
    .context("wrap OpenXR Vulkan instance into wgpu-hal")?;
    let exposed_adapter = hal_instance
        .expose_adapter(vk_physical_device)
        .context("expose OpenXR Vulkan physical device to wgpu")?;
    let required_features = wgpu::Features::empty();
    let hal_device = unsafe {
        exposed_adapter.adapter.device_from_raw(
            vk_device,
            Some(Box::new(|| {})),
            &[],
            required_features,
            &wgpu::MemoryHints::Performance,
            queue_family_index,
            0,
        )
    }
    .context("wrap OpenXR Vulkan device into wgpu")?;

    let wgpu_instance = unsafe { wgpu::Instance::from_hal::<HalVulkan>(hal_instance) };
    let wgpu_adapter = unsafe { wgpu_instance.create_adapter_from_hal(exposed_adapter) };
    let (device, queue) = unsafe {
        wgpu_adapter.create_device_from_hal(
            hal_device,
            &wgpu::DeviceDescriptor {
                label: Some("mclone_android_xr_vulkan_device"),
                required_features,
                ..Default::default()
            },
        )
    }
    .context("create wgpu device from OpenXR Vulkan device")?;

    let session_vk_device = unsafe {
        device.as_hal::<HalVulkan, _, _>(|device| {
            device
                .expect("wgpu device is not backed by Vulkan")
                .raw_device()
                .handle()
                .as_raw()
        })
    };
    let (session, frame_wait, frame_stream) = unsafe {
        xr_instance.create_session::<AppGraphics>(
            system,
            &xr::vulkan::SessionCreateInfo {
                instance: vk_instance.handle().as_raw() as _,
                physical_device: vk_physical_device.as_raw() as _,
                device: session_vk_device as _,
                queue_family_index,
                queue_index: 0,
            },
        )
    }
    .context("create OpenXR Vulkan session")?;

    Ok(VulkanGraphicsSession {
        frame_wait,
        frame_stream,
        session,
        device,
        queue,
        physical_device_name,
        physical_device_api_version,
        queue_family_index,
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
    let vk_format = wgpu_format_to_vk_format(color_format)
        .ok_or_else(|| anyhow!("unsupported XR color format {color_format:?}"))?;
    let xr_format = vk_format.as_raw() as u32;
    let supported_formats = session
        .enumerate_swapchain_formats()
        .context("enumerate OpenXR Vulkan swapchain formats")?;
    if !supported_formats.contains(&xr_format) {
        bail!("OpenXR runtime does not advertise {vk_format:?} swapchain images");
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
        .context("create OpenXR Vulkan eye swapchain")?;

    let textures = swapchain
        .enumerate_images()
        .context("enumerate OpenXR Vulkan swapchain images")?
        .into_iter()
        .map(|vk_image_raw| {
            create_vulkan_swapchain_texture(
                device,
                vk_image_raw,
                eye_width,
                eye_height,
                color_format,
            )
        })
        .collect();
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
        textures,
        depth_view: depth_texture.create_view(&Default::default()),
        width: eye_width,
        height: eye_height,
    })
}

fn create_vulkan_swapchain_texture(
    device: &wgpu::Device,
    vk_image_raw: <AppGraphics as xr::Graphics>::SwapchainImage,
    width: u32,
    height: u32,
    color_format: wgpu::TextureFormat,
) -> wgpu::Texture {
    let hal_texture = unsafe {
        wgpu::hal::vulkan::Device::texture_from_raw(
            vk::Image::from_raw(vk_image_raw),
            &wgpu::hal::TextureDescriptor {
                label: Some("mclone_android_xr_swapchain"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: color_format,
                usage: wgpu::wgt::TextureUses::COLOR_TARGET | wgpu::wgt::TextureUses::COPY_SRC,
                memory_flags: wgpu::hal::MemoryFlags::empty(),
                view_formats: vec![],
            },
            None,
        )
    };
    unsafe {
        device.create_texture_from_hal::<HalVulkan>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("mclone_android_xr_swapchain"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
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

fn physical_device_name(properties: &vk::PhysicalDeviceProperties) -> String {
    properties
        .device_name_as_c_str()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "<invalid UTF-8 device name>".to_owned())
}

fn format_vulkan_api_version(version: u32) -> String {
    format!(
        "{}.{}.{}",
        vk::api_version_major(version),
        vk::api_version_minor(version),
        vk::api_version_patch(version)
    )
}

fn wgpu_format_to_vk_format(format: wgpu::TextureFormat) -> Option<vk::Format> {
    match format {
        wgpu::TextureFormat::Rgba8UnormSrgb => Some(vk::Format::R8G8B8A8_SRGB),
        wgpu::TextureFormat::Rgba8Unorm => Some(vk::Format::R8G8B8A8_UNORM),
        wgpu::TextureFormat::Bgra8UnormSrgb => Some(vk::Format::B8G8R8A8_SRGB),
        wgpu::TextureFormat::Bgra8Unorm => Some(vk::Format::B8G8R8A8_UNORM),
        _ => None,
    }
}
