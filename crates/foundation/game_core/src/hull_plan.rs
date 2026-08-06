//! The rectangle a hull MOVES as, which is not the box it is SHOT AT.
//!
//! P2.1 of `docs/contact-and-tracks-program.md`. Until now one number did both jobs: the
//! `HitboxProfile` plan decided where shells connect AND where the hull could be. That is one job
//! too many for a box that is deliberately generous — a shell volume wants a hair of margin around
//! the metal so a graze registers, and a movement footprint wants none at all, because every
//! millimetre of margin is a millimetre of daylight between two tanks that are supposed to be
//! touching.
//!
//! Measured before this existed: a T-54's collision box reaches ±1.750 m while the widest thing
//! drawn — the outer face of the track belt — reaches ±1.610, and the box runs to ±3.2675 against
//! hull plates that stop at ±3.1175. Fourteen centimetres of invisible width per side and fifteen
//! per end, which two tanks parked side by side each contribute to: 0.28 m of air that nothing
//! explains. The register has carried it as M14 since 2026-07-29 with a ceiling instead of a fix,
//! because narrowing the box moved ramming, spotting and terrain contact all at once.
//!
//! Splitting the two questions is what makes it safe. Nothing about shells changes here: the
//! hitbox is untouched and still owns hit resolution. What changes is that the hull now MOVES as
//! the metal it is drawn with.

use crate::vehicle_blueprint::VehicleBlueprint;
use crate::{HitboxProfile, VehicleKind};

/// The hull's plan on the ground — the oriented rectangle movement and contact resolve against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HullPlan {
    /// Half the widest metal that reaches the ground: the outer face of the track belt.
    pub half_width_m: f32,
    /// Half the hull's own length, over the plates. The gun is not part of it — it is a ghost by
    /// decision (`docs/contact-and-tracks-program.md`), and a barrel that shoved hulls around would
    /// be a five-metre lever on a thirty-six-tonne body in a planar solver.
    pub half_length_m: f32,
}

impl HullPlan {
    /// The plan a vehicle drives as, taken from the same blueprint the mesh is built from — so
    /// there is no second number to keep in step with the first. Vehicles without a blueprint (the
    /// test-only prototype) fall back to the hitbox plan, which is what they used to use.
    pub fn for_vehicle(kind: VehicleKind) -> Self {
        match VehicleBlueprint::for_vehicle(kind) {
            Some(blueprint) => Self {
                half_width_m: blueprint.track.outer_x,
                half_length_m: blueprint.hull.half_len,
            },
            None => Self::from_hitbox(&HitboxProfile::for_vehicle(kind)),
        }
    }

    /// The plan a hitbox implies — the pre-split behaviour, kept for vehicles with no shape to
    /// read and for anything that genuinely wants the shell volume's footprint.
    pub fn from_hitbox(hitbox: &HitboxProfile) -> Self {
        Self { half_width_m: hitbox.half_width_m, half_length_m: hitbox.half_length_m }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plan is the METAL, and the hitbox is allowed to be generous around it — but never the
    /// other way round. A movement footprint reaching past the shell volume would let a hull be
    /// stopped by something no shell could hit.
    #[test]
    fn every_hull_moves_as_metal_that_fits_inside_its_shell_volume() {
        for kind in VehicleKind::PLAYABLE {
            let plan = HullPlan::for_vehicle(kind);
            let hitbox = HitboxProfile::for_vehicle(kind);
            let blueprint =
                VehicleBlueprint::for_vehicle(kind).expect("playable vehicles author one");

            assert_eq!(plan.half_width_m, blueprint.track.outer_x, "{kind:?} width is the belt");
            assert_eq!(
                plan.half_length_m, blueprint.hull.half_len,
                "{kind:?} length is the plates"
            );
            assert!(
                plan.half_width_m <= hitbox.half_width_m
                    && plan.half_length_m <= hitbox.half_length_m,
                "{kind:?} moves as something bigger than it can be shot at"
            );
        }
    }

    /// How much air the split takes out from between two hulls parked side by side, per vehicle.
    /// Printed rather than asserted to a figure: the number belongs to the blueprint, and pinning
    /// it here would be a second copy of it.
    #[test]
    fn the_air_between_two_parked_hulls_is_reported() {
        for kind in VehicleKind::PLAYABLE {
            let plan = HullPlan::for_vehicle(kind);
            let hitbox = HitboxProfile::for_vehicle(kind);
            println!(
                "{kind:?}: side {:.3} m of phantom removed, end {:.3} m",
                2.0 * (hitbox.half_width_m - plan.half_width_m),
                2.0 * (hitbox.half_length_m - plan.half_length_m)
            );
        }
    }
}
