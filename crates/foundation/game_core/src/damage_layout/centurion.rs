//! Centurion Mk 3 component placement.
//!
//! A late-war British design that answered the question the T-34 and the Tiger had answered
//! badly, and it shows in where things are: the Meteor and the Merritt-Brown box are sealed in a
//! rear compartment behind a firewall, the fuel is back there with them rather than beside the
//! crew, and the 20-pdr's ammunition lies low in armoured bins on the hull floor.
//!
//! The result is a hull with no cheap flank — the sponsons carry Horstmann bogies and stowage,
//! not tanks and racks. What it costs is length: this is the longest hull in the fleet, and the
//! shot that does find the engine bay has a large target.

use super::authoring::{
    DriveEnd, HullEnvelope, TurretFit, final_drive_pair, flank_fuel_pair, hull_component, obb,
    suspension_pair, turret_group,
};
use super::{DamageComponentKind as K, DamageLayout, DamageMaterial as M};
use crate::ModuleSlot;

pub(super) fn layout(env: &HullEnvelope) -> DamageLayout {
    // The 20-pdr in a roomy British turret with a long bustle.
    let mut components = turret_group(
        env,
        TurretFit { breech_fill: 0.9, turret_drive: true, bustle_rack: Some(0.85) },
    );
    components.extend(hull_components(env));
    components.extend(flank_fuel_pair(env, [8, 9], [0.20, 0.32, 0.46], 0.34));
    components.extend(final_drive_pair(env, [12, 13], DriveEnd::Rear, 0.23));
    components.extend(suspension_pair(env, [14, 15], 0.19));
    DamageLayout { components }
}

fn hull_components(env: &HullEnvelope) -> Vec<super::DamageComponent> {
    vec![
        // Armoured floor bins beside and behind the driver — the main 20-pdr stowage, kept low.
        hull_component(
            5,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            obb([0.0, env.standing_on_floor(0.28), 0.62], [0.66, 0.28, 0.76], 0.0),
            32,
            1.35,
        ),
        // A second bin on the loader's side of the fighting compartment.
        hull_component(
            16,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            obb(
                [
                    env.against_wall_at(env.standing_on_floor(0.52), -0.42, 0.20),
                    env.standing_on_floor(0.26),
                    -0.42,
                ],
                [0.20, 0.26, 0.46],
                0.0,
            ),
            32,
            1.35,
        ),
        // Fuel behind the firewall, with the engine rather than with the crew.
        // The Rolls-Royce Meteor.
        hull_component(
            10,
            K::Engine,
            ModuleSlot::Engine,
            M::Machinery,
            obb(
                [
                    0.0,
                    env.standing_on_floor(0.44),
                    env.fit_station(
                        env.along_hull(0.19),
                        0.72,
                        env.floor_y,
                        env.standing_on_floor(2.0 * 0.44),
                    ),
                ],
                [0.60, 0.44, 0.72],
                0.0,
            ),
            30,
            0.9,
        ),
        // The Merritt-Brown box in the tail, behind the engine.
        hull_component(
            11,
            K::Transmission,
            ModuleSlot::Engine,
            M::Driveline,
            obb(
                [
                    0.0,
                    env.standing_on_floor(0.38),
                    env.fit_station(-env.half_len, 0.34, env.floor_y, env.standing_on_floor(0.76)),
                ],
                [0.64, 0.38, 0.34],
                0.0,
            ),
            30,
            0.9,
        ),
    ]
}
