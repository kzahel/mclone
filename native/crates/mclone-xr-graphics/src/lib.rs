#![allow(unsafe_code)]

pub mod vulkan {
    use anyhow::{Context, Result, anyhow, bail};
    use ash::vk;
    use ash::vk::Handle as _;
    use openxr as xr;
    use std::ffi::CStr;
    use wgpu::hal::api::Vulkan as HalVulkan;

    pub type AppGraphics = xr::Vulkan;

    #[derive(Clone, Copy, Debug, Default)]
    pub struct VulkanMultiviewDiagnostics {
        pub instance_properties2_extension: bool,
        pub device_khr_multiview_extension: bool,
        pub raw_feature_multiview: bool,
        pub raw_feature_multiview_geometry_shader: bool,
        pub raw_feature_multiview_tessellation_shader: bool,
        pub raw_max_multiview_view_count: u32,
        pub raw_max_multiview_instance_index: u32,
        pub wgpu_adapter_multiview: bool,
        pub wgpu_device_multiview: bool,
    }

    pub struct VulkanGraphicsSession {
        pub frame_wait: xr::FrameWaiter,
        pub frame_stream: xr::FrameStream<AppGraphics>,
        pub session: xr::Session<AppGraphics>,
        pub device: wgpu::Device,
        pub queue: wgpu::Queue,
        pub physical_device_name: String,
        pub physical_device_api_version: String,
        pub queue_family_index: u32,
        pub multiview_diagnostics: VulkanMultiviewDiagnostics,
    }

    pub struct VulkanEyeSwapchain {
        swapchain: xr::Swapchain<AppGraphics>,
        #[allow(dead_code)]
        foveation_profile: Option<xr::FoveationProfileFB>,
        textures: Vec<wgpu::Texture>,
        width: u32,
        height: u32,
    }

    pub struct VulkanStereoSwapchain {
        swapchain: xr::Swapchain<AppGraphics>,
        #[allow(dead_code)]
        foveation_profile: Option<xr::FoveationProfileFB>,
        textures: Vec<wgpu::Texture>,
        width: u32,
        height: u32,
        array_size: u32,
    }

    impl VulkanEyeSwapchain {
        pub fn swapchain(&self) -> &xr::Swapchain<AppGraphics> {
            &self.swapchain
        }

        pub fn swapchain_mut(&mut self) -> &mut xr::Swapchain<AppGraphics> {
            &mut self.swapchain
        }

        pub fn textures(&self) -> &[wgpu::Texture] {
            &self.textures
        }

        pub fn width(&self) -> u32 {
            self.width
        }

        pub fn height(&self) -> u32 {
            self.height
        }
    }

    impl VulkanStereoSwapchain {
        pub fn swapchain(&self) -> &xr::Swapchain<AppGraphics> {
            &self.swapchain
        }

        pub fn swapchain_mut(&mut self) -> &mut xr::Swapchain<AppGraphics> {
            &mut self.swapchain
        }

        pub fn textures(&self) -> &[wgpu::Texture] {
            &self.textures
        }

        pub fn width(&self) -> u32 {
            self.width
        }

        pub fn height(&self) -> u32 {
            self.height
        }

        pub fn array_size(&self) -> u32 {
            self.array_size
        }
    }

    impl mclone_xr_host::XrEyeSwapchain<AppGraphics> for VulkanEyeSwapchain {
        fn swapchain(&self) -> &xr::Swapchain<AppGraphics> {
            self.swapchain()
        }

        fn swapchain_mut(&mut self) -> &mut xr::Swapchain<AppGraphics> {
            self.swapchain_mut()
        }

        fn textures(&self) -> &[wgpu::Texture] {
            self.textures()
        }

        fn width(&self) -> u32 {
            self.width()
        }

        fn height(&self) -> u32 {
            self.height()
        }
    }

    impl mclone_xr_host::XrStereoSwapchain<AppGraphics> for VulkanStereoSwapchain {
        fn swapchain(&self) -> &xr::Swapchain<AppGraphics> {
            self.swapchain()
        }

        fn swapchain_mut(&mut self) -> &mut xr::Swapchain<AppGraphics> {
            self.swapchain_mut()
        }

        fn textures(&self) -> &[wgpu::Texture] {
            self.textures()
        }

        fn width(&self) -> u32 {
            self.width()
        }

        fn height(&self) -> u32 {
            self.height()
        }

        fn array_size(&self) -> u32 {
            self.array_size()
        }
    }

    pub fn create_graphics_session(
        xr_instance: &xr::Instance,
        system: xr::SystemId,
        device_label: &'static str,
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

        let vk_entry = load_vulkan_entry()?;
        let enabled_instance_extensions = openxr_wgpu_instance_extensions(&vk_entry)?;
        let enabled_instance_extension_names = enabled_instance_extensions
            .iter()
            .map(|name| name.as_ptr())
            .collect::<Vec<_>>();
        let instance_properties2_extension =
            enabled_instance_extensions.contains(&vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_NAME);
        let vk_app_info = vk::ApplicationInfo::default()
            .application_name(c"mclone")
            .application_version(1)
            .engine_name(c"mclone")
            .engine_version(1)
            .api_version(vk_target_version);
        let vk_instance_info = vk::InstanceCreateInfo::default()
            .application_info(&vk_app_info)
            .enabled_extension_names(&enabled_instance_extension_names);

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
        let mut multiview_diagnostics = query_multiview_diagnostics(
            &vk_instance,
            vk_physical_device,
            instance_properties2_extension,
        )?;

        let queue_family_index =
            unsafe { vk_instance.get_physical_device_queue_family_properties(vk_physical_device) }
                .into_iter()
                .enumerate()
                .find_map(|(index, info)| {
                    info.queue_flags
                        .contains(vk::QueueFlags::GRAPHICS)
                        .then_some(index as u32)
                })
                .ok_or_else(|| {
                    anyhow!("OpenXR Vulkan physical device has no graphics queue family")
                })?;

        let hal_instance = unsafe {
            wgpu::hal::vulkan::Instance::from_raw(
                vk_entry.clone(),
                vk_instance.clone(),
                vk_target_version,
                0,
                None,
                enabled_instance_extensions,
                wgpu::InstanceFlags::empty(),
                false,
                Some(Box::new(|| {})),
            )
        }
        .context("wrap OpenXR Vulkan instance into wgpu-hal")?;

        let exposed_adapter = hal_instance
            .expose_adapter(vk_physical_device)
            .context("expose OpenXR Vulkan physical device to wgpu")?;
        multiview_diagnostics.wgpu_adapter_multiview =
            exposed_adapter.features.contains(wgpu::Features::MULTIVIEW);
        let required_features = optional_openxr_wgpu_features(exposed_adapter.features);
        let mut enabled_extensions = exposed_adapter
            .adapter
            .required_device_extensions(required_features);
        ensure_timeline_semaphore_extension_for_vulkan_1_1(
            &vk_instance,
            vk_physical_device,
            vk_target_version,
            &mut enabled_extensions,
        )?;
        if required_features.contains(wgpu::Features::MULTIVIEW)
            && multiview_diagnostics.device_khr_multiview_extension
            && !enabled_extensions.contains(&vk::KHR_MULTIVIEW_NAME)
        {
            enabled_extensions.push(vk::KHR_MULTIVIEW_NAME);
        }
        let enabled_extension_names = enabled_extensions
            .iter()
            .map(|name| name.as_ptr())
            .collect::<Vec<_>>();
        let mut physical_device_features = exposed_adapter
            .adapter
            .physical_device_features(&enabled_extensions, required_features);

        let queue_priorities = [1.0];
        let queue_infos = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities)];
        let vk_device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&enabled_extension_names);
        let vk_device_info = physical_device_features.add_to_device_create(vk_device_info);
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

        let hal_device = unsafe {
            exposed_adapter.adapter.device_from_raw(
                vk_device,
                Some(Box::new(|| {})),
                &enabled_extensions,
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
                    label: Some(device_label),
                    required_features,
                    ..Default::default()
                },
            )
        }
        .context("create wgpu device from OpenXR Vulkan device")?;
        multiview_diagnostics.wgpu_device_multiview =
            device.features().contains(wgpu::Features::MULTIVIEW);

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
            multiview_diagnostics,
        })
    }

    fn openxr_wgpu_instance_extensions(vk_entry: &ash::Entry) -> Result<Vec<&'static CStr>> {
        let available = unsafe { vk_entry.enumerate_instance_extension_properties(None) }
            .context("enumerate Vulkan instance extensions")?;
        let mut extensions = Vec::new();
        if extension_properties_contain(&available, vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_NAME) {
            extensions.push(vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_NAME);
        }
        Ok(extensions)
    }

    fn query_multiview_diagnostics(
        vk_instance: &ash::Instance,
        vk_physical_device: vk::PhysicalDevice,
        instance_properties2_extension: bool,
    ) -> Result<VulkanMultiviewDiagnostics> {
        let device_extensions =
            unsafe { vk_instance.enumerate_device_extension_properties(vk_physical_device) }
                .context("enumerate OpenXR Vulkan device extensions")?;
        let device_khr_multiview_extension =
            extension_properties_contain(&device_extensions, vk::KHR_MULTIVIEW_NAME);

        let mut multiview_features = vk::PhysicalDeviceMultiviewFeatures::default();
        let mut features2 =
            vk::PhysicalDeviceFeatures2::default().push_next(&mut multiview_features);
        unsafe { vk_instance.get_physical_device_features2(vk_physical_device, &mut features2) };

        let mut multiview_properties = vk::PhysicalDeviceMultiviewProperties::default();
        let mut properties2 =
            vk::PhysicalDeviceProperties2::default().push_next(&mut multiview_properties);
        unsafe {
            vk_instance.get_physical_device_properties2(vk_physical_device, &mut properties2)
        };

        Ok(VulkanMultiviewDiagnostics {
            instance_properties2_extension,
            device_khr_multiview_extension,
            raw_feature_multiview: multiview_features.multiview != 0,
            raw_feature_multiview_geometry_shader: multiview_features.multiview_geometry_shader
                != 0,
            raw_feature_multiview_tessellation_shader: multiview_features
                .multiview_tessellation_shader
                != 0,
            raw_max_multiview_view_count: multiview_properties.max_multiview_view_count,
            raw_max_multiview_instance_index: multiview_properties.max_multiview_instance_index,
            ..Default::default()
        })
    }

    fn extension_properties_contain(extensions: &[vk::ExtensionProperties], name: &CStr) -> bool {
        extensions
            .iter()
            .any(|extension| matches!(extension.extension_name_as_c_str(), Ok(extension_name) if extension_name == name))
    }

    fn optional_openxr_wgpu_features(adapter_features: wgpu::Features) -> wgpu::Features {
        let mut features = wgpu::Features::empty();
        if adapter_features.contains(wgpu::Features::MULTIVIEW) {
            features |= wgpu::Features::MULTIVIEW;
        }
        features
    }

    fn ensure_timeline_semaphore_extension_for_vulkan_1_1(
        vk_instance: &ash::Instance,
        vk_physical_device: vk::PhysicalDevice,
        vk_target_version: u32,
        enabled_extensions: &mut Vec<&'static CStr>,
    ) -> Result<()> {
        if vk_target_version >= vk::API_VERSION_1_2
            || enabled_extensions.contains(&vk::KHR_TIMELINE_SEMAPHORE_NAME)
        {
            return Ok(());
        }

        let available_extensions =
            unsafe { vk_instance.enumerate_device_extension_properties(vk_physical_device) }
                .context("enumerate OpenXR Vulkan device extensions for timeline semaphore")?;
        if extension_properties_contain(&available_extensions, vk::KHR_TIMELINE_SEMAPHORE_NAME) {
            // wgpu-hal classifies promoted features from the physical device's
            // API version. The OpenXR instance intentionally targets Vulkan
            // 1.1, though, so a loader may expose only the KHR entry points.
            // Listing the extension makes wgpu-hal use those entry points
            // instead of calling absent Vulkan 1.2 core functions.
            enabled_extensions.push(vk::KHR_TIMELINE_SEMAPHORE_NAME);
        }
        Ok(())
    }

    pub fn create_eye_swapchain(
        device: &wgpu::Device,
        session: &xr::Session<AppGraphics>,
        eye_width: u32,
        eye_height: u32,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        texture_label: &'static str,
        foveation: Option<xr::FoveationLevelProfile>,
    ) -> Result<VulkanEyeSwapchain> {
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
        let foveation_profile = create_and_apply_foveation_profile(
            session,
            &swapchain,
            foveation,
            "OpenXR Vulkan eye swapchain",
        )?;

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
                    1,
                    color_format,
                    texture_label,
                )
            })
            .collect();

        Ok(VulkanEyeSwapchain {
            swapchain,
            foveation_profile,
            textures,
            width: eye_width,
            height: eye_height,
        })
    }

    pub fn create_stereo_swapchain(
        device: &wgpu::Device,
        session: &xr::Session<AppGraphics>,
        eye_width: u32,
        eye_height: u32,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        texture_label: &'static str,
        foveation: Option<xr::FoveationLevelProfile>,
    ) -> Result<VulkanStereoSwapchain> {
        let vk_format = wgpu_format_to_vk_format(color_format)
            .ok_or_else(|| anyhow!("unsupported XR color format {color_format:?}"))?;
        let xr_format = vk_format.as_raw() as u32;
        let supported_formats = session
            .enumerate_swapchain_formats()
            .context("enumerate OpenXR Vulkan swapchain formats")?;
        if !supported_formats.contains(&xr_format) {
            bail!("OpenXR runtime does not advertise {vk_format:?} swapchain images");
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
            .context("create OpenXR Vulkan stereo array swapchain")?;
        let foveation_profile = create_and_apply_foveation_profile(
            session,
            &swapchain,
            foveation,
            "OpenXR Vulkan stereo array swapchain",
        )?;

        let textures = swapchain
            .enumerate_images()
            .context("enumerate OpenXR Vulkan stereo array swapchain images")?
            .into_iter()
            .map(|vk_image_raw| {
                create_vulkan_swapchain_texture(
                    device,
                    vk_image_raw,
                    eye_width,
                    eye_height,
                    array_size,
                    color_format,
                    texture_label,
                )
            })
            .collect();

        Ok(VulkanStereoSwapchain {
            swapchain,
            foveation_profile,
            textures,
            width: eye_width,
            height: eye_height,
            array_size,
        })
    }

    fn create_and_apply_foveation_profile(
        session: &xr::Session<AppGraphics>,
        swapchain: &xr::Swapchain<AppGraphics>,
        foveation: Option<xr::FoveationLevelProfile>,
        label: &str,
    ) -> Result<Option<xr::FoveationProfileFB>> {
        let Some(foveation) = foveation else {
            return Ok(None);
        };
        let profile = session
            .create_foveation_profile(Some(foveation))
            .context("create OpenXR foveation profile")?;
        apply_foveation_profile(swapchain, &profile, label)?;
        Ok(Some(profile))
    }

    fn apply_foveation_profile(
        swapchain: &xr::Swapchain<AppGraphics>,
        profile: &xr::FoveationProfileFB,
        label: &str,
    ) -> Result<()> {
        let fp = swapchain
            .instance()
            .exts()
            .fb_swapchain_update_state
            .as_ref()
            .ok_or_else(|| anyhow!("OpenXR XR_FB_swapchain_update_state is not enabled"))?;
        let state = xr::sys::SwapchainStateFoveationFB {
            ty: xr::sys::SwapchainStateFoveationFB::TYPE,
            next: std::ptr::null_mut(),
            flags: xr::SwapchainStateFoveationFlagsFB::EMPTY,
            profile: profile.as_raw(),
        };
        let result = unsafe {
            (fp.update_swapchain)(
                swapchain.as_raw(),
                &state as *const _ as *const xr::sys::SwapchainStateBaseHeaderFB,
            )
        };
        if result.into_raw() < 0 {
            bail!("apply foveation profile to {label} failed: {result:?}");
        }
        Ok(())
    }

    fn load_vulkan_entry() -> Result<ash::Entry> {
        unsafe { ash::Entry::load().context("load Vulkan loader") }
    }

    fn create_vulkan_swapchain_texture(
        device: &wgpu::Device,
        vk_image_raw: <AppGraphics as xr::Graphics>::SwapchainImage,
        width: u32,
        height: u32,
        array_layers: u32,
        color_format: wgpu::TextureFormat,
        texture_label: &'static str,
    ) -> wgpu::Texture {
        let hal_texture = unsafe {
            wgpu::hal::vulkan::Device::texture_from_raw(
                vk::Image::from_raw(vk_image_raw),
                &wgpu::hal::TextureDescriptor {
                    label: Some(texture_label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: array_layers,
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
                    label: Some(texture_label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: array_layers,
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
}
