//! Rocks 1.0 (Świat 2.0 PR 8): the end of the rock-as-a-box.
//!
//! A rock was one rotated cuboid, 12 triangles, the same grey on all four maps — the last of the
//! three bare cuboids in art-direction defect D6. What replaces it is built the way the stone
//! itself is made, because that is the only construction the eye reads as stone:
//!
//! - a [`RockForm::Shard`] is a FRACTURE — an irregular n-gon in the bedding plane pulled to a
//!   crest above and a shallower keel below, so every face is flat and every edge is an arête;
//! - a [`RockForm::Erratic`] is WEATHERING — a displaced ellipsoid whose corners are rounded by
//!   the two relief octaves the art direction asks of every surface, cut flat underneath and set
//!   into the soil rather than laid on top of it, with the frost spall it sheds at its foot.
//!
//! Renderer-free and deterministic, like `world_forge::tree` next door: a seed picks the
//! individual, the golden hashes below are the review gate, and the consumer colours the mesh
//! (the map's own rock palette decides what stone this map is made of — never a constant here).

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, SmoothingGroup};

use crate::WorldMaterial;
use crate::shape::{Rng, fracture_solid, icosphere, merge_meshes};

/// What broke this stone. Append-only in spirit for the same reason the species table is: the
/// golden hashes below name each form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RockForm {
    /// A frost-shattered fragment: one dominant fracture face, sharp arêtes, flat everywhere.
    /// The atom of every apron and talus — cheap enough to be used by the dozen.
    Shard,
    /// A glacial erratic / field stone: weathering has rounded it, so unlike the shard it is
    /// smooth-shaded — geology, not a styling choice. It EMERGES from the ground on a flat cut
    /// base and sheds a little frost spall at its foot.
    Erratic,
}

impl RockForm {
    pub const ALL: [RockForm; 2] = [RockForm::Shard, RockForm::Erratic];
}

/// One baked stone, standing on the origin: `y = 0` is the ground line it is set into, so a
/// consumer translates to the terrain sample and is done.
#[derive(Debug, Clone)]
pub struct BakedRock {
    pub form: RockForm,
    pub body: GeometryMesh,
}

impl BakedRock {
    pub fn triangle_count(&self) -> usize {
        self.body.triangle_count()
    }

    /// How far the stone stands above its ground line, metres — the number the honesty locks
    /// measure against the fleet's belly line.
    pub fn height_m(&self) -> f32 {
        self.body.bounds().map(|bounds| bounds.max.y).unwrap_or(0.0)
    }

    /// How far the stone reaches below its ground line, metres. A rock that does not reach below
    /// is a rock resting ON the world instead of IN it.
    pub fn sink_m(&self) -> f32 {
        self.body.bounds().map(|bounds| -bounds.min.y).unwrap_or(0.0)
    }

    pub fn deterministic_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for vertex in self.body.vertices() {
            for value in vertex.position.to_array().into_iter().chain(vertex.normal.to_array()) {
                super::fnv(&mut hash, u64::from(value.to_bits()));
            }
        }
        for index in self.body.indices() {
            super::fnv(&mut hash, u64::from(*index));
        }
        hash
    }
}

/// Per-form triangle ceilings. These are RATCHETS against silent growth, not a frame budget —
/// the two are different numbers and confusing them freezes today's quality (see the one-look
/// policy: budgets rise per item, with a measurement on the min spec). Each sits a few triangles
/// above what the construction below actually costs.
/// Measured today: a 6-sided shard is 12 triangles, an erratic 100 (80 for the displaced body,
/// 10 for each of its two frost chips).
pub const ROCK_MAX_TRIS: [(RockForm, usize); 2] = [(RockForm::Shard, 16), (RockForm::Erratic, 108)];

/// The review gate for the whole form table at seed 0 (goldens; bless deliberately).
pub const ROCK_GOLDEN_HASHES: [(RockForm, u64); 2] = [
    // Blessed 2026-08-07 with the rock forge (Świat 2.0 PR 8) — the first bake of both forms.
    (RockForm::Shard, 0x4faf_8799_b4df_76f2),
    (RockForm::Erratic, 0xa5c7_a3d6_c594_dc99),
];

/// The two relief octaves, as fractions of the radius. Art-direction rule 5 asks every surface
/// for a macro variation and a micro grain and forbids both flatness and shimmer. On an object
/// smaller than the rule's 2–5 m macro band what carries over is the RATIO, not the metres: one
/// octave shapes the silhouette, a second roughens the faces under it.
const MACRO_RELIEF: f32 = 0.21;
const MICRO_RELIEF: f32 = 0.075;

/// The erratic's nominal half extents, metres. Flattened in Y because a stone that has been
/// rolled and dropped lies on its broad face; a sphere reads as a ball, never as a boulder.
const ERRATIC_RADIUS_M: Vec3 = Vec3::new(0.90, 0.52, 0.70);

/// How much of the erratic's lower half the flat cut takes away, as a fraction of its Y radius.
const ERRATIC_BASE_CUT: f32 = 0.34;

/// How far the cut base sits BELOW the ground line. Without it the stone stands on the turf with
/// a visible seam; with it the ground line is an irregular contour, which is what a half-buried
/// field stone actually shows.
const ERRATIC_SINK_M: f32 = 0.09;

/// Bake one stone. `seed` picks the individual — the same form never repeats within a scatter,
/// yet every bake of the same seed is identical.
pub fn bake_rock(form: RockForm, seed: u64) -> BakedRock {
    let mut rng = Rng(seed ^ 0x5709_E000 ^ form as u64);
    let body = match form {
        RockForm::Shard => {
            fracture_solid(&mut rng, Vec3::ZERO, 0.34, 0.30, 0.12, 6, WorldMaterial::PlinthStone)
        }
        RockForm::Erratic => erratic(&mut rng),
    };
    BakedRock { form, body }
}

/// A weathered field stone: an ellipsoid displaced by the two relief octaves, cut flat underneath
/// and lowered into the soil, with a little frost spall at its foot. Smooth-shaded from the
/// displaced surface itself, so the relief actually catches the light instead of being a
/// silhouette trick.
fn erratic(rng: &mut Rng) -> GeometryMesh {
    // Per-AXIS variance, not one uniform scale: a scatter of uniformly scaled stones is still a
    // scatter of clones, only in different sizes.
    let radius = Vec3::new(
        ERRATIC_RADIUS_M.x * (0.80 + rng.unit() * 0.45),
        ERRATIC_RADIUS_M.y * (0.78 + rng.unit() * 0.50),
        ERRATIC_RADIUS_M.z * (0.80 + rng.unit() * 0.45),
    );
    let phase = rng.next() as u32 as f32 * 1.0e-6;

    let (unit_positions, indices) = icosphere(1);
    let mut positions: Vec<Vec3> = unit_positions
        .iter()
        .map(|unit| {
            let macro_wave = (unit.x * 2.7 + unit.y * 1.9 + phase).sin()
                * (unit.z * 2.3 - unit.x * 1.7 + phase * 1.3).cos();
            let micro_wave =
                (unit.x * 8.3 + unit.z * 6.7 + phase * 2.1).sin() * (unit.y * 7.1 + phase).cos();
            *unit * (1.0 + MACRO_RELIEF * macro_wave + MICRO_RELIEF * micro_wave) * radius
        })
        .collect();

    // The flat cut, then the sink: everything under the cut collapses onto it (topology
    // untouched, no wasted buried volume), and the whole stone drops so that plane sits below
    // the ground line.
    let cut_y = -radius.y * (1.0 - ERRATIC_BASE_CUT);
    let lift = -cut_y - ERRATIC_SINK_M;
    for position in &mut positions {
        position.y = position.y.max(cut_y) + lift;
    }

    let normals = smooth_normals(&positions, &indices);
    let vertices: Vec<GeometryVertex> = positions
        .iter()
        .zip(normals)
        .map(|(position, normal)| {
            GeometryVertex::new(
                *position,
                normal,
                WorldMaterial::PlinthStone.carrier(),
                SmoothingGroup(1),
            )
        })
        .collect();

    // Frost spall: the chips a weathered stone sheds, resting against its foot. Two of them —
    // enough to break the seam where stone meets turf, cheap enough not to matter.
    let mut body = GeometryMesh::new(vertices, indices);
    for chip in 0..2 {
        let angle = rng.unit() * std::f32::consts::TAU + chip as f32 * 2.4;
        let reach = 0.80 + rng.unit() * 0.35;
        let center = Vec3::new(
            angle.cos() * radius.x * reach,
            -ERRATIC_SINK_M * 0.5,
            angle.sin() * radius.z * reach,
        );
        let size = radius.y * (0.24 + rng.unit() * 0.20);
        body = merge_meshes(
            body,
            fracture_solid(
                rng,
                center,
                size,
                size * 0.75,
                size * 0.4,
                5,
                WorldMaterial::PlinthStone,
            ),
        );
    }
    body
}

/// Area-weighted vertex normals over the DISPLACED surface. Taking the undisplaced sphere normal
/// instead would light the stone as the ball it no longer is.
fn smooth_normals(positions: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for triangle in indices.chunks_exact(3) {
        let (a, b, c) = (
            positions[triangle[0] as usize],
            positions[triangle[1] as usize],
            positions[triangle[2] as usize],
        );
        let face = (b - a).cross(c - a);
        for index in triangle {
            normals[*index as usize] += face;
        }
    }
    normals.into_iter().map(|normal| normal.normalize_or_zero()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn max_tris(form: RockForm) -> usize {
        ROCK_MAX_TRIS
            .iter()
            .find(|(candidate, _)| *candidate == form)
            .map(|(_, budget)| *budget)
            .expect("every form carries a triangle ceiling")
    }

    #[test]
    fn every_form_carries_a_budget_and_a_golden() {
        for form in RockForm::ALL {
            assert!(ROCK_MAX_TRIS.iter().any(|(candidate, _)| *candidate == form), "{form:?}");
            assert!(ROCK_GOLDEN_HASHES.iter().any(|(candidate, _)| *candidate == form), "{form:?}");
        }
    }

    #[test]
    fn every_rock_form_bakes_deterministic_within_budget_and_on_its_golden() {
        for (form, golden) in ROCK_GOLDEN_HASHES {
            let first = bake_rock(form, 0);
            let again = bake_rock(form, 0);
            assert_eq!(
                first.deterministic_hash(),
                again.deterministic_hash(),
                "{form:?} bakes deterministically"
            );
            assert_eq!(
                first.deterministic_hash(),
                golden,
                "{form:?} moved off its golden: {:#018x}",
                first.deterministic_hash()
            );
            let tris = first.triangle_count();
            assert!(tris > 0, "{form:?} must draw something");
            assert!(tris <= max_tris(form), "{form:?} broke its ceiling: {tris}");
            assert!(
                first
                    .body
                    .indices()
                    .iter()
                    .all(|index| (*index as usize) < first.body.vertex_count()),
                "{form:?} indexes inside its own vertex buffer"
            );
            assert!(first.body.indices().len().is_multiple_of(3));
        }
    }

    /// A closed solid whose faces point outward. This is the test that decides the fan winding in
    /// `shard` — get it backwards and every stone lights inside out.
    ///
    /// Measured as SIGNED VOLUME, deliberately: a centroid test is wrong here, because an erratic
    /// carries its frost chips as separate components and a chip's inner faces honestly point
    /// back toward the boulder's centre. Divergence theorem does not care where the parts are —
    /// every closed, consistently outward-wound component adds positive volume.
    #[test]
    fn every_rock_triangle_winds_outward() {
        for form in RockForm::ALL {
            for seed in [0, 7, 31] {
                let rock = bake_rock(form, seed);
                let vertices = rock.body.vertices();
                let volume: f32 = rock
                    .body
                    .indices()
                    .chunks_exact(3)
                    .map(|triangle| {
                        let (a, b, c) = (
                            vertices[triangle[0] as usize].position,
                            vertices[triangle[1] as usize].position,
                            vertices[triangle[2] as usize].position,
                        );
                        a.dot(b.cross(c)) / 6.0
                    })
                    .sum();
                assert!(volume > 0.0, "{form:?} seed {seed} is wound inside out: {volume} m³");
            }
        }
    }

    /// The seam killer: a stone reaches BELOW the ground line it stands on. The box it replaces
    /// sank 6.5 cm at full scale and read as laid on the turf.
    #[test]
    fn a_rock_sits_in_the_ground_it_stands_on() {
        for seed in 0..24 {
            let rock = bake_rock(RockForm::Erratic, seed);
            assert!(
                rock.sink_m() >= ERRATIC_SINK_M * 0.9,
                "seed {seed} floats: sink {} m",
                rock.sink_m()
            );
            assert!(rock.height_m() > 0.2, "seed {seed} is not a stone: {} m", rock.height_m());
        }
    }

    /// Individuals differ, the form family holds — the same promise the species table makes.
    #[test]
    fn individuals_differ_but_the_form_family_holds() {
        let heights: Vec<f32> =
            (0..16).map(|seed| bake_rock(RockForm::Erratic, seed).height_m()).collect();
        let first = heights[0];
        assert!(
            heights.iter().any(|height| (height - first).abs() > 0.05),
            "every erratic came out the same stone: {heights:?}"
        );
        for height in &heights {
            assert!(
                (0.35..=1.45).contains(height),
                "an erratic left the family silhouette: {height} m"
            );
        }
        let hashes: std::collections::HashSet<u64> =
            (0..16).map(|seed| bake_rock(RockForm::Erratic, seed).deterministic_hash()).collect();
        assert_eq!(hashes.len(), 16, "two seeds baked the same stone");
    }
}
