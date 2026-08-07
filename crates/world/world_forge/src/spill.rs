//! What a building leaves on the street.
//!
//! `SceneryKind::DebrisHeap` used to be a grey frustum with two grey boxes beside it — three
//! lumps of one colour, in a town whose facades are brick, plaster, roof tile and timber. It read
//! as "some rubble" rather than as the thing it is: a piece of a house, on the ground.
//!
//! So the spill is baked from the SAME material vocabulary the facades are (`WorldMaterial`), and
//! the scene decodes it through the same one function they do. That buys the Fasada 2.0 surface
//! treatments for nothing — the plaster gets its trowel blotches, the tile its courses, the joist
//! its grain — and it means a change to what a wall is made of reaches the wall's wreckage too.
//!
//! It stays LOOSE and knee-high by construction. Loose is what earns it the right to sit above
//! the fleet's belly line at all (see `scene_build::clutter::BELLY_LINE_M`): a hull drives through
//! spilled brick, scattering it, and nothing lies. A solid at this height would have to be cover.

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, SmoothingGroup};

use crate::WorldMaterial;
use crate::shape::{Rng, fracture_solid, merge_meshes};

/// The tallest the spill may stand above its ground line, metres. Knee-high on purpose: the
/// honest-blockers rule says nothing that reads as cover may be render-only, and a masonry pile
/// tall enough to hide a hull would be exactly that.
pub const SPILL_HEIGHT_CAP_M: f32 = 0.62;

/// Ratchet against silent growth. Measured today: 72 triangles — a 7-sided mass, three tumbled
/// wall pieces, two roof shards and one snapped joist.
pub const SPILL_MAX_TRIS: usize = 88;

/// The review gate at seed 0 (golden; bless deliberately).
pub const SPILL_GOLDEN_HASH: u64 = 0x312e_ea26_071f_db8f;

/// Bake one masonry spill, standing on the origin with `y = 0` its ground line.
pub fn bake_debris_spill(seed: u64) -> GeometryMesh {
    let mut rng = Rng(seed ^ 0x5D18_B115);

    // The mass: what a wall becomes when it stops being a wall. Wide, low, and plaster-toned,
    // because the broken face of rendered masonry is mostly its render.
    let mut spill = fracture_solid(
        &mut rng,
        Vec3::new(0.0, -0.06, 0.0),
        0.92,
        0.34,
        0.14,
        7,
        WorldMaterial::Wall,
    );

    // Tumbled pieces around it: the parts that fell far enough to still be pieces. Placed on a
    // spread bearing so the spill has a direction — masonry falls AWAY from the wall it was.
    let throw = rng.unit() * std::f32::consts::TAU;
    for piece in 0..3 {
        let angle = throw + (piece as f32 - 1.0) * 0.9 + rng.signed() * 0.3;
        let reach = 0.55 + rng.unit() * 0.55;
        let size = 0.20 + rng.unit() * 0.14;
        let centre = Vec3::new(angle.cos() * reach, -size * 0.35, angle.sin() * reach);
        spill = merge_meshes(
            spill,
            fracture_solid(&mut rng, centre, size, size * 0.8, size * 0.5, 5, WorldMaterial::Wall),
        );
    }

    // Roof tile: platy, so the rise is a fraction of the radius rather than a match for it. Two
    // shards is enough to name the material — a roof does not survive its walls.
    for shard in 0..2 {
        let angle = throw + std::f32::consts::PI + shard as f32 * 1.4 + rng.signed() * 0.4;
        let reach = 0.45 + rng.unit() * 0.5;
        let size = 0.22 + rng.unit() * 0.10;
        let centre = Vec3::new(angle.cos() * reach, -0.02, angle.sin() * reach);
        spill = merge_meshes(
            spill,
            fracture_solid(&mut rng, centre, size, size * 0.22, size * 0.2, 4, WorldMaterial::Roof),
        );
    }

    // The snapped joist: one strong diagonal that says STRUCTURE. Without it the pile is
    // geology; with it, it is a building.
    let joist_angle = throw + 2.1 + rng.signed() * 0.5;
    let joist_dir = Vec3::new(joist_angle.cos(), 0.0, joist_angle.sin());
    let joist_rise = 0.16 + rng.unit() * 0.12;
    spill = merge_meshes(
        spill,
        beam(
            Vec3::new(-joist_dir.x * 0.30, 0.02, -joist_dir.z * 0.30),
            joist_dir * (0.85 + rng.unit() * 0.45) + Vec3::Y * joist_rise,
            0.055,
            WorldMaterial::Timber,
        ),
    );

    // Knee-high by construction, then by clamp — the same discipline the talus drift takes.
    let vertices: Vec<GeometryVertex> = spill
        .vertices()
        .iter()
        .map(|vertex| {
            let mut pressed = *vertex;
            pressed.position.y = pressed.position.y.min(SPILL_HEIGHT_CAP_M);
            pressed
        })
        .collect();
    GeometryMesh::new(vertices, spill.indices().to_vec())
}

/// A square-section beam from `a` to `a + run`: four flat sides, capped at both ends. Distinct
/// from a fracture solid — a sawn timber has parallel faces, which is exactly what makes it read
/// as milled rather than broken.
fn beam(a: Vec3, run: Vec3, half_thickness: f32, material: WorldMaterial) -> GeometryMesh {
    let axis = run.normalize_or_zero();
    let reference = if axis.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let u = axis.cross(reference).normalize_or_zero() * half_thickness;
    let v = axis.cross(u.normalize_or_zero()).normalize_or_zero() * half_thickness;
    let b = a + run;
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let near: Vec<Vec3> = corners.iter().map(|(su, sv)| a + u * *su + v * *sv).collect();
    let far: Vec<Vec3> = corners.iter().map(|(su, sv)| b + u * *su + v * *sv).collect();

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face = |p: [Vec3; 4]| {
        let normal = (p[1] - p[0]).cross(p[3] - p[0]).normalize_or_zero();
        let base = vertices.len() as u32;
        for corner in p {
            vertices.push(GeometryVertex::new(
                corner,
                normal,
                material.carrier(),
                SmoothingGroup::hard_edges(),
            ));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    for side in 0..4 {
        let next = (side + 1) % 4;
        face([near[side], far[side], far[next], near[next]]);
    }
    face([near[3], near[2], near[1], near[0]]);
    face([far[0], far[1], far[2], far[3]]);
    GeometryMesh::new(vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spill_bakes_deterministic_within_budget_and_on_its_golden() {
        let first = bake_debris_spill(0);
        let again = bake_debris_spill(0);
        let hash = |mesh: &GeometryMesh| {
            let mut h = 0xcbf2_9ce4_8422_2325_u64;
            for vertex in mesh.vertices() {
                for value in vertex.position.to_array().into_iter().chain(vertex.normal.to_array())
                {
                    crate::fnv(&mut h, u64::from(value.to_bits()));
                }
            }
            for index in mesh.indices() {
                crate::fnv(&mut h, u64::from(*index));
            }
            h
        };
        assert_eq!(hash(&first), hash(&again), "the spill bakes deterministically");
        assert_eq!(hash(&first), SPILL_GOLDEN_HASH, "moved off golden: {:#018x}", hash(&first));
        let tris = first.triangle_count();
        assert!(tris > 0 && tris <= SPILL_MAX_TRIS, "spill broke its ceiling: {tris}");
        assert!(first.indices().iter().all(|i| (*i as usize) < first.vertex_count()));
    }

    /// The honest-blockers rule, at the source: a render-only pile stays knee-high at every seed.
    #[test]
    fn the_spill_stays_knee_high() {
        for seed in 0..24 {
            let spill = bake_debris_spill(seed);
            let top = spill.bounds().expect("a spill has geometry").max.y;
            assert!(top <= SPILL_HEIGHT_CAP_M + 1.0e-4, "seed {seed} stands {top} m");
            assert!(top > 0.15, "seed {seed} is not a pile: {top} m");
        }
    }

    /// It reads as a BUILDING, not as geology: the spill carries the wall's render, the roof's
    /// tile and a snapped timber, so the scene's one material decode dresses each in its own
    /// treatment. One grey pile is the defect this replaces.
    #[test]
    fn the_spill_is_made_of_the_building_it_fell_from() {
        let spill = bake_debris_spill(3);
        let mut seen: Vec<WorldMaterial> = spill
            .vertices()
            .iter()
            .map(|vertex| WorldMaterial::from_carrier(vertex.material))
            .collect();
        seen.sort_by_key(|material| format!("{material:?}"));
        seen.dedup();
        for expected in [WorldMaterial::Wall, WorldMaterial::Roof, WorldMaterial::Timber] {
            assert!(seen.contains(&expected), "the spill has no {expected:?}: {seen:?}");
        }
    }

    /// Closed and outward-wound, the beam included — a milled timber with an inverted cap would
    /// light inside out exactly where the eye goes.
    #[test]
    fn every_spill_triangle_winds_outward() {
        for seed in [0, 5, 19] {
            let spill = bake_debris_spill(seed);
            let vertices = spill.vertices();
            let volume: f32 = spill
                .indices()
                .chunks_exact(3)
                .map(|t| {
                    let (a, b, c) = (
                        vertices[t[0] as usize].position,
                        vertices[t[1] as usize].position,
                        vertices[t[2] as usize].position,
                    );
                    a.dot(b.cross(c)) / 6.0
                })
                .sum();
            assert!(volume > 0.0, "seed {seed} is wound inside out: {volume} m³");
        }
    }
}
