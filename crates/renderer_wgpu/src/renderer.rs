use renderer_api::{
    PipelineCacheMode, PipelineKey, PipelineWarmupPlan, RenderAdapterReport, RenderBackend,
    RenderError, RenderFeaturePlan, RenderFrame, RenderLimitProfile, RenderSettings,
    select_render_feature_plan,
};

use crate::{
    PipelineRegistry, basic_tank_shader_source, probe_startup_report, request_limits_for_profile,
};

pub struct WgpuRenderer {
    _instance: wgpu::Instance,
    settings: RenderSettings,
    requested_limits: wgpu::Limits,
    startup_report: Option<RenderAdapterReport>,
    feature_plan: RenderFeaturePlan,
    pipeline_registry: PipelineRegistry,
}

impl WgpuRenderer {
    pub fn new(settings: RenderSettings) -> Self {
        Self::new_with_limit_profile(settings, settings.limit_profile)
    }

    pub fn new_with_limit_profile(
        mut settings: RenderSettings,
        limit_profile: RenderLimitProfile,
    ) -> Self {
        settings.limit_profile = limit_profile;
        let instance = wgpu::Instance::default();
        let requested_limits = request_limits_for_profile(limit_profile);
        let startup_report = probe_startup_report(&instance).ok();
        let feature_plan = startup_report
            .as_ref()
            .map(|report| select_render_feature_plan(report, &[]))
            .unwrap_or_else(RenderFeaturePlan::baseline);
        let mut pipeline_registry = PipelineRegistry::default();
        pipeline_registry
            .prewarm(PipelineWarmupPlan::baseline(cache_mode()).keys().iter().copied());
        Self {
            _instance: instance,
            settings,
            requested_limits,
            startup_report,
            feature_plan,
            pipeline_registry,
        }
    }

    pub fn settings(&self) -> RenderSettings {
        self.settings
    }

    pub fn limit_profile(&self) -> RenderLimitProfile {
        self.settings.limit_profile
    }

    pub fn requested_limits(&self) -> &wgpu::Limits {
        &self.requested_limits
    }

    pub fn feature_plan(&self) -> &RenderFeaturePlan {
        &self.feature_plan
    }

    pub fn startup_report(&self) -> Option<&RenderAdapterReport> {
        self.startup_report.as_ref()
    }

    pub fn pipeline_registry(&self) -> &PipelineRegistry {
        &self.pipeline_registry
    }

    pub fn require_pipeline_for_draw(&self, key: &PipelineKey) -> Result<(), RenderError> {
        self.pipeline_registry.require_for_draw(key)
    }

    pub fn shader_source() -> &'static str {
        basic_tank_shader_source()
    }
}

impl RenderBackend for WgpuRenderer {
    fn name(&self) -> &'static str {
        "wgpu"
    }

    fn adapter_report(&self) -> Option<&RenderAdapterReport> {
        self.startup_report()
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.settings.width = width.max(1);
        self.settings.height = height.max(1);
    }

    fn render_frame(&mut self, _frame: &RenderFrame) -> Result<(), RenderError> {
        Ok(())
    }
}

fn cache_mode() -> PipelineCacheMode {
    if cfg!(debug_assertions) {
        PipelineCacheMode::DevelopmentHotReload
    } else {
        PipelineCacheMode::ReleasePrewarm
    }
}
