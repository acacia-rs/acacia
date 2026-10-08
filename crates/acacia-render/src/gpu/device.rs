//! Opens the GPU device and configures the surface.

use crate::Error;

pub(super) struct Gpu {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
}

pub(super) fn open(target: impl Into<wgpu::SurfaceTarget<'static>>, (width, height): (u32, u32)) -> Result<Gpu, Error> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let surface = instance.create_surface(target).map_err(|e| Error::Surface(e.to_string()))?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        ..Default::default()
    }))
    .map_err(|e| Error::Adapter(e.to_string()))?;
    tracing::info!(adapter = ?adapter.get_info().name, backend = ?adapter.get_info().backend, "gpu");
    let limits = adapter.limits();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("acacia"),
        required_limits: wgpu::Limits {
            max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
            max_buffer_size: limits.max_buffer_size,
            max_texture_array_layers: limits.max_texture_array_layers,
            ..wgpu::Limits::default()
        },
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        ..Default::default()
    }))
    .map_err(|e| Error::Device(e.to_string()))?;
    let mut config = surface
        .get_default_config(&adapter, width.max(1), height.max(1))
        .ok_or_else(|| Error::Surface("surface unsupported by adapter".into()))?;
    config.format = config.format.add_srgb_suffix();
    // The UI pass draws through a plain view of the same texture: both games blend UI in gamma space.
    config.view_formats = vec![config.format.remove_srgb_suffix()];
    if surface.get_capabilities(&adapter).usages.contains(wgpu::TextureUsages::COPY_SRC) {
        config.usage |= wgpu::TextureUsages::COPY_SRC;
    }
    config.present_mode = wgpu::PresentMode::AutoVsync;
    surface.configure(&device, &config);
    Ok(Gpu { surface, device, queue, config })
}
