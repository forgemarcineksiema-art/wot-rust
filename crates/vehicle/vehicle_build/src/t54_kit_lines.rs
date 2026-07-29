//! The T-54's line-work kit: the glacis splash board, turret handrails, stowed tow cables, the
//! unditching beam, the fixed course machine gun's port and the rear smoke canisters. Split from
//! `t54_kit` to keep each module within the reviewability budget.
//!
//! There is no gun travel lock here. One was drawn on the rear deck with no reference behind it,
//! and the dossier is explicit that obr. 1951 carried none (register M10). A fitting nobody can
//! cite is a fitting the vehicle does not have.

use game_core::{HybridVisual, TurretLoftVisual};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};

/// Every line-work part: splash board, turret handrails, tow cables, unditching beam, the course
/// MG's glacis port and the two rear smoke canisters.
pub(crate) fn t54_line_kit_parts(v: &HybridVisual, glacis_deg: f32) -> Vec<VehiclePart> {
    let mut parts = vec![splash_board(v, glacis_deg), unditching_beam(v)];
    parts.extend(turret_rails(&v.turret_loft));
    parts.extend(tow_cables(v, glacis_deg));
    parts.extend(course_mg_port(v, glacis_deg));
    parts.extend(smoke_canisters(v));
    parts
}

/// The unditching beam: the log carried horizontally across the lower rear plate, as the
/// references' rear/top views show. Steel-strapped dark timber at this fidelity. Its ends stop
/// WELL inside the hull side planes (±1.03) and the log floats a hand off the raked rear plate —
/// any coplanar contact with the hull z-fights as the camera moves.
fn unditching_beam(v: &HybridVisual) -> VehiclePart {
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
            // Stowed against the rear plate, a hand's width off it — so it follows the
            // stern rather than sitting at a Z somebody typed once.
            Vec3::new(0.0, 1.02, -v.hull.half_len - 0.04),
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    }
}

/// The fixed course machine gun: an SG-43 laid in the glacis right of centre, fired by the driver
/// (who sits left). What shows outside is a cast boss around the aperture and a short length of
/// jacket — the gun itself is inside the hull.
///
/// The bore runs along +Z because the gun is FIXED and fires straight ahead, so a body of
/// revolution about the vehicle's forward axis is the shape, not a convenience: the boss is a
/// cylinder pushed through a raked plate, which is what the aperture is.
fn course_mg_port(v: &HybridVisual, glacis_deg: f32) -> Vec<VehiclePart> {
    // Right of centre, at the height the driver's shoulder reaches. The driver's hatch sits at
    // x = -0.45, so this is the other side of the plate from him — as every reference view shows.
    let seat = glacis_point(v, glacis_deg, 0.42, 1.15, 0.0);
    let profile = [
        (-0.10_f32, 0.000_f32),
        (-0.10, 0.105),
        (0.02, 0.105),
        (0.06, 0.074),
        (0.06, 0.030),
        (0.17, 0.026),
        (0.17, 0.000),
    ];
    vec![VehiclePart {
        key: PartKey::new("course_mg_port"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: vehicle_geometry::SmoothingGroup(3),
        shape: PartShape::Mesh(revolve::translate(
            &revolve::revolve(
                Vec3::Z,
                &profile,
                16,
                MaterialRole::RolledArmor,
                vehicle_geometry::SmoothingGroup(3),
            ),
            seat,
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    }]
}

/// Two MDSh smoke canisters on the rear plate, below the unditching beam.
///
/// The dossier records the FITTING (2x MDSh on the rear plate) but neither its dimensions nor how
/// it is bracketed, so two things fix the drum here and neither of them is a source, which is
/// worth saying out loud.
///
/// The first is the vehicle's own rear convention: the unditching beam already hangs at
/// `half_len + 0.04` with a 0.10 radius, so its back face lands 10 mm inside the hitbox. Rear
/// fittings on this tank tuck into the 0.15 m the collision box carries past the plates, because
/// the honesty doctrine is that the collision box IS the visual footprint — a canister sticking
/// 0.39 m out the back would be metal a shell flies through. So these lie ACROSS the plate rather
/// than pointing out of it, and reach the same depth the beam does.
///
/// The second is that a drum bounded that way is smaller than an MDSh really is. That is a known
/// approximation, recorded here rather than in the dimension gate — the gate is for numbers with
/// sources, and this one has a constraint instead.
fn smoke_canisters(v: &HybridVisual) -> Vec<VehiclePart> {
    // Axis ACROSS the vehicle: the drum lies against the plate instead of pointing off the stern.
    let profile = [(-0.275_f32, 0.0_f32), (-0.275, 0.11), (0.275, 0.11), (0.275, 0.0)];
    let mut parts = Vec::new();
    for (i, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
        parts.push(VehiclePart {
            key: PartKey::indexed("smoke_canister", i as u16),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::TrackMetal,
            smoothing: vehicle_geometry::SmoothingGroup(3),
            shape: PartShape::Mesh(revolve::translate(
                &revolve::revolve(
                    Vec3::X,
                    &profile,
                    14,
                    MaterialRole::TrackMetal,
                    vehicle_geometry::SmoothingGroup(3),
                ),
                // Clear of the beam above them (y 1.02) and inside the hull sides.
                Vec3::new(side * 0.55, 0.70, -v.hull.half_len - 0.03),
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        });
    }
    parts
}

/// The V-shaped splash board across the upper glacis: a raised bead standing just proud of the
/// plate, vertex low on the centreline, arms rising outward — every front view shows it.
fn splash_board(v: &HybridVisual, glacis_deg: f32) -> VehiclePart {
    let path: Vec<Vec3> = [(-0.85_f32, 1.42_f32), (0.0, 1.26), (0.85, 1.42)]
        .iter()
        .map(|&(x, y)| glacis_point(v, glacis_deg, x, y, 0.03))
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
fn glacis_point(v: &HybridVisual, glacis_deg: f32, x: f32, y: f32, standoff: f32) -> Vec3 {
    // The rake comes from the blueprint (`armor.hull_front.0`) — the same number the plate is
    // built at. It used to be a literal 60.0 while the doc comment claimed otherwise, so any
    // future change to the glacis angle would have left this line work floating off the plate.
    let (sin, cos) = glacis_deg.to_radians().sin_cos();
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
fn tow_cables(v: &HybridVisual, glacis_deg: f32) -> Vec<VehiclePart> {
    // The diagonal stays in the LOWER half of the plate, under the splash board's V, so the two
    // never cross into an X.
    let glacis: Vec<Vec3> = [(0.95_f32, 1.02_f32), (0.35, 1.10), (-0.35, 1.20), (-0.95, 1.30)]
        .iter()
        .map(|&(x, y)| glacis_point(v, glacis_deg, x, y, 0.04))
        .collect();
    let rear = vec![
        Vec3::new(-0.65, 1.22, -v.hull.half_len - 0.02),
        Vec3::new(-0.20, 1.34, -v.hull.half_len - 0.05),
        Vec3::new(0.30, 1.34, -v.hull.half_len - 0.05),
        Vec3::new(0.70, 1.22, -v.hull.half_len - 0.02),
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
