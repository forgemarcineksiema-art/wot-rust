//! Trees 2.0 (Inna Liga B2): the end of the frustum-stack tree. A species is a PARAMETER SET,
//! not a model — two levels of deterministic branching (never L-systems; overkill for a
//! battlefield read), a tapered trunk with limbs, and a crown of 2–4 FBM-displaced icosphere
//! lobes whose normals are bent away from the crown centroid: the classic painterly trick that
//! lights a canopy as one soft mass instead of a triangle salad. Trunk and canopy come back as
//! separate meshes so the consumer colors them without any material-enum churn.

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, SmoothingGroup};

use crate::WorldMaterial;

/// The authored species. Numbers live in [`TreeSpecies::params`] — one table, review-gated by
/// the goldens below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TreeSpecies {
    Oak,
    Poplar,
    Willow,
    FruitTree,
    Bush,
}

impl TreeSpecies {
    pub const ALL: [TreeSpecies; 5] = [
        TreeSpecies::Oak,
        TreeSpecies::Poplar,
        TreeSpecies::Willow,
        TreeSpecies::FruitTree,
        TreeSpecies::Bush,
    ];

    fn params(self) -> SpeciesParams {
        match self {
            // Broad, muscular: a tall trunk, heavy limbs, a wide 4-lobe crown.
            TreeSpecies::Oak => SpeciesParams {
                trunk_height: 4.6,
                trunk_radius: 0.34,
                taper: 0.55,
                limbs: 4,
                limb_length: 2.2,
                limb_pitch: 0.9,
                lobes: 4,
                lobe_radius: 2.3,
                crown_height: 5.9,
                crown_spread: 1.7,
                fbm_amplitude: 0.34,
            },
            // A column: minimal limbs, stacked narrow lobes.
            TreeSpecies::Poplar => SpeciesParams {
                trunk_height: 6.5,
                trunk_radius: 0.22,
                taper: 0.45,
                limbs: 2,
                limb_length: 0.9,
                limb_pitch: 1.25,
                lobes: 3,
                lobe_radius: 1.35,
                crown_height: 7.4,
                crown_spread: 0.45,
                fbm_amplitude: 0.22,
            },
            // Weeping: a short trunk, long drooping limbs, a low wide crown.
            TreeSpecies::Willow => SpeciesParams {
                trunk_height: 3.2,
                trunk_radius: 0.30,
                taper: 0.6,
                limbs: 5,
                limb_length: 2.6,
                limb_pitch: 0.35,
                lobes: 3,
                lobe_radius: 2.5,
                crown_height: 4.1,
                crown_spread: 1.9,
                fbm_amplitude: 0.42,
            },
            // Orchard scale: short and round.
            TreeSpecies::FruitTree => SpeciesParams {
                trunk_height: 2.2,
                trunk_radius: 0.18,
                taper: 0.55,
                limbs: 3,
                limb_length: 1.2,
                limb_pitch: 0.8,
                lobes: 2,
                lobe_radius: 1.5,
                crown_height: 3.1,
                crown_spread: 0.8,
                fbm_amplitude: 0.30,
            },
            // No trunk to speak of: the honest concealment bush stays a soft blob cluster.
            TreeSpecies::Bush => SpeciesParams {
                trunk_height: 0.5,
                trunk_radius: 0.10,
                taper: 0.7,
                limbs: 0,
                limb_length: 0.0,
                limb_pitch: 0.0,
                lobes: 3,
                lobe_radius: 1.1,
                crown_height: 1.1,
                crown_spread: 0.9,
                fbm_amplitude: 0.38,
            },
        }
    }
}

struct SpeciesParams {
    trunk_height: f32,
    trunk_radius: f32,
    /// Top radius as a fraction of the base radius.
    taper: f32,
    limbs: u32,
    limb_length: f32,
    /// Radians above horizontal the limbs reach.
    limb_pitch: f32,
    lobes: u32,
    lobe_radius: f32,
    crown_height: f32,
    /// How far lobe centers scatter from the crown axis.
    crown_spread: f32,
    fbm_amplitude: f32,
}

/// A baked tree: trunk (bark) and canopy, separate meshes so the scene builder colors each.
#[derive(Debug, Clone)]
pub struct BakedTree {
    pub species: TreeSpecies,
    pub trunk: GeometryMesh,
    pub canopy: GeometryMesh,
}

impl BakedTree {
    pub fn triangle_count(&self) -> usize {
        self.trunk.triangle_count() + self.canopy.triangle_count()
    }

    pub fn deterministic_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for mesh in [&self.trunk, &self.canopy] {
            for vertex in mesh.vertices() {
                for value in vertex.position.to_array().into_iter().chain(vertex.normal.to_array())
                {
                    super::fnv(&mut hash, u64::from(value.to_bits()));
                }
            }
            for index in mesh.indices() {
                super::fnv(&mut hash, u64::from(*index));
            }
        }
        hash
    }
}

/// LOD0/LOD1 budgets: the full close tree and the mid-distance one. LOD2 stays the existing
/// painted frustum stack in `foliage.rs` — at 300 m it reads perfectly and costs nothing.
pub const TREE_LOD0_TRIS: std::ops::RangeInclusive<usize> = 180..=1_200;
pub const TREE_LOD1_MAX_TRIS: usize = 260;

/// The review gate for the whole species table at seed 0 (goldens; bless deliberately).
pub const TREE_GOLDEN_HASHES: [(TreeSpecies, u64); 5] = [
    (TreeSpecies::Oak, 0x0e88_3970_b14c_9477),
    (TreeSpecies::Poplar, 0x5a9a_fc2b_4554_b887),
    (TreeSpecies::Willow, 0xd92e_1100_8379_9c62),
    (TreeSpecies::FruitTree, 0xbca2_9510_33bc_79f2),
    (TreeSpecies::Bush, 0x9706_2456_e825_0149),
];

/// Bake one tree. `seed` varies the individual (limb headings, lobe scatter, FBM phases) —
/// same species, same silhouette family, never the same tree twice in a shelterbelt.
pub fn bake_tree(species: TreeSpecies, seed: u64) -> BakedTree {
    bake_tree_lod(species, seed, TreeLod::Close)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeLod {
    Close,
    Mid,
}

pub fn bake_tree_lod(species: TreeSpecies, seed: u64, lod: TreeLod) -> BakedTree {
    let params = species.params();
    let mut rng = Rng(seed ^ 0x7EE5_0000 ^ species as u64);
    let (trunk_sides, lobe_subdiv) = match lod {
        TreeLod::Close => (7, 1),
        TreeLod::Mid => (5, 0),
    };

    // Trunk: a tapered tube with a slight deterministic lean.
    let lean = Vec3::new(rng.signed() * 0.10, 0.0, rng.signed() * 0.10);
    let mut trunk = tapered_tube(
        Vec3::ZERO,
        Vec3::new(lean.x * params.trunk_height, params.trunk_height, lean.z * params.trunk_height),
        params.trunk_radius,
        params.trunk_radius * params.taper,
        trunk_sides,
    );
    // Limbs: level-one branches reaching from the upper trunk toward the crown.
    if lod == TreeLod::Close {
        for limb in 0..params.limbs {
            let heading = rng.unit() * std::f32::consts::TAU
                + limb as f32 / params.limbs.max(1) as f32 * std::f32::consts::TAU;
            let pitch = params.limb_pitch + rng.signed() * 0.15;
            let start = Vec3::new(0.0, params.trunk_height * (0.55 + rng.unit() * 0.25), 0.0);
            let direction =
                Vec3::new(heading.cos() * pitch.cos(), pitch.sin(), heading.sin() * pitch.cos());
            let end = start + direction * (params.limb_length * (0.8 + rng.unit() * 0.4));
            let limb_mesh =
                tapered_tube(start, end, params.trunk_radius * 0.42, params.trunk_radius * 0.16, 5);
            trunk = merge_meshes(trunk, limb_mesh);
        }
    }

    // Crown: FBM-displaced icosphere lobes; normals bent from the CROWN centroid afterwards.
    let mut canopy_vertices: Vec<GeometryVertex> = Vec::new();
    let mut canopy_indices: Vec<u32> = Vec::new();
    let mut lobe_centers = Vec::new();
    for lobe in 0..params.lobes {
        let angle = lobe as f32 / params.lobes as f32 * std::f32::consts::TAU + rng.unit();
        let scatter = if params.lobes == 1 { 0.0 } else { params.crown_spread };
        let center = Vec3::new(
            angle.cos() * scatter * (0.6 + rng.unit() * 0.4),
            params.crown_height + rng.signed() * 0.35 * params.lobe_radius,
            angle.sin() * scatter * (0.6 + rng.unit() * 0.4),
        );
        lobe_centers.push(center);
        let radius = params.lobe_radius * (0.75 + rng.unit() * 0.4);
        let phase = rng.next() as u32 as f32 * 1.0e-6;
        let base = canopy_vertices.len() as u32;
        let (positions, indices) = icosphere(lobe_subdiv);
        for unit in &positions {
            let wobble = 1.0
                + params.fbm_amplitude
                    * (0.6 * (unit.x * 3.1 + unit.y * 5.3 + phase).sin()
                        + 0.4 * (unit.z * 7.7 - unit.y * 2.9 + phase * 1.7).sin());
            canopy_vertices.push(GeometryVertex::new(
                center + *unit * radius * wobble,
                *unit,
                WorldMaterial::Canopy.carrier(),
                SmoothingGroup(1),
            ));
        }
        canopy_indices.extend(indices.iter().map(|index| index + base));
    }
    // The painterly trick: light the canopy as ONE mass — every normal points away from the
    // crown centroid, so lobes shade like a single soft volume, not intersecting balls.
    let centroid = lobe_centers.iter().copied().sum::<Vec3>() / lobe_centers.len().max(1) as f32;
    for vertex in &mut canopy_vertices {
        vertex.normal = (vertex.position - centroid).normalize_or_zero();
    }

    BakedTree { species, trunk, canopy: GeometryMesh::new(canopy_vertices, canopy_indices) }
}

/// Deterministic splitmix64 walk (the house randomness: process-stable, seed-keyed).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

fn merge_meshes(a: GeometryMesh, b: GeometryMesh) -> GeometryMesh {
    let mut vertices = a.vertices().to_vec();
    let offset = vertices.len() as u32;
    vertices.extend_from_slice(b.vertices());
    let mut indices = a.indices().to_vec();
    indices.extend(b.indices().iter().map(|index| index + offset));
    GeometryMesh::new(vertices, indices)
}

/// A tapered open tube from `a` to `b` (no caps: the base sits in the ground, the tip inside
/// the canopy). Flat side facets — bark reads hard-edged at battle range.
fn tapered_tube(a: Vec3, b: Vec3, radius_a: f32, radius_b: f32, sides: u32) -> GeometryMesh {
    let axis = (b - a).normalize_or_zero();
    let reference = if axis.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let u = axis.cross(reference).normalize_or_zero();
    let v = axis.cross(u);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for side in 0..sides {
        let a0 = side as f32 / sides as f32 * std::f32::consts::TAU;
        let a1 = (side + 1) as f32 / sides as f32 * std::f32::consts::TAU;
        let mid = (a0 + a1) * 0.5;
        let normal = u * mid.cos() + v * mid.sin();
        let ring = |angle: f32, radius: f32, origin: Vec3| {
            origin + (u * angle.cos() + v * angle.sin()) * radius
        };
        let base = vertices.len() as u32;
        for corner in [
            ring(a0, radius_a, a),
            ring(a1, radius_a, a),
            ring(a1, radius_b, b),
            ring(a0, radius_b, b),
        ] {
            vertices.push(GeometryVertex::new(
                corner,
                normal,
                WorldMaterial::Bark.carrier(),
                SmoothingGroup::hard_edges(),
            ));
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    GeometryMesh::new(vertices, indices)
}

/// A unit icosphere: the icosahedron, optionally subdivided once (0 → 20 tris, 1 → 80 tris).
fn icosphere(subdivisions: u32) -> (Vec<Vec3>, Vec<u32>) {
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

    #[test]
    fn every_species_bakes_deterministic_within_budget_and_on_its_golden() {
        for (species, golden) in TREE_GOLDEN_HASHES {
            let first = bake_tree(species, 0);
            let second = bake_tree(species, 0);
            assert_eq!(first.deterministic_hash(), second.deterministic_hash(), "{species:?}");
            assert!(
                TREE_LOD0_TRIS.contains(&first.triangle_count()),
                "{species:?} LOD0 budget: {} tris",
                first.triangle_count()
            );
            let mid = bake_tree_lod(species, 0, TreeLod::Mid);
            assert!(
                mid.triangle_count() <= TREE_LOD1_MAX_TRIS,
                "{species:?} LOD1 budget: {} tris",
                mid.triangle_count()
            );
            assert_eq!(
                first.deterministic_hash(),
                golden,
                "{species:?}: the silhouette changed — bless deliberately (0x{:016x})",
                first.deterministic_hash()
            );
        }
    }

    #[test]
    fn individuals_differ_but_the_species_family_holds() {
        let a = bake_tree(TreeSpecies::Oak, 1);
        let b = bake_tree(TreeSpecies::Oak, 2);
        assert_ne!(a.deterministic_hash(), b.deterministic_hash(), "no two oaks alike");
        // Family: both stay inside the LOD0 budget and within ~30% height of each other.
        let height =
            |tree: &BakedTree| tree.canopy.bounds().map(|bounds| bounds.max.y).unwrap_or_default();
        let (ha, hb) = (height(&a), height(&b));
        assert!((ha - hb).abs() / ha.max(hb) < 0.3, "oaks stay oak-sized: {ha} vs {hb}");
    }

    #[test]
    fn canopy_normals_point_away_from_the_crown_centroid() {
        let tree = bake_tree(TreeSpecies::Oak, 0);
        let centroid =
            tree.canopy.vertices().iter().map(|vertex| vertex.position).sum::<glam::Vec3>()
                / tree.canopy.vertex_count().max(1) as f32;
        let aligned = tree
            .canopy
            .vertices()
            .iter()
            .filter(|vertex| {
                (vertex.position - centroid).normalize_or_zero().dot(vertex.normal) > 0.5
            })
            .count();
        assert!(
            aligned * 10 >= tree.canopy.vertex_count() * 9,
            "the painterly one-mass normal trick must hold: {aligned}/{}",
            tree.canopy.vertex_count()
        );
    }
}
