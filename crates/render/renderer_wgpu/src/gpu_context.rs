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
        // A surface means a player. The headless probes may fall back to a software rasterizer
        // (a screenshot on a machine with no GPU is still a screenshot), but a game that silently
        // runs on WARP at one frame a second is a lie about the machine it is on. Refuse, and
        // say why; `WOT_ALLOW_CPU_ADAPTER=1` is the dev escape for exactly that screenshot.
        let info = adapter.get_info();
        let allow_cpu =
            std::env::var("WOT_ALLOW_CPU_ADAPTER").is_ok_and(|value| value.trim() == "1");
        if compatible_surface.is_some() && !windowed_adapter_acceptable(info.device_type, allow_cpu)
        {
            return Err(RenderError::new(format!(
                "no hardware GPU: the only adapter is a software rasterizer ({}) — refusing to run \
                 the game on it (WOT_ALLOW_CPU_ADAPTER=1 overrides)",
                info.name
            )));
        }
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

/// Whether a WINDOWED context may run on this adapter: any hardware adapter, and a software
/// rasterizer only when the dev override says so. Pure, so the rule is tested without a GPU.
pub(crate) fn windowed_adapter_acceptable(device_type: wgpu::DeviceType, allow_cpu: bool) -> bool {
    allow_cpu || device_type != wgpu::DeviceType::Cpu
}

#[cfg(test)]
mod tests {
    use super::windowed_adapter_acceptable;

    /// The game never ships on a software rasterizer by accident: a CPU adapter is refused for
    /// a window unless the dev override is set, and every hardware class is accepted.
    #[test]
    fn a_window_refuses_a_software_rasterizer_unless_the_dev_override_says_so() {
        assert!(!windowed_adapter_acceptable(wgpu::DeviceType::Cpu, false));
        assert!(windowed_adapter_acceptable(wgpu::DeviceType::Cpu, true), "the dev escape");
        for hardware in [
            wgpu::DeviceType::IntegratedGpu,
            wgpu::DeviceType::DiscreteGpu,
            wgpu::DeviceType::VirtualGpu,
            wgpu::DeviceType::Other,
        ] {
            assert!(windowed_adapter_acceptable(hardware, false), "{hardware:?} is hardware");
        }
    }
}
