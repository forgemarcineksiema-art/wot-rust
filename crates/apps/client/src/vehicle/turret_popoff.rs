//! The ammo-rack jack-in-the-box, client side: a detached turret (server truth, protocol v20)
//! flies off on a ballistic arc, tumbles, and settles on the ground. Every input is derived
//! deterministically from the tank id and the ring position the server replicates, so the arc and
//! the rest pose are IDENTICAL on every client with no transform sent over the wire — and the
//! flying turret ignores the wreck's still-replicated turret yaw entirely (its transform is
//! computed here, not posed from the snapshot), which freezes the turret at detonation as it must.

use game_core::math::splitmix64;
use game_core::{MountFrames, TankId, VehicleKind};
use glam::{Mat3, Mat4, Vec3};
use terrain::HeightMap;

/// Presentation gravity for the arc — a touch snappier than the sim's arcade value reads better
/// on a light tumbling casting.
const GRAVITY_MPS2: f32 = 11.0;
/// The turret casting rests this far above the ground once it lands (roughly its half-height).
const REST_CLEARANCE_M: f32 = 0.45;

/// One flying turret. Deterministic in `(tank_id, ring)` — construct it once at detonation and ask
/// it for the turret and gun transforms each frame.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TurretPopoff {
    /// Turret-ring world position at detonation (the launch origin).
    origin: Vec3,
    launch_velocity: Vec3,
    spin_axis: Vec3,
    spin_rate: f32,
    /// Trunnion minus ring in authoring space: the gun rides the turret rigidly by this offset.
    gun_offset: Vec3,
    /// Ground height the ring center rests at, sampled once at launch.
    rest_y: f32,
    /// Seconds until the arc reaches `rest_y` — after this the turret is settled and stops spinning.
    settle_s: f32,
    age_s: f32,
}

impl TurretPopoff {
    /// Launch a turret from `ring_world` for `kind`, seeded by `tank_id`. `heightmap` sets the
    /// rest height; a dry/offscreen path may pass `None` (rests at the launch height).
    pub fn launch(
        tank_id: TankId,
        kind: VehicleKind,
        ring_world: Vec3,
        heightmap: Option<&HeightMap>,
    ) -> Self {
        let mut seed = splitmix64(tank_id.0 ^ 0x0DDB_1A5E_5EED_1234);
        let up = 6.5 + next_unit(&mut seed) * 3.5; // 6.5–10 m/s straight up
        let lean_angle = next_unit(&mut seed) * std::f32::consts::TAU;
        let lean_mag = 1.2 + next_unit(&mut seed) * 2.3;
        let launch_velocity =
            Vec3::new(lean_angle.cos() * lean_mag, up, lean_angle.sin() * lean_mag);
        let spin_axis = Vec3::new(
            next_unit(&mut seed) * 2.0 - 1.0,
            next_unit(&mut seed) * 2.0 - 1.0,
            next_unit(&mut seed) * 2.0 - 1.0,
        )
        .normalize_or(Vec3::X);
        let spin_rate = 5.0 + next_unit(&mut seed) * 7.0;

        let mounts = MountFrames::for_vehicle(kind);
        let gun_offset = mounts.gun_trunnion.translation - mounts.turret_ring.translation;

        let ground = heightmap
            .and_then(|map| map.sample_height(ring_world.x, ring_world.z))
            .unwrap_or(ring_world.y - REST_CLEARANCE_M);
        let rest_y = ground + REST_CLEARANCE_M;

        // Positive root of origin.y + vy t - g/2 t^2 = rest_y.
        let vy = launch_velocity.y;
        let drop = (origin_above(ring_world.y, rest_y)).max(0.0);
        let settle_s = (vy + (vy * vy + 2.0 * GRAVITY_MPS2 * drop).max(0.0).sqrt()) / GRAVITY_MPS2;

        Self {
            origin: ring_world,
            launch_velocity,
            spin_axis,
            spin_rate,
            gun_offset,
            rest_y,
            settle_s: settle_s.max(0.0),
            age_s: 0.0,
        }
    }

    pub fn tick(&mut self, dt_s: f32) {
        self.age_s += dt_s.max(0.0);
    }

    pub fn settled(&self) -> bool {
        self.age_s >= self.settle_s
    }

    /// Ring-center world position at the current age: ballistic until it lands, then pinned at rest.
    fn ring_position(&self) -> Vec3 {
        let t = self.age_s.min(self.settle_s);
        let mut pos =
            self.origin + self.launch_velocity * t - Vec3::Y * (0.5 * GRAVITY_MPS2 * t * t);
        if self.settled() {
            pos.y = self.rest_y;
        }
        pos
    }

    /// The turret's tumble orientation; the spin freezes the instant it settles.
    fn rotation(&self) -> Mat3 {
        let angle = self.spin_rate * self.age_s.min(self.settle_s);
        Mat3::from_axis_angle(self.spin_axis, angle)
    }

    /// World transform for the turret submesh (authored around the ring).
    pub fn turret_transform(&self) -> Mat4 {
        Mat4::from_translation(self.ring_position()) * Mat4::from_mat3(self.rotation())
    }

    /// World transform for the gun submesh (authored around the trunnion), riding the turret.
    pub fn gun_transform(&self) -> Mat4 {
        self.turret_transform() * Mat4::from_translation(self.gun_offset)
    }
}

fn origin_above(origin_y: f32, rest_y: f32) -> f32 {
    origin_y - rest_y
}

/// Advance the seed and map it to `[0, 1)`.
fn next_unit(seed: &mut u64) -> f32 {
    *seed = splitmix64(*seed);
    ((*seed >> 40) as f32) / ((1u64 << 24) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_map() -> HeightMap {
        HeightMap::flat(65, 65, 5.0, 0.0).expect("flat map")
    }

    #[test]
    fn the_launch_is_deterministic_in_tank_id_and_ring() {
        let map = flat_map();
        let ring = Vec3::new(12.0, 2.0, -4.0);
        let mut a = TurretPopoff::launch(TankId(7), VehicleKind::T54_1951, ring, Some(&map));
        let mut b = TurretPopoff::launch(TankId(7), VehicleKind::T54_1951, ring, Some(&map));
        for _ in 0..30 {
            a.tick(0.05);
            b.tick(0.05);
        }
        assert_eq!(a.turret_transform(), b.turret_transform());
        assert_eq!(a.gun_transform(), b.gun_transform());
        // Different tanks throw their turrets differently.
        let mut c = TurretPopoff::launch(TankId(8), VehicleKind::T54_1951, ring, Some(&map));
        for _ in 0..30 {
            c.tick(0.05);
        }
        assert_ne!(a.turret_transform(), c.turret_transform());
    }

    #[test]
    fn the_turret_leaps_up_then_settles_on_the_ground_and_stays() {
        let map = flat_map(); // ground at y = 0
        let ring = Vec3::new(0.0, 2.0, 0.0);
        let mut popoff = TurretPopoff::launch(TankId(1), VehicleKind::T54_1951, ring, Some(&map));

        popoff.tick(0.1);
        let rising = popoff.turret_transform().w_axis.y;
        assert!(rising > 2.0, "the turret leaps up off the ring, got y {rising}");

        // Run well past the arc: it lands at rest and never sinks or falls further.
        for _ in 0..200 {
            popoff.tick(0.05);
        }
        assert!(popoff.settled());
        let rest = popoff.turret_transform().w_axis.y;
        assert!(
            (rest - REST_CLEARANCE_M).abs() < 1.0e-3,
            "the settled turret rests on the ground + clearance, got y {rest}"
        );

        // Age further: the settled pose is frozen (position and rotation both).
        let frozen = popoff.turret_transform();
        for _ in 0..40 {
            popoff.tick(0.05);
        }
        assert_eq!(popoff.turret_transform(), frozen, "a settled turret is frozen");
    }

    #[test]
    fn the_gun_rides_the_flying_turret() {
        let map = flat_map();
        let ring = Vec3::new(5.0, 2.0, 5.0);
        let mut popoff = TurretPopoff::launch(TankId(3), VehicleKind::T54_1951, ring, Some(&map));
        popoff.tick(0.2);
        // The gun sits a rigid offset from the turret ring center, carried by the same rotation.
        let turret = popoff.turret_transform();
        let gun = popoff.gun_transform();
        let expected = turret * Mat4::from_translation(popoff.gun_offset);
        assert_eq!(gun, expected);
    }
}
