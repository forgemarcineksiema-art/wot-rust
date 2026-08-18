//! Residual projectile lives after an armor contact: one ricochet or kinetic through-flight.

use game_core::TankId;
use glam::Vec3;

use crate::ShellState;

const RICOCHET_SPEED_RETENTION: f32 = 0.75;
const RICOCHET_PENETRATION_RETENTION: f32 = 0.6;
const RICOCHET_LIFT_M: f32 = 0.15;
const PENETRATION_EXIT_CLEARANCE_M: f32 = 0.08;
const MIN_CONTINUATION_PENETRATION_MM: f32 = 5.0;

/// What a perforating shell had left the moment it cleared the far plate — the ONE answer both
/// the visible exit hole and the projectile's continued flight are built from.
///
/// It is produced only when the round genuinely got out: it beat the entry plate, then whatever
/// of the internal path it had to cross, and then the exit plate's own line-of-sight steel. A
/// round that fails any of those stays inside the hull, and no hole is cut for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ShellExit {
    /// Where the round left the armour — the outer face of the exit plate, not the entry point.
    pub position: Vec3,
    /// Armour it can still defeat, after the exit plate took its share.
    pub residual_penetration_mm: f32,
    /// Fraction of the impact speed carried out through the far plate.
    pub speed_scale: f32,
}

/// Whether a round that got out is still a KINETIC threat to whatever is behind the hull. The
/// chemical rounds do not continue: a shaped charge is spent on the plate it fired against, and
/// HE never got inside in the first place — but both still blow an exit hole, which is why this
/// is a separate question from [`ShellExit`] existing at all.
pub(crate) fn kinetic_penetration_continues(shell: &ShellState, exit: &ShellExit) -> bool {
    // B4: "kinetic" is the SHELL's terminal identity (`ShellSpec::is_kinetic`, penetrator-keyed),
    // not an ammo-class guess.
    shell.shell.is_kinetic() && exit.residual_penetration_mm >= MIN_CONTINUATION_PENETRATION_MM
}

/// Carry a perforating kinetic round out through the hole it just opened, with the armour budget
/// that survived the crossing. It reappears at the EXIT plate, which is where it actually is —
/// the old path put it back beside the entry wound and scaled its budget by the entry plate
/// alone, so a round could leave a hull it never really got out of.
pub(crate) fn continue_through_armor(
    shell: &mut ShellState,
    exit: &ShellExit,
    target: TankId,
    distance_m: f32,
) {
    let carried_at_range = shell.shell.penetration_mm_at_distance(distance_m).max(1.0);
    let ratio = (exit.residual_penetration_mm / carried_at_range).clamp(0.0, 1.0);
    shell.shell.penetration_mm_at_100m *= ratio;
    shell.velocity_mps *= exit.speed_scale.clamp(0.25, 0.95);
    let direction = shell.velocity_mps.normalize_or_zero();
    shell.position = exit.position
        + direction * (shell.shell.collision_radius_m() + PENETRATION_EXIT_CLEARANCE_M);
    shell.traveled_m = distance_m;
    shell.last_penetrated_target = Some(target);
}

pub(crate) fn deflect_shell(
    shell: &mut ShellState,
    hit_position: Vec3,
    plate_normal: Vec3,
    distance_m: f32,
) {
    let velocity = shell.velocity_mps;
    let reflected = velocity - 2.0 * velocity.dot(plate_normal) * plate_normal;
    shell.velocity_mps = reflected * RICOCHET_SPEED_RETENTION;
    shell.position = hit_position + plate_normal * RICOCHET_LIFT_M;
    shell.traveled_m = distance_m;
    shell.shell.penetration_mm_at_100m *= RICOCHET_PENETRATION_RETENTION;
    shell.ricocheted_once = true;
    shell.last_penetrated_target = None;
}
