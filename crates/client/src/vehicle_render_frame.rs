use engine::PresentationTank;
use game_core::TankId;
use net::TankSnapshot;
use renderer_api::{RenderFrame, RenderObject};

use crate::{VehicleMeshCatalog, tank_render_objects};

#[derive(Debug, Clone, PartialEq)]
pub struct VehicleRenderFrame {
    pub objects: Vec<RenderObject>,
}

pub fn split_vehicle_render_frame(
    catalog: &mut VehicleMeshCatalog,
    tanks: Vec<PresentationTank>,
    player_tank: TankId,
) -> VehicleRenderFrame {
    let mut objects = Vec::new();
    for tank in tanks {
        let hull_color =
            if tank.id == player_tank { [0.30, 0.40, 0.28] } else { [0.46, 0.29, 0.25] };
        let snapshot = render_snapshot(&tank);
        objects.append(&mut tank_render_objects(catalog, &snapshot, hull_color));
    }
    VehicleRenderFrame { objects }
}

/// Adapt a presentation entity into the pose-only `TankSnapshot` the procedural mesh kernels
/// consume. The fields the meshes never read (`reload_remaining_s`, `aim_dispersion_mrad`) are
/// zeroed — they belong to the player's HUD path, not vehicle geometry.
pub(crate) fn render_snapshot(tank: &PresentationTank) -> TankSnapshot {
    TankSnapshot {
        tank_id: tank.id,
        vehicle: tank.vehicle,
        position: tank.translation,
        yaw_rad: tank.hull_yaw_rad,
        turret_yaw_rad: tank.turret_yaw_rad,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: tank.gun_pitch_rad,
        hit_points: tank.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: tank.vehicle.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: tank.destroyed_modules_mask,
    }
}

pub fn render_frame_from_objects(objects: Vec<RenderObject>) -> RenderFrame {
    RenderFrame { objects, ..RenderFrame::default() }
}

#[cfg(test)]
mod tests {
    use game_core::VehicleKind;

    use super::*;

    #[test]
    fn render_snapshot_carries_every_pose_field_the_meshes_read() {
        let tank = PresentationTank {
            id: TankId(7),
            vehicle: VehicleKind::TigerII,
            translation: [1.0, 2.0, 3.0],
            hull_yaw_rad: 0.4,
            turret_yaw_rad: 0.5,
            gun_pitch_rad: -0.1,
            hit_points: 1200,
            destroyed_modules_mask: 0b101,
        };

        let snapshot = render_snapshot(&tank);

        assert_eq!(snapshot.tank_id, TankId(7));
        assert_eq!(snapshot.vehicle, VehicleKind::TigerII);
        assert_eq!(snapshot.position, [1.0, 2.0, 3.0]);
        assert_eq!(snapshot.yaw_rad, 0.4);
        assert_eq!(snapshot.turret_yaw_rad, 0.5);
        assert_eq!(snapshot.gun_pitch_rad, -0.1);
        assert_eq!(snapshot.destroyed_modules_mask, 0b101);
    }
}
