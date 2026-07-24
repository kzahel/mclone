pub(crate) fn optional_gpu_features(adapter_features: wgpu::Features) -> wgpu::Features {
    let mut features = wgpu::Features::empty();
    if adapter_features.contains(wgpu::Features::TIMESTAMP_QUERY) {
        features |= wgpu::Features::TIMESTAMP_QUERY;
    }
    if adapter_features.contains(wgpu::Features::MULTIVIEW) {
        features |= wgpu::Features::MULTIVIEW;
    }
    if adapter_features.contains(wgpu::Features::MULTI_DRAW_INDIRECT) {
        features |= wgpu::Features::MULTI_DRAW_INDIRECT;
    }
    features
}

pub(crate) fn native_backends() -> wgpu::Backends {
    if let Some(backends) = wgpu::Backends::from_env() {
        return backends;
    }

    #[cfg(target_os = "windows")]
    {
        wgpu::Backends::DX12
    }

    #[cfg(target_vendor = "apple")]
    {
        wgpu::Backends::METAL
    }

    #[cfg(all(not(target_os = "windows"), not(target_vendor = "apple")))]
    {
        wgpu::Backends::PRIMARY
    }
}
