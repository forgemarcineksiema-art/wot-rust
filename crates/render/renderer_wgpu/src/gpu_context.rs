use renderer_api::RenderError;

/// A live GPU device + queue. Created headless (no surface) for offscreen rendering
/// and screenshots; the windowed client configures a surface against the same adapter.
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// What a context is being built FOR, where that changes what the device must be created with.
///
/// Empty by default on purpose: the shipped game asks for no optional feature on any adapter, and
/// every field here has to justify itself against that.
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuContextOptions {
    /// Ask for `TIMESTAMP_QUERY`, so a frame can report where its time went pass by pass.
    ///
    /// Off for the game. An optional feature has to be requested at device creation or it cannot
    /// be used later, so this is decided once, here, rather than discovered mid-frame. If the
    /// adapter cannot do it the request is dropped rather than failing the device — see
    /// `frame_profiler::required_features`.
    pub pass_timing: bool,
}

impl GpuContext {
    /// Create a context with no surface (offscreen / screenshots).
    pub fn headless() -> Result<Self, RenderError> {
        Self::new(wgpu::Instance::default(), None)
    }

    /// A headless context built for something in particular — today, per-pass timing.
    pub fn headless_with_options(options: GpuContextOptions) -> Result<Self, RenderError> {
        Self::new_with_options(wgpu::Instance::default(), None, options)
    }

    /// Create a context, optionally compatible with a presentation surface.
    pub fn new(
        instance: wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, RenderError> {
        Self::new_with_options(instance, compatible_surface, GpuContextOptions::default())
    }

    /// As [`Self::new`], with the device built for a stated purpose.
    pub fn new_with_options(
        instance: wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
        options: GpuContextOptions,
    ) -> Result<Self, RenderError> {
        let adapter = request_adapter(&instance, compatible_surface)?;
        // Downlevel baseline, with the 2D texture dimension lifted to what the adapter actually
        // offers (capped at 8k): the focused sun shadow map wants 4096 texels for wheel-scale
        // detail, and consumers clamp to `device.limits()` so weaker adapters still work.
        let mut required_limits = wgpu::Limits::downlevel_defaults();
        required_limits.max_texture_dimension_2d = adapter
            .limits()
            .max_texture_dimension_2d
            .clamp(required_limits.max_texture_dimension_2d, 8_192);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("wot_device"),
            // Empty unless the caller stated a purpose that needs more, and then only what this
            // adapter actually offers: an unavailable optional feature fails device creation
            // outright, so asking hopefully would break startup on the weakest machines.
            required_features: crate::frame_profiler::required_features(
                options.pass_timing,
                adapter.features(),
            ),
            required_limits,
            experimental_features: wgpu::ExperimentalFeatures::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| RenderError::new(format!("failed to create GPU device: {error}")))?;

        // Make the `GpuErrorPolicy` honest and give a lost device or an escaped GPU error a TRACE
        // instead of a silent black screen (the policy used to CLAIM a handler that did not exist).
        // A driver reset / Windows TDR fires the device-lost callback; a validation or out-of-memory
        // error that no error scope caught fires the uncaptured handler. Both LOG rather than abort —
        // a shipped game must not crash a player on a transient driver quirk; the surface loop
        // recovers, and a persistent failure keeps leaving a trail instead of vanishing.
        device.set_device_lost_callback(|reason, message| {
            tracing::error!(?reason, %message, "GPU device lost");
        });
        device.on_uncaptured_error(std::sync::Arc::new(|error: wgpu::Error| {
            tracing::error!(%error, "uncaptured GPU error");
        }));

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
