//! Generic contact-cavity bands derived from a [`VehicleBlueprint`] — Program C's answer to
//! the fleet's flat ambient look. The T-54's bespoke `t54_surface_bake` proved the bands
//! (turret-ring seam, mantlet seat, running-gear recess, deck shadow, glacis weld); this
//! module computes the SAME semantics for every blueprint vehicle from the fields it already
//! authors, so a new RON file ships with honest contact shading on day one. Amounts follow the
//! T-54 calibration; every centre and extent restates a blueprint dimension.

use game_core::VehicleBlueprint;
use glam::Vec3;

use crate::CavityBand;

/// The per-submesh band sets: `(hull, turret, gun)`, all in the shared recipe-local space
/// (origin on the ground under the hull centre — the space the bands' blueprint fields use).
pub(crate) fn blueprint_cavity_bands(
    bp: &VehicleBlueprint,
) -> (Vec<CavityBand>, Vec<CavityBand>, Vec<CavityBand>) {
    let (hull, track, turret, gun) = (&bp.hull, &bp.track, &bp.turret, &bp.gun);

    // The running gear and belt sit in the recess under the sponson/skirt — the darkest band
    // on the hull in ambient light. A skirted hull keeps the band INSIDE the skirt plane so
    // the outer plate itself stays lit.
    let gear_half_x =
        if hull.skirt.is_some() { track.outer_x - 0.02 } else { track.outer_x + 0.10 };
    let hull_bands = vec![
        CavityBand {
            center: Vec3::new(
                0.0,
                (track.top_y + track.bottom_y) * 0.5,
                (track.wheel_first_z + track.wheel_last_z) * 0.5,
            ),
            half_extents: Vec3::new(
                gear_half_x,
                (track.top_y - track.bottom_y) * 0.5 + 0.12,
                track.end_z + 0.35,
            ),
            falloff: 0.12,
            amount: 0.42,
        },
        // The engine deck's rear third falls into the intake/louver shadow.
        CavityBand {
            center: Vec3::new(0.0, hull.deck_y - 0.02, -hull.half_len * 0.55),
            half_extents: Vec3::new(hull.half_width * 0.75, 0.05, hull.half_len * 0.30),
            falloff: 0.10,
            amount: 0.28,
        },
        // The glacis-to-roof weld line catches a thin contact shadow across the hull front.
        CavityBand {
            center: Vec3::new(0.0, hull.deck_y, hull.half_len * 0.64),
            half_extents: Vec3::new(hull.half_width * 0.75, 0.04, 0.05),
            falloff: 0.08,
            amount: 0.30,
        },
    ];

    let mantlet_seat = CavityBand {
        center: Vec3::new(
            0.0,
            gun.trunnion_y,
            (turret.mantlet_back_z + turret.mantlet_front_z) * 0.5,
        ),
        half_extents: Vec3::new(
            turret.mantlet_radius + 0.14,
            turret.mantlet_radius + 0.05,
            (turret.mantlet_front_z - turret.mantlet_back_z) * 0.5 + 0.05,
        ),
        falloff: 0.16,
        amount: 0.45,
    };
    let turret_bands = vec![
        // The casting/casemate seats on its ring (or superstructure base): the seam reads as a
        // deep ambient shadow around the bottom of the turret.
        CavityBand {
            center: Vec3::new(0.0, turret.ring_y, turret.ring_z),
            half_extents: Vec3::new(
                turret.base_radius + 0.05,
                0.06,
                turret.plan_half_length + 0.05,
            ),
            falloff: 0.14,
            amount: 0.38,
        },
        mantlet_seat,
    ];

    // The barrel root darkens into the same seat it emerges from.
    let gun_bands = vec![mantlet_seat];

    (hull_bands, turret_bands, gun_bands)
}
