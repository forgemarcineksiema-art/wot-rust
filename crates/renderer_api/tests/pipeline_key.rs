use std::collections::HashSet;

use renderer_api::{
    AlphaMode, ColorFormat, DEFAULT_MSAA_SAMPLES, DepthFormat, MaterialPipelineFlags,
    PipelineCacheMode, PipelineKey, PipelineWarmupPlan, ShaderHandle, VertexLayoutKey,
};

#[test]
fn pipeline_key_records_full_render_state() {
    let key = PipelineKey {
        shader: ShaderHandle(7),
        vertex_layout: VertexLayoutKey::StaticMesh,
        material_flags: MaterialPipelineFlags::ALBEDO_TEXTURED,
        color_format: ColorFormat::Bgra8UnormSrgb,
        depth_format: Some(DepthFormat::Depth32Float),
        msaa_samples: 4,
        skinning_enabled: false,
        alpha_mode: AlphaMode::Opaque,
    };

    assert_eq!(key.shader, ShaderHandle(7));
    assert_eq!(key.vertex_layout, VertexLayoutKey::StaticMesh);
    assert_eq!(key.material_flags, MaterialPipelineFlags::ALBEDO_TEXTURED);
    assert_eq!(key.color_format, ColorFormat::Bgra8UnormSrgb);
    assert_eq!(key.depth_format, Some(DepthFormat::Depth32Float));
    assert_eq!(key.msaa_samples, 4);
    assert!(!key.skinning_enabled);
    assert_eq!(key.alpha_mode, AlphaMode::Opaque);
}

#[test]
fn baseline_tank_pipeline_uses_default_msaa() {
    let key = PipelineKey::tank_baseline(AlphaMode::Opaque);

    assert_eq!(key.msaa_samples, DEFAULT_MSAA_SAMPLES);
}

#[test]
fn pipeline_key_hash_includes_material_and_alpha_variants() {
    let opaque = PipelineKey::tank_baseline(AlphaMode::Opaque);
    let masked = PipelineKey::tank_baseline(AlphaMode::Masked);

    let mut keys = HashSet::new();
    keys.insert(opaque);
    keys.insert(masked);

    assert_eq!(keys.len(), 2);
}

#[test]
fn warmup_plan_is_explicit_and_deduplicated() {
    let plan = PipelineWarmupPlan::new(
        PipelineCacheMode::ReleasePrewarm,
        [
            PipelineKey::tank_baseline(AlphaMode::Opaque),
            PipelineKey::tank_baseline(AlphaMode::Opaque),
            PipelineKey::tank_baseline(AlphaMode::Masked),
        ],
    );

    assert_eq!(plan.mode, PipelineCacheMode::ReleasePrewarm);
    assert_eq!(plan.keys().len(), 2);
    assert!(plan.keys().contains(&PipelineKey::tank_baseline(AlphaMode::Opaque)));
    assert!(plan.keys().contains(&PipelineKey::tank_baseline(AlphaMode::Masked)));
}

#[test]
fn baseline_warmup_plan_contains_predictable_tank_variants() {
    let plan = PipelineWarmupPlan::baseline(PipelineCacheMode::ReleasePrewarm);

    assert!(plan.keys().contains(&PipelineKey::tank_baseline(AlphaMode::Opaque)));
    assert!(plan.keys().contains(&PipelineKey::tank_baseline(AlphaMode::Masked)));
    assert!(!plan.keys().contains(&PipelineKey::tank_baseline(AlphaMode::Blend)));
}
