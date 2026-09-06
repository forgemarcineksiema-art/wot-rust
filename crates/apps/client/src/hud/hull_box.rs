//! The screen-space box of a hull (H27): the projection the markers (H10) hang from. The A9
//! corner brackets that used to be drawn from it are gone — they answered „where are the
//! enemies", a question the reticle does not answer and the markers do.

use game_core::{HitboxProfile, VehicleKind};
use glam::Vec3;

use super::reticle::world_to_clip_xy;

/// The screen-space box of the hull's hitbox: its eight corners, posed by the hull yaw,
/// projected; `None` when any corner is behind the camera (a hull the frustum has cut is
/// left to the eye rather than bracketed by a guess).
pub(crate) fn projected_hitbox(
    translation: [f32; 3],
    hull_yaw_rad: f32,
    vehicle: VehicleKind,
    view_projection: [[f32; 4]; 4],
) -> Option<([f32; 2], [f32; 2])> {
    let hitbox = HitboxProfile::for_vehicle(vehicle);
    let center = Vec3::from_array(translation) + Vec3::Y * hitbox.center_y_m;
    let (sin_yaw, cos_yaw) = hull_yaw_rad.sin_cos();
    let forward = Vec3::new(sin_yaw, 0.0, cos_yaw) * hitbox.half_length_m;
    let right = Vec3::new(cos_yaw, 0.0, -sin_yaw) * hitbox.half_width_m;
    let up = Vec3::Y * hitbox.half_height_m;
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                let corner = center + forward * sz + right * sx + up * sy;
                let clip = world_to_clip_xy(corner, view_projection)?;
                min = [min[0].min(clip[0]), min[1].min(clip[1])];
                max = [max[0].max(clip[0]), max[1].max(clip[1])];
            }
        }
    }
    Some((min, max))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::PresentationTank;
    use game_core::{TankId, TeamId};
    use renderer_api::{Camera, view_projection_matrix};

    fn tank(id: u64, team: u16, spotted_by: u8, hit_points: u32) -> PresentationTank {
        let vehicle = VehicleKind::T54_1951;
        PresentationTank {
            id: TankId(id),
            team: TeamId(team),
            vehicle,
            translation: [0.0, 0.0, 300.0],
            hull_yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.0,
            hit_points,
            destroyed_modules_mask: 0,
            spotted_by_teams_mask: spotted_by,
            module_hit_points: vehicle.spec().module_health.hit_points_by_slot(),
            track_damage_mask: 0,
            track_hp: [game_core::TRACK_HP_MAX; 2],
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            armor_breaches: Default::default(),
            track_left_m: 0.0,
            track_right_m: 0.0,
            attitude_pitch_rad: 0.0,
            attitude_roll_rad: 0.0,
            attitude_heave_m: 0.0,
            accel_long_mps2: 0.0,
            gun_recoil_m: 0.0,
        }
    }

    /// The projection the markers hang from: a hull ahead of the camera boxes; one behind it
    /// does not — no guess is framed.
    #[test]
    fn a_hull_ahead_projects_and_one_behind_does_not() {
        let camera =
            Camera { eye: [0.0, 5.0, -20.0], target: [0.0, 1.0, 0.0], vertical_fov_degrees: 60.0 };
        let view_proj = view_projection_matrix(&camera, 16.0 / 9.0, 0.1, 2000.0);
        let ahead = tank(1, 2, 1, 100);
        let (min, max) =
            projected_hitbox(ahead.translation, ahead.hull_yaw_rad, ahead.vehicle, view_proj)
                .expect("boxed");
        assert!(min[0] < max[0] && min[1] < max[1]);
        assert!(min[0] > -1.0 && max[0] < 1.0, "on screen: {min:?} {max:?}");
        assert!(projected_hitbox([0.0, 0.0, -400.0], 0.0, ahead.vehicle, view_proj).is_none());
    }
}
