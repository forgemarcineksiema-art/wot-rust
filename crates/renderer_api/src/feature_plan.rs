use crate::{RenderAdapterReport, RenderCapabilityTier};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderFeature {
    ForwardPlus,
    Deferred,
    ShadowMaps,
    TerrainChunks,
    Instancing,
    Lod,
    FrustumCulling,
    OcclusionCulling,
    Particles,
    PostProcess,
    Ui,
    DebugDraw,
    RayTracing,
    MeshShaders,
    BindlessResources,
    GpuDrivenRendering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackReason {
    UnsupportedFeature,
    ExperimentalOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureFallback {
    pub requested: RenderFeature,
    pub fallback: RenderFeature,
    pub reason: FallbackReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderFeaturePlan {
    pub enabled: Vec<RenderFeature>,
    pub fallbacks: Vec<FeatureFallback>,
}

impl RenderFeaturePlan {
    pub fn baseline() -> Self {
        Self {
            enabled: vec![
                RenderFeature::ForwardPlus,
                RenderFeature::ShadowMaps,
                RenderFeature::TerrainChunks,
                RenderFeature::Instancing,
                RenderFeature::Lod,
                RenderFeature::FrustumCulling,
                RenderFeature::OcclusionCulling,
                RenderFeature::Particles,
                RenderFeature::PostProcess,
                RenderFeature::Ui,
                RenderFeature::DebugDraw,
            ],
            fallbacks: Vec::new(),
        }
    }

    pub fn has_feature(&self, feature: RenderFeature) -> bool {
        self.enabled.contains(&feature)
    }

    pub fn fallback_for(&self, feature: RenderFeature) -> Option<RenderFeature> {
        self.fallbacks
            .iter()
            .find(|fallback| fallback.requested == feature)
            .map(|fallback| fallback.fallback)
    }

    fn enable_once(&mut self, feature: RenderFeature) {
        if !self.has_feature(feature) {
            self.enabled.push(feature);
        }
    }

    fn add_fallback(&mut self, requested: RenderFeature, fallback: RenderFeature) {
        self.fallbacks.push(FeatureFallback {
            requested,
            fallback,
            reason: FallbackReason::ExperimentalOnly,
        });
    }
}

pub fn select_render_feature_plan(
    report: &RenderAdapterReport,
    requested_features: &[RenderFeature],
) -> RenderFeaturePlan {
    let mut plan = RenderFeaturePlan::baseline();

    for feature in requested_features {
        match *feature {
            RenderFeature::RayTracing if supports(report, "experimental:ray-query") => {
                plan.enable_once(RenderFeature::RayTracing);
            }
            RenderFeature::MeshShaders if supports(report, "experimental:mesh-shader") => {
                plan.enable_once(RenderFeature::MeshShaders);
            }
            RenderFeature::BindlessResources
                if supports(report, "texture-binding-array")
                    && supports(report, "storage-resource-binding-array")
                    && supports(report, "partially-bound-binding-array")
                    && report.capability_tier == RenderCapabilityTier::Experimental =>
            {
                plan.enable_once(RenderFeature::BindlessResources);
            }
            RenderFeature::GpuDrivenRendering
                if supports(report, "experimental:mesh-shader")
                    && report.capability_tier == RenderCapabilityTier::Experimental =>
            {
                plan.enable_once(RenderFeature::GpuDrivenRendering);
            }
            RenderFeature::RayTracing => plan.add_fallback(*feature, RenderFeature::ShadowMaps),
            RenderFeature::MeshShaders => plan.add_fallback(*feature, RenderFeature::Instancing),
            RenderFeature::BindlessResources => {
                plan.add_fallback(*feature, RenderFeature::ForwardPlus);
            }
            RenderFeature::GpuDrivenRendering => {
                plan.add_fallback(*feature, RenderFeature::FrustumCulling);
            }
            other => plan.enable_once(other),
        }
    }

    plan
}

fn supports(report: &RenderAdapterReport, feature_name: &str) -> bool {
    report.feature_names.iter().any(|feature| feature == feature_name)
}
