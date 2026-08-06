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

use crate::collision::{TankFootprint, TankObstacle, footprint_contact_within, obstacles_contact};

/// Coefficient of restitution for steel on steel. Zero: armour plate does not bounce, it shoves.
const RESTITUTION: f32 = 0.0;

/// Radius of the circle circumscribing a footprint, grown by the contact skin. Two hulls further
/// apart than the sum of theirs cannot touch under ANY heading, which skips almost every pair on a
/// 14-hull roster before the four-axis SAT is ever projected — the same broadphase the cover tests
/// carry, and worth the same here: this loop runs over every pair, four iterations deep, twice a
/// tick.
fn contact_reach(body: &ContactBody) -> f32 {
    let half_width = body.footprint.half_width_m + SPECULATIVE_MARGIN_M;
    let half_length = body.footprint.half_length_m + SPECULATIVE_MARGIN_M;
    (half_width * half_width + half_length * half_length).sqrt()
}

/// Whether two hulls are close enough to be worth a full contact test, allowing for the ground
/// each may cover this tick.
fn worth_testing(a: &ContactBody, b: &ContactBody, dt: f32) -> bool {
    let travel = (a.velocity.length() + b.velocity.length()) * dt;
    let reach = contact_reach(a) + contact_reach(b) + travel;
    let dx = a.position.x - b.position.x;
    let dz = a.position.z - b.position.z;
    dx * dx + dz * dz <= reach * reach
}

/// How close a pair has to be before the solver bothers looking at it, over and above the ground
/// they could cover this tick.
///
/// This is only a "is it worth solving" range, and that distinction is the whole of P1.2. It used
/// to be a SKIN: one hull's footprint was grown by 0.12 m and any overlap of the grown shape was
/// treated as contact, whereupon the solver cancelled the entire approach. That stopped the pair
/// wherever they happened to be — which, measured, was a hitbox gap of 0.1217 m, the constant to
/// the millimetre, and 0.40 m of daylight between two T-54s once the phantom hitbox margins were
/// added. A detection range had quietly become a parking distance.
///
/// Now the solver is told the REAL separation and allows exactly the closing that shuts it, so
/// this number decides only how early the arithmetic starts, never where the hulls come to rest.
/// Generous is therefore cheap and stingy is not: too small and a contact is missed, too large and
/// a handful of pairs get projected that did not need it.
const SPECULATIVE_MARGIN_M: f32 = 0.05;

/// Coulomb friction at a steel-on-steel contact, as a fraction of the normal impulse.
///
/// Without it a contact resists only along its normal, and hulls pressed together slide along each
/// other for free. A queue of tanks leaning on one another then squirts sideways like wet soap —
/// which at a river ford means squirting into the channel, and is exactly what the Bystra bot soak
/// caught. Armour plate grinding on armour plate does not slide freely, and the model should not
/// let it.
const CONTACT_FRICTION_MU: f32 = 0.7;

/// Overlap this deep is left alone, so hulls resting against each other do not jitter.
const POSITION_SLOP_M: f32 = 0.02;

/// Fraction of a remaining overlap the contact asks back per tick, as a VELOCITY.
///
/// Hulls do end up inside each other — a pivot swings an oriented footprint into a neighbour, a
/// spawn lands badly — and something has to undo it. Doing that by moving the position is a
/// teleport: the hull arrives somewhere it never travelled to, the drive knows nothing about it,
/// the attitude and ride height were computed for where it used to be, and the measured result was
/// a pair squirting apart at 1.19 m/s with no velocity behind it. Asking for the same separation
/// as a velocity instead makes it an ordinary part of the tick — the drive can push back against
/// it, friction applies to it, and the hull travels the distance rather than being placed at the
/// end of it.
///
/// A fifth per tick: fast enough that a real overlap is gone in a few ticks, slow enough that the
/// correction never becomes the loudest thing in the frame.
const RECOVERY_RATE: f32 = 0.2;

/// Ceiling on that recovery, so a deep spawn overlap eases apart instead of launching.
const MAX_RECOVERY_MPS: f32 = 1.0;

/// Fraction of the remaining overlap removed per iteration. Below 1 so a crowd converges instead
/// of ringing; over [`ITERATIONS`] passes this clears essentially all of it within the tick.
const POSITION_CORRECTION: f32 = 0.85;

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
    /// A wreck still blocks, but nothing moves it: it takes part in every contact as an immovable
    /// obstacle, so a hull that runs into one is stopped by the contact rather than by a veto.
    pub movable: bool,
}

impl ContactBody {
    fn obstacle(&self) -> TankObstacle {
        TankObstacle::new(self.position, self.yaw_rad, self.footprint)
    }

    fn inverse_mass(&self) -> f32 {
        if self.movable { 1.0 / self.mass_kg.max(1.0) } else { 0.0 }
    }

    /// Planar moment of inertia of the hull footprint as a uniform rectangle about its centre.
    fn inverse_inertia(&self) -> f32 {
        let width = self.footprint.half_width_m * 2.0;
        let length = self.footprint.half_length_m * 2.0;
        if !self.movable {
            return 0.0;
        }
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

/// One PAIR's collision, as the solver resolved it. Ram damage reads these instead of measuring a
/// closing speed of its own against a slop radius of its own: the collision that hurts is the
/// collision the physics actually resolved, or the two answer differently and one of them is lying.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactPair {
    pub a: usize,
    pub b: usize,
    /// Total normal impulse exchanged, in newton-seconds — mass and closing speed already folded
    /// together, which is exactly what a collision's severity is.
    pub normal_impulse_ns: f32,
}

/// Everything one tick of contact produced: what each hull took, and what each pair exchanged.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContactReport {
    pub bodies: Vec<ContactImpulse>,
    pub pairs: Vec<ContactPair>,
}

/// Solve every hull-to-hull contact in the roster and report what each hull took. `bodies` is left
/// untouched; the caller applies the deltas, because the caller owns the authoritative state.
pub fn resolve_contacts(bodies: &[ContactBody], dt: f32) -> ContactReport {
    let mut out = vec![ContactImpulse::default(); bodies.len()];
    if bodies.len() < 2 {
        return ContactReport { bodies: out, pairs: Vec::new() };
    }
    let mut pairs: Vec<ContactPair> = Vec::new();
    // Working copy: iterations converge against the accumulating solution, but within a single
    // iteration every pair reads the SAME state, so the pass order cannot change the result.
    let mut velocity: Vec<Vec3> = bodies.iter().map(|body| body.velocity).collect();
    let mut yaw_rate: Vec<f32> = bodies.iter().map(|body| body.yaw_rate_rad_s).collect();

    for _ in 0..ITERATIONS {
        let mut delta_v = vec![Vec3::ZERO; bodies.len()];
        let mut delta_w = vec![0.0_f32; bodies.len()];
        for a in 0..bodies.len() {
            for b in a + 1..bodies.len() {
                if !worth_testing(&bodies[a], &bodies[b], dt) {
                    continue;
                }
                // Where the hulls ARE, plus how far apart they still are — signed, so the same
                // number covers "a hand apart" and "already inside each other". The pair is
                // reached for while there is still room to stop it: the margin carries the ground
                // both could cover before the next solve, so an approach is never seen for the
                // first time from the far side of the plate it should have hit.
                let reach =
                    SPECULATIVE_MARGIN_M + (velocity[a].length() + velocity[b].length()) * dt;
                let (here_a, here_b) = (bodies[a].obstacle(), bodies[b].obstacle());
                let Some(contact) = footprint_contact_within(&here_a, &here_b, reach) else {
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
                let delta =
                    Vec2::new(here_b.center.x - here_a.center.x, here_b.center.z - here_a.center.z);
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

                // How fast this pair is ALLOWED to close, which is the whole of the change.
                //
                // Still apart: exactly fast enough to shut the remaining gap by the end of the
                // tick and no faster. The hulls finish it TOUCHING instead of stopped a detection
                // margin short of each other, and nothing has to be stopped dead to get there — a
                // charge is trimmed to the gap, not cancelled.
                //
                // Already overlapped: the target turns positive and asks them to ease back out at
                // `RECOVERY_RATE` of the excess, as a velocity rather than as a teleport. Overlap
                // shallower than `POSITION_SLOP_M` is left alone, so a pair resting against each
                // other is not endlessly nudged.
                let separation = -contact.depth_m;
                let target = if separation > 0.0 {
                    -separation / dt
                } else {
                    (RECOVERY_RATE * (-separation - POSITION_SLOP_M).max(0.0) / dt)
                        .min(MAX_RECOVERY_MPS)
                };
                if approach >= target {
                    // Closing no faster than the gap allows, or already separating: a contact
                    // constraint pushes, it never pulls.
                    continue;
                }

                let (inv_ma, inv_mb) = (bodies[a].inverse_mass(), bodies[b].inverse_mass());
                let (inv_ia, inv_ib) = (bodies[a].inverse_inertia(), bodies[b].inverse_inertia());
                let effective =
                    inv_ma + inv_mb + lever_a * lever_a * inv_ia + lever_b * lever_b * inv_ib;
                if effective <= f32::EPSILON {
                    continue;
                }
                let magnitude = (target - approach * (1.0 + RESTITUTION)) / effective;
                let impulse = normal * magnitude;

                delta_v[a] -= Vec3::new(impulse.x, 0.0, impulse.y) * inv_ma;
                delta_v[b] += Vec3::new(impulse.x, 0.0, impulse.y) * inv_mb;
                delta_w[a] += lever_a * magnitude * inv_ia;
                delta_w[b] -= lever_b * magnitude * inv_ib;

                // Coulomb friction ACROSS the contact, bounded by what the normal impulse can
                // hold. This is what stops a pressed-together queue from sliding along itself.
                let sliding = relative.dot(tangent);
                let linear = inv_ma + inv_mb;
                if sliding.abs() > 1.0e-4 && linear > f32::EPSILON {
                    let wanted = -sliding / linear;
                    let cap = CONTACT_FRICTION_MU * magnitude;
                    let friction = wanted.clamp(-cap, cap);
                    let along = tangent * friction;
                    delta_v[a] -= Vec3::new(along.x, 0.0, along.y) * inv_ma;
                    delta_v[b] += Vec3::new(along.x, 0.0, along.y) * inv_mb;
                }

                out[a].normal_impulse_ns += magnitude;
                out[b].normal_impulse_ns += magnitude;
                match pairs.iter_mut().find(|pair| pair.a == a && pair.b == b) {
                    Some(pair) => pair.normal_impulse_ns += magnitude,
                    None => pairs.push(ContactPair { a, b, normal_impulse_ns: magnitude }),
                }
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
    ContactReport { bodies: out, pairs }
}

/// Push overlapping hulls apart, and report how far each one had to give.
///
/// With the tick in its honest order — integrate, exchange momentum, THEN separate — this is the
/// non-overlap guarantee itself, not a touch-up. It replaces `tank_resolve`'s "try each axis, then
/// hold the previous position", and with it the interpenetration ESCAPE routine that existed only
/// because nothing could push hulls apart: hulls that end a tick inside each other are simply
/// pushed out, along the shallowest axis, split by mass so the heavy hull barely gives ground.
///
/// Order-independent for the same reason the impulse pass is: each iteration reads one shared set
/// of positions and every correction is accumulated before any is applied.
pub fn separate_overlaps(bodies: &[ContactBody]) -> Vec<Vec3> {
    let mut total = vec![Vec3::ZERO; bodies.len()];
    if bodies.len() < 2 {
        return total;
    }
    let mut position: Vec<Vec3> = bodies.iter().map(|body| body.position).collect();
    for _ in 0..ITERATIONS {
        let mut delta = vec![Vec3::ZERO; bodies.len()];
        let mut any = false;
        for a in 0..bodies.len() {
            for b in a + 1..bodies.len() {
                if !worth_testing(&bodies[a], &bodies[b], 0.0) {
                    continue;
                }
                let moved_a = ContactBody { position: position[a], ..bodies[a] };
                let moved_b = ContactBody { position: position[b], ..bodies[b] };
                let Some(contact) = obstacles_contact(&moved_a.obstacle(), &moved_b.obstacle())
                else {
                    continue;
                };
                let excess = contact.depth_m - POSITION_SLOP_M;
                if excess <= 0.0 {
                    continue;
                }
                let (inv_ma, inv_mb) = (moved_a.inverse_mass(), moved_b.inverse_mass());
                let share = inv_ma + inv_mb;
                if share <= f32::EPSILON {
                    // Two immovable hulls: nothing to do, and nothing that could be done.
                    continue;
                }
                any = true;
                let push = contact.normal * (excess * POSITION_CORRECTION / share);
                let push = Vec3::new(push.x, 0.0, push.y);
                delta[a] -= push * inv_ma;
                delta[b] += push * inv_mb;
            }
        }
        if !any {
            break;
        }
        for index in 0..bodies.len() {
            position[index] += delta[index];
            total[index] += delta[index];
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn hull(x: f32, z: f32, yaw_rad: f32, velocity: Vec3, mass_kg: f32) -> ContactBody {
        ContactBody {
            position: Vec3::new(x, 0.0, z),
            velocity,
            yaw_rad,
            yaw_rate_rad_s: 0.0,
            footprint: TankFootprint { half_width_m: 1.75, half_length_m: 3.2 },
            mass_kg,
            movable: true,
        }
    }

    /// The headline: a charging hull SHOVES a parked one. Before this the charger simply stopped
    /// and the parked hull never learned it had been hit.
    #[test]
    fn a_charging_hull_pushes_a_parked_one_and_pays_for_it() {
        let charger = hull(0.0, 0.0, 0.0, Vec3::new(0.0, 0.0, 12.0), 36_000.0);
        let parked = hull(0.0, 6.0, 0.0, Vec3::ZERO, 36_000.0);
        let impulses = resolve_contacts(&[charger, parked], DT).bodies;

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
        let impulses = resolve_contacts(&[light, heavy], DT).bodies;

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
        let spun = resolve_contacts(&[charger, offset], DT).bodies[1].delta_yaw_rate_rad_s.abs();

        let centred = hull(0.0, 4.4, std::f32::consts::FRAC_PI_2, Vec3::ZERO, 36_000.0);
        let square = resolve_contacts(&[charger, centred], DT).bodies[1].delta_yaw_rate_rad_s.abs();

        assert!(spun > 0.05, "a nose-on t-bone must slew the victim, got {spun} rad/s");
        assert!(square < spun * 0.25, "a centred hit must barely spin it: {square} vs {spun}");
    }

    /// Roster order must not reach the result. This is the property `ramming.rs` had to be fixed
    /// for, and a sequential solver would have reintroduced it — so it is locked here too.
    #[test]
    fn the_result_does_not_depend_on_roster_order() {
        let a = hull(0.0, 0.0, 0.3, Vec3::new(1.0, 0.0, 11.0), 34_000.0);
        let b = hull(0.6, 5.8, 1.1, Vec3::new(0.0, 0.0, -2.0), 48_000.0);

        let forward = resolve_contacts(&[a, b], DT).bodies;
        let reversed = resolve_contacts(&[b, a], DT).bodies;

        assert_eq!(forward[0], reversed[1], "the same hull must take the same impulse");
        assert_eq!(forward[1], reversed[0]);
    }

    /// A contact pushes; it never pulls. Two hulls already flying apart take nothing, however
    /// deeply their footprints still overlap.
    #[test]
    fn separating_hulls_are_left_alone() {
        let a = hull(0.0, 0.0, 0.0, Vec3::new(0.0, 0.0, -6.0), 36_000.0);
        let b = hull(0.0, 5.5, 0.0, Vec3::new(0.0, 0.0, 6.0), 36_000.0);
        let impulses = resolve_contacts(&[a, b], DT).bodies;
        assert_eq!(impulses[0].normal_impulse_ns, 0.0);
        assert_eq!(impulses[1].normal_impulse_ns, 0.0);
        assert_eq!(impulses[0].delta_velocity, Vec3::ZERO);
    }

    /// Overlapping hulls are pushed apart along the shallowest axis, split by mass — the heavy
    /// one barely gives ground, the light one yields — and the pass actually CLEARS the overlap
    /// rather than merely easing it, because with the tick in its honest order this pass IS the
    /// non-overlap guarantee.
    #[test]
    fn the_separation_pass_clears_an_overlap_and_splits_it_by_mass() {
        let light = hull(0.0, 0.0, 0.0, Vec3::ZERO, 20_000.0);
        let heavy = hull(0.0, 5.0, 0.0, Vec3::ZERO, 60_000.0);
        let pushed = separate_overlaps(&[light, heavy]);

        assert!(pushed[0].z < 0.0, "the light hull backs off");
        assert!(pushed[1].z > 0.0, "the heavy one gives a little");
        assert!(
            pushed[0].length() > pushed[1].length() * 2.0,
            "and the light hull does most of the moving: {:?} vs {:?}",
            pushed[0],
            pushed[1]
        );

        let settled_light = ContactBody { position: light.position + pushed[0], ..light };
        let settled_heavy = ContactBody { position: heavy.position + pushed[1], ..heavy };
        assert!(
            obstacles_contact(&settled_light.obstacle(), &settled_heavy.obstacle())
                .is_none_or(|contact| contact.depth_m <= POSITION_SLOP_M * 2.0),
            "the pass must leave the pair essentially clear"
        );
    }

    /// A wreck blocks but never budges: the living hull does all the giving.
    #[test]
    fn an_immovable_hull_takes_neither_impulse_nor_ground() {
        let mut wreck = hull(0.0, 5.0, 0.0, Vec3::ZERO, 36_000.0);
        wreck.movable = false;
        let charger = hull(0.0, 0.0, 0.0, Vec3::new(0.0, 0.0, 12.0), 36_000.0);

        let impulses = resolve_contacts(&[charger, wreck], DT).bodies;
        assert_eq!(impulses[1].delta_velocity, Vec3::ZERO, "a wreck is not shoved");
        assert!(impulses[0].delta_velocity.z < -0.5, "and the charger is stopped by it");

        let pushed = separate_overlaps(&[charger, wreck]);
        assert_eq!(pushed[1], Vec3::ZERO, "a wreck never gives ground");
        assert!(pushed[0].z < 0.0, "the living hull backs out of it");
    }

    /// A hull touching nobody is never touched.
    #[test]
    fn a_lone_hull_takes_nothing() {
        let impulses =
            resolve_contacts(&[hull(0.0, 0.0, 0.0, Vec3::new(0.0, 0.0, 12.0), 36_000.0)], DT)
                .bodies;
        assert!(impulses[0].is_empty());
        assert_eq!(impulses[0].delta_velocity, Vec3::ZERO);
    }
}
