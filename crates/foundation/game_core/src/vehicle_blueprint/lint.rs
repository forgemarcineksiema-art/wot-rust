//! Blueprint validation with TEACHING errors: every message carries the offending values, the
//! units, and the *why* — the physical or historical constraint the numbers violate — so an
//! author editing a `.blueprint.ron` (human or AI) learns the domain from the failure instead
//! of chasing a bare "invalid value". Errors block the load; warnings surface in the Forge
//! Studio report every iteration.

use super::{TurretForm, VehicleBlueprint};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The blueprint describes an impossible vehicle; the loader refuses it.
    Error,
    /// Suspicious but buildable; surfaced in the studio report.
    Warning,
}

#[derive(Debug, Clone)]
pub struct BlueprintIssue {
    pub field: &'static str,
    pub severity: Severity,
    pub message: String,
}

fn issue(
    issues: &mut Vec<BlueprintIssue>,
    severity: Severity,
    field: &'static str,
    message: String,
) {
    issues.push(BlueprintIssue { field, severity, message });
}

/// Validate every cross-field constraint the shape must satisfy. Pure — callable from the
/// loader, the tests, and the studio report.
pub fn validate_blueprint(bp: &VehicleBlueprint) -> Vec<BlueprintIssue> {
    let mut issues = Vec::new();
    let e = Severity::Error;
    let w = Severity::Warning;
    let (hull, track, turret, gun) = (&bp.hull, &bp.track, &bp.turret, &bp.gun);

    // --- Hull ---------------------------------------------------------------------------
    if hull.belly_y <= 0.0 || hull.belly_y >= hull.deck_y {
        issue(
            &mut issues,
            e,
            "hull.belly_y",
            format!(
                "belly_y ({:.2} m) must sit between the ground (0) and deck_y ({:.2} m): it is the \
             hull floor's ground clearance",
                hull.belly_y, hull.deck_y
            ),
        );
    }
    if hull.lower_half_width > hull.half_width + 1.0e-6 {
        issue(
            &mut issues,
            e,
            "hull.lower_half_width",
            format!(
                "lower_half_width ({:.2} m) must be <= half_width ({:.2} m): the tub between the \
             tracks is never wider than the sponson deck above it",
                hull.lower_half_width, hull.half_width
            ),
        );
    }
    for (field, value) in [
        ("hull.glacis_slope_deg", hull.glacis_slope_deg),
        ("hull.rear_slope_deg", hull.rear_slope_deg),
        ("hull.pike_sweep_deg", hull.pike_sweep_deg),
    ] {
        if !(0.0..=80.0).contains(&value) {
            issue(
                &mut issues,
                e,
                field,
                format!(
                    "{value:.1}° is outside 0..=80°: plate slopes are measured from VERTICAL in \
                 degrees — check for a radians value or a sign slip"
                ),
            );
        }
    }
    if hull.half_len > hull.hitbox_half_length + 1.0e-6 {
        issue(
            &mut issues,
            e,
            "hull.half_len",
            format!(
                "visual half_len ({:.2} m) exceeds hitbox_half_length ({:.2} m): the visible hull \
             must fit inside the collision box — what you see is what you hit",
                hull.half_len, hull.hitbox_half_length
            ),
        );
    }
    if hull.deck_y > hull.hitbox_center_y + hull.hitbox_half_height + 1.0e-6 {
        issue(
            &mut issues,
            e,
            "hull.deck_y",
            format!(
                "deck_y ({:.2} m) pokes above the hitbox top ({:.2} m): raise the hitbox or lower \
             the deck — an honest silhouette never overhangs its collision volume",
                hull.deck_y,
                hull.hitbox_center_y + hull.hitbox_half_height
            ),
        );
    }

    // --- Turret -------------------------------------------------------------------------
    if turret.form == TurretForm::CastDome && turret.ring_radius >= turret.base_radius {
        issue(
            &mut issues,
            e,
            "turret.ring_radius",
            format!(
                "ring_radius ({:.2} m) must be < base_radius ({:.2} m): a cast dome OVERHANGS its \
             ring race — the casting is always wider than the bearing it sits on, never the \
             reverse",
                turret.ring_radius, turret.base_radius
            ),
        );
    }
    if turret.roof_y <= turret.ring_y {
        issue(
            &mut issues,
            e,
            "turret.roof_y",
            format!(
                "roof_y ({:.2} m) must be above ring_y ({:.2} m): the turret has to rise off its \
             own ring seat",
                turret.roof_y, turret.ring_y
            ),
        );
    }
    if turret.roof_radius >= turret.base_radius {
        issue(
            &mut issues,
            w,
            "turret.roof_radius",
            format!(
                "roof_radius ({:.2} m) >= base_radius ({:.2} m) reads as an inverted cone: turrets \
             taper toward the roof",
                turret.roof_radius, turret.base_radius
            ),
        );
    }
    if turret.mantlet_front_z <= turret.mantlet_back_z {
        issue(
            &mut issues,
            e,
            "turret.mantlet_front_z",
            format!(
                "mantlet_front_z ({:.2}) must be ahead of mantlet_back_z ({:.2}): +Z is the muzzle \
             direction",
                turret.mantlet_front_z, turret.mantlet_back_z
            ),
        );
    }

    // --- Gun ----------------------------------------------------------------------------
    if gun.muzzle_z <= turret.mantlet_front_z {
        issue(
            &mut issues,
            e,
            "gun.muzzle_z",
            format!(
                "muzzle_z ({:.2} m) must reach past the mantlet front ({:.2} m): a barrel that \
             ends inside its own mantlet has negative length",
                gun.muzzle_z, turret.mantlet_front_z
            ),
        );
    }
    if !(0.02..=0.2).contains(&gun.barrel_radius) {
        issue(
            &mut issues,
            w,
            "gun.barrel_radius",
            format!(
                "barrel_radius {:.3} m is outside the plausible 0.02..0.20 band (a 20 mm autocannon \
             to a 400 mm mortar) — check the units, this is the OUTER tube radius in metres",
                gun.barrel_radius
            ),
        );
    }

    // --- Track / running gear -------------------------------------------------------------
    if track.outer_x <= track.inner_x {
        issue(
            &mut issues,
            e,
            "track.outer_x",
            format!(
                "outer_x ({:.2} m) must be outside inner_x ({:.2} m): the belt band needs width",
                track.outer_x, track.inner_x
            ),
        );
    }
    if track.wheel_count == 0 {
        issue(&mut issues, e, "track.wheel_count", "a tracked vehicle needs road wheels".into());
    }
    if let Some(stations) = track.wheel_stations {
        if stations.len() != track.wheel_count {
            issue(
                &mut issues,
                e,
                "track.wheel_stations",
                format!(
                    "{} explicit stations authored but wheel_count is {} — the two must agree \
                 (stations are the per-axle hull-local Z list)",
                    stations.len(),
                    track.wheel_count
                ),
            );
        }
        let span = track.wheel_first_z.min(track.wheel_last_z) - 0.01
            ..=track.wheel_first_z.max(track.wheel_last_z) + 0.01;
        for &z in stations {
            if !span.contains(&z) {
                issue(
                    &mut issues,
                    e,
                    "track.wheel_stations",
                    format!(
                        "station z = {z:.2} lies outside the wheel span {:.2}..{:.2}: \
                     wheel_first_z/wheel_last_z bound the axle line",
                        track.wheel_first_z, track.wheel_last_z
                    ),
                );
            }
        }
    }
    if track.return_rollers > 0 && track.roller_radius <= 0.0 {
        issue(
            &mut issues,
            e,
            "track.roller_radius",
            format!(
                "{} return rollers authored with roller_radius {:.2}: rollers that carry the top \
             run need a real radius",
                track.return_rollers, track.roller_radius
            ),
        );
    }
    if track.top_y <= track.bottom_y {
        issue(
            &mut issues,
            e,
            "track.top_y",
            format!(
                "top_y ({:.2}) must be above bottom_y ({:.2}): the belt wraps a loop",
                track.top_y, track.bottom_y
            ),
        );
    }

    // --- Skirts ---------------------------------------------------------------------------
    if let Some(skirt) = hull.skirt {
        if skirt.bottom_y >= skirt.top_y {
            issue(
                &mut issues,
                e,
                "hull.skirt.bottom_y",
                format!(
                    "skirt bottom_y ({:.2}) must hang below top_y ({:.2})",
                    skirt.bottom_y, skirt.top_y
                ),
            );
        }
        if skirt.standoff_m <= 0.0 {
            issue(
                &mut issues,
                e,
                "hull.skirt.standoff_m",
                format!(
                    "standoff_m ({:.3}) must be positive: the air gap IS the spaced armour — a \
                 zero-gap skirt is just a thicker track",
                    skirt.standoff_m
                ),
            );
        }
    }

    // --- Armour ---------------------------------------------------------------------------
    for (field, (slope, weakspot)) in [
        ("armor.hull_front", bp.armor.hull_front),
        ("armor.hull_side", bp.armor.hull_side),
        ("armor.hull_rear", bp.armor.hull_rear),
        ("armor.turret_front", bp.armor.turret_front),
        ("armor.turret_side", bp.armor.turret_side),
        ("armor.turret_rear", bp.armor.turret_rear),
    ] {
        if !(0.0..=80.0).contains(&slope) {
            issue(
                &mut issues,
                e,
                field,
                format!("slope {slope:.1}° outside 0..=80° (degrees from vertical)"),
            );
        }
        if !(0.1..=1.5).contains(&weakspot) {
            issue(
                &mut issues,
                e,
                field,
                format!(
                    "weakspot multiplier {weakspot:.2} outside 0.1..=1.5: it scales the facet's \
                 effective thickness (1.0 = the plate as rolled)"
                ),
            );
        }
    }

    issues
}
