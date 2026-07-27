//! The honesty lock for the T-54's museum interior: **nothing that lives inside the tank may be
//! seen from outside it**. `DamageLayout::fits_within` only checks the gameplay hitbox box, so it
//! is blind to this class — it passed while the bow rack stood on the glacis (model-logic audit
//! #9), while the radio panel hung off the turret front (#13), while the ready rounds' brass sat
//! on the bustle roof, and while the whole D-10T cradle/recoil/sight line pierced the casting top.
//!
//! The gate here is geometric: every interior part in the turret group must sit inside the cast
//! shell with clearance, and no interior part in the hull group may rise through the deck skin
//! above it.

use game_core::{VehicleBlueprint, VehicleKind};
use glam::{Vec2, Vec3};
use vehicle_build::{VehiclePart, t54_description};
use vehicle_geometry::{MaterialRole, SubmeshKind};

/// Interior materials: what a breach reveals, and what the eye must never meet on an intact tank.
const INTERIOR: [MaterialRole; 3] =
    [MaterialRole::Ammunition, MaterialRole::InteriorPrimer, MaterialRole::InteriorMachinery];

/// Wall-hung equipment legitimately hugs the armour, so the bar is a visible-gap bar, not a
/// stowage-clearance bar: 5 mm of casting between the interior and the outside world.
const MIN_CLEARANCE_M: f32 = 0.005;

/// Below this the casting is open by design (the ring plane is at 1.58): that volume belongs to
/// the hull's fighting compartment, and the shell's bottom rim would answer the query instead of
/// its walls.
const CLOSED_DOME_FLOOR_M: f32 = 1.70;

struct Tri {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    outward: Vec3,
}

#[test]
fn t54_turret_interior_never_pierces_the_casting() {
    let description = t54_description();
    let shell = turret_shell(&description);
    assert!(shell.len() > 200, "the turret casting must be a real shell, not a stub");

    let mut worst: Option<(f32, String)> = None;
    for part in interior_parts(&description, SubmeshKind::Turret) {
        for vertex in part.mesh().vertices() {
            let point = vertex.position;
            if point.y < CLOSED_DOME_FLOOR_M {
                continue;
            }
            let clearance = -signed_distance(&shell, point);
            if worst.as_ref().is_none_or(|(value, _)| clearance < *value) {
                worst = Some((
                    clearance,
                    format!(
                        "{} #{} at ({:+.2}, {:.2}, {:+.2}) [{:?}]",
                        part.key.name, part.key.instance, point.x, point.y, point.z, part.material
                    ),
                ));
            }
        }
    }

    let (clearance, who) = worst.expect("the T-54 turret carries interior parts");
    assert!(
        clearance >= MIN_CLEARANCE_M,
        "interior showing through the T-54 turret casting: {who} has {clearance:+.3} m of \
         clearance (needs {MIN_CLEARANCE_M:.3} m). Move the part INTO the casting — do not \
         relax this bar.",
    );
}

/// The hull's counterpart. The hull is not one closed shell (deck panels, grille strips and
/// covers stack on the roof), so the query is the one the eye makes: from straight above, is
/// there armour over this point, and is the point under it? Anything beyond the flanks belongs
/// to the running gear's world (final-drive housings, torsion-bar ends) and is covered by the
/// track band, not by the hull skin — those are out of this gate's scope.
#[test]
fn t54_hull_interior_stays_under_the_deck_skin() {
    let half_width = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951)
        .expect("T-54 blueprint")
        .hull
        .half_width;
    let description = t54_description();
    let skin = hull_skin(&description);
    assert!(skin.len() > 200, "the hull skin must be real geometry, not a stub");

    for part in interior_parts(&description, SubmeshKind::Hull) {
        for vertex in part.mesh().vertices() {
            let point = vertex.position;
            if point.x.abs() > half_width {
                continue;
            }
            let Some(skin_y) = skin_above(&skin, point.x, point.z) else {
                continue;
            };
            assert!(
                point.y <= skin_y - MIN_CLEARANCE_M,
                "{} #{} reaches {:.3} m at ({:+.2}, {:+.2}) where the hull skin is {skin_y:.3} m \
                 — interior standing on the deck for everyone to see",
                part.key.name,
                part.key.instance,
                point.y,
                point.x,
                point.z,
            );
        }
    }
}

fn interior_parts(
    description: &vehicle_build::VehicleDescription,
    submesh: SubmeshKind,
) -> impl Iterator<Item = &VehiclePart> {
    description
        .parts
        .iter()
        .filter(move |part| part.submesh == submesh && INTERIOR.contains(&part.material))
        // The inner skin IS the casting's back face, offset inwards by the armour thickness.
        .filter(|part| part.key.name != "turret_inner_skin")
}

fn turret_shell(description: &vehicle_build::VehicleDescription) -> Vec<Tri> {
    let mut tris = Vec::new();
    for part in description
        .parts
        .iter()
        .filter(|part| part.submesh == SubmeshKind::Turret && part.key.name == "turret_shell")
    {
        let mesh = part.mesh();
        let vertices = mesh.vertices();
        for triangle in mesh.indices().chunks_exact(3) {
            let [a, b, c] = [0, 1, 2].map(|i| vertices[triangle[i] as usize].position);
            // Orientation comes from the authored vertex normals; the winding convention differs
            // between the kernels that feed this mesh.
            let authored = [0, 1, 2]
                .map(|i| vertices[triangle[i] as usize].normal)
                .into_iter()
                .sum::<Vec3>()
                .normalize_or_zero();
            let geometric = (b - a).cross(c - a).normalize_or_zero();
            if geometric.length_squared() < 0.5 {
                continue;
            }
            let outward = if geometric.dot(authored) < 0.0 { -geometric } else { geometric };
            tris.push(Tri { a, b, c, outward });
        }
    }
    tris
}

fn hull_skin(description: &vehicle_build::VehicleDescription) -> Vec<[Vec3; 3]> {
    let mut tris = Vec::new();
    for part in description.parts.iter().filter(|part| {
        part.submesh == SubmeshKind::Hull
            && matches!(part.material, MaterialRole::RolledArmor | MaterialRole::CastArmor)
    }) {
        let mesh = part.mesh();
        let vertices = mesh.vertices();
        for triangle in mesh.indices().chunks_exact(3) {
            tris.push([0, 1, 2].map(|i| vertices[triangle[i] as usize].position));
        }
    }
    tris
}

/// Height of the highest hull skin triangle straight above (x, z), if any covers that column.
fn skin_above(skin: &[[Vec3; 3]], x: f32, z: f32) -> Option<f32> {
    let p = Vec2::new(x, z);
    let mut highest: Option<f32> = None;
    for [a, b, c] in skin {
        let (a2, b2, c2) = (Vec2::new(a.x, a.z), Vec2::new(b.x, b.z), Vec2::new(c.x, c.z));
        let area = (b2 - a2).perp_dot(c2 - a2);
        if area.abs() < 1.0e-9 {
            continue;
        }
        let (w0, w1) = ((b2 - p).perp_dot(c2 - p) / area, (c2 - p).perp_dot(a2 - p) / area);
        let w2 = 1.0 - w0 - w1;
        if w0 < -1.0e-5 || w1 < -1.0e-5 || w2 < -1.0e-5 {
            continue;
        }
        let y = w0 * a.y + w1 * b.y + w2 * c.y;
        if highest.is_none_or(|best| y > best) {
            highest = Some(y);
        }
    }
    highest
}

/// Negative inside the casting, positive outside it, in metres.
fn signed_distance(shell: &[Tri], point: Vec3) -> f32 {
    let mut nearest = f32::MAX;
    let mut sign = 1.0;
    for tri in shell {
        let closest = closest_point_on_triangle(tri, point);
        let distance = (point - closest).length();
        if distance < nearest {
            nearest = distance;
            sign = if (point - closest).dot(tri.outward) >= 0.0 { 1.0 } else { -1.0 };
        }
    }
    nearest * sign
}

/// Ericson, *Real-Time Collision Detection*, §5.1.5 — Voronoi regions of a triangle.
fn closest_point_on_triangle(tri: &Tri, p: Vec3) -> Vec3 {
    let (a, b, c) = (tri.a, tri.b, tri.c);
    let (ab, ac) = (b - a, c - a);
    let ap = p - a;
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denominator = 1.0 / (va + vb + vc);
    a + ab * (vb * denominator) + ac * (vc * denominator)
}
