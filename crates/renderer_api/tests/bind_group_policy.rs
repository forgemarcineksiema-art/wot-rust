use renderer_api::{
    BindGroupRole, RenderFeature, RenderFeaturePlan, TextureBindingStrategy,
    baseline_bind_group_layout, baseline_binding_policy, binding_policy_for_feature_plan,
};

#[test]
fn baseline_bind_groups_use_four_stable_slots() {
    let slots: Vec<_> =
        baseline_bind_group_layout().iter().map(|slot| (slot.index, slot.role)).collect();

    assert_eq!(
        slots,
        vec![
            (0, BindGroupRole::FrameGlobal),
            (1, BindGroupRole::CameraView),
            (2, BindGroupRole::Material),
            (3, BindGroupRole::ObjectInstance),
        ]
    );
}

#[test]
fn baseline_policy_uses_bounded_material_textures_not_bindless() {
    let policy = baseline_binding_policy();

    assert_eq!(policy.texture_strategy, TextureBindingStrategy::BoundedMaterialSet);
    assert!(!policy.uses_bindless_resources());
    assert!(policy.slots().iter().all(|slot| slot.index <= 3));
}

#[test]
fn bindless_policy_requires_bindless_feature_plan() {
    let baseline_policy = binding_policy_for_feature_plan(&RenderFeaturePlan::baseline());

    assert_eq!(baseline_policy.texture_strategy, TextureBindingStrategy::BoundedMaterialSet);

    let bindless_plan = RenderFeaturePlan {
        enabled: vec![RenderFeature::ForwardPlus, RenderFeature::BindlessResources],
        fallbacks: Vec::new(),
    };
    let bindless_policy = binding_policy_for_feature_plan(&bindless_plan);

    assert_eq!(bindless_policy.texture_strategy, TextureBindingStrategy::BindlessArray);
    assert!(bindless_policy.uses_bindless_resources());
}
