use game_core::{DamageComponentId, DamageComponentKind, DamageLayout, DamageShape};
use glam::Vec3;

#[derive(Clone, Copy)]
pub(crate) struct ObbAnchor {
    pub center: Vec3,
    pub half: Vec3,
}

#[derive(Clone, Copy)]
pub(crate) struct CylinderAnchor {
    pub center: Vec3,
    pub axis: Vec3,
    pub half_length: f32,
    pub radius: f32,
}

#[derive(Clone, Copy)]
pub(super) struct CapsuleAnchor {
    pub a: Vec3,
    pub b: Vec3,
}

pub(crate) fn obb_anchor(
    layout: &DamageLayout,
    kind: DamageComponentKind,
    center_y: f32,
) -> ObbAnchor {
    layout
        .components()
        .iter()
        .find_map(|component| match (&component.shape, component.kind == kind) {
            (DamageShape::Obb { center, half_extents, .. }, true) => {
                Some(ObbAnchor { center: *center + Vec3::Y * center_y, half: *half_extents })
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing T-54 {kind:?} OBB anchor"))
}

/// Anchor for kinds that own several volumes (the three ammunition racks): the id picks the
/// physical rack, so each visual group hangs off its own authoritative box.
pub(super) fn obb_anchor_by_id(
    layout: &DamageLayout,
    id: DamageComponentId,
    center_y: f32,
) -> ObbAnchor {
    layout
        .components()
        .iter()
        .find_map(|component| match (&component.shape, component.id == id) {
            (DamageShape::Obb { center, half_extents, .. }, true) => {
                Some(ObbAnchor { center: *center + Vec3::Y * center_y, half: *half_extents })
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing T-54 component {id:?} OBB anchor"))
}

pub(crate) fn cylinder_anchors(
    layout: &DamageLayout,
    kind: DamageComponentKind,
    center_y: f32,
) -> Vec<CylinderAnchor> {
    layout
        .components()
        .iter()
        .filter_map(|component| match (&component.shape, component.kind == kind) {
            (DamageShape::Cylinder { center, axis, half_length, radius }, true) => {
                Some(CylinderAnchor {
                    center: *center + Vec3::Y * center_y,
                    axis: *axis,
                    half_length: *half_length,
                    radius: *radius,
                })
            }
            _ => None,
        })
        .collect()
}

pub(super) fn capsule_anchors(
    layout: &DamageLayout,
    kind: DamageComponentKind,
    center_y: f32,
) -> Vec<CapsuleAnchor> {
    layout
        .components()
        .iter()
        .filter_map(|component| match (&component.shape, component.kind == kind) {
            (DamageShape::Capsule { a, b, .. }, true) => {
                Some(CapsuleAnchor { a: *a + Vec3::Y * center_y, b: *b + Vec3::Y * center_y })
            }
            _ => None,
        })
        .collect()
}
