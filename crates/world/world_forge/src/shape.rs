//! The primitives every world generator builds from: one randomness, one sphere, one merge.
//!
//! Hoisted out of `building`, `tree` and `rock` (Skały 1.0), which had each grown a private copy
//! of the same splitmix64 walk and — for two of them — the same icosahedron. Three copies of a
//! seeded RNG is three chances for a determinism fix to land in one file and be missed in the
//! others, which is precisely the failure the golden hashes exist to make loud.

use glam::Vec3;
use vehicle_geometry::GeometryMesh;

/// Deterministic splitmix64 walk — the house randomness: process-stable, seed-keyed, no
/// dependency. The forges' goldens rest on this sequence, so a change here re-blesses the world.
pub(crate) struct Rng(pub(crate) u64);

impl Rng {
    pub(crate) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub(crate) fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub(crate) fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

/// Two meshes as one, indices rebased. Neither is validated — the callers own their topology.
pub(crate) fn merge_meshes(a: GeometryMesh, b: GeometryMesh) -> GeometryMesh {
    let mut vertices = a.vertices().to_vec();
    let offset = vertices.len() as u32;
    vertices.extend_from_slice(b.vertices());
    let mut indices = a.indices().to_vec();
    indices.extend(b.indices().iter().map(|index| index + offset));
    GeometryMesh::new(vertices, indices)
}

/// A unit icosphere: the icosahedron, optionally subdivided (0 → 20 tris, 1 → 80 tris). The
/// tree's crown lobes and the rock's weathered body are the same sphere before displacement.
pub(crate) fn icosphere(subdivisions: u32) -> (Vec<Vec3>, Vec<u32>) {
    let phi = (1.0 + 5.0_f32.sqrt()) / 2.0;
    let mut positions: Vec<Vec3> = [
        (-1.0, phi, 0.0),
        (1.0, phi, 0.0),
        (-1.0, -phi, 0.0),
        (1.0, -phi, 0.0),
        (0.0, -1.0, phi),
        (0.0, 1.0, phi),
        (0.0, -1.0, -phi),
        (0.0, 1.0, -phi),
        (phi, 0.0, -1.0),
        (phi, 0.0, 1.0),
        (-phi, 0.0, -1.0),
        (-phi, 0.0, 1.0),
    ]
    .into_iter()
    .map(|(x, y, z)| Vec3::new(x, y, z).normalize())
    .collect();
    let mut indices: Vec<u32> = vec![
        0, 11, 5, 0, 5, 1, 0, 1, 7, 0, 7, 10, 0, 10, 11, 1, 5, 9, 5, 11, 4, 11, 10, 2, 10, 7, 6, 7,
        1, 8, 3, 9, 4, 3, 4, 2, 3, 2, 6, 3, 6, 8, 3, 8, 9, 4, 9, 5, 2, 4, 11, 6, 2, 10, 8, 6, 7, 9,
        8, 1,
    ];
    for _ in 0..subdivisions {
        let mut next_indices = Vec::with_capacity(indices.len() * 4);
        let mut midpoints = std::collections::HashMap::new();
        let mut midpoint = |a: u32, b: u32, positions: &mut Vec<Vec3>| -> u32 {
            let key = (a.min(b), a.max(b));
            *midpoints.entry(key).or_insert_with(|| {
                let mid = ((positions[a as usize] + positions[b as usize]) * 0.5).normalize();
                positions.push(mid);
                (positions.len() - 1) as u32
            })
        };
        for triangle in indices.chunks_exact(3) {
            let (a, b, c) = (triangle[0], triangle[1], triangle[2]);
            let ab = midpoint(a, b, &mut positions);
            let bc = midpoint(b, c, &mut positions);
            let ca = midpoint(c, a, &mut positions);
            next_indices.extend_from_slice(&[a, ab, ca, b, bc, ab, c, ca, bc, ab, bc, ca]);
        }
        indices = next_indices;
    }
    (positions, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one sequence three forges bet their goldens on.
    #[test]
    fn the_house_randomness_is_stable_and_bounded() {
        let mut first = Rng(0x1234_5678);
        let mut again = Rng(0x1234_5678);
        for _ in 0..64 {
            assert_eq!(first.next(), again.next());
        }
        let mut rng = Rng(9);
        for _ in 0..512 {
            let unit = rng.unit();
            assert!((0.0..=1.0).contains(&unit), "unit left its range: {unit}");
            assert!((-1.0..=1.0).contains(&rng.signed()));
        }
    }

    /// Closed, outward-wound and the documented size at both rungs — the contract the rock's
    /// displaced body and the tree's crown lobes both inherit.
    #[test]
    fn the_icosphere_is_a_closed_outward_solid_at_every_rung() {
        for (subdivisions, tris) in [(0, 20), (1, 80)] {
            let (positions, indices) = icosphere(subdivisions);
            assert_eq!(indices.len() / 3, tris, "subdiv {subdivisions}");
            assert!(indices.iter().all(|index| (*index as usize) < positions.len()));
            assert!(positions.iter().all(|p| (p.length() - 1.0).abs() < 1.0e-5), "unit sphere");
            let volume: f32 = indices
                .chunks_exact(3)
                .map(|t| {
                    let (a, b, c) = (
                        positions[t[0] as usize],
                        positions[t[1] as usize],
                        positions[t[2] as usize],
                    );
                    a.dot(b.cross(c)) / 6.0
                })
                .sum();
            assert!(volume > 0.0, "subdiv {subdivisions} is wound inside out: {volume}");
        }
    }
}
