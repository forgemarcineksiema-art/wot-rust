//! What a hull turns ABOUT, which is a fact about its gearbox.
//!
//! P4.5 of `docs/contact-and-tracks-program.md`. Every vehicle in this game pivots on the spot the
//! same way — both belts counter-rotating about the hull's centre — because the drive model was
//! given one turn rate per suspension module and no reason to ask what produced it. Most of these
//! tanks could not do that.
//!
//! A steering mechanism that can drive one track BACKWARDS while the other goes forward turns the
//! hull about its own centre. One that can only slow or stop a track turns the hull about that
//! track: it still spins on the spot, but around a point a half-gauge off centre, at half the rate,
//! and it creeps forward while it does it. That is a different manoeuvre in a tight street, and
//! whether a vehicle has it is documented per vehicle rather than being a balance knob.
//!
//! **The split is by design school, not by era, and that surprised the plan.** The program sketched
//! three kinds mapped onto the three eras — later meaning better. The research says otherwise: the
//! 1942 Tiger I turns about its centre and the 1951 T-54 does not. What separates them is that the
//! British Merritt-Brown triple differential and the Argus unit Henschel derived from it are
//! regenerative, while the Soviet school standardised on two-stage planetary side mechanisms.

use crate::vehicle_kind::VehicleKind;

/// Which point a hull rotates about when it turns on the spot.
///
/// Two variants, not five, on purpose: these are the two behaviours the sources actually
/// distinguish. A single-radius German gearbox, a Soviet two-stage planetary mechanism and a
/// clutch-and-brake T-34 differ in how gracefully they give up power, but none of them can reverse
/// a track, so all three swing the hull about the inner belt. Splitting them further would be
/// inventing distinctions the evidence does not carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SteeringKind {
    /// Regenerative or double/triple-differential: one track forward, one back, and the hull spins
    /// about its own centre without going anywhere.
    Regenerative,
    /// Everything that can only slow or stop a track: the hull swings about the inner belt.
    BrakedTrack,
}

impl SteeringKind {
    /// True when the hull turns about its own centre rather than about a track.
    pub fn counter_rotates(self) -> bool {
        matches!(self, Self::Regenerative)
    }

    /// What this vehicle's gearbox actually is, with the source of the claim.
    ///
    /// * **Tiger I** — the Argus/Henschel unit, derived from the Merritt-Brown type and mounted
    ///   transversely in the bow, "fully regenerative and continuous": in neutral gear the tracks
    ///   could be turned in opposite directions, so the Tiger I pivoted in place. → `Regenerative`.
    /// * **Centurion** — Merritt-Brown Z51R triple differential, continuously variable steering
    ///   that performs "a neutral turn on the spot by rotating its tracks in opposite directions".
    ///   → `Regenerative`.
    /// * **T-54** — a two-stage planetary steering gear working with single-stage final drives.
    ///   It slows the inner track; it cannot reverse it. → `BrakedTrack`.
    /// * **IS-3** — the same two-stage planetary side mechanisms, at the ends of the main shaft,
    ///   with multi-disc dry locking clutches and band brakes (three sources; see
    ///   `docs/vehicles/is-3.md`). → `BrakedTrack`.
    /// * **T-34-85** — clutch-and-brake side clutches: pulling a lever disengages that track's
    ///   clutch and then brakes it, and because the power is REMOVED from the inner track rather
    ///   than reversed, it cannot perform a true neutral steer. → `BrakedTrack`.
    /// * **Panther II** — the MAN single-radius steering system behind the ZF AK 7-200: one fixed
    ///   radius per gear, tighter turns on the brakes. → `BrakedTrack`.
    /// * **Tiger II and Jagdtiger** — the L 801 *Zweiradienlenkgetriebe*, a double-RADIUS unit
    ///   quoted at a 2.08 m minimum turning radius. A vehicle whose tightest turn is quoted as a
    ///   radius wider than its own half-width is not counter-rotating. This one is inferred from
    ///   the stated minimum rather than from an explicit statement about neutral steer, and it is
    ///   the weakest claim in this list. → `BrakedTrack`.
    pub fn for_vehicle(kind: VehicleKind) -> Self {
        match kind {
            VehicleKind::TigerI | VehicleKind::Centurion => Self::Regenerative,
            VehicleKind::T54_1951
            | VehicleKind::IS3
            | VehicleKind::T34_85
            | VehicleKind::PantherII
            | VehicleKind::TigerII
            | VehicleKind::Jagdtiger => Self::BrakedTrack,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every playable vehicle has an answer, and the answer came from a source rather than from a
    /// fall-through. A new vehicle joining the roster must be researched, not defaulted.
    #[test]
    fn every_playable_vehicle_has_a_researched_gearbox() {
        for kind in VehicleKind::PLAYABLE {
            let steering = SteeringKind::for_vehicle(kind);
            let expected = match kind {
                VehicleKind::TigerI | VehicleKind::Centurion => SteeringKind::Regenerative,
                _ => SteeringKind::BrakedTrack,
            };
            assert_eq!(steering, expected, "{kind:?} fell through to a default");
        }
    }

    /// The split runs along design schools, not along the era ladder — the finding that corrected
    /// the program's own sketch. A 1942 heavy counter-rotates where a 1951 medium cannot.
    #[test]
    fn the_split_is_not_the_era_ladder() {
        assert!(SteeringKind::for_vehicle(VehicleKind::TigerI).counter_rotates());
        assert!(!SteeringKind::for_vehicle(VehicleKind::T54_1951).counter_rotates());
    }
}
