//! IS-3 component placement.
//!
//! The D-25T fires two-piece ammunition, and that single fact shapes this layout. Shells and
//! propellant charges are stowed SEPARATELY: the projectiles lie low in the hull where their
//! weight belongs, while the charges — the part that actually burns — ride in the turret bustle
//! and against the rear bulkhead.
//!
//! Both are `AmmunitionRack` to the repair layer, because a crew fighting a rack fire does not
//! care which half went up. They are distinct physical components because a shell that reaches the
//! charges and one that reaches the projectiles are not the same event, and the through-flight
//! model can tell them apart.

use super::authoring::{
    DriveEnd, HullEnvelope, TurretFit, final_drive_pair, flank_fuel_pair, hull_component, obb,
    suspension_pair, turret_group,
};
use super::{DamageComponentKind as K, DamageLayout, DamageMaterial as M};
use crate::ModuleSlot;

pub(super) fn layout(env: &HullEnvelope) -> DamageLayout {
    // A 122 mm breech in a flat, wide casting: the gun fills it.
    let mut components = turret_group(
        env,
        TurretFit { breech_fill: 1.0, turret_drive: true, bustle_rack: Some(1.0) },
    );
    components.extend(hull_components(env));
    components.extend(flank_fuel_pair(env, [8, 9], [0.22, 0.34, 0.50], 0.3));
    components.extend(final_drive_pair(env, [12, 13], DriveEnd::Rear, 0.24));
    components.extend(suspension_pair(env, [14, 15], 0.18));
    DamageLayout { components }
}

fn hull_components(env: &HullEnvelope) -> Vec<super::DamageComponent> {
    let sponson_half_height = ((env.deck_y - env.sponson_y) * 0.5 - 0.04).max(0.10);
    let sponson_center_y = env.sponson_y + sponson_half_height + 0.02;
    vec![
        // Projectiles racked upright on the hull floor under the ring, where their weight sits low.
        hull_component(
            5,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            obb([0.0, env.standing_on_floor(0.30), 0.24], [0.62, 0.30, 0.62], 0.0),
            32,
            1.35,
        ),
        // The rest of the charges against the engine-compartment bulkhead.
        hull_component(
            16,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            obb(
                [0.0, sponson_center_y, env.along_hull(0.32)],
                [0.52, sponson_half_height, 0.16],
                0.0,
            ),
            32,
            1.35,
        ),
        // The V-11 in the rear bay.
        hull_component(
            10,
            K::Engine,
            ModuleSlot::Engine,
            M::Machinery,
            obb(
                [
                    0.0,
                    env.standing_on_floor(0.40),
                    env.fit_station(
                        env.along_hull(0.17),
                        0.62,
                        env.floor_y,
                        env.standing_on_floor(0.80),
                    ),
                ],
                [0.58, 0.40, 0.62],
                0.0,
            ),
            30,
            0.9,
        ),
        hull_component(
            11,
            K::Transmission,
            ModuleSlot::Engine,
            M::Driveline,
            obb(
                [
                    0.0,
                    env.standing_on_floor(0.36),
                    env.fit_station(-env.half_len, 0.32, env.floor_y, env.standing_on_floor(0.72)),
                ],
                [0.64, 0.36, 0.32],
                0.0,
            ),
            30,
            0.9,
        ),
    ]
}
