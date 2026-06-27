use renderer_api::RenderError;

/// A live GPU device + queue. Created headless (no surface) for offscreen rendering
/// and screenshots; the windowed client configures a surface against the same adapter.
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuContext {
    /// Create a context with no surface (offscreen / screenshots).
    pub fn headless() -> Result<Self, RenderError> {
        Self::new(wgpu::Instance::default(), None)
    }

    /// Create a context, optionally compatible with a presentation surface.
    pub fn new(
        instance: wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, RenderError> {
        let adapter = request_adapter(&instance, compatible_surface)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("wot_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| RenderError::new(format!("failed to create GPU device: {error}")))?;

        Ok(Self { instance, adapter, device, queue })
    }
}

fn request_adapter(
    instance: &wgpu::Instance,
    compatible_surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, RenderError> {
    let primary = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface,
    }));
    if let Ok(adapter) = primary {
        return Ok(adapter);
    }
    // Fall back to any adapter, including a software rasterizer (e.g. DX12 WARP), so
    // headless screenshots work on machines without a discrete/integrated GPU.
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        force_fallback_adapter: true,
        compatible_surface,
    }))
    .map_err(|error| RenderError::new(format!("no usable GPU adapter: {error}")))
}
