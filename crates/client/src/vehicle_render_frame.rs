use engine::PresentationTank;
use game_core::TankId;
use glam::{Mat4, Vec3};
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
    player_gun_scale: f32,
) -> VehicleRenderFrame {
    let mut objects = Vec::new();
    for tank in tanks {
        let is_player = tank.id == player_tank;
        let hull_color = if is_player { [0.30, 0.40, 0.28] } else { [0.46, 0.29, 0.25] };
        let snapshot = render_snapshot(&tank);
        let mut tank_objects = tank_render_objects(catalog, &snapshot, hull_color);
        // The player's installed gun may have a longer/shorter barrel than the baked stock mesh;
        // stretch its gun submesh (index 2: [hull, turret, gun]) along the barrel axis to match the
        // muzzle the sim fires from. Enemies are always stock (scale 1.0).
        if is_player
            && (player_gun_scale - 1.0).abs() > 1.0e-3
            && let Some(gun) = tank_objects.get_mut(2)
        {
            let scaled = Mat4::from_cols_array_2d(&gun.transform)
                * Mat4::from_scale(Vec3::new(1.0, 1.0, player_gun_scale));
            gun.transform = scaled.to_cols_array_2d();
        }
        objects.append(&mut tank_objects);
    }
    VehicleRenderFrame { objects }
}

/// Adapt a presentation entity into the pose-only `TankSnapshot` the procedural mesh kernels
/// consume. The fields the meshes never read (`reload_remaining_s`, `aim_dispersion_mrad`) are
/// zeroed — they belong to the player's HUD path, not vehicle geometry.
pub(crate) fn render_snapshot(tank: &PresentationTank) -> TankSnapshot {
    TankSnapshot {
        tank_id: tank.id,
        team: tank.team,
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
            team: game_core::TeamId(2),
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
