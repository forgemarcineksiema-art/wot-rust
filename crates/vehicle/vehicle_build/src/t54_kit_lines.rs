//! The T-54's line-work kit: the glacis splash board, turret handrails, stowed tow cables, the
//! unditching beam and the gun travel lock. Split from `t54_kit` to keep each module within the
//! reviewability budget.

use game_core::{HybridVisual, TurretLoftVisual};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::t54_details::detail_plate;

/// Every line-work part: splash board, turret handrails, tow cables, unditching beam, travel lock.
pub(crate) fn t54_line_kit_parts(v: &HybridVisual) -> Vec<VehiclePart> {
    let mut parts = vec![splash_board(v), unditching_beam()];
    parts.extend(turret_rails(&v.turret_loft));
    parts.extend(tow_cables(v));
    parts.extend(travel_lock(v));
    parts
}

/// The unditching beam: the log carried horizontally across the lower rear plate, as the
/// references' rear/top views show. Steel-strapped dark timber at this fidelity. Its ends stop
/// WELL inside the hull side planes (±1.05) and the log floats a hand off the raked rear plate —
/// any coplanar contact with the hull z-fights as the camera moves.
fn unditching_beam() -> VehiclePart {
    let profile = [(-0.95_f32, 0.0_f32), (-0.95, 0.10), (0.95, 0.10), (0.95, 0.0)];
    VehiclePart {
        key: PartKey::new("unditching_beam"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::TrackMetal,
        smoothing: vehicle_geometry::SmoothingGroup(5),
        shape: PartShape::Mesh(revolve::translate(
            &revolve::revolve(
                Vec3::X,
                &profile,
                12,
                MaterialRole::TrackMetal,
                vehicle_geometry::SmoothingGroup(5),
            ),
            Vec3::new(0.0, 1.02, -3.04),
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    }
}

/// The stowed gun travel lock: a small clamp crutch on the rear deck centreline, between the
/// grille and the deck's rear edge (used with the turret reversed on the march).
fn travel_lock(v: &HybridVisual) -> Vec<VehiclePart> {
    let deck_top = v.deck.center.y + v.deck.half.y;
    let z = -2.64;
    let mut parts = vec![detail_plate(
        PartKey::new("travel_lock_base"),
        SubmeshKind::Hull,
        MaterialRole::TrackMetal,
        solid::chamfered_box(Vec3::new(0.0, deck_top + 0.02, z), Vec3::new(0.09, 0.02, 0.06), 0.01),
    )];
    for (i, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
        parts.push(detail_plate(
            PartKey::indexed("travel_lock_jaw", i as u16),
            SubmeshKind::Hull,
            MaterialRole::TrackMetal,
            solid::chamfered_box(
                Vec3::new(side * 0.065, deck_top + 0.095, z),
                Vec3::new(0.02, 0.055, 0.04),
                0.01,
            ),
        ));
    }
    parts
}

/// The V-shaped splash board across the upper glacis: a raised bead standing just proud of the
/// plate, vertex low on the centreline, arms rising outward — every front view shows it.
fn splash_board(v: &HybridVisual) -> VehiclePart {
    let path: Vec<Vec3> = [(-0.85_f32, 1.42_f32), (0.0, 1.26), (0.85, 1.42)]
        .iter()
        .map(|&(x, y)| glacis_point(v, x, y, 0.03))
        .collect();
    VehiclePart {
        key: PartKey::new("splash_board"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: vehicle_geometry::SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(detail::weld_bead(&path, 0.035)),
        lod: PartLod::Detail,
        generator: GeneratorKind::Sweep,
    }
}

/// A point on the 60° glacis plate at hull-local `(x, y)`, pushed `standoff` along the plate
/// normal. Derived from the blueprint's glacis plane, so it tracks the armour rake.
fn glacis_point(v: &HybridVisual, x: f32, y: f32, standoff: f32) -> Vec3 {
    let (sin, cos) = 60.0_f32.to_radians().sin_cos();
    let z = (v.hull.glacis_offset - sin * y) / cos;
    Vec3::new(x, y, z) + Vec3::new(0.0, sin, cos) * standoff
}

/// One handrail per turret side, following the cast dome at grab height with a small standoff —
/// the curved rails every reference view shows along the casting.
fn turret_rails(loft: &TurretLoftVisual) -> Vec<VehiclePart> {
    let y = 1.90;
    [1.0_f32, -1.0]
        .into_iter()
        .enumerate()
        .map(|(i, side)| {
            let path: Vec<Vec3> = (0..=6)
                .map(|k| {
                    let phi = (40.0 + 95.0 * k as f32 / 6.0).to_radians();
                    loft_ring_point(loft, y, side * phi, 0.05)
                })
                .collect();
            VehiclePart {
                key: PartKey::indexed("turret_rail", i as u16),
                submesh: SubmeshKind::Turret,
                material: MaterialRole::BarrelSteel,
                smoothing: vehicle_geometry::SmoothingGroup::hard_edges(),
                shape: PartShape::Mesh(detail::handle_rail(&path, 0.018)),
                lod: PartLod::Detail,
                generator: GeneratorKind::Sweep,
            }
        })
        .collect()
}

/// A point on (or just off) the lofted turret shell at height `y` and azimuth `phi` (0 = forward,
/// positive toward +X), `standoff` metres proud of the casting. Interpolates the blueprint's loft
/// stations and evaluates the same superellipse family the shell is skinned from.
fn loft_ring_point(loft: &TurretLoftVisual, y: f32, phi: f32, standoff: f32) -> Vec3 {
    let s = &loft.stations;
    let above = s.iter().position(|st| st.y >= y).unwrap_or(s.len() - 1).max(1);
    let (a, b) = (&s[above - 1], &s[above]);
    let t = ((y - a.y) / (b.y - a.y).max(1.0e-4)).clamp(0.0, 1.0);
    let lerp = |p: f32, q: f32| p + (q - p) * t;
    let half_width = lerp(a.half_width, b.half_width);
    let z_center = lerp(a.z_center, b.z_center);
    let (dx, dz) = (phi.sin(), phi.cos());
    let half_len = if dz >= 0.0 {
        lerp(a.half_len_front, b.half_len_front)
    } else {
        lerp(a.half_len_rear, b.half_len_rear)
    };
    let n = loft.exponent;
    let scale = ((dx.abs() / half_width).powf(n) + (dz.abs() / half_len).powf(n)).powf(-1.0 / n);
    Vec3::new(dx * (scale + standoff), y, z_center + dz * (scale + standoff))
}

/// The stowed tow cables: one running diagonally across the glacis (the top view's signature
/// diagonal), one draped across the hull rear plate.
fn tow_cables(v: &HybridVisual) -> Vec<VehiclePart> {
    // The diagonal stays in the LOWER half of the plate, under the splash board's V, so the two
    // never cross into an X.
    let glacis: Vec<Vec3> = [(0.95_f32, 1.02_f32), (0.35, 1.10), (-0.35, 1.20), (-0.95, 1.30)]
        .iter()
        .map(|&(x, y)| glacis_point(v, x, y, 0.04))
        .collect();
    let rear = vec![
        Vec3::new(-0.65, 1.22, -3.02),
        Vec3::new(-0.20, 1.34, -3.05),
        Vec3::new(0.30, 1.34, -3.05),
        Vec3::new(0.70, 1.22, -3.02),
    ];
    [glacis, rear]
        .into_iter()
        .enumerate()
        .map(|(i, path)| VehiclePart {
            key: PartKey::indexed("tow_cable", i as u16),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::BarrelSteel,
            smoothing: vehicle_geometry::SmoothingGroup::hard_edges(),
            shape: PartShape::Mesh(detail::handle_rail(&path, 0.022)),
            lod: PartLod::Detail,
            generator: GeneratorKind::Sweep,
        })
        .collect()
}
