//! Crew field repair — the fix for the permanent-statue failure mode. Without it a thrown
//! track, a rammed-dead suspension or a shot-out engine immobilized a hull for the REST OF THE
//! BATTLE: the module model only ever subtracted. The crew now puts mobility back: a thrown
//! track is re-seated after [`TRACK_REPAIR_S`], and a DESTROYED engine or suspension is field-
//! patched after [`MODULE_PATCH_S`] to a fraction of its pool — running, but wounded (the
//! partial-damage power/agility floors in `game_core` shape how wounded).
//!
//! Deliberately mobility-only: a knocked-out gun, turret ring or ammo rack stays knocked out.
//! Losing your ability to FIGHT is an honest wound you play around; losing the ability to MOVE
//! forever just parks the battle (bots included — see the seed-23 stall).

use game_core::{ModuleSlot, TrackSide};
use serde::{Deserialize, Serialize};

use crate::TankState;

/// Seconds the crew needs to re-seat a thrown track.
pub const TRACK_REPAIR_S: f32 = 10.0;
/// Seconds the crew needs to field-patch a destroyed engine or suspension.
pub const MODULE_PATCH_S: f32 = 15.0;
/// The field patch restores this fraction of the module's full pool — enough to run at the
/// partial-damage floor, nowhere near shop condition.
pub const MODULE_PATCH_FRACTION: f32 = 0.25;

/// Per-tank crew repair clocks: seconds each mobility system has been down. Counts up while
/// broken, snaps to zero when whole (including when a fresh hit re-breaks a repaired system —
/// the crew starts over). `serde(default)` keeps pre-repair fixtures loading unchanged.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CrewRepair {
    left_track_s: f32,
    right_track_s: f32,
    engine_s: f32,
    suspension_s: f32,
}

/// One fixed tick of crew repair for one living hull. Runs in the same per-tank pass as reload
/// and aim recovery, before movement — deterministic, server-authoritative.
pub(crate) fn step_crew_repair(tank: &mut TankState, dt: f32) {
    if tank.hit_points == 0 {
        return;
    }
    for side in [TrackSide::Left, TrackSide::Right] {
        let clock = match side {
            TrackSide::Left => &mut tank.repair.left_track_s,
            TrackSide::Right => &mut tank.repair.right_track_s,
        };
        if tank.tracks.is_broken(side) {
            *clock += dt;
            if *clock >= TRACK_REPAIR_S {
                tank.tracks.repair(side);
                *clock = 0.0;
            }
        } else {
            *clock = 0.0;
        }
    }
    for slot in [ModuleSlot::Engine, ModuleSlot::Suspension] {
        let clock = match slot {
            ModuleSlot::Engine => &mut tank.repair.engine_s,
            _ => &mut tank.repair.suspension_s,
        };
        if tank.modules.is_functional(slot) {
            *clock = 0.0;
            continue;
        }
        *clock += dt;
        if *clock >= MODULE_PATCH_S {
            let full = tank.spec.module_health.hit_points(slot);
            let patched = ((full as f32 * MODULE_PATCH_FRACTION) as u32).max(1);
            tank.modules.restore_to(slot, patched);
            *clock = 0.0;
        }
    }
}
