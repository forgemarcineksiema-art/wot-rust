//! T-34-85 component placement.
//!
//! The layout that defines the tank's reputation. Two facts drive everything here:
//!
//!   * the V-2 sits transverse-free in a rear bay behind a bulkhead, with the gearbox behind IT
//!     and the sprockets on the tail — so nothing of the driveline crosses the crew space, and a
//!     bow penetration meets ammunition and men, never the transmission;
//!   * the fuel does NOT live in the engine bay. Both main tanks stand in the fighting-compartment
//!     sponsons, alongside the crew, which is why a T-34 struck in the flank so often burns. That
//!     is not a balance decision here; it is where the tanks were, and the honest consequence is
//!     that this hull's sides are its worst face.

use super::authoring::{
    CrewPlan, DriveEnd, HullEnvelope, TurretFit, crew_stations_from_plan, final_drive_pair,
    hull_component, obb, suspension_pair, turret_group,
};
use super::{DamageComponentKind as K, DamageLayout, DamageMaterial as M};
use crate::ModuleSlot;

pub(super) fn layout(env: &HullEnvelope) -> DamageLayout {
    // The 85 mm in a casting that was famously tight for three men.
    let mut components = turret_group(
        env,
        TurretFit { breech_fill: 0.86, turret_drive: true, bustle_rack: Some(0.9) },
    );
    components.extend(hull_components(env));
    components.extend(final_drive_pair(env, [12, 13], DriveEnd::Rear, 0.22));
    components.extend(suspension_pair(env, [14, 15], 0.18));
    // Five men: driver and the bow radio-operator/hull-gunner up front (the famous MG port in
    // the glacis is HIS), the 85 mm tower's three behind them — beside the sponson fuel, which
    // is the whole story of this hull's flank.
    components.extend(crew_stations_from_plan(
        env,
        18,
        CrewPlan { driver_x_sign: 1.0, bow_radio_operator: true, gunner_x_sign: 1.0 },
    ));
    DamageLayout { components }
}

fn hull_components(env: &HullEnvelope) -> Vec<super::DamageComponent> {
    let sponson_half_height = ((env.deck_y - env.sponson_y) * 0.5 - 0.04).max(0.10);
    let sponson_center_y = env.sponson_y + sponson_half_height + 0.02;
    vec![
        // The floor bins under the fighting compartment: the T-34's main stowage, laid flat in
        // the belly where a lower-plate penetration walks straight into it.
        hull_component(
            5,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            obb([0.0, env.standing_on_floor(0.15), 0.30], [0.70, 0.15, 0.82], 0.0),
            32,
            1.35,
        ),
        // Ready rounds clipped along the loader's wall, above the sponson step.
        hull_component(
            16,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            obb(
                [
                    env.against_wall_at(sponson_center_y + sponson_half_height, 0.05, 0.11),
                    sponson_center_y,
                    0.05,
                ],
                [0.11, sponson_half_height * 0.8, 0.58],
                0.0,
            ),
            32,
            1.35,
        ),
        // Both fighting-compartment fuel tanks, out in the sponsons beside the crew. The reason
        // this hull's flank is the one to shoot at.
        fuel_sponson(env, 8, -1.0, sponson_center_y, sponson_half_height),
        fuel_sponson(env, 9, 1.0, sponson_center_y, sponson_half_height),
        // The V-2-34, longitudinal in the rear bay.
        hull_component(
            10,
            K::Engine,
            ModuleSlot::Engine,
            M::Machinery,
            obb(
                [
                    0.0,
                    env.standing_on_floor(0.42),
                    env.fit_station(
                        env.along_hull(0.18),
                        0.58,
                        env.floor_y,
                        env.standing_on_floor(2.0 * 0.42),
                    ),
                ],
                [0.55, 0.42, 0.58],
                0.0,
            ),
            30,
            0.9,
        ),
        // The gearbox behind the engine, hard against the tail plate.
        hull_component(
            11,
            K::Transmission,
            ModuleSlot::Engine,
            M::Driveline,
            obb(
                [
                    0.0,
                    env.standing_on_floor(0.38),
                    env.fit_station(-env.half_len, 0.30, env.floor_y, env.standing_on_floor(0.76)),
                ],
                [0.62, 0.38, 0.30],
                0.0,
            ),
            30,
            0.9,
        ),
    ]
}

fn fuel_sponson(
    env: &HullEnvelope,
    id: u16,
    side: f32,
    center_y: f32,
    half_height: f32,
) -> super::DamageComponent {
    let half_width = 0.19;
    hull_component(
        id,
        K::FuelTank,
        ModuleSlot::Engine,
        M::Fuel,
        obb(
            [
                side * env.against_wall_at(center_y + half_height, 0.0, half_width),
                center_y,
                env.fit_station(0.62, 0.62, center_y - half_height, center_y + half_height),
            ],
            [half_width, half_height, 0.62],
            0.0,
        ),
        25,
        1.1,
    )
}
