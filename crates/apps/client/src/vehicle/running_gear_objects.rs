//! Shared mapping from running-gear placements to render objects. Both the geometry and the PBR
//! vehicle paths cache the four unit meshes (road wheel, idler, sprocket, shoe link) per
//! vehicle and instance them here, so the wheels spin and the track links scroll from the per-side
//! track distance the presentation world accumulates.

use game_core::TankId;
use glam::Mat4;
use renderer_api::{MaterialHandle, MeshHandle, RenderObject};
use vehicle_geometry::{GearDynamics, GearPart, RunningGearKinematics, running_gear_placements_dynamic};

/// Cached unit-mesh handles for one vehicle's animatable running gear.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GearMeshHandles {
    pub road_wheel: MeshHandle,
    pub idler: MeshHandle,
    pub sprocket: MeshHandle,
    pub link: MeshHandle,
    pub swing_arm: MeshHandle,
}

/// Emit one render object per moving running-gear part, instanced from the cached unit meshes and
/// placed in the world by `hull_transform * placement`. `left_m`/`right_m` are the per-side track
/// distances; at `0.0` the gear sits at its rest pose (used by static/offscreen consumers).
#[allow(clippy::too_many_arguments)]
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
) -> Vec<RenderObject> {
    running_gear_placements_dynamic(kin, left_m, right_m, dynamics)
        .into_iter()
        .map(|placement| {
            let mesh = match placement.part {
                GearPart::RoadWheel => handles.road_wheel,
                GearPart::Idler => handles.idler,
                GearPart::Sprocket => handles.sprocket,
                GearPart::Link => handles.link,
                GearPart::SwingArm => handles.swing_arm,
            };
            RenderObject {
                tank_id: Some(tank_id),
                mesh,
                material,
                transform: (hull_transform * placement.transform).to_cols_array_2d(),
                tint,
            }
        })
        .collect()
}
