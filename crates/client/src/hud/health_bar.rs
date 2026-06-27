use engine::PresentationTank;
use game_core::{HitboxProfile, TankId, TeamId};
use glam::Vec3;
use renderer_api::HudVertex;

use crate::hud;
use crate::hud::reticle::world_to_clip_xy;

const BAR_HALF_WIDTH: f32 = 0.055;
const BAR_HALF_HEIGHT: f32 = 0.008;

/// Floating HP bars over **live enemies** only: teammates carry no bar (they are not targets)
/// and wrecks stop advertising a permanent "0".
pub(crate) fn enemy_health_bars(
    tanks: &[PresentationTank],
    player_tank_id: TankId,
    player_team: TeamId,
    view_projection: [[f32; 4]; 4],
    aspect: f32,
) -> Vec<HudVertex> {
    let mut vertices = Vec::new();

    for tank in tanks {
        if tank.id == player_tank_id || tank.team == player_team || tank.hit_points == 0 {
            continue;
        }

        let hitbox = HitboxProfile::for_vehicle(tank.vehicle);
        let bar_world_y = tank.translation[1] + hitbox.center_y_m + hitbox.half_height_m + 0.35;
        let world_pos = Vec3::new(tank.translation[0], bar_world_y, tank.translation[2]);

        let Some(clip_pos) = world_to_clip_xy(world_pos, view_projection) else {
            continue;
        };

        let max_hp = tank.vehicle.spec().hit_points.max(1) as f32;
        let hp_frac = (tank.hit_points as f32 / max_hp).clamp(0.0, 1.0);
        let color = hud::health_color(hp_frac);

        let bar_half_w = BAR_HALF_WIDTH / aspect;
        let bar_center = clip_pos;

        hud::push_quad(
            &mut vertices,
            bar_center,
            [bar_half_w, BAR_HALF_HEIGHT],
            [0.0, 0.0, 0.0, 0.55],
        );

        let fill_w = bar_half_w * hp_frac;
        hud::push_quad(
            &mut vertices,
            [bar_center[0] - bar_half_w + fill_w, bar_center[1]],
            [fill_w, BAR_HALF_HEIGHT],
            color,
        );

        let num_digits = crate::hud::number::digit_count(tank.hit_points.min(9_999));
        let num_width = num_digits as f32 * 0.035 * 0.6;
        crate::hud::number::push_number(
            &mut vertices,
            tank.hit_points.min(9_999),
            bar_center[0] + num_width * 0.5,
            bar_center[1] - BAR_HALF_HEIGHT - 0.012,
            0.035,
            aspect,
            crate::hud::number::HP_COLOR,
        );
    }

    vertices
}

#[cfg(test)]
mod tests {
    use game_core::VehicleKind;

    use super::*;

    fn tank(id: u64, team: u16, hit_points: u32) -> PresentationTank {
        PresentationTank {
            id: TankId(id),
            team: TeamId(team),
            vehicle: VehicleKind::T55A,
            // Low enough that the floating bar lands inside the identity clip volume.
            translation: [0.0, -2.0, 0.0],
            hull_yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.0,
            hit_points,
            destroyed_modules_mask: 0,
        }
    }

    const IDENTITY: [[f32; 4]; 4] =
        [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

    #[test]
    fn bars_appear_over_live_enemies_only() {
        let player = TankId(1);
        let team = TeamId(1);
        let aspect = 16.0 / 9.0;

        let enemy = [tank(2, 2, 900)];
        assert!(!enemy_health_bars(&enemy, player, team, IDENTITY, aspect).is_empty());

        let ally = [tank(3, 1, 900)];
        assert!(
            enemy_health_bars(&ally, player, team, IDENTITY, aspect).is_empty(),
            "teammates are not targets and carry no bar"
        );

        let wreck = [tank(4, 2, 0)];
        assert!(
            enemy_health_bars(&wreck, player, team, IDENTITY, aspect).is_empty(),
            "wrecks must stop advertising a permanent zero"
        );

        let own = [tank(1, 1, 900)];
        assert!(enemy_health_bars(&own, player, team, IDENTITY, aspect).is_empty());
    }
}
