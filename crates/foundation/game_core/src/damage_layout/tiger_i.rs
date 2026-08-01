//! Tiger I Ausf. E component placement.
//!
//! The classic Henschel arrangement: HL230 in the tail, gearbox in the bow, and 92 rounds of
//! 8.8 cm ammunition filling both sponsons almost end to end. The turret carries the gun and the
//! crew and almost nothing else — there is no bustle rack here, which is what separates this hull
//! from its successor.

use super::DamageLayout;
use super::authoring::{HullEnvelope, TurretFit, final_drive_pair, suspension_pair, turret_group};
use super::german;

pub(super) fn layout(env: &HullEnvelope) -> DamageLayout {
    // 8.8 cm KwK 36. No bustle rack: this turret carried the gun, the crew and the wireless, and the 92 rounds went into the sponsons.
    let mut components =
        turret_group(env, TurretFit { breech_fill: 0.92, turret_drive: true, bustle_rack: None });
    components.extend(german::powertrain(env, [0.60, 0.42, 0.62]));
    // Long racks: the Tiger filled its sponsons from the firewall almost to the driver.
    components.extend(german::sponson_racks(env, [5, 16], env.half_len * 0.36));
    components.extend(german::engine_bay_fuel(env, [8, 9]));
    components.extend(final_drive_pair(env, [12, 13], german::DRIVE, 0.25));
    components.extend(suspension_pair(env, [14, 15], 0.20));
    DamageLayout { components }
}
