//! The bays shared by every German hull in this fleet.
//!
//! All four run the same architecture, and it is the opposite of the Soviet one: the engine is in
//! the tail, the GEARBOX IS IN THE BOW between the driver's knees, and a propeller shaft joins
//! them down the centreline, under the fighting-compartment floor.
//!
//! That single decision produces most of the gameplay difference between the two schools:
//!
//!   * a bow penetration that would meet only ammunition on a T-34 meets the transmission here,
//!     so a hit that fails to kill still immobilises;
//!   * the shaft tunnel raises the fighting-compartment floor, which is why the ammunition went
//!     out into the SPONSONS instead of lying in the belly — and why a flank hit above the tracks
//!     finds rounds rather than empty air;
//!   * the fuel stays back with the engine, behind a firewall, so these hulls do not burn from a
//!     side hit the way a T-34 does. They lose their gearbox instead.

use glam::Vec3;

use super::authoring::{
    DriveEnd, HullEnvelope, cylinder_shape, flank_fuel_pair, hull_component, obb,
};
use super::{DamageComponent, DamageComponentKind as K, DamageMaterial as M};
use crate::ModuleSlot;

/// The rear engine, the bow gearbox and the shaft between them.
pub(super) fn powertrain(env: &HullEnvelope, engine_half: [f32; 3]) -> Vec<DamageComponent> {
    let gearbox_half = [env.tub_half_width - 0.14, 0.34, 0.34];
    vec![
        hull_component(
            10,
            K::Engine,
            ModuleSlot::Engine,
            M::Machinery,
            obb(
                [
                    0.0,
                    env.standing_on_floor(engine_half[1]),
                    env.fit_station(
                        env.along_hull(0.17),
                        engine_half[2],
                        env.floor_y,
                        env.floor_y + 2.0 * engine_half[1],
                    ),
                ],
                engine_half,
                0.0,
            ),
            30,
            0.9,
        ),
        // The gearbox in the nose, ahead of the driver's feet and behind the glacis.
        hull_component(
            11,
            K::Transmission,
            ModuleSlot::Engine,
            M::Driveline,
            obb(
                [
                    0.0,
                    env.standing_on_floor(gearbox_half[1]),
                    env.fit_station(
                        env.half_len,
                        gearbox_half[2],
                        env.floor_y,
                        env.floor_y + 2.0 * gearbox_half[1],
                    ),
                ],
                gearbox_half,
                0.0,
            ),
            30,
            0.9,
        ),
        // The propeller shaft, low on the centreline, running the length of the crew space. Thin
        // and hard to hit on purpose — but a shell that does find it takes the drive with it.
        hull_component(
            17,
            K::Transmission,
            ModuleSlot::Engine,
            M::Driveline,
            cylinder_shape(
                [0.0, env.standing_on_floor(0.13), env.along_hull(0.55)],
                Vec3::Z,
                env.half_len * 0.42,
                0.10,
            ),
            26,
            0.9,
        ),
    ]
}

/// The sponson ammunition racks over the tracks, both flanks — the German answer to a hull floor
/// occupied by a driveshaft, and the reason these tanks fear a side hit above the belt.
pub(super) fn sponson_racks(
    env: &HullEnvelope,
    ids: [u16; 2],
    half_len_z: f32,
) -> Vec<DamageComponent> {
    let half_height = ((env.deck_y - env.sponson_y) * 0.5 - 0.04).max(0.10);
    let half_width = 0.20;
    [(-1.0, ids[0]), (1.0, ids[1])]
        .into_iter()
        .map(|(side, id)| {
            hull_component(
                id,
                K::AmmunitionRack,
                ModuleSlot::AmmoRack,
                M::Ammunition,
                obb(
                    [
                        side * env.against_wall_at(
                            env.sponson_y + 2.0 * half_height,
                            half_len_z,
                            half_width,
                        ),
                        env.sponson_y + half_height + 0.02,
                        0.0,
                    ],
                    [half_width, half_height, half_len_z],
                    0.0,
                ),
                32,
                1.35,
            )
        })
        .collect()
}

/// Fuel behind the firewall, flanking the engine.
pub(super) fn engine_bay_fuel(env: &HullEnvelope, ids: [u16; 2]) -> Vec<DamageComponent> {
    flank_fuel_pair(env, ids, [0.20, 0.32, 0.44], 0.30)
}
/// Every German hull here drives from the front.
pub(super) const DRIVE: DriveEnd = DriveEnd::Front;
