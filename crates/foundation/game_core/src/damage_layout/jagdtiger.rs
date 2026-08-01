//! Jagdtiger component placement.
//!
//! The only vehicle in the fleet with no turret, and the layout says so: there is no turret drive,
//! and the 12.8 cm breech is bolted into a fixed casemate that the sim clamps to zero traverse.
//! Casemate components still use the turret frame — with traverse held at zero the two frames are
//! the same, and using the fleet's frame keeps one rule for everybody.
//!
//! What a casemate buys is a gun no turret could carry; what it costs is that the ammunition has
//! nowhere to go but the walls of the compartment the crew stands in. Forty rounds line both
//! sides at chest height, which is why a penetration of this superstructure is so often the end
//! of it — the hull below is the same front-drive German architecture as the Tigers.

use super::authoring::{
    HullEnvelope, TurretFit, final_drive_pair, obb, suspension_pair, turret_component, turret_group,
};
use super::german;
use super::{DamageComponent, DamageComponentKind as K, DamageLayout, DamageMaterial as M};
use crate::ModuleSlot;

pub(super) fn layout(env: &HullEnvelope) -> DamageLayout {
    // The 12.8 cm PaK 44 in a fixed casemate: no traverse gear at all, and no bustle — the rounds
    // line the compartment walls instead.
    let mut components =
        turret_group(env, TurretFit { breech_fill: 1.0, turret_drive: false, bustle_rack: None });
    components.extend(wall_racks(env));
    components.extend(german::powertrain(env, [0.60, 0.42, 0.64]));
    components.extend(german::engine_bay_fuel(env, [8, 9]));
    components.extend(final_drive_pair(env, [12, 13], german::DRIVE, 0.26));
    components.extend(suspension_pair(env, [14, 15], 0.20));
    DamageLayout { components }
}

/// Forty rounds along both casemate walls, at the height the loaders worked at.
///
/// This is the whole trade the design makes. A turreted tank can put its ammunition below the
/// ring, in the hull, behind the thickest plate it owns; a casemate has the gun where the turret
/// basket would be and nowhere to stow rounds except beside the men serving it. A penetration of
/// this superstructure reaches ammunition almost wherever it enters.
fn wall_racks(env: &HullEnvelope) -> Vec<DamageComponent> {
    let half_height = 0.24;
    let half_width = 0.15;
    let center_y = env.ring_y + half_height + 0.06;
    // Measured against the casting, not the tub: the compartment narrows with height, and the
    // rounds sit against whatever wall is actually there at the loaders' shoulder.
    let span = env.casting_half_span_at(center_y + half_height);
    let x = (span * 0.95 - half_width).max(0.0);
    [(-1.0_f32, 5_u16), (1.0, 16)]
        .into_iter()
        .map(|(side, id)| {
            turret_component(
                id,
                K::AmmunitionRack,
                ModuleSlot::AmmoRack,
                M::Ammunition,
                obb(
                    [side * x, center_y, env.ring_z - 0.20],
                    [half_width, half_height, span * 0.45],
                    0.0,
                ),
                32,
                1.35,
            )
        })
        .collect()
}
