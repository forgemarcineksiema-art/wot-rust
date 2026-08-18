//! Panther II component placement.
//!
//! The medium of the German school: the same bow gearbox and sponson racks as the Tigers in a
//! narrower, lighter hull. The tub is the tightest of the four, so the driveshaft, the fuel and
//! the crew are packed closer together than on anything else here — a penetration that gets
//! inside this hull has fewer places to end up harmlessly.

use super::DamageLayout;
use super::authoring::{
    CrewPlan, HullEnvelope, TurretFit, crew_stations_from_plan, final_drive_pair, suspension_pair,
    turret_group,
};
use super::german;

pub(super) fn layout(env: &HullEnvelope) -> DamageLayout {
    // 7.5 cm KwK 42. Ammunition rides in the sponsons, not up here.
    let mut components =
        turret_group(env, TurretFit { breech_fill: 0.88, turret_drive: true, bustle_rack: None });
    components.extend(german::powertrain(env, [0.55, 0.40, 0.60]));
    components.extend(german::sponson_racks(env, [5, 16], env.half_len * 0.34));
    components.extend(german::engine_bay_fuel(env, [8, 9]));
    components.extend(final_drive_pair(env, [12, 13], german::DRIVE, 0.23));
    components.extend(suspension_pair(env, [14, 15], 0.19));
    // Five men, German layout: driver and radio operator flank the bow gearbox, three in the turret.
    components.extend(crew_stations_from_plan(
        env,
        18,
        CrewPlan { driver_x_sign: 1.0, bow_radio_operator: true, gunner_x_sign: 1.0 },
    ));
    DamageLayout { components }
}
