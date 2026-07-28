//! Contact carries momentum.
//!
//! Movement collision answers one question — "may this hull be here" — and answers it by REFUSING
//! the move. That is a correct constraint and a completely silent physics: fifty tonnes at 15 m/s
//! meets a parked hull and simply stops, and nothing about the parked hull ever learns it was hit.
//!
//! This module adds the other half. Once a tick the whole roster is solved at once: every pair of
//! touching hulls exchanges a normal impulse, and an off-centre contact also exchanges angular
//! momentum, so a T-bone spins its victim instead of merely bruising it. The non-overlap
//! constraint stays exactly where it was — nothing here lets hulls interpenetrate, and the client
//! predictor keeps the same hard stop it always had. What changes is that the hull doing the
//! blocking now gets pushed, and next tick the blocked hull is free to follow.
//!
//! **Order independence is a requirement, not a nicety.** Impulses are gathered against the
//! start-of-pass state and applied together (Jacobi), never folded in pair by pair, so the outcome
//! cannot depend on where a tank sits in the roster. That property was bought at some cost in
//! `ramming.rs` — the ram bill used to be a function of an array index — and a sequential solver
//! would hand it straight back.

use glam::{Vec2, Vec3};

use crate::collision::{TankFootprint, TankObstacle, obstacles_contact};

/// Coefficient of restitution for steel on steel. Zero: armour plate does not bounce, it shoves.
const RESTITUTION: f32 = 0.0;

/// How close counts as touching.
///
/// This is load-bearing, and its absence was the first thing to go wrong here: the movement
/// constraint refuses any move that WOULD overlap, so two hulls pressed together never actually
/// overlap — a solver that waits for penetration therefore never sees a single contact and the
/// whole exchange is silently dead. Hulls within a skin of each other are treated as in contact,
/// which is the same trick `ramming.rs` uses to catch a genuine hull-to-hull touch.
const CONTACT_SKIN_M: f32 = 0.12;

/// Penetration this deep is left alone, so hulls resting against each other do not jitter.
const POSITION_SLOP_M: f32 = 0.02;

/// Fraction of the remaining penetration pushed out per tick. Below 1 so the correction eases
/// hulls apart instead of teleporting them, which would fight the movement constraint.
const POSITION_CORRECTION: f32 = 0.35;

/// Solver iterations. A handful is plenty for a 7v7 pile-up and keeps the cost flat and known.
const ITERATIONS: usize = 4;

/// One hull the solver may push, as the solver sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactBody {
    pub position: Vec3,
    /// World-frame velocity; only the horizontal plane participates.
    pub velocity: Vec3,
    pub yaw_rad: f32,
    pub yaw_rate_rad_s: f32,
    pub footprint: TankFootprint,
    pub mass_kg: f32,
}

impl ContactBody {
    fn obstacle(&self) -> TankObstacle {
        TankObstacle::new(self.position, self.yaw_rad, self.footprint)
    }

    /// The hull grown by the contact skin, so "resting against" registers as touching.
    fn skinned(&self) -> TankObstacle {
        TankObstacle::new(
            self.position,
            self.yaw_rad,
            TankFootprint {
                half_width_m: self.footprint.half_width_m + CONTACT_SKIN_M,
                half_length_m: self.footprint.half_length_m + CONTACT_SKIN_M,
            },
        )
    }

    fn inverse_mass(&self) -> f32 {
        1.0 / self.mass_kg.max(1.0)
    }

    /// Planar moment of inertia of the hull footprint as a uniform rectangle about its centre.
    fn inverse_inertia(&self) -> f32 {
        let width = self.footprint.half_width_m * 2.0;
        let length = self.footprint.half_length_m * 2.0;
        let inertia = self.mass_kg.max(1.0) * (width * width + length * length) / 12.0;
        1.0 / inertia.max(1.0)
    }

    /// How far the hull reaches along a planar axis — the same support radius the SAT projects.
    fn reach_along(&self, axis: Vec2) -> f32 {
        let forward = Vec2::new(self.yaw_rad.sin(), self.yaw_rad.cos());
        let right = Vec2::new(forward.y, -forward.x);
        self.footprint.half_width_m * axis.dot(right).abs()
            + self.footprint.half_length_m * axis.dot(forward).abs()
    }
}

/// What one hull took out of this tick's contacts. The sim writes the deltas back onto its own
/// state, bills ram damage from `normal_impulse_ns`, and forwards the whole thing to the owning
/// client so its predictor learns about a shove it could not have predicted.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ContactImpulse {
    pub delta_velocity: Vec3,
    pub delta_yaw_rate_rad_s: f32,
    /// Position correction applied to ease the hull out of a penetration.
    pub delta_position: Vec3,
    /// Total normal impulse this hull absorbed, in newton-seconds. This is the honest measure of
    /// how hard it was hit — mass and closing speed already folded together — which is why ram
    /// damage reads it instead of recomputing a closing speed of its own.
    pub normal_impulse_ns: f32,
}

impl ContactImpulse {
    pub fn is_empty(&self) -> bool {
        self.normal_impulse_ns <= 0.0
    }
}

/// Solve every hull-to-hull contact in the roster and report what each hull took. `bodies` is left
/// untouched; the caller applies the deltas, because the caller owns the authoritative state.
pub fn resolve_contacts(bodies: &[ContactBody]) -> Vec<ContactImpulse> {
    let mut out = vec![ContactImpulse::default(); bodies.len()];
    if bodies.len() < 2 {
        return out;
    }
    // Working copy: iterations converge against the accumulating solution, but within a single
    // iteration every pair reads the SAME state, so the pass order cannot change the result.
    let mut velocity: Vec<Vec3> = bodies.iter().map(|body| body.velocity).collect();
    let mut yaw_rate: Vec<f32> = bodies.iter().map(|body| body.yaw_rate_rad_s).collect();

    for _ in 0..ITERATIONS {
        let mut delta_v = vec![Vec3::ZERO; bodies.len()];
        let mut delta_w = vec![0.0_f32; bodies.len()];
        for a in 0..bodies.len() {
            for b in a + 1..bodies.len() {
                let Some(contact) = obstacles_contact(&bodies[a].skinned(), &bodies[b].obstacle())
                else {
                    continue;
                };
                let normal = contact.normal;
                let tangent = Vec2::new(-normal.y, normal.x);

                // The contact acts at the midpoint of the two centres, so each hull's lever is its
                // own TANGENTIAL offset to that point — equal and OPPOSITE. A hull struck dead
                // amidships has no lever and takes no spin; one caught near the nose has a long
                // one and slews. Getting this equal-and-opposite is what makes the torque a fact
                // about the geometry rather than about which hull the loop happened to call `a`:
                // swapping the pair flips the tangent AND both levers, and the two flips cancel.
                // (They did not, at first — the torque took its sign from the role, and the
                // order-independence test caught it.) Clamped to each hull's own reach so a deep
                // overlap cannot invent a lever longer than the vehicle.
                let delta = Vec2::new(
                    bodies[b].position.x - bodies[a].position.x,
                    bodies[b].position.z - bodies[a].position.z,
                );
                let offset = delta.dot(tangent) * 0.5;
                let reach_a = bodies[a].reach_along(tangent);
                let reach_b = bodies[b].reach_along(tangent);
                let lever_a = offset.clamp(-reach_a, reach_a);
                let lever_b = (-offset).clamp(-reach_b, reach_b);

                // Closing speed at the contact point. The normal component a spin contributes at
                // a tangential lever is `-omega * lever` (see the module's planar convention).
                let relative =
                    Vec2::new(velocity[b].x - velocity[a].x, velocity[b].z - velocity[a].z);
                let approach = relative.dot(normal) - yaw_rate[b] * lever_b + yaw_rate[a] * lever_a;
                if approach >= 0.0 {
                    // Already separating: a contact constraint pushes, it never pulls.
                    continue;
                }

                let (inv_ma, inv_mb) = (bodies[a].inverse_mass(), bodies[b].inverse_mass());
                let (inv_ia, inv_ib) = (bodies[a].inverse_inertia(), bodies[b].inverse_inertia());
                let effective =
                    inv_ma + inv_mb + lever_a * lever_a * inv_ia + lever_b * lever_b * inv_ib;
                if effective <= f32::EPSILON {
                    continue;
                }
                let magnitude = -(1.0 + RESTITUTION) * approach / effective;
                let impulse = normal * magnitude;

                delta_v[a] -= Vec3::new(impulse.x, 0.0, impulse.y) * inv_ma;
                delta_v[b] += Vec3::new(impulse.x, 0.0, impulse.y) * inv_mb;
                delta_w[a] += lever_a * magnitude * inv_ia;
                delta_w[b] -= lever_b * magnitude * inv_ib;

                out[a].normal_impulse_ns += magnitude;
                out[b].normal_impulse_ns += magnitude;
            }
        }
        for index in 0..bodies.len() {
            velocity[index] += delta_v[index];
            yaw_rate[index] += delta_w[index];
        }
    }

    for index in 0..bodies.len() {
        out[index].delta_velocity = velocity[index] - bodies[index].velocity;
        out[index].delta_yaw_rate_rad_s = yaw_rate[index] - bodies[index].yaw_rate_rad_s;
    }
    ease_overlap(bodies, &mut out);
    out
}

/// Ease overlapping hulls apart. The movement constraint already refuses moves that would create
/// an overlap, so this only ever cleans up the ones that arrive some other way — a pivot swinging
/// an oriented footprint into a neighbour, or two hulls each clearing the other's pre-step pose.
/// It is the honest replacement for the interpenetration ESCAPE hack in `tank_resolve`, which
/// existed purely because nothing could push hulls apart.
///
/// Named for overlap rather than penetration on purpose: in this codebase "penetration" is what a
/// shell does to armour (`game_core::resolve_penetration`), and the architecture gate is right to
/// refuse a second meaning for it.
fn ease_overlap(bodies: &[ContactBody], out: &mut [ContactImpulse]) {
    for a in 0..bodies.len() {
        for b in a + 1..bodies.len() {
            // Real overlap only: measured against the bare footprints, not the skinned ones, so
            // hulls merely resting against each other are never shoved apart.
            let Some(contact) = obstacles_contact(&bodies[a].obstacle(), &bodies[b].obstacle())
            else {
                continue;
            };
            let excess = contact.depth_m - POSITION_SLOP_M;
            if excess <= 0.0 {
                continue;
            }
            let (inv_ma, inv_mb) = (bodies[a].inverse_mass(), bodies[b].inverse_mass());
            let total = inv_ma + inv_mb;
            if total <= f32::EPSILON {
                continue;
            }
            let push = contact.normal * (excess * POSITION_CORRECTION / total);
            out[a].delta_position -= Vec3::new(push.x, 0.0, push.y) * inv_ma;
            out[b].delta_position += Vec3::new(push.x, 0.0, push.y) * inv_mb;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hull(x: f32, z: f32, yaw_rad: f32, velocity: Vec3, mass_kg: f32) -> ContactBody {
        ContactBody {
            position: Vec3::new(x, 0.0, z),
            velocity,
            yaw_rad,
            yaw_rate_rad_s: 0.0,
            footprint: TankFootprint { half_width_m: 1.75, half_length_m: 3.2 },
            mass_kg,
        }
    }

    /// The headline: a charging hull SHOVES a parked one. Before this the charger simply stopped
    /// and the parked hull never learned it had been hit.
    #[test]
    fn a_charging_hull_pushes_a_parked_one_and_pays_for_it() {
        let charger = hull(0.0, 0.0, 0.0, Vec3::new(0.0, 0.0, 12.0), 36_000.0);
        let parked = hull(0.0, 6.0, 0.0, Vec3::ZERO, 36_000.0);
        let impulses = resolve_contacts(&[charger, parked]);

        assert!(impulses[1].delta_velocity.z > 0.5, "the parked hull must be shoved forward");
        assert!(impulses[0].delta_velocity.z < -0.5, "and the charger must lose that momentum");
        assert!(impulses[0].normal_impulse_ns > 0.0);
    }

    /// Newton's third law, as arithmetic: with nothing else touching them, the momentum one hull
    /// gains is the momentum the other loses.
    #[test]
    fn the_pair_conserves_momentum() {
        let light = hull(0.0, 0.0, 0.0, Vec3::new(0.0, 0.0, 14.0), 20_000.0);
        let heavy = hull(0.0, 6.0, 0.0, Vec3::ZERO, 60_000.0);
        let impulses = resolve_contacts(&[light, heavy]);

        let gained = impulses[1].delta_velocity.z * 60_000.0;
        let lost = -impulses[0].delta_velocity.z * 20_000.0;
        assert!((gained - lost).abs() / gained.abs().max(1.0) < 0.02, "{gained} vs {lost}");
        // ...and mass decides who actually moves.
        assert!(
            impulses[0].delta_velocity.z.abs() > impulses[1].delta_velocity.z.abs(),
            "the light hull must be the one that bounces off"
        );
    }

    /// An off-centre hit slews the victim. A hull caught near the nose spins; one caught dead
    /// amidships is only pushed. This is what makes a T-bone read as a T-bone.
    #[test]
    fn an_off_centre_hit_spins_the_victim_and_a_centred_one_does_not() {
        // Victim broadside across the charger's path, struck near its nose.
        let charger = hull(0.0, 0.0, 0.0, Vec3::new(0.0, 0.0, 14.0), 36_000.0);
        let offset = hull(-2.6, 4.4, std::f32::consts::FRAC_PI_2, Vec3::ZERO, 36_000.0);
        let spun = resolve_contacts(&[charger, offset])[1].delta_yaw_rate_rad_s.abs();

        let centred = hull(0.0, 4.4, std::f32::consts::FRAC_PI_2, Vec3::ZERO, 36_000.0);
        let square = resolve_contacts(&[charger, centred])[1].delta_yaw_rate_rad_s.abs();

        assert!(spun > 0.05, "a nose-on t-bone must slew the victim, got {spun} rad/s");
        assert!(square < spun * 0.25, "a centred hit must barely spin it: {square} vs {spun}");
    }

    /// Roster order must not reach the result. This is the property `ramming.rs` had to be fixed
    /// for, and a sequential solver would have reintroduced it — so it is locked here too.
    #[test]
    fn the_result_does_not_depend_on_roster_order() {
        let a = hull(0.0, 0.0, 0.3, Vec3::new(1.0, 0.0, 11.0), 34_000.0);
        let b = hull(0.6, 5.8, 1.1, Vec3::new(0.0, 0.0, -2.0), 48_000.0);

        let forward = resolve_contacts(&[a, b]);
        let reversed = resolve_contacts(&[b, a]);

        assert_eq!(forward[0], reversed[1], "the same hull must take the same impulse");
        assert_eq!(forward[1], reversed[0]);
    }

    /// A contact pushes; it never pulls. Two hulls already flying apart take nothing, however
    /// deeply their footprints still overlap.
    #[test]
    fn separating_hulls_are_left_alone() {
        let a = hull(0.0, 0.0, 0.0, Vec3::new(0.0, 0.0, -6.0), 36_000.0);
        let b = hull(0.0, 5.5, 0.0, Vec3::new(0.0, 0.0, 6.0), 36_000.0);
        let impulses = resolve_contacts(&[a, b]);
        assert_eq!(impulses[0].normal_impulse_ns, 0.0);
        assert_eq!(impulses[1].normal_impulse_ns, 0.0);
        assert_eq!(impulses[0].delta_velocity, Vec3::ZERO);
    }

    /// Overlapping hulls are eased apart along the shallowest axis, split by mass — the heavy one
    /// barely gives ground, the light one yields.
    #[test]
    fn a_penetration_is_eased_apart_by_mass() {
        let light = hull(0.0, 0.0, 0.0, Vec3::ZERO, 20_000.0);
        let heavy = hull(0.0, 5.0, 0.0, Vec3::ZERO, 60_000.0);
        let impulses = resolve_contacts(&[light, heavy]);

        assert!(impulses[0].delta_position.z < 0.0, "the light hull backs off");
        assert!(impulses[1].delta_position.z > 0.0, "the heavy one gives a little");
        assert!(
            impulses[0].delta_position.length() > impulses[1].delta_position.length() * 2.0,
            "and the light hull does most of the moving"
        );
    }

    /// A hull touching nobody is never touched.
    #[test]
    fn a_lone_hull_takes_nothing() {
        let impulses =
            resolve_contacts(&[hull(0.0, 0.0, 0.0, Vec3::new(0.0, 0.0, 12.0), 36_000.0)]);
        assert!(impulses[0].is_empty());
        assert_eq!(impulses[0].delta_velocity, Vec3::ZERO);
    }
}
