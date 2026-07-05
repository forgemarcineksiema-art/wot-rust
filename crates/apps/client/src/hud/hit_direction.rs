//! Directional incoming-hit indicators: a short red arc on a fixed ring around the screen
//! center, at the attacker's bearing relative to the camera, fading over about a second and a
//! half. The arc answers the only question that matters in the moment Ă˘â‚¬â€ť WHERE did that come
//! from Ă˘â‚¬â€ť without turning the HUD into a radar.

use renderer_api::HudVertex;

use super::primitives::push_arc;

/// Incoming-hit arc red: penetrating hits at full saturation, bounces dimmer. Unique bytes
/// (HUD tests tag features by exact vertex-color equality).
pub(crate) const HIT_DIRECTION_COLOR: [f32; 4] = [0.92, 0.24, 0.18, 0.90];

pub(crate) const HIT_DIRECTION_TTL_S: f32 = 1.4;
/// Fixed screen radius of the bearing ring: outside the reload arc and any realistic dispersion
/// ring, inside the panel regions.
const HIT_ARC_RADIUS: f32 = 0.30;
/// Angular width of one bearing arc.
const HIT_ARC_SWEEP_RAD: f32 = 0.55;

/// One incoming hit, already resolved to a screen bearing: `0` = the attacker is dead ahead of
/// the camera, positive = to the right (clockwise on screen).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IncomingHit {
    pub bearing_rad: f32,
    pub age_s: f32,
    pub penetrated: bool,
}

/// App-side store: hits keep their world direction and are resolved to a screen bearing each
/// frame at draw time Ă˘â‚¬â€ť the camera turns underneath them, the arcs must not turn with it.
#[derive(Debug, Default)]
pub(crate) struct IncomingHitFeed {
    hits: Vec<WorldHit>,
}

#[derive(Debug, Clone, Copy)]
struct WorldHit {
    /// Unit XZ direction from the player toward the attacker at hit time.
    direction_xz: [f32; 2],
    age_s: f32,
    penetrated: bool,
}

impl IncomingHitFeed {
    /// Ingest a snapshot's damage events aimed at the player Ă˘â‚¬â€ť including zero-damage bounces:
    /// being shot at deserves a direction even when the armor held.
    pub(crate) fn ingest(
        &mut self,
        events: &[game_core::DamageEvent],
        player: game_core::TankId,
        tanks: &[net::TankSnapshot],
    ) {
        let Some(player_pos) =
            tanks.iter().find(|tank| tank.tank_id == player).map(|tank| tank.position)
        else {
            return;
        };
        for event in events {
            if event.target != player || event.source == player {
                continue;
            }
            // Attacker position when still known; a despawned attacker falls back to the hit
            // point, which at least names the struck side.
            let toward = tanks
                .iter()
                .find(|tank| tank.tank_id == event.source)
                .map(|tank| glam::Vec3::from_array(tank.position))
                .unwrap_or(event.hit_position);
            let dx = toward.x - player_pos[0];
            let dz = toward.z - player_pos[2];
            let len = (dx * dx + dz * dz).sqrt();
            if len <= 1.0e-4 {
                continue;
            }
            self.hits.push(WorldHit {
                direction_xz: [dx / len, dz / len],
                age_s: 0.0,
                penetrated: event.penetrated,
            });
        }
    }

    pub(crate) fn tick(&mut self, dt: f32) {
        for hit in &mut self.hits {
            hit.age_s += dt;
        }
        self.hits.retain(|hit| hit.age_s < HIT_DIRECTION_TTL_S);
    }

    /// Resolve to screen bearings against the camera's current XZ forward.
    pub(crate) fn screen_hits(&self, camera_forward_xz: [f32; 2]) -> Vec<IncomingHit> {
        let camera_yaw = camera_forward_xz[0].atan2(camera_forward_xz[1]);
        self.hits
            .iter()
            .map(|hit| {
                let world_yaw = hit.direction_xz[0].atan2(hit.direction_xz[1]);
                let mut bearing = world_yaw - camera_yaw;
                while bearing > std::f32::consts::PI {
                    bearing -= std::f32::consts::TAU;
                }
                while bearing < -std::f32::consts::PI {
                    bearing += std::f32::consts::TAU;
                }
                IncomingHit { bearing_rad: bearing, age_s: hit.age_s, penetrated: hit.penetrated }
            })
            .collect()
    }
}

pub(crate) fn push_hit_direction(vertices: &mut Vec<HudVertex>, hits: &[IncomingHit], aspect: f32) {
    for hit in hits {
        let fade = ((HIT_DIRECTION_TTL_S - hit.age_s) / HIT_DIRECTION_TTL_S).clamp(0.0, 1.0);
        if fade <= 0.0 {
            continue;
        }
        let mut color = HIT_DIRECTION_COLOR;
        color[3] *= fade * if hit.penetrated { 1.0 } else { 0.55 };
        // Screen angle: bearing 0 (dead ahead) sits at 12 o'clock; positive bearing (attacker to
        // the right) rotates the arc clockwise.
        let center_angle = std::f32::consts::FRAC_PI_2 - hit.bearing_rad;
        push_arc(
            vertices,
            [0.0, 0.0],
            HIT_ARC_RADIUS,
            center_angle - HIT_ARC_SWEEP_RAD / 2.0,
            HIT_ARC_SWEEP_RAD,
            10,
            aspect,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use game_core::{DamageEvent, TankId, TeamId, VehicleKind};
    use glam::Vec3;

    use super::*;

    fn tank_at(id: u64, x: f32, z: f32) -> net::TankSnapshot {
        let vehicle = VehicleKind::T54_1951;
        net::TankSnapshot {
            tank_id: TankId(id),
            team: TeamId(1),
            vehicle,
            position: [x, 0.0, z],
            yaw_rad: 0.0,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: 100,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 0.0,
            module_hit_points: vehicle.spec().module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
        }
    }

    fn hit_on_player(source: u64) -> DamageEvent {
        DamageEvent {
            source: TankId(source),
            target: TankId(1),
            hit_position: Vec3::ZERO,
            damage_hp: 100,
            penetrated: true,
            ..Default::default()
        }
    }

    #[test]
    fn the_bearing_is_relative_to_the_camera_not_the_world() {
        let mut feed = IncomingHitFeed::default();
        // Attacker due east (+x) of the player.
        let tanks = [tank_at(1, 0.0, 0.0), tank_at(2, 100.0, 0.0)];
        feed.ingest(&[hit_on_player(2)], TankId(1), &tanks);

        // Camera facing north (+z): the attacker is 90 degrees to the right.
        let facing_north = feed.screen_hits([0.0, 1.0]);
        assert!((facing_north[0].bearing_rad - std::f32::consts::FRAC_PI_2).abs() < 1.0e-4);

        // Camera facing east (+x): the attacker is dead ahead.
        let facing_east = feed.screen_hits([1.0, 0.0]);
        assert!(facing_east[0].bearing_rad.abs() < 1.0e-4);
    }

    #[test]
    fn a_hit_from_the_right_draws_its_arc_on_the_right_side_of_the_screen() {
        let hits = [IncomingHit {
            bearing_rad: std::f32::consts::FRAC_PI_2,
            age_s: 0.0,
            penetrated: true,
        }];
        let mut v = Vec::new();
        push_hit_direction(&mut v, &hits, 16.0 / 9.0);

        let arc: Vec<_> =
            v.iter().filter(|vert| vert.color[..3] == HIT_DIRECTION_COLOR[..3]).collect();
        assert!(!arc.is_empty(), "a fresh hit draws its bearing arc");
        assert!(
            arc.iter().all(|vert| vert.position[0] > 0.0 && vert.position[1].abs() < 0.12),
            "a right-hand bearing sits on the screen's right, near the horizontal axis"
        );
    }

    #[test]
    fn arcs_fade_with_age_and_expire_from_the_feed() {
        let fresh = [IncomingHit { bearing_rad: 0.0, age_s: 0.0, penetrated: true }];
        let old = [IncomingHit { bearing_rad: 0.0, age_s: 1.2, penetrated: true }];
        let (mut v_fresh, mut v_old) = (Vec::new(), Vec::new());
        push_hit_direction(&mut v_fresh, &fresh, 16.0 / 9.0);
        push_hit_direction(&mut v_old, &old, 16.0 / 9.0);
        let alpha = |v: &[renderer_api::HudVertex]| v.first().map(|vert| vert.color[3]).unwrap();
        assert!(alpha(&v_old) < alpha(&v_fresh), "arcs fade as they age");

        let mut feed = IncomingHitFeed::default();
        let tanks = [tank_at(1, 0.0, 0.0), tank_at(2, 100.0, 0.0)];
        feed.ingest(&[hit_on_player(2)], TankId(1), &tanks);
        feed.tick(HIT_DIRECTION_TTL_S + 0.1);
        assert!(feed.screen_hits([0.0, 1.0]).is_empty(), "expired hits leave the feed");
    }
}
