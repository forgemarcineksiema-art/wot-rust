//! Crew field repair — the fix for the permanent-statue failure mode. Without it a thrown
//! track, a rammed-dead suspension or a shot-out engine immobilized a hull for the REST OF THE
//! BATTLE: the module model only ever subtracted. The crew now puts mobility back: a thrown
//! track is re-seated after [`TRACK_REPAIR_S`], and a DESTROYED engine or suspension is field-
//! patched after [`MODULE_PATCH_S`] to a fraction of its pool — running, but wounded (the
//! partial-damage power/agility floors in `game_core` shape how wounded).
//!
//! The gun and ammo rack are field-patched the same way (after [`MODULE_PATCH_S`], to
//! [`MODULE_PATCH_FRACTION`] of pool). A knocked-out gun was previously permanent — one frontal
//! penetration and the hull could not fire for the rest of the match, with no feedback, which
//! read as a bug. Now it comes back WOUNDED: functional but reloading slower (the gun-damage
//! reload penalty in `game_core::gun_reload_multiplier`) — an honest wound you play around, not a
//! silent lockout. The turret ring and radio are still left alone.

use game_core::{ModuleSlot, TrackSeverity, TrackSide};
use serde::{Deserialize, Serialize};

use crate::TankState;

/// Seconds the crew needs to re-seat a thrown track.
pub const TRACK_REPAIR_S: f32 = 10.0;
/// Seconds between each chunk of regen on a merely DAMAGED (not thrown) track pool.
pub const TRACK_REGEN_INTERVAL_S: f32 = 3.5;
/// HP the crew restores to a damaged pool each interval (capped at full). A half-drained pool
/// climbs back over ~two intervals; a near-thrown one over the repair window's worth.
pub const TRACK_REGEN_CHUNK: u8 = 20;
/// Seconds the crew needs to field-patch a destroyed engine, suspension, gun or ammo rack.
pub const MODULE_PATCH_S: f32 = 15.0;
/// The field patch restores this fraction of the module's full pool — enough to run at the
/// partial-damage floor, nowhere near shop condition.
pub const MODULE_PATCH_FRACTION: f32 = 0.25;

/// The module slots the crew field-patches when destroyed, and the clock order. Mobility first
/// (engine/suspension), then the fighting modules (gun/ammo rack). Turret ring and radio are
/// deliberately absent — they stay knocked out.
const PATCHED_SLOTS: [ModuleSlot; 4] =
    [ModuleSlot::Engine, ModuleSlot::Suspension, ModuleSlot::Gun, ModuleSlot::AmmoRack];

/// Per-tank crew repair clocks: seconds each patchable system has been down. Counts up while
/// broken, snaps to zero when whole (including when a fresh hit re-breaks a repaired system —
/// the crew starts over). `serde(default)` keeps pre-repair fixtures loading unchanged.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CrewRepair {
    left_track_s: f32,
    right_track_s: f32,
    engine_s: f32,
    suspension_s: f32,
    #[serde(default)]
    gun_s: f32,
    #[serde(default)]
    ammo_rack_s: f32,
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
        match tank.tracks.severity(side) {
            // A thrown pool is latched: it does not regen chunk-by-chunk, it is re-seated whole
            // after the full repair window (and a fresh throw restarts the clock).
            TrackSeverity::Broken => {
                *clock += dt;
                if *clock >= TRACK_REPAIR_S {
                    tank.tracks.reseat(side);
                    let index = match side {
                        TrackSide::Left => 0,
                        TrackSide::Right => 1,
                    };
                    tank.track_break_t[index] = None;
                    *clock = 0.0;
                }
            }
            // A merely damaged pool the crew nurses back up in chunks, so two-damaged eases to
            // one-damaged and then to whole on its own.
            TrackSeverity::Damaged => {
                *clock += dt;
                if *clock >= TRACK_REGEN_INTERVAL_S {
                    tank.tracks.heal(side, TRACK_REGEN_CHUNK);
                    *clock = 0.0;
                }
            }
            TrackSeverity::Healthy => *clock = 0.0,
        }
    }
    for slot in PATCHED_SLOTS {
        let clock = match slot {
            ModuleSlot::Engine => &mut tank.repair.engine_s,
            ModuleSlot::Suspension => &mut tank.repair.suspension_s,
            ModuleSlot::Gun => &mut tank.repair.gun_s,
            _ => &mut tank.repair.ammo_rack_s,
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
            // Putting the engine right does NOT put its fire out: the burn is owned by
            // `crate::fire`, which the crew fights on its own clock. Two owners of one flag is how
            // a tank ends up "repaired" and still alight, or extinguished by a patch it never got.
            *clock = 0.0;
        }
    }
}
