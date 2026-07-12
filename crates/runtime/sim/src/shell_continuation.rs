//! Residual projectile lives after an armor contact: one ricochet or kinetic through-flight.

use game_core::{DamageEvent, ShellType};
use glam::Vec3;

use crate::ShellState;

const RICOCHET_SPEED_RETENTION: f32 = 0.75;
const RICOCHET_PENETRATION_RETENTION: f32 = 0.6;
const RICOCHET_LIFT_M: f32 = 0.15;
const PENETRATION_EXIT_CLEARANCE_M: f32 = 0.08;
const MIN_CONTINUATION_PENETRATION_MM: f32 = 5.0;

pub(crate) fn kinetic_penetration_continues(shell: &ShellState, event: &DamageEvent) -> bool {
    event.penetrated
        && matches!(shell.shell.shell_type, ShellType::ArmorPiercing | ShellType::Apcr)
        && event.shell_penetration_mm - event.effective_armor_mm >= MIN_CONTINUATION_PENETRATION_MM
}

/// Carry a perforating kinetic round out of the struck hull with its residual armor budget.
pub(crate) fn continue_through_armor(shell: &mut ShellState, event: &DamageEvent, distance_m: f32) {
    let incoming_pen = event.shell_penetration_mm.max(1.0);
    let remaining_pen = (incoming_pen - event.effective_armor_mm).max(0.0);
    let ratio = (remaining_pen / incoming_pen).clamp(0.0, 1.0);
    shell.shell.penetration_mm_at_100m *= ratio;
    shell.velocity_mps *= ratio.sqrt().clamp(0.25, 0.95);
    let direction = shell.velocity_mps.normalize_or_zero();
    shell.position = event.hit_position
        + direction * (shell.shell.collision_radius_m() + PENETRATION_EXIT_CLEARANCE_M);
    shell.traveled_m = distance_m;
    shell.last_penetrated_target = Some(event.target);
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
