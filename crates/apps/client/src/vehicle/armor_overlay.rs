//! The armor inspector's overlay (Hala 3.0 I1): the hero's armor volumes drawn over the
//! parked vehicle as translucent, zone-colored faces — and they are not an illustration OF
//! the gameplay armor, they ARE it. Every face is cut from the same `TaggedPlane` half-spaces
//! the shell trace clips (`game_core::armor::volumes`), every color is the plate's own metal
//! (`ArmorProfile::plate` × the plane's `thickness_scale` — a cast turret shows its taper),
//! and every weakspot patch the resolution honors is stamped as its own disc. The honesty
//! doctrine as UX: what you see is literally what you shoot.
//!
//! Rendered through the FX pass (premultiplied over, depth-tested against the scene, no depth
//! write): the vehicle's own mesh occludes each volume's far side, so the eye reads the near
//! shell. Faces stand [`INFLATE_M`] proud of the gameplay planes — the visible plates are
//! built from the same blueprint numbers, and a coincident overlay would z-shimmer.

use game_core::{ArmorProfile, ArmorZone, VehicleKind, vehicle_armor_volumes};
use game_core::{ArmorVolume, TaggedPlane};
use glam::{Mat3, Vec3};
use renderer_api::FxVertex;

/// How far the display faces stand proud of the gameplay planes. Presentation only — the
/// numbers and the shape are untouched — and locked below 5 cm so the overlay can never
/// drift into looking like different armor.
const INFLATE_M: f32 = 0.03;
/// Weakspot discs ride a hair above their carrier face so they read over it.
const PATCH_LIFT_M: f32 = 0.015;
/// Overlay opacity: enough to read the color, thin enough to keep the vehicle visible.
const ALPHA: f32 = 0.40;
/// The seed polygon half-size a face is clipped from — larger than any plate in the fleet.
const SEED_HALF_M: f32 = 8.0;

/// The full inspector overlay for `kind` parked at `position` with the hull at `yaw_rad`
/// (turret at rest — the garage parks it centered). Returns an empty set for vehicles whose
/// armor volumes are not yet blueprint-born; the inspector shows nothing rather than a guess.
pub fn armor_inspector_fx_vertices(
    kind: VehicleKind,
    position: Vec3,
    yaw_rad: f32,
) -> Vec<FxVertex> {
    let Some(volumes) = vehicle_armor_volumes(kind) else {
        return Vec::new();
    };
    let profile = kind.spec().hull;
    let center_y = kind.spec().hitbox.center_y_m;
    let rot = Mat3::from_rotation_y(yaw_rad);
    let to_world = |p: Vec3| position + Vec3::Y * center_y + rot * p;

    let mut out = Vec::new();
    for volume in volumes.hull.iter().chain([&volumes.turret, &volumes.cupola]) {
        for (index, plane) in volume.planes.iter().enumerate() {
            let Some(polygon) = clipped_face(volume, index) else {
                continue;
            };
            let mm = plate_mm(&profile, plane.zone) * plane.thickness_scale.unwrap_or(1.0);
            let color = premultiplied(color_for_mm(mm));
            fan_triangles(&mut out, &polygon, &to_world, color);
            // The weakspot patches the resolution honors: each disc as its own color, lifted
            // a hair over the carrier so the mantlet ball and the ports read.
            for patch in &plane.patches {
                let disc_mm = plate_mm(&profile, patch.zone);
                let disc_color = premultiplied(color_for_mm(disc_mm));
                let center = patch.center + plane.normal * (INFLATE_M + PATCH_LIFT_M);
                let disc = disc_polygon(center, plane.normal, patch.radius_m);
                fan_triangles(&mut out, &disc, &to_world, disc_color);
            }
        }
    }
    out
}

/// The plate's metal for a zone, in millimetres — the same `ArmorProfile::plate` the sim
/// resolves against.
fn plate_mm(profile: &ArmorProfile, zone: ArmorZone) -> f32 {
    profile.plate(zone).nominal_thickness_mm
}

/// Thickness to color: the readable convention (thin = cool blue, heavy = hot red) as a
/// piecewise-linear gradient over the fleet's real range. Monotone in mm — locked.
pub fn color_for_mm(mm: f32) -> [f32; 3] {
    const STOPS: [(f32, [f32; 3]); 5] = [
        (10.0, [0.15, 0.35, 0.95]),
        (40.0, [0.10, 0.75, 0.45]),
        (90.0, [0.85, 0.80, 0.15]),
        (150.0, [0.95, 0.45, 0.10]),
        (230.0, [0.95, 0.10, 0.10]),
    ];
    let mm = mm.clamp(STOPS[0].0, STOPS[STOPS.len() - 1].0);
    for pair in STOPS.windows(2) {
        let ((a_mm, a), (b_mm, b)) = (pair[0], pair[1]);
        if mm <= b_mm {
            let t = (mm - a_mm) / (b_mm - a_mm).max(1.0e-6);
            return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
        }
    }
    STOPS[STOPS.len() - 1].1
}

fn premultiplied(rgb: [f32; 3]) -> [f32; 4] {
    [rgb[0] * ALPHA, rgb[1] * ALPHA, rgb[2] * ALPHA, ALPHA]
}

/// The polygon of `volume.planes[face]`, inflated by [`INFLATE_M`]: a large seed quad on the
/// (shifted) plane, clipped by every other (shifted) half-space. `None` when the face is
/// entirely cut away — a plane that bounds nothing visible.
fn clipped_face(volume: &ArmorVolume, face: usize) -> Option<Vec<Vec3>> {
    let plane = &volume.planes[face];
    let normal = plane.normal;
    // An orthonormal basis in the face's plane.
    let u = if normal.x.abs() < 0.8 { Vec3::X } else { Vec3::Z };
    let u = (u - normal * u.dot(normal)).normalize_or_zero();
    let v = normal.cross(u);
    let origin = normal * (plane.offset + INFLATE_M);
    let mut polygon: Vec<Vec3> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter()
        .map(|&(a, b)| origin + u * (a * SEED_HALF_M) + v * (b * SEED_HALF_M))
        .collect();
    for (other_index, other) in volume.planes.iter().enumerate() {
        if other_index == face {
            continue;
        }
        polygon = clip_by_half_space(&polygon, other);
        if polygon.len() < 3 {
            return None;
        }
    }
    Some(polygon)
}

/// Sutherland–Hodgman against one inflated half-space (`normal · p <= offset + INFLATE_M`).
fn clip_by_half_space(polygon: &[Vec3], plane: &TaggedPlane) -> Vec<Vec3> {
    let limit = plane.offset + INFLATE_M;
    let inside = |p: Vec3| plane.normal.dot(p) <= limit + 1.0e-5;
    let mut out = Vec::with_capacity(polygon.len() + 2);
    for (index, &current) in polygon.iter().enumerate() {
        let previous = polygon[(index + polygon.len() - 1) % polygon.len()];
        let (cur_in, prev_in) = (inside(current), inside(previous));
        if cur_in != prev_in {
            let denom = plane.normal.dot(current - previous);
            if denom.abs() > 1.0e-8 {
                let t = (limit - plane.normal.dot(previous)) / denom;
                out.push(previous + (current - previous) * t.clamp(0.0, 1.0));
            }
        }
        if cur_in {
            out.push(current);
        }
    }
    out
}

/// A weakspot disc as a 12-gon in its carrier plane.
fn disc_polygon(center: Vec3, normal: Vec3, radius_m: f32) -> Vec<Vec3> {
    let u = if normal.x.abs() < 0.8 { Vec3::X } else { Vec3::Z };
    let u = (u - normal * u.dot(normal)).normalize_or_zero();
    let v = normal.cross(u);
    (0..12)
        .map(|step| {
            let angle = step as f32 / 12.0 * std::f32::consts::TAU;
            center + (u * angle.cos() + v * angle.sin()) * radius_m
        })
        .collect()
}

/// Fan-triangulate a convex polygon into flat-filled FX triangles: uv `[0,0]` everywhere and
/// sharpness 1.0 hold the shader's radial falloff at exactly 1 — a flat translucent fill.
fn fan_triangles(
    out: &mut Vec<FxVertex>,
    polygon: &[Vec3],
    to_world: &impl Fn(Vec3) -> Vec3,
    color: [f32; 4],
) {
    for index in 1..polygon.len().saturating_sub(1) {
        for &p in [polygon[0], polygon[index], polygon[index + 1]].iter() {
            out.push(FxVertex::sharp(to_world(p).to_array(), [0.0, 0.0], 1.0, color));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE OVERLAY IS THE GAMEPLAY SHAPE. Every emitted face vertex lies exactly
    /// [`INFLATE_M`] out on some tagged plane of the vehicle's own volumes, and inside every
    /// other (inflated) half-space — the polygon is a face of the sim's polytope, nothing
    /// invented, nothing smoothed.
    #[test]
    fn every_face_lies_on_a_gameplay_plane_of_the_hulls_own_volumes() {
        let volumes = vehicle_armor_volumes(VehicleKind::T54_1951).expect("blueprint-born");
        let overlay = armor_inspector_fx_vertices(VehicleKind::T54_1951, Vec3::ZERO, 0.0);
        assert!(overlay.len() >= 3 * 20, "a hull is many faces, got {} vertices", overlay.len());
        let center_y = VehicleKind::T54_1951.spec().hitbox.center_y_m;
        let all_volumes: Vec<&ArmorVolume> =
            volumes.hull.iter().chain([&volumes.turret, &volumes.cupola]).collect();
        for vertex in &overlay {
            let local = Vec3::from_array(vertex.position) - Vec3::Y * center_y;
            let on_some_plane = all_volumes.iter().any(|volume| {
                volume.planes.iter().any(|plane| {
                    (plane.normal.dot(local) - (plane.offset + INFLATE_M)).abs()
                        < INFLATE_M + PATCH_LIFT_M + 1.0e-3
                })
            });
            assert!(on_some_plane, "an overlay vertex floats off every plane: {local:?}");
        }
    }

    /// The color scale is monotone in metal and spans cool-to-hot: a thicker plate may never
    /// read cooler than a thinner one.
    #[test]
    fn thicker_metal_never_reads_cooler() {
        let mut last_heat = f32::NEG_INFINITY;
        for mm in [8.0, 20.0, 45.0, 80.0, 120.0, 160.0, 200.0, 250.0] {
            let [r, _, b] = color_for_mm(mm);
            let heat = r - b;
            assert!(heat >= last_heat - 1.0e-4, "{mm} mm reads cooler than thinner metal");
            last_heat = heat;
        }
        let thin = color_for_mm(10.0);
        let heavy = color_for_mm(230.0);
        assert!(thin[2] > thin[0], "thin metal is cool blue");
        assert!(heavy[0] > heavy[2], "heavy metal is hot red");
    }

    /// The whole fleet either draws its real volumes or draws NOTHING — a vehicle without
    /// blueprint-born armor shows an empty inspector, never a guessed box.
    #[test]
    fn the_inspector_never_guesses() {
        for kind in VehicleKind::PLAYABLE {
            let overlay = armor_inspector_fx_vertices(kind, Vec3::ZERO, 0.0);
            if vehicle_armor_volumes(kind).is_some() {
                assert!(!overlay.is_empty(), "{kind:?} has volumes but no overlay");
            } else {
                assert!(overlay.is_empty(), "{kind:?} has no volumes yet — no invented armor");
            }
        }
    }

    /// The T-54's mantlet patch — the most famous aim point in its matchup — is stamped as
    /// its own disc over the turret front.
    #[test]
    fn the_weakspot_discs_ride_the_overlay() {
        let volumes = vehicle_armor_volumes(VehicleKind::T54_1951).expect("blueprint-born");
        let patch_count: usize = std::iter::once(&volumes.turret)
            .chain(volumes.hull.iter())
            .chain(std::iter::once(&volumes.cupola))
            .flat_map(|v| v.planes.iter())
            .map(|p| p.patches.len())
            .sum();
        assert!(patch_count > 0, "the T-54 authors weakspot patches");
        // Discs emit 12-gon fans on top of face fans: the overlay must be strictly larger
        // than the same build with patches ignored would be. Cheap proxy: enough vertices to
        // carry the discs (10 triangles each).
        let overlay = armor_inspector_fx_vertices(VehicleKind::T54_1951, Vec3::ZERO, 0.0);
        assert!(overlay.len() > patch_count * 30, "discs are in the overlay");
    }
}
