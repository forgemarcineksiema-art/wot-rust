//! Shared mapping from running-gear placements to render objects. Both the geometry and the PBR
//! vehicle paths cache the three unit meshes (one road wheel, one end wheel, one shoe link) per
//! vehicle and instance them here, so the wheels spin and the track links scroll from the per-side
//! track distance the presentation world accumulates.

use game_core::TankId;
use glam::Mat4;
use renderer_api::{MaterialHandle, MeshHandle, RenderObject};
use vehicle_geometry::{GearPart, RunningGearKinematics, running_gear_placements};

/// Cached unit-mesh handles for one vehicle's animatable running gear.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GearMeshHandles {
    pub road_wheel: MeshHandle,
    pub end_wheel: MeshHandle,
    pub link: MeshHandle,
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
    tint: [f32; 3],
) -> Vec<RenderObject> {
    running_gear_placements(kin, left_m, right_m)
        .into_iter()
        .map(|placement| {
            let mesh = match placement.part {
                GearPart::RoadWheel => handles.road_wheel,
                GearPart::EndWheel => handles.end_wheel,
                GearPart::Link => handles.link,
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
