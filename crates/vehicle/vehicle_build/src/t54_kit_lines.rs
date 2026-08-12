//! The T-54's line-work kit: the glacis splash board, turret handrails, stowed tow cables, the
//! unditching beam, the fixed course machine gun's port and the rear smoke canisters. Split from
//! `t54_kit` to keep each module within the reviewability budget.
//!
//! There is no gun travel lock here. One was drawn on the rear deck with no reference behind it,
//! and the dossier is explicit that obr. 1951 carried none (register M10). A fitting nobody can
//! cite is a fitting the vehicle does not have.

use game_core::roundness::round_segments;
use game_core::{CompleteVisual, TurretLoftVisual};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};

/// Every line-work part: splash board, turret handrails, tow cables, unditching beam, the course
/// MG's glacis port and the two rear smoke canisters.
pub(crate) fn t54_line_kit_parts(
    v: CompleteVisual<'_>,
    glacis_deg: f32,
    stern: SternLine,
) -> Vec<VehiclePart> {
    let mut parts = vec![splash_board(v, glacis_deg), unditching_beam(stern)];
    parts.extend(turret_rails(v.turret_loft));
    parts.extend(tow_cables(v, glacis_deg));
    parts.extend(course_mg_port(v, glacis_deg));
    parts.extend(smoke_canisters(stern));
    parts.push(turret_casting_seam(v.turret_loft));
    parts.push(beam_bands(stern));
    parts.extend(stern_furniture(stern));
    parts
}

/// The upper rear plate's service furniture: two dished round plugs and the bolt rows along the
/// plate's edges. Obr. 1951 pulls its transmission by UNBOLTING this whole plate — the rear
/// photographs show a big bolted panel with two round covers, and a stern without them reads as
/// one anonymous wall. Only a knuckled stern carries them: the fleet's single-plate tails have
/// no such panel.
fn stern_furniture(stern: SternLine) -> Vec<VehiclePart> {
    if stern.knuckle.is_none() {
        return Vec::new();
    }
    let rear = stern.rear_deg.to_radians();
    // The plate's outward normal: rearward, tipped up by the 17-degree rake.
    let normal = Vec3::new(0.0, rear.sin(), -rear.cos());
    let mut parts = Vec::with_capacity(3);
    // The two round access plugs, dished with a rim, standing a plug's height off the plate.
    let plug_profile: [(f32, f32); 5] =
        [(0.0, 0.0), (0.0, 0.140), (0.016, 0.140), (0.034, 0.095), (0.040, 0.0)];
    for (i, x) in [-0.55_f32, 0.55].into_iter().enumerate() {
        let seat_y = 1.33;
        let seat = Vec3::new(x, seat_y, stern.z_at(seat_y)) + normal * 0.002;
        parts.push(VehiclePart {
            key: PartKey::indexed("stern_plug", i as u16),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::RolledArmor,
            smoothing: vehicle_geometry::SmoothingGroup(3),
            shape: PartShape::Mesh(revolve::translate(
                &revolve::revolve(
                    normal,
                    &plug_profile,
                    round_segments(0.140),
                    MaterialRole::RolledArmor,
                    vehicle_geometry::SmoothingGroup(3),
                ),
                seat,
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        });
    }
    // The bolt rows: a run under the deck edge and a short column down each side — the fasteners
    // that make "removable panel" legible. Small studs, one merged mesh.
    let bolt_profile: [(f32, f32); 4] = [(0.0, 0.0), (0.0, 0.016), (0.018, 0.016), (0.018, 0.0)];
    let mut studs = Vec::with_capacity(13);
    let mut seat_bolt = |x: f32, y: f32| {
        let seat = Vec3::new(x, y, stern.z_at(y)) + normal * 0.002;
        studs.push(revolve::translate(
            &revolve::revolve(
                normal,
                &bolt_profile,
                8,
                MaterialRole::BarrelSteel,
                vehicle_geometry::SmoothingGroup(3),
            ),
            seat,
        ));
    };
    for k in 0..7 {
        seat_bolt(-0.84 + k as f32 * 0.28, 1.52);
    }
    for side in [-1.0_f32, 1.0] {
        for y in [1.27_f32, 1.37, 1.47] {
            seat_bolt(side * 0.93, y);
        }
    }
    parts.push(VehiclePart {
        key: PartKey::new("stern_plate_bolts"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::BarrelSteel,
        smoothing: vehicle_geometry::SmoothingGroup(3),
        shape: PartShape::Mesh(revolve::merge(&studs).weld_and_smooth()),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    });
    parts
}

/// The stern's side profile, for everything that hangs ON the rear plates: upper-plate angle and
/// the knuckle/undercut pair, read from the same authored armour fields the metal and the armour
/// volumes fold at. Before this, the kit hinged its private 5-degree plate at the DECK while the
/// hull solid hinged its own at the BELLY — the beam and the drums floated against a plate that
/// existed only here.
#[derive(Clone, Copy)]
pub(crate) struct SternLine {
    pub half_len: f32,
    pub belly_y: f32,
    pub rear_deg: f32,
    pub knuckle: Option<(f32, f32)>,
}

impl SternLine {
    /// The stern surface's z at height `y`: on the upper plate above the knuckle, on the
    /// undercut below it; with no knuckle authored, the fleet's single plate hinged at the
    /// hull's rearmost line (the belly, as the solid builds it).
    fn z_at(&self, y: f32) -> f32 {
        match self.knuckle {
            Some((ky, lower_deg)) => {
                if y >= ky {
                    -self.half_len + (y - ky) * self.rear_deg.to_radians().tan()
                } else {
                    -self.half_len + (ky - y) * lower_deg.to_radians().tan()
                }
            }
            None => -self.half_len + (y - self.belly_y) * self.rear_deg.to_radians().tan(),
        }
    }
}

/// The steel bands that strap the unditching log to its brackets.
///
/// A log lashed to a tank is held by something, and the bands are what a viewer reads as that
/// something. Without them the beam is a cylinder resting on air beside the plate (register K9).
///
/// The log itself stays `TrackMetal` for now: giving wood its own `MaterialRole` is open decision
/// #6, and it belongs with the material families rather than being smuggled in here. The bands
/// are the part of this defect that can be closed honestly today.
fn beam_bands(stern: SternLine) -> VehiclePart {
    let seat_y = 0.90;
    let z = stern.z_at(seat_y) - 0.04 - 0.098;
    let mut pieces = Vec::with_capacity(2);
    for side in [-1.0_f32, 1.0] {
        // Outboard of the BDSh-5 drums above (their ends reach |x| 0.685): the bands and the
        // drums share the stern, and the log is 1.9 m wide — there is room for both.
        let x = side * 0.78;
        pieces.push(detail::coaming(
            Vec3::new(x, seat_y, z),
            Vec3::X,
            0.118,
            0.030,
            0.020,
            MaterialRole::BarrelSteel,
            round_segments(0.118),
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
    // AT THE WIDEST BAND, ON THE REAL SURFACE. The seam used to run at y 1.95 off the bare
    // superellipse family — 20 cm above the casting's widest band (1.66-1.76), which is where a
    // mould actually parts (a mould split above its widest section cannot open), and blind to
    // the bumps: the face plateau pushed the metal 49 mm out from under it (seam buried) while
    // the gun window pulled it 71 mm in (seam hanging in air). 480 triangles reading as an
    // error. `surface_point` follows the loft WITH its modulations, so the seam now rides the
    // cheeks and dips through the window the way the mould line on the reference casting does.
    let y = 1.71;
    // No repeated first point: the sweep closes the loop itself. Repeating it puts two end caps
    // in the same place, and the weld turns that into non-manifold edges on the turret submesh.
    let path: Vec<Vec3> = (0..48)
        .map(|k| {
            let phi = std::f32::consts::TAU * k as f32 / 48.0;
            loft.surface_point(y, phi, 0.004)
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
/// The unditching beam: a HEWN timber, not a turned pole.
///
/// It was a twelve-sided lathe revolve of radius 0.10 — which is a pipe. A beam a crew straps to
/// the stern and drives a track onto is squared from a log with an axe: flat faces, knocked-off
/// arrises, sawn ends. `chamfered_box` says exactly that, and says it in eight planes instead of
/// twelve sides of nothing.
///
/// (The comment that used to sit beside this said the log "stays `TrackMetal` for now" pending a
/// wood role. The role landed; the beam has been `Timber` since, and the note outlived its fact.)
fn unditching_beam(stern: SternLine) -> VehiclePart {
    // Stowed LOW across the undercut, a hand's width off the plate and tucked under the BDSh
    // drums that hang on the upper plate above — the stacking every rear walkaround shows. The
    // seat height and the plate's z both follow the authored stern, not a Z somebody typed once.
    let seat_y = 0.90;
    let center = Vec3::new(0.0, seat_y, stern.z_at(seat_y) - 0.04 - 0.098);
    let half = Vec3::new(0.95, 0.098, 0.098);
    VehiclePart {
        key: PartKey::new("unditching_beam"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::Timber,
        smoothing: vehicle_geometry::SmoothingGroup(0),
        // A generous arris: an axed beam is nearer octagonal than square, and the chamfer is where
        // the light catches the edge of the wood.
        shape: PartShape::Plates(solid::chamfered_box(center, half, 0.034)),
        lod: PartLod::Detail,
        generator: GeneratorKind::Solid,
    }
}

/// The fixed course machine gun: an SG-43 laid in the glacis right of centre, fired by the driver
/// (who sits left). What shows outside is a cast boss around the aperture and a short length of
/// jacket — the gun itself is inside the hull.
///
/// The bore runs along +Z because the gun is FIXED and fires straight ahead, so a body of
/// revolution about the vehicle's forward axis is the shape, not a convenience: the boss is a
/// cylinder pushed through a raked plate, which is what the aperture is.
fn course_mg_port(v: CompleteVisual<'_>, glacis_deg: f32) -> Vec<VehiclePart> {
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
                round_segments(0.17),
                MaterialRole::RolledArmor,
                vehicle_geometry::SmoothingGroup(3),
            ),
            seat,
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    }]
}

/// Two BDSh-5 smoke canisters HIGH on the upper rear plate, tucked under the deck edge — the
/// documented seat (rear photographs show the pair hung just below the deck lip, not lying at
/// the tail's foot where the first pass left them).
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
fn smoke_canisters(stern: SternLine) -> Vec<VehiclePart> {
    // Axis ACROSS the vehicle, one drum each side of the centreline. The hang point follows the
    // AUTHORED upper plate — the same 17-degree plane the armour resolves — with the drum tops
    // held under the 1.58 deck line.
    const RADIUS: f32 = 0.225;
    const HALF_LEN: f32 = 0.325;
    let profile = [(-HALF_LEN, 0.0_f32), (-HALF_LEN, RADIUS), (HALF_LEN, RADIUS), (HALF_LEN, 0.0)];
    let hang_y = 1.26;
    let plate_z = stern.z_at(hang_y);
    // Stand the drum a strap's thickness off the PLATE PLANE, along its normal — an offset down
    // the z axis alone would bury the upper edge of the drum in the leaning plate.
    let rear = stern.rear_deg.to_radians();
    let normal = Vec3::new(0.0, rear.sin(), -rear.cos());
    let mut parts = Vec::new();
    for (i, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
        let center = Vec3::new(side * 0.36, hang_y, plate_z) + normal * (RADIUS + 0.012);
        parts.push(VehiclePart {
            key: PartKey::indexed("smoke_canister", i as u16),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::TrackMetal,
            smoothing: vehicle_geometry::SmoothingGroup(3),
            shape: PartShape::Mesh(revolve::translate(
                &revolve::revolve(
                    Vec3::X,
                    &profile,
                    round_segments(RADIUS),
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
                round_segments(RADIUS),
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
fn splash_board(v: CompleteVisual<'_>, glacis_deg: f32) -> VehiclePart {
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
fn glacis_point(v: CompleteVisual<'_>, glacis_deg: f32, x: f32, y: f32, standoff: f32) -> Vec3 {
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
                    loft.ring_point(y, side * phi, 0.05)
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

// `loft_ring_point` lived here as a private duplicate of the blueprint's superellipse family —
// two copies of one curve, and the seam's whole defect class came from the copy being blind to
// the bumps. Both callers read `TurretLoftVisual::{ring_point, surface_point}` now.

/// The stowed tow cables: one running diagonally across the glacis (the top view's signature
/// diagonal), one draped across the hull rear plate.
fn tow_cables(v: CompleteVisual<'_>, glacis_deg: f32) -> Vec<VehiclePart> {
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
            round_segments(0.052),
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
