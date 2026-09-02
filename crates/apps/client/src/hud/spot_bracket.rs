//! A spotted enemy is marked in the scope (Inny Poziom A9). Spotting used to be cull-only: a
//! spotted hull drew, an unspotted one did not, and at 8° of field a T-54 at 300 m is ninety
//! pixels of the same value as the field behind it. Now every spotted, live enemy wears a
//! faint corner bracket on its projected hitbox — four short L-shaped corners in the reticle's
//! quiet off-white family, sniper mode only, never a box: the bracket says "there is a hull
//! here and the server says you see it", and leaves the silhouette to the eye. Friendlies,
//! wrecks and hulls the player's team has not spotted draw nothing — the same visibility bit
//! that gates the floating HP bar and the minimap blip.

use engine::PresentationTank;
use game_core::{HitboxProfile, TankId, TeamId, VehicleKind};
use glam::Vec3;
use renderer_api::HudVertex;

use super::reticle::world_to_clip_xy;

/// The bracket's tone: the reticle's neutral family, one step quieter and its own bytes (HUD
/// tests tag features by exact vertex-colour equality).
pub(crate) const SPOT_BRACKET_COLOR: [f32; 4] = [0.86, 0.88, 0.82, 0.62];
/// Each arm is this fraction of the projected box's shorter side, never under `MIN_ARM`.
const BRACKET_ARM_FRAC: f32 = 0.22;
const MIN_ARM: f32 = 0.014;
/// Half thickness of an arm in clip units (vertical); the horizontal one divides by aspect.
const ARM_HALF_THICKNESS: f32 = 0.0025;
/// The bracket stands a little off the hitbox so it never touches the silhouette.
const BRACKET_PAD: f32 = 0.006;

/// Corner brackets for every spotted, live enemy — sniper mode only. Third person returns an
/// empty batch: the bracket is a scope instrument, not a radar.
pub(crate) fn spotted_enemy_brackets(
    tanks: &[PresentationTank],
    player_tank_id: TankId,
    player_team: TeamId,
    view_projection: [[f32; 4]; 4],
    aspect: f32,
    sniper: bool,
) -> Vec<HudVertex> {
    let mut vertices = Vec::new();
    if !sniper {
        return vertices;
    }
    let player_bit = player_team.spotting_bit();
    for tank in tanks {
        if tank.id == player_tank_id || tank.team == player_team || tank.hit_points == 0 {
            continue;
        }
        if tank.spotted_by_teams_mask & player_bit == 0 {
            continue;
        }
        spot_bracket_for_hull(
            tank.translation,
            tank.hull_yaw_rad,
            tank.vehicle,
            view_projection,
            aspect,
            &mut vertices,
        );
    }
    vertices
}

/// One hull's bracket, from its pose alone — the presentation loop above and the `battle_hud`
/// probe (which brackets every enemy in frame as a review aid) share it. Nothing is drawn when
/// a corner of the hitbox is behind the camera.
pub fn spot_bracket_for_hull(
    translation: [f32; 3],
    hull_yaw_rad: f32,
    vehicle: VehicleKind,
    view_projection: [[f32; 4]; 4],
    aspect: f32,
    vertices: &mut Vec<HudVertex>,
) {
    let Some(([min_x, min_y], [max_x, max_y])) =
        projected_hitbox(translation, hull_yaw_rad, vehicle, view_projection)
    else {
        return;
    };
    let width = max_x - min_x;
    let height = max_y - min_y;
    let arm = (width.min(height) * BRACKET_ARM_FRAC).max(MIN_ARM);
    let (left, right) = (min_x - BRACKET_PAD / aspect, max_x + BRACKET_PAD / aspect);
    let (bottom, top) = (min_y - BRACKET_PAD, max_y + BRACKET_PAD);
    for (x, sx) in [(left, 1.0), (right, -1.0)] {
        for (y, sy) in [(bottom, 1.0), (top, -1.0)] {
            // The horizontal arm runs inward from the corner, the vertical arm up/down it.
            super::push_quad(
                vertices,
                [x + sx * arm * 0.5 / aspect, y],
                [arm * 0.5 / aspect, ARM_HALF_THICKNESS],
                SPOT_BRACKET_COLOR,
            );
            super::push_quad(
                vertices,
                [x, y + sy * arm * 0.5],
                [ARM_HALF_THICKNESS / aspect, arm * 0.5],
                SPOT_BRACKET_COLOR,
            );
        }
    }
}

/// The screen-space box of the hull's hitbox: its eight corners, posed by the hull yaw,
/// projected; `None` when any corner is behind the camera (a hull the frustum has cut is
/// left to the eye rather than bracketed by a guess).
fn projected_hitbox(
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

    fn scope() -> [[f32; 4]; 4] {
        let camera =
            Camera { eye: [0.0, 2.0, 0.0], target: [0.0, 1.5, 300.0], vertical_fov_degrees: 8.0 };
        view_projection_matrix(&camera, 16.0 / 9.0, 0.5, 1500.0)
    }

    fn brackets(tanks: &[PresentationTank], sniper: bool) -> Vec<HudVertex> {
        spotted_enemy_brackets(tanks, TankId(1), TeamId(1), scope(), 16.0 / 9.0, sniper)
    }

    /// The bracket draws for a spotted enemy and never for an unspotted one, a friendly, a
    /// wreck, or outside the scope.
    #[test]
    fn the_bracket_marks_a_spotted_enemy_and_nothing_else() {
        let ours = TeamId(1).spotting_bit();
        let theirs = TeamId(2).spotting_bit();
        let spotted = brackets(&[tank(2, 2, ours, 900)], true);
        assert!(!spotted.is_empty(), "a spotted enemy wears its bracket");
        assert!(spotted.iter().all(|v| v.color == SPOT_BRACKET_COLOR));
        // Four corners, two arms each, one quad per arm: 8 quads.
        assert_eq!(spotted.len() % 8, 0);
        assert!(brackets(&[tank(2, 2, theirs, 900)], true).is_empty(), "unspotted: nothing");
        assert!(brackets(&[tank(3, 1, ours, 900)], true).is_empty(), "a friendly: nothing");
        assert!(brackets(&[tank(2, 2, ours, 0)], true).is_empty(), "a wreck: nothing");
        assert!(brackets(&[tank(2, 2, ours, 900)], false).is_empty(), "third person: nothing");
    }

    /// The bracket frames the hull: its corners straddle the hull's projected centre on both
    /// axes, and it is eight short arms — not a box around the silhouette.
    #[test]
    fn the_bracket_frames_the_hull_without_boxing_it() {
        let ours = TeamId(1).spotting_bit();
        let enemy = tank(2, 2, ours, 900);
        let verts = brackets(std::slice::from_ref(&enemy), true);
        let centre = world_to_clip_xy(Vec3::from_array(enemy.translation) + Vec3::Y, scope())
            .expect("the hull is in front of the scope");
        let bound = |axis: usize| {
            verts
                .iter()
                .map(|v| v.position[axis])
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| (lo.min(p), hi.max(p)))
        };
        let (min_x, max_x) = bound(0);
        let (min_y, max_y) = bound(1);
        assert!(min_x < centre[0] && centre[0] < max_x, "straddles the hull in x");
        assert!(min_y < centre[1] && centre[1] < max_y, "straddles the hull in y");
        // Eight arms, one quad each, and nothing more.
        let per_quad = verts.len() / 8;
        assert!(per_quad == 4 || per_quad == 6, "eight arms: {} vertices", verts.len());
        // The arms are short of the box: each horizontal arm covers well under half the width.
        let arm_span = 2.0 * (max_x - min_x) * BRACKET_ARM_FRAC;
        assert!(arm_span < (max_x - min_x) * 0.5, "corners, not a box: {arm_span}");
    }
}
