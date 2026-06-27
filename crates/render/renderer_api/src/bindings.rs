use crate::{RenderFeature, RenderFeaturePlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindGroupRole {
    FrameGlobal,
    CameraView,
    Material,
    ObjectInstance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindGroupSlot {
    pub index: u32,
    pub role: BindGroupRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureBindingStrategy {
    BoundedMaterialSet,
    BindlessArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererBindingPolicy {
    slots: &'static [BindGroupSlot],
    pub texture_strategy: TextureBindingStrategy,
}

impl RendererBindingPolicy {
    pub fn slots(&self) -> &'static [BindGroupSlot] {
        self.slots
    }

    pub fn uses_bindless_resources(&self) -> bool {
        self.texture_strategy == TextureBindingStrategy::BindlessArray
    }
}

const BASELINE_BIND_GROUP_LAYOUT: [BindGroupSlot; 4] = [
    BindGroupSlot { index: 0, role: BindGroupRole::FrameGlobal },
    BindGroupSlot { index: 1, role: BindGroupRole::CameraView },
    BindGroupSlot { index: 2, role: BindGroupRole::Material },
    BindGroupSlot { index: 3, role: BindGroupRole::ObjectInstance },
];

pub fn baseline_bind_group_layout() -> &'static [BindGroupSlot] {
    &BASELINE_BIND_GROUP_LAYOUT
}

pub fn baseline_binding_policy() -> RendererBindingPolicy {
    RendererBindingPolicy {
        slots: baseline_bind_group_layout(),
        texture_strategy: TextureBindingStrategy::BoundedMaterialSet,
    }
}

pub fn binding_policy_for_feature_plan(plan: &RenderFeaturePlan) -> RendererBindingPolicy {
    let texture_strategy = if plan.has_feature(RenderFeature::BindlessResources) {
        TextureBindingStrategy::BindlessArray
    } else {
        TextureBindingStrategy::BoundedMaterialSet
    };

    RendererBindingPolicy { slots: baseline_bind_group_layout(), texture_strategy }
}
