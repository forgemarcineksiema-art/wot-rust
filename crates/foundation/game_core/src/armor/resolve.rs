//! The penetration test itself: one armor layer's line-of-sight steel against a shell arriving
//! at its TRUE angle of incidence, plus the layered spaced-armor variant the track zones use.
//! Plate slope never appears here — it lives in the plate NORMAL ([`crate::math::plate_normal`]),
//! and only the resulting real angle reaches these functions.

use serde::{Deserialize, Serialize};

use super::{ArmorFacet, ArmorFacing, ArmorProfile};
use crate::{ArmorZone, ShellSpec, ShellType};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PenetrationResult {
    pub penetrated: bool,
    pub ricocheted: bool,
    pub effective_armor_mm: f32,
    pub remaining_penetration_mm: f32,
    pub damage_hp: u32,
    pub module_damage_hp: u32,
}

pub fn resolve_penetration(
    shell: &ShellSpec,
    armor: &ArmorProfile,
    facing: ArmorFacing,
    impact_angle_degrees: f32,
) -> PenetrationResult {
    resolve_penetration_at_distance(shell, armor, facing, impact_angle_degrees, 100.0)
}

pub fn resolve_penetration_at_distance(
    shell: &ShellSpec,
    armor: &ArmorProfile,
    facing: ArmorFacing,
    impact_angle_degrees: f32,
    distance_m: f32,
) -> PenetrationResult {
    let facet = armor.facet(facing);
    resolve_penetration_at_distance_on_facet(shell, facet, impact_angle_degrees, distance_m)
}

/// A shell whose caliber overmatches the plate this many times over "bites" instead of glancing:
/// no ricochet (see [`ricochets`]) and the line-of-sight thickness gain is capped.
const OVERMATCH_CALIBER_RATIO: f32 = 3.0;
/// The steepest angle an overmatched plate can present for LOS thickness.
const OVERMATCH_ANGLE_CAP_DEGREES: f32 = 70.0;

/// `impact_angle_degrees` is the TRUE angle of incidence — between the shell path and the plate's
/// real 3D normal ([`crate::math::plate_normal`]), slope already included. Plate slope must never
/// be added here; it reaches this function only through that geometry.
pub(crate) fn resolve_penetration_at_distance_on_facet(
    shell: &ShellSpec,
    facet: ArmorFacet,
    impact_angle_degrees: f32,
    distance_m: f32,
) -> PenetrationResult {
    let ricocheted = ricochets(shell, &facet, impact_angle_degrees);
    let effective_armor_mm = layer_los_mm(shell, facet, impact_angle_degrees);
    let remaining_penetration_mm =
        shell.penetration_mm_at_distance(distance_m) - effective_armor_mm;
    let penetrated = !ricocheted && remaining_penetration_mm >= 0.0;
    let damage_hp = shell_damage_hp(shell, penetrated, ricocheted);

    PenetrationResult {
        penetrated,
        ricocheted,
        effective_armor_mm,
        remaining_penetration_mm,
        damage_hp,
        module_damage_hp: module_damage_hp(shell, penetrated, ricocheted),
    }
}

/// The armour "test" for a round that arrives through a hole an earlier shell already opened.
///
/// There is no steel across its path, so there is nothing to resolve: the round is simply INSIDE,
/// carrying everything it still has at this range, and what it meets there is the interior. It is
/// a function rather than a literal at the call site so that the damage a perforation deals stays
/// ONE rule — a round that threads an open channel hurts exactly as much as one that had to earn
/// its way in, because by the time either is in the fighting compartment nothing about them
/// differs. The caller is responsible for having checked that the projectile's full cross-section
/// actually clears the aperture ([`crate::ArmorBreachSet::passage_at`]).
pub fn resolve_penetration_through_open_channel(
    shell: &ShellSpec,
    distance_m: f32,
) -> PenetrationResult {
    PenetrationResult {
        penetrated: true,
        ricocheted: false,
        effective_armor_mm: 0.0,
        remaining_penetration_mm: shell.penetration_mm_at_distance(distance_m),
        damage_hp: shell_damage_hp(shell, true, false),
        module_damage_hp: module_damage_hp(shell, true, false),
    }
}

/// How much harder a spaced screen is on a shaped charge: the standoff detonates the jet early,
/// so the track band eats it at a multiple of its line-of-sight steel.
const HEAT_SCREEN_FACTOR: f32 = 2.0;

/// A track hit is a SPACED-ARMOR test, not a hull plate — the single-screen case of
/// [`resolve_penetration_through_screens`].
pub fn resolve_penetration_through_track(
    shell: &ShellSpec,
    armor: &ArmorProfile,
    track_zone: ArmorZone,
    track_angle_degrees: f32,
    side_angle_degrees: f32,
    distance_m: f32,
) -> PenetrationResult {
    resolve_penetration_through_screens(
        shell,
        armor,
        &[track_zone],
        track_angle_degrees,
        side_angle_degrees,
        distance_m,
    )
}

/// A flank hit through every SPACED layer standing off the hull side, outermost first.
///
/// Each screen strips its own line-of-sight thickness off the shell (the whole stack a multiple
/// for HEAT — the standoff kills the jet), and only the remainder challenges the hull side plate
/// behind them. HE fuzes on the first surface it touches, so it bursts on the outermost screen
/// and never reaches the plate at all. An EMPTY screen list is a bare side plate (a thrown track
/// is not there any more) and resolves as an ordinary hull-side hit.
///
/// The list is what keeps the doctrine honest in the one place it used to invert it: a side
/// skirt hangs OUTSIDE the track band, so a shell that met the skirt still has the belt to
/// cross. Resolving the skirt alone made bolting Schürzen onto a hull *reduce* its flank armour
/// — the opposite of what spaced plate is for, and visible in the fleet (Centurion, Tiger II).
///
/// Angles are TRUE angles of incidence against each layer's own 3D normal; the screens share
/// `screen_angle_degrees` because they are parallel plates a track-width apart.
pub fn resolve_penetration_through_screens(
    shell: &ShellSpec,
    armor: &ArmorProfile,
    screens: &[ArmorZone],
    screen_angle_degrees: f32,
    side_angle_degrees: f32,
    distance_m: f32,
) -> PenetrationResult {
    let Some(&outermost) = screens.first() else {
        // Nothing standing off the hull: the bare side plate on its own terms.
        return resolve_penetration_at_distance_on_facet(
            shell,
            armor.facet(ArmorFacing::HullSide),
            side_angle_degrees,
            distance_m,
        );
    };
    let outer_plate = armor.plate(outermost);
    let outer_los = layer_los_mm(shell, outer_plate, screen_angle_degrees);

    if shell.shell_type == ShellType::HighExplosive {
        // Surface burst on the outermost layer: external HE chip damage, never an interior hit.
        return PenetrationResult {
            penetrated: false,
            ricocheted: false,
            effective_armor_mm: outer_los,
            remaining_penetration_mm: shell.penetration_mm_at_distance(distance_m) - outer_los,
            damage_hp: shell_damage_hp(shell, false, false),
            module_damage_hp: module_damage_hp(shell, false, false),
        };
    }

    // Only the OUTERMOST layer can deflect: past it the shell is already inside the stack.
    let ricocheted = ricochets(shell, &outer_plate, screen_angle_degrees);
    let stack_los: f32 = screens
        .iter()
        .map(|&zone| layer_los_mm(shell, armor.plate(zone), screen_angle_degrees))
        .sum();
    let screen_mm = if shell.shell_type == ShellType::Heat {
        stack_los * HEAT_SCREEN_FACTOR
    } else {
        stack_los
    };
    let side = armor.facet(ArmorFacing::HullSide);
    let side_los = layer_los_mm(shell, side, side_angle_degrees);
    let effective_armor_mm = screen_mm + side_los;
    let remaining_penetration_mm =
        shell.penetration_mm_at_distance(distance_m) - effective_armor_mm;
    let penetrated = !ricocheted && remaining_penetration_mm >= 0.0;
    let damage_hp = shell_damage_hp(shell, penetrated, ricocheted);

    PenetrationResult {
        penetrated,
        ricocheted,
        effective_armor_mm,
        remaining_penetration_mm,
        damage_hp,
        module_damage_hp: module_damage_hp(shell, penetrated, ricocheted),
    }
}

/// One armor layer's line-of-sight steel for a shell arriving at the given TRUE angle:
/// normalization turns the shell into the plate, overmatch caps how steep an overmatched plate
/// can present itself.
fn layer_los_mm(shell: &ShellSpec, facet: ArmorFacet, impact_angle_degrees: f32) -> f32 {
    let normalized_angle = normalized_impact_angle(shell, impact_angle_degrees);
    let thickness_angle = if overmatches(shell, &facet) {
        normalized_angle.min(OVERMATCH_ANGLE_CAP_DEGREES)
    } else {
        normalized_angle
    };
    effective_facet_thickness_mm(facet, thickness_angle)
}

fn overmatches(shell: &ShellSpec, facet: &ArmorFacet) -> bool {
    facet.nominal_thickness_mm > 0.0
        && shell.caliber_mm > facet.nominal_thickness_mm * OVERMATCH_CALIBER_RATIO
}

pub(super) fn effective_facet_thickness_mm(facet: ArmorFacet, impact_angle_degrees: f32) -> f32 {
    let nominal = facet.nominal_thickness_mm * facet.weakspot_multiplier.clamp(0.1, 2.0);
    let clamped_angle = impact_angle_degrees.abs().clamp(0.0, 89.0);
    let cosine = clamped_angle.to_radians().cos().max(0.01);
    nominal / cosine
}

fn normalized_impact_angle(shell: &ShellSpec, impact_angle_degrees: f32) -> f32 {
    let normalization = match shell.shell_type {
        ShellType::ArmorPiercing => 5.0,
        ShellType::Apcr => 2.0,
        ShellType::Heat | ShellType::HighExplosive => 0.0,
    };
    (impact_angle_degrees.abs() - normalization).max(0.0)
}

fn ricochets(shell: &ShellSpec, facet: &ArmorFacet, impact_angle_degrees: f32) -> bool {
    match shell.shell_type {
        ShellType::ArmorPiercing | ShellType::Apcr => {
            impact_angle_degrees > 70.0 && shell.caliber_mm <= facet.nominal_thickness_mm * 3.0
        }
        ShellType::Heat => impact_angle_degrees > 85.0,
        ShellType::HighExplosive => false,
    }
}

fn shell_damage_hp(shell: &ShellSpec, penetrated: bool, ricocheted: bool) -> u32 {
    if penetrated {
        shell.damage_hp
    } else if !ricocheted && shell.shell_type == ShellType::HighExplosive {
        ((shell.damage_hp as f32) * 0.18).round().max(1.0) as u32
    } else {
        0
    }
}

fn module_damage_hp(shell: &ShellSpec, penetrated: bool, ricocheted: bool) -> u32 {
    if ricocheted {
        return 0;
    }
    let multiplier = match (shell.shell_type, penetrated) {
        (ShellType::ArmorPiercing, true) => 1.0,
        (ShellType::Apcr, true) => 0.75,
        (ShellType::Heat, true) => 0.85,
        (ShellType::HighExplosive, true) => 1.25,
        (ShellType::HighExplosive, false) => 0.65,
        _ => 0.0,
    };
    ((shell.damage_hp as f32) * multiplier).round() as u32
}
