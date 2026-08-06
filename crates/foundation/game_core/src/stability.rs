//! Where a loaded hull's mass sits, and the lever that resists tipping it over.
//!
//! P0.2 of `docs/contact-and-tracks-program.md`. Rollover is not switched off in this game by
//! decree — it is in the model and provably out of reach, and "provably" needs two numbers per
//! vehicle: how high the mass sits, and how far out the track edge it tips about is.
//!
//! The centre-of-mass height is DERIVED, not authored, and that is a deliberate choice. Published
//! centre-of-gravity figures for these vehicles are scarce and inconsistent, so eight hand-typed
//! numbers would be eight numbers nobody can check — the failure this codebase has already paid
//! for once (an instrument calibrated against the thing it is meant to judge cannot report the
//! judgement). Every input here is instead something the repo already researched 1:1: the
//! blueprint's own heights, and the installed modules' own masses. Swap a heavier turret in the
//! garage and the centre of mass rises, because it does.
//!
//! **Every part is placed at the mid-height of its own volume**, and that rule is chosen for its
//! BIAS as much as its simplicity. A real turret casting is heaviest at the ring — thick cheeks, a
//! wide base, a thin roof — so its true centroid sits below mid-height; likewise an engine sits on
//! the hull floor rather than halfway up the fighting compartment. Mid-height therefore reads the
//! centre of mass HIGHER than it really is, which is the pessimistic direction for every question
//! this module exists to answer. A gate that clears with an inflated centre of mass clears with
//! the real one.

use crate::modules::VehicleModules;
use crate::vehicle_blueprint::VehicleBlueprint;
use crate::vehicle_kind::VehicleKind;

/// The mass distribution a rollover argument needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stability {
    /// Height of the centre of mass above the ground, in metres.
    pub com_height_m: f32,
    /// Distance from the centreline to the outer edge of the track's ground contact — the edge the
    /// hull would tip about, and the lever that resists it.
    pub tip_edge_m: f32,
    /// Combat mass the distribution was taken over, in kilograms.
    pub mass_kg: f32,
}

impl Stability {
    /// Lateral acceleration, as a multiple of gravity, at which the inner track starts to lift.
    ///
    /// This is the vehicle-dynamics static stability factor. A sports car sits near 1.4 and a
    /// loaded truck near 0.7; a tank's wide track and low hull put it above both, which is the
    /// whole reason tanks do not roll over in turns.
    pub fn tipping_threshold_g(&self) -> f32 {
        self.tip_edge_m / self.com_height_m.max(1.0e-3)
    }

    /// Side slope, as rise/run, that would tip a parked hull. Equal to the tipping threshold —
    /// the same lever ratio, read as a gradient instead of an acceleration.
    pub fn tipping_grade(&self) -> f32 {
        self.tipping_threshold_g()
    }
}

/// The mass distribution of one loadout on one vehicle, or `None` for a vehicle with no blueprint
/// to take heights from (the test-only prototype).
pub fn stability(kind: VehicleKind, modules: &VehicleModules) -> Option<Stability> {
    let blueprint = VehicleBlueprint::for_vehicle(kind)?;
    Some(stability_from(&blueprint, modules))
}

/// The default loadout's distribution — what the vehicle rolls out of the hangar as.
pub fn stock_stability(kind: VehicleKind) -> Option<Stability> {
    stability(kind, &kind.default_loadout())
}

fn stability_from(blueprint: &VehicleBlueprint, modules: &VehicleModules) -> Stability {
    let hull = &blueprint.hull;
    let track = &blueprint.track;
    let turret = &blueprint.turret;

    // The hull structure, the powerplant and the radio all live inside the same armoured box
    // between the belly plate and the deck. Separating them would need a centroid for each, and
    // the engine and radio together are a few percent of the combat mass — a precision this
    // derivation does not have and does not need.
    let hull_box = midpoint(hull.belly_y, hull.deck_y);
    let hull_mass = modules.hull.mass_kg + modules.engine.mass_kg + modules.radio.mass_kg;

    // The running gear is the belt band and the wheels inside it: bottom run to top run.
    let gear = midpoint(track.bottom_y, track.top_y);

    // The casting, from its ring to its roof. The cupola stands above `roof_y` and is left out —
    // it is a drum of hatch armour, not a share of the turret's tonnage.
    let casting = midpoint(turret.ring_y, turret.roof_y);

    // The gun hangs on its trunnions, which is where its mass acts.
    let barrel = blueprint.gun.trunnion_y;

    let mass_kg = modules.total_mass_kg();
    let moment = hull_mass * hull_box
        + modules.suspension.mass_kg * gear
        + modules.turret.mass_kg * casting
        + modules.gun.mass_kg * barrel;

    Stability {
        com_height_m: moment / mass_kg.max(1.0),
        // The widest metal that touches the ground: the outer face of the belt.
        tip_edge_m: track.outer_x,
        mass_kg,
    }
}

fn midpoint(low: f32, high: f32) -> f32 {
    0.5 * (low + high)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The derivation must place the mass INSIDE the vehicle, and low. A tracked hull carries its
    /// tonnage in the floor, the belts and the lower plates, so the centre of mass belongs in the
    /// lower half of the silhouette — and this rule biases high, so the upper bound is the one
    /// worth holding.
    #[test]
    fn the_centre_of_mass_lands_in_the_lower_half_of_every_hull() {
        for kind in VehicleKind::PLAYABLE {
            let stability = stock_stability(kind).expect("every playable vehicle has a blueprint");
            let blueprint = VehicleBlueprint::for_vehicle(kind).expect("blueprint");
            let apex = blueprint.turret.roof_y;
            let fraction = stability.com_height_m / apex;
            assert!(
                (0.35..0.55).contains(&fraction),
                "{kind:?}: centre of mass at {:.3} m is {fraction:.2} of the {apex:.2} m \
                 silhouette — outside the band a tracked hull can sit in",
                stability.com_height_m
            );
        }
    }

    /// A heavier turret raises the centre of mass, because it does. This is the property that
    /// makes deriving worth more than authoring: the number follows the garage.
    #[test]
    fn a_heavier_turret_raises_the_centre_of_mass() {
        let kind = VehicleKind::T54_1951;
        let stock = kind.default_loadout();
        let baseline = stability(kind, &stock).expect("blueprint");

        let mut heavier = stock.clone();
        heavier.turret.mass_kg += 2_000.0;
        let loaded = stability(kind, &heavier).expect("blueprint");

        assert!(
            loaded.com_height_m > baseline.com_height_m,
            "two more tonnes of turret must raise the mass: {:.4} vs {:.4}",
            loaded.com_height_m,
            baseline.com_height_m
        );
        assert!(
            loaded.tipping_threshold_g() < baseline.tipping_threshold_g(),
            "...and a raised mass must tip more easily"
        );
    }

    /// The tipping edge is the widest metal on the ground, not the collision box. The box is wider
    /// than the vehicle today (register M14) and a lever measured on phantom width would flatter
    /// every hull in the fleet.
    #[test]
    fn the_tipping_edge_is_the_track_not_the_hitbox() {
        for kind in VehicleKind::PLAYABLE {
            let stability = stock_stability(kind).expect("blueprint");
            let blueprint = VehicleBlueprint::for_vehicle(kind).expect("blueprint");
            assert_eq!(stability.tip_edge_m, blueprint.track.outer_x);
            assert!(
                stability.tip_edge_m <= blueprint.hull.hitbox_half_width,
                "{kind:?}: the belt cannot reach past the collision box"
            );
        }
    }
}
