//! The T-54's line-work kit: the glacis splash board, turret handrails, stowed tow cables, the
//! unditching beam, the fixed course machine gun's port and the rear smoke canisters. Split from
//! `t54_kit` to keep each module within the reviewability budget.
//!
//! There is no gun travel lock here. One was drawn on the rear deck with no reference behind it,
//! and the dossier is explicit that obr. 1951 carried none (register M10). A fitting nobody can
//! cite is a fitting the vehicle does not have.

use game_core::{TurretLoftVisual, VisualDetail};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};

/// Every line-work part: splash board, turret handrails, tow cables, unditching beam, the course
/// MG's glacis port and the two rear smoke canisters.
pub(crate) fn t54_line_kit_parts(v: &VisualDetail, glacis_deg: f32) -> Vec<VehiclePart> {
    let mut parts = vec![splash_board(v, glacis_deg), unditching_beam(v)];
    parts.extend(turret_rails(&v.turret_loft));
    parts.extend(tow_cables(v, glacis_deg));
    parts.extend(course_mg_port(v, glacis_deg));
    parts.extend(smoke_canisters(v));
    parts.push(turret_casting_seam(&v.turret_loft));
    parts.push(beam_bands(v));
    parts
}

/// The steel bands that strap the unditching log to its brackets.
///
/// A log lashed to a tank is held by something, and the bands are what a viewer reads as that
/// something. Without them the beam is a cylinder resting on air beside the plate (register K9).
///
/// The log itself stays `TrackMetal` for now: giving wood its own `MaterialRole` is open decision
/// #6, and it belongs with the material families rather than being smuggled in here. The bands
/// are the part of this defect that can be closed honestly today.
fn beam_bands(v: &VisualDetail) -> VehiclePart {
    let z = -v.hull.half_len - 0.04;
    let mut pieces = Vec::with_capacity(2);
    for side in [-1.0_f32, 1.0] {
        // Outboard of the BDSh-5 drums below (their ends reach |x| 0.685): the bands and the
        // drums share the rear plate, and the log is 1.9 m wide — there is room for both.
        let x = side * 0.78;
        pieces.push(detail::coaming(
            Vec3::new(x, 1.02, z),
            Vec3::X,
            0.118,
            0.030,
            0.020,
            MaterialRole::BarrelSteel,
            12,
        ));
    }
    VehiclePart {
        key: PartKey::new("unditching_beam_bands"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::BarrelSteel,
        smoothing: vehicle_geometry::SmoothingGroup(3),
        shape: PartShape::Mesh(revolve::merge(&pieces).weld_and_smooth()),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    }
}

/// The turret's mould line: the raised seam left where the two halves of the casting mould met,
/// running round the dome at cheek height.
///
/// `detail::casting_seam` has existed since the detail kernel was written and had never been
/// called once. A cast turret without its mould line reads as a pressing — and this is a vehicle
/// whose whole front is one casting.
fn turret_casting_seam(loft: &TurretLoftVisual) -> VehiclePart {
    // At the widest band of the casting, where the mould parts. Traced right round, so the seam
    // closes on itself the way the mould does.
    let y = 1.95;
    // No repeated first point: the sweep closes the loop itself. Repeating it puts two end caps
    // in the same place, and the weld turns that into non-manifold edges on the turret submesh.
    let path: Vec<Vec3> = (0..48)
        .map(|k| {
            let phi = std::f32::consts::TAU * k as f32 / 48.0;
            loft_ring_point(loft, y, phi, 0.004)
        })
        .collect();
    VehiclePart {
        key: PartKey::new("turret_casting_seam"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::CastArmor,
        smoothing: vehicle_geometry::SmoothingGroup(7),
        shape: PartShape::Mesh(detail::casting_seam_loop(&path)),
        lod: PartLod::Detail,
        generator: GeneratorKind::Sweep,
    }
}

/// The unditching beam: the log carried horizontally across the lower rear plate, as the
/// references' rear/top views show. TIMBER, in its own material — open decision #6, resolved:
/// rendering wood as track steel was the last recorded material compromise on this vehicle. Its
/// ends stop WELL inside the hull side planes (±1.03) and the log floats a hand off the raked
/// rear plate — any coplanar contact with the hull z-fights as the camera moves.
fn unditching_beam(v: &VisualDetail) -> VehiclePart {
    let profile = [(-0.95_f32, 0.0_f32), (-0.95, 0.10), (0.95, 0.10), (0.95, 0.0)];
    VehiclePart {
        key: PartKey::new("unditching_beam"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::Timber,
        smoothing: vehicle_geometry::SmoothingGroup(5),
        shape: PartShape::Mesh(revolve::translate(
            &revolve::revolve(
                Vec3::X,
                &profile,
                12,
                MaterialRole::Timber,
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
fn course_mg_port(v: &VisualDetail, glacis_deg: f32) -> Vec<VehiclePart> {
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

/// Two BDSh-5 smoke canisters on the lower rear plate, below the unditching beam.
///
/// At their DOCUMENTED size. The tank smoke canister is the BDSh-5 (developed 1944 for the
/// T-34-85's rear plate, carried until exhaust smoke systems displaced it; "MDSh" in the
/// modelling literature is its naval parent): a drum 650 mm long and 450 mm across, 45-50 kg.
/// The first pass here drew 220 mm drums, sized by the collision box instead of by a source, and
/// recorded that as a compromise. The compromise is withdrawn.
///
/// A 450 mm drum hung on the plate necessarily reaches past the hitbox — by the same honest
/// exception the MAIN GUN already holds: the box is the fighting body, and thin or expendable
/// stowage stands outside it (2.6 m of barrel already does). The reach is asserted, not hidden:
/// `t54_carries_two_smoke_canisters_on_the_rear_plate` locks both the documented diameter and
/// the documented protrusion.
fn smoke_canisters(v: &VisualDetail) -> Vec<VehiclePart> {
    // Axis ACROSS the vehicle, one drum each side of the centreline, under the beam. The lower
    // plate rakes 5 degrees, so the hang point follows it down.
    const RADIUS: f32 = 0.225;
    const HALF_LEN: f32 = 0.325;
    let profile = [(-HALF_LEN, 0.0_f32), (-HALF_LEN, RADIUS), (HALF_LEN, RADIUS), (HALF_LEN, 0.0)];
    let hang_y = 0.665;
    let plate_z = -v.hull.half_len + (1.58 - hang_y) * (5.0_f32).to_radians().tan();
    let mut parts = Vec::new();
    for (i, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
        let center = Vec3::new(side * 0.36, hang_y, plate_z - 0.012 - RADIUS);
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
                center,
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        });
        // The quick-release straps that hold a 50 kg drum to a moving tank.
        let mut straps = Vec::with_capacity(2);
        for s in [-1.0_f32, 1.0] {
            straps.push(detail::coaming(
                center + Vec3::new(s * HALF_LEN * 0.62, 0.0, 0.0),
                Vec3::X,
                RADIUS + 0.012,
                0.030,
                0.014,
                MaterialRole::BarrelSteel,
                12,
            ));
        }
        parts.push(VehiclePart {
            key: PartKey::indexed("smoke_canister_strap", i as u16),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::BarrelSteel,
            smoothing: vehicle_geometry::SmoothingGroup(3),
            shape: PartShape::Mesh(revolve::merge(&straps).weld_and_smooth()),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        });
    }
    parts
}

/// The V-shaped splash board across the upper glacis: a raised bead standing just proud of the
/// plate, vertex low on the centreline, arms rising outward — every front view shows it.
fn splash_board(v: &VisualDetail, glacis_deg: f32) -> VehiclePart {
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
fn glacis_point(v: &VisualDetail, glacis_deg: f32, x: f32, y: f32, standoff: f32) -> Vec3 {
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
fn tow_cables(v: &VisualDetail, glacis_deg: f32) -> Vec<VehiclePart> {
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
    // A stowed cable is not a floating tube. Each end carries a THIMBLE — the teardrop steel eye
    // the rope is spliced round — and the run is held down by CLAMPS bolted to the plate. Ours
    // levitated on a 0.04 standoff with neither, which is a rope lying on air (register K9).
    let mut parts: Vec<VehiclePart> = Vec::new();
    for (i, path) in [&glacis, &rear].into_iter().enumerate() {
        parts.push(VehiclePart {
            key: PartKey::indexed("tow_cable", i as u16),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::BarrelSteel,
            smoothing: vehicle_geometry::SmoothingGroup::hard_edges(),
            shape: PartShape::Mesh(detail::handle_rail(path, 0.022)),
            lod: PartLod::Detail,
            generator: GeneratorKind::Sweep,
        });
        parts.push(cable_hardware(i as u16, path));
    }
    parts
}

/// The steel a stowed cable actually hangs on: a thimble at each end and two clamps along the run.
fn cable_hardware(index: u16, path: &[Vec3]) -> VehiclePart {
    let mut pieces = Vec::with_capacity(4);
    // Thimbles: a ring at each spliced end, standing across the rope.
    for end in [path[0], path[path.len() - 1]] {
        pieces.push(detail::coaming(
            end,
            Vec3::Z,
            0.052,
            0.024,
            0.020,
            MaterialRole::BarrelSteel,
            12,
        ));
    }
    // Clamps: straps over the run at the quarter points, where a fitter would put them.
    for t in [0.33_f32, 0.67] {
        let i = ((path.len() - 1) as f32 * t).round() as usize;
        let at = path[i.min(path.len() - 1)];
        pieces.push(detail::grab_handle(
            at - Vec3::X * 0.055,
            at + Vec3::X * 0.055,
            Vec3::Y,
            0.030,
        ));
    }
    VehiclePart {
        key: PartKey::indexed("tow_cable_hardware", index),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::BarrelSteel,
        smoothing: vehicle_geometry::SmoothingGroup(3),
        shape: PartShape::Mesh(revolve::merge(&pieces).weld_and_smooth()),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    }
}
