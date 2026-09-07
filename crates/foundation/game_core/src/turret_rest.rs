//! Where a blown-off turret comes to REST (the one program's Z13, GDD §12 „wieża ląduje jako
//! prop"). The jack-in-the-box is server truth (`turret_detached`, protocol v20) and its arc was
//! the client's alone — deterministic in the tank id and the ring, so every client drew the same
//! flight, but nothing BLOCKED on the landed casting: a picture. The launch lives here now, below
//! the sim and the client both: the authority computes the rest from the same id and ring, puts
//! a low solid there (what the shell stops in, the eye stops at) and replicates the spot; the
//! client flies its casting onto exactly that spot.

use glam::Vec3;

use crate::TankId;
use crate::math::splitmix64;

/// Presentation gravity for the arc — a touch snappier than the sim's arcade value reads better
/// on a light tumbling casting.
pub const POPOFF_GRAVITY_MPS2: f32 = 11.0;
/// The turret casting rests this far above the ground once it lands (roughly its half-height).
pub const TURRET_REST_CLEARANCE_M: f32 = 0.45;
/// The landed casting as a low solid: the half extents of the box the eye and the shell meet,
/// centred on the rest (its floor on the ground, its top at twice the clearance).
pub const TURRET_REST_HALF_M: [f32; 3] = [0.9, TURRET_REST_CLEARANCE_M, 0.9];

/// One turret's throw: the velocity it leaves the ring with, how long until it lands, where it
/// rests, and the seed to keep drawing from (the client's tumble continues the sequence).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurretLaunch {
    pub velocity: Vec3,
    pub settle_s: f32,
    pub rest: Vec3,
    pub seed: u64,
}

/// The throw of `tank_id`'s turret from `ring_world` onto ground at `ground_y`: 6.5–10 m/s
/// straight up with a lean of 1.2–3.5 m/s some way round the compass, all from the id.
pub fn turret_launch(tank_id: TankId, ring_world: Vec3, ground_y: f32) -> TurretLaunch {
    let mut seed = splitmix64(tank_id.0 ^ 0x0DDB_1A5E_5EED_1234);
    let up = 6.5 + popoff_unit(&mut seed) * 3.5;
    let lean_angle = popoff_unit(&mut seed) * std::f32::consts::TAU;
    let lean_mag = 1.2 + popoff_unit(&mut seed) * 2.3;
    let velocity = Vec3::new(lean_angle.cos() * lean_mag, up, lean_angle.sin() * lean_mag);
    let rest_y = ground_y + TURRET_REST_CLEARANCE_M;
    // Positive root of ring.y + up t - g/2 t^2 = rest_y.
    let drop = (ring_world.y - rest_y).max(0.0);
    let settle_s = ((up + (up * up + 2.0 * POPOFF_GRAVITY_MPS2 * drop).max(0.0).sqrt())
        / POPOFF_GRAVITY_MPS2)
        .max(0.0);
    let rest = Vec3::new(
        ring_world.x + velocity.x * settle_s,
        rest_y,
        ring_world.z + velocity.z * settle_s,
    );
    TurretLaunch { velocity, settle_s, rest, seed }
}

/// Advance the seed and map it to `[0, 1)`.
pub fn popoff_unit(seed: &mut u64) -> f32 {
    *seed = splitmix64(*seed);
    ((*seed >> 40) as f32) / ((1u64 << 24) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Z13: the rest is a function of the id and the ring — the same on the authority and on
    /// every client — on the ground plus the clearance, within the throw's reach of the ring,
    /// and a different tank lands somewhere else.
    #[test]
    fn a_turret_rests_where_its_id_and_ring_say_and_nowhere_else() {
        let ring = Vec3::new(12.0, 2.0, -4.0);
        let a = turret_launch(TankId(7), ring, 0.3);
        let again = turret_launch(TankId(7), ring, 0.3);
        assert_eq!(a, again, "one throw per id and ring");
        assert!((a.rest.y - (0.3 + TURRET_REST_CLEARANCE_M)).abs() < 1e-6, "on the ground");
        assert!(a.settle_s > 1.0 && a.settle_s < 3.0, "a second or two in the air: {}", a.settle_s);
        let flat = Vec3::new(a.rest.x - ring.x, 0.0, a.rest.z - ring.z).length();
        assert!(flat > 1.0 && flat <= 3.5 * a.settle_s + 1e-3, "within the lean's reach: {flat}");
        let arc_y = ring.y + a.velocity.y * a.settle_s
            - 0.5 * POPOFF_GRAVITY_MPS2 * a.settle_s * a.settle_s;
        assert!((arc_y - a.rest.y).abs() < 1e-3, "the arc lands on the rest");
        let b = turret_launch(TankId(8), ring, 0.3);
        assert!(a.rest.distance(b.rest) > 0.5, "another tank throws its turret elsewhere");
    }
}
