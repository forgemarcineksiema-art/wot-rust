//! Tiger II Ausf. B component placement.
//!
//! Same architecture as the Tiger I and one consequential difference: the Henschel turret is built
//! around a long bustle, and 22 rounds ride in it. That rack is the reason a Tiger II can be
//! killed by a shot into the back of its turret — a shot that would find only a radio set on its
//! predecessor.

use super::DamageLayout;
use super::authoring::{HullEnvelope, TurretFit, final_drive_pair, suspension_pair, turret_group};
use super::german;

pub(super) fn layout(env: &HullEnvelope) -> DamageLayout {
    // The Henschel bustle rack — 22 rounds, and the reason a shot into the back of this turret kills it.
    let mut components = turret_group(
        env,
        TurretFit { breech_fill: 0.95, turret_drive: true, bustle_rack: Some(1.0) },
    );
    components.extend(german::powertrain(env, [0.60, 0.42, 0.66]));
    components.extend(german::sponson_racks(env, [5, 16], env.half_len * 0.32));
    components.extend(german::engine_bay_fuel(env, [8, 9]));
    components.extend(final_drive_pair(env, [12, 13], german::DRIVE, 0.26));
    components.extend(suspension_pair(env, [14, 15], 0.20));
    DamageLayout { components }
}
