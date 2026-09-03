//! Shared mapping from running-gear placements to render objects. Both the geometry and the PBR
//! vehicle paths cache the four unit meshes (road wheel, idler, sprocket, shoe link) per
//! vehicle and instance them here, so the wheels spin and the track links scroll from the per-side
//! track distance the presentation world accumulates.

use game_core::TankId;
use glam::Mat4;
use renderer_api::{MaterialHandle, MeshHandle, RenderObject};
use vehicle_geometry::{
    GearDetail, GearDynamics, GearPart, GearPlacement, RunningGearKinematics,
    running_gear_placements_dynamic,
};

/// Cached unit-mesh handles for one vehicle's animatable running gear, at one detail tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GearMeshSet {
    pub road_wheel: MeshHandle,
    pub idler: MeshHandle,
    pub sprocket: MeshHandle,
    pub link: MeshHandle,
    pub swing_arm: MeshHandle,
    /// The left-hand arm: mirrored geometry, because the torsion boss faces the hull on both
    /// sides and an arm is the one gear part a rotation cannot turn around.
    pub swing_arm_left: MeshHandle,
    pub return_roller: MeshHandle,
    /// The lever shock absorbers at the blueprint's damped stations (and the left mirror).
    pub damper: MeshHandle,
    pub damper_left: MeshHandle,
}

/// Both detail tiers of one vehicle's running gear, registered once at load.
///
/// The gear is the largest body of geometry on a vehicle — 38.6k triangles across 204 instances
/// on a T-54 — and it used to be drawn at full detail on every tank on the field regardless of
/// range. Keeping both tiers resident costs one extra mesh set per vehicle kind and saves
/// 47-61% of the gear's triangles on everything past `vehicle_geometry::GEAR_DETAIL_SWITCH_M`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GearMeshHandles {
    pub near: GearMeshSet,
    pub far: GearMeshSet,
}

impl GearMeshHandles {
    fn set(&self, detail: GearDetail) -> &GearMeshSet {
        match detail {
            GearDetail::Near => &self.near,
            GearDetail::Far => &self.far,
        }
    }
}

/// Emit one render object per moving running-gear part, instanced from the cached unit meshes and
/// placed in the world by `hull_transform * placement`. `left_m`/`right_m` are the per-side track
/// distances; at `0.0` the gear sits at its rest pose (used by static/offscreen consumers).
#[expect(clippy::too_many_arguments)]
pub(crate) fn gear_render_objects(
    tank_id: TankId,
    handles: GearMeshHandles,
    material: MaterialHandle,
    hull_transform: Mat4,
    kin: &RunningGearKinematics,
    left_m: f32,
    right_m: f32,
    dynamics: GearDynamics<'_>,
    tint: [f32; 3],
    detail: GearDetail,
) -> Vec<RenderObject> {
    let placements = running_gear_placements_dynamic(kin, left_m, right_m, dynamics);
    gear_objects_from(tank_id, handles, material, hull_transform, &placements, tint, detail)
}

/// The same, from placements the caller already has. The battle path keeps them in a per-tank
/// cache (`gear_cache.rs`) because most frames do not move them; this half is the only part that
/// has to run either way.
pub(crate) fn gear_objects_from(
    tank_id: TankId,
    handles: GearMeshHandles,
    material: MaterialHandle,
    hull_transform: Mat4,
    placements: &[GearPlacement],
    tint: [f32; 3],
    detail: GearDetail,
) -> Vec<RenderObject> {
    // The tier picks MESHES, never placements: a tank at range stands in the same place, on the
    // same number of wheels, with the same track scroll as one up close. That is also why the
    // tier is not part of the placement cache's key.
    let set = handles.set(detail);
    placements
        .iter()
        .map(|placement| {
            let mesh = match placement.part {
                GearPart::RoadWheel => set.road_wheel,
                GearPart::Idler => set.idler,
                GearPart::Sprocket => set.sprocket,
                GearPart::Link => set.link,
                GearPart::SwingArm => set.swing_arm,
                GearPart::SwingArmLeft => set.swing_arm_left,
                GearPart::Damper => set.damper,
                GearPart::DamperLeft => set.damper_left,
                GearPart::ReturnRoller => set.return_roller,
            };
            RenderObject {
                tank_id: Some(tank_id),
                mesh,
                material,
                transform: (hull_transform * placement.transform).to_cols_array_2d(),
                tint,
                dither: [0.0, 1.0],
            }
        })
        .collect()
}
