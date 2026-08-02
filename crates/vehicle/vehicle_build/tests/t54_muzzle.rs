//! A GUN MUZZLE IS A HOLE.
//!
//! It was not one. The bore was written as a fraction of the OUTSIDE of the tube
//! (`muzzle_radius * 0.55`), which is backwards — a gun is a bore with steel wrapped round it —
//! and the tube itself was ⌀170 at the muzzle with a ⌀192 collar, on a gun whose documented
//! muzzle OD is about 120-126 mm. Forty per cent too fat, and the hole 40 mm too narrow inside
//! it: the face came out 76% flat steel with a shallow dimple in the middle.
//!
//! That face is what a tank points at you. These tests measure it the way a viewer meets it —
//! head on, down the axis — rather than trusting the numbers that build it.

use game_core::{VehicleBlueprint, VehicleKind};
use glam::Vec3;
use vehicle_geometry::SubmeshKind;

fn t54_gun() -> (vehicle_geometry::GeometryMesh, game_core::GunVisual, Vec3) {
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let hybrid = blueprint.complete_visual().expect("hybrid visual");
    let description = vehicle_build::t54_description();
    let muzzle = description.mounts.muzzle.translation;
    let baked = description.build();
    let mesh = baked.submesh(SubmeshKind::Gun).expect("gun submesh").mesh.clone();
    (mesh, *hybrid.gun, muzzle)
}

/// The steel ring at the muzzle face is a WALL, not a plate.
///
/// The D-10T is a 100 mm gun with a documented muzzle OD around 120-126 mm — a 10-13 mm wall.
/// Expressed as a fraction of calibre that is about 0.2 to 0.26, and anything much past that is
/// a different gun.
#[test]
fn the_muzzle_wall_is_a_wall_and_not_a_plate() {
    let (_, gun, _) = t54_gun();
    let wall = gun.muzzle_radius - gun.bore_radius;
    let calibre = gun.bore_radius * 2.0;

    assert!(
        (0.0495..=0.0505).contains(&gun.bore_radius),
        "the D-10T is a 100 mm gun, so the bore is 50 mm: got {:.4} m",
        gun.bore_radius
    );
    assert!(
        wall / calibre <= 0.20,
        "the muzzle wall is {:.1} mm on a {:.0} mm bore ({:.0}% of calibre) — the documented tube \
         is 10-13 mm, and a fat tube reads as a capped rod",
        wall * 1000.0,
        calibre * 1000.0,
        wall / calibre * 100.0
    );
    assert!(
        wall > 0.006,
        "but there IS a wall: {:.1} mm of steel, not a paper edge",
        wall * 1000.0
    );
}

/// Looking down the barrel: most of what is inside the tube's outline must be HOLE.
///
/// Measured by firing rays down the gun axis at the muzzle face and asking, ring by ring, where
/// the geometry stops. Independent of how the profile is authored.
#[test]
fn the_muzzle_face_reads_as_an_opening_not_a_steel_disc() {
    let (mesh, gun, muzzle) = t54_gun();
    let vertices = mesh.vertices();
    let indices = mesh.indices();

    // How far a viewer's line of sight travels past the muzzle plane before it meets metal, on
    // a ring of radius `r` about the bore axis. A hole HAS a bottom; what makes it a hole rather
    // than a dent is that the bottom is far away.
    let depth_at = |radius: f32| -> f32 {
        const SAMPLES: usize = 72;
        (0..SAMPLES)
            .map(|sample| {
                let angle = (sample as f32 + 0.31) / SAMPLES as f32 * std::f32::consts::TAU;
                let (sin, cos) = angle.sin_cos();
                let point = Vec3::new(muzzle.x + radius * sin, muzzle.y + radius * cos, muzzle.z);
                first_hit_depth(vertices, indices, point, muzzle.z)
            })
            .fold(f32::INFINITY, f32::min)
    };

    // Just inside the tube's outer skin: steel, right at the face.
    let wall_depth = depth_at(gun.muzzle_radius * 0.96);
    assert!(
        wall_depth.is_finite() && wall_depth < 0.01,
        "the tube's own wall must be at the face, met it {wall_depth:.3} m back"
    );
    // Down the bore: nothing stops the eye for a calibre and a half.
    let bore_depth = depth_at(gun.bore_radius * 0.80);
    assert!(
        bore_depth >= gun.bore_radius * 3.0,
        "the line of sight down the bore stops {bore_depth:.3} m in — under 1.5 calibres that is          a dimple in a steel disc, not the hole a shell comes out of"
    );

    // And the opening is most of the face: the hole's area against the tube's.
    let open_fraction = (gun.bore_radius / gun.muzzle_radius).powi(2);
    assert!(
        open_fraction >= 0.60,
        "only {:.0}% of the muzzle face is hole — it reads as a steel disc with a dimple",
        open_fraction * 100.0
    );
}

/// The hole is DEEP. A shallow recess reads as a dent, because you can see its bottom.
#[test]
fn the_bore_runs_deep_enough_to_be_a_hole() {
    let (mesh, gun, muzzle) = t54_gun();
    let deepest = mesh
        .vertices()
        .iter()
        .filter(|vertex| {
            let radial = (vertex.position.x - muzzle.x).hypot(vertex.position.y - muzzle.y);
            radial <= gun.bore_radius * 1.05 && vertex.position.z <= muzzle.z + 1.0e-4
        })
        .map(|vertex| muzzle.z - vertex.position.z)
        .fold(0.0_f32, f32::max);

    assert!(
        deepest >= gun.bore_radius * 3.0,
        "the bore recedes {:.3} m into a {:.3} m tube — under 1.5 calibres of depth you can see \
         the bottom of it, and it reads as a dimple",
        deepest,
        gun.bore_radius * 2.0
    );
}

/// And the shadow is where the hole is: the bake darkens the bore, so the bright ring of steel at
/// the face stands against a dark opening instead of being lit exactly like it.
#[test]
fn the_bore_carries_its_own_shadow() {
    let description = vehicle_build::t54_description();
    assert!(
        description.surface_bake.signals().contains(&"gun_bore"),
        "the muzzle needs its ambient band — the legacy generator had a dark funnel here and the \
         hybrid lost it, leaving the deepest recess on the vehicle lit like the tube around it"
    );
}

/// How far past `face_z` a ray along the gun axis through `point` travels before it meets metal.
/// Infinite when it meets none.
fn first_hit_depth(
    vertices: &[vehicle_geometry::GeometryVertex],
    indices: &[u32],
    point: Vec3,
    face_z: f32,
) -> f32 {
    let origin = Vec3::new(point.x, point.y, point.z + 4.0);
    let direction = -Vec3::Z;
    let mut nearest = f32::INFINITY;
    for triangle in indices.chunks_exact(3) {
        let a = vertices[triangle[0] as usize].position;
        let b = vertices[triangle[1] as usize].position;
        let c = vertices[triangle[2] as usize].position;
        let edge1 = b - a;
        let edge2 = c - a;
        let h = direction.cross(edge2);
        let det = edge1.dot(h);
        if det.abs() < 1.0e-9 {
            continue;
        }
        let inv_det = 1.0 / det;
        let s = origin - a;
        let u = s.dot(h) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = s.cross(edge1);
        let v = direction.dot(q) * inv_det;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let t = edge2.dot(q) * inv_det;
        if t > 0.0 {
            let hit_z = origin.z - t;
            if hit_z <= face_z + 1.0e-4 {
                nearest = nearest.min(face_z - hit_z);
            }
        }
    }
    nearest
}
