//! The Eastern-European building generator (Inna Liga B3) — the content track's flagship. A
//! building is a STYLE plus a seed, baked in two authoritative FORMS that mirror the battle's
//! v21 cover phases: Intact and Rubble (Gone draws nothing, so it needs no mesh). The honesty
//! rule is structural: every vertex of every form stays inside the collision footprint the
//! caller derives from the SAME numbers — what blocks the shell is what blocks the eye. The
//! rubble ceiling fraction comes IN from the caller (the terrain crate owns
//! `rubble_height_frac`), so the single source of truth never forks.

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

/// The authored styles. Kamienna's street fronts, the farmyards and the mill all compose from
/// these three; the church tower joins with map dressing (B4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuildingStyle {
    /// The village izba: one storey, a steep gable, door on the long side.
    Cottage,
    /// The working barn: long, tall doors, shallower roof, almost no windows.
    Barn,
    /// The small-town two-storey house: taller walls, regular window grid.
    Townhouse,
}

impl BuildingStyle {
    pub const ALL: [BuildingStyle; 3] =
        [BuildingStyle::Cottage, BuildingStyle::Barn, BuildingStyle::Townhouse];

    fn params(self) -> StyleParams {
        match self {
            BuildingStyle::Cottage => StyleParams {
                half_width: 3.4,
                half_depth: 4.6,
                eaves_height: 2.7,
                ridge_height: 4.6,
                plinth_height: 0.45,
                windows_per_side: 2,
                window_size: (0.55, 0.7),
                door_size: (0.6, 1.05),
            },
            BuildingStyle::Barn => StyleParams {
                half_width: 4.4,
                half_depth: 7.0,
                eaves_height: 3.4,
                ridge_height: 5.4,
                plinth_height: 0.35,
                windows_per_side: 1,
                window_size: (0.45, 0.5),
                door_size: (1.4, 1.6),
            },
            BuildingStyle::Townhouse => StyleParams {
                half_width: 3.8,
                half_depth: 5.2,
                eaves_height: 5.6,
                ridge_height: 7.6,
                plinth_height: 0.55,
                windows_per_side: 3,
                window_size: (0.6, 0.85),
                door_size: (0.7, 1.15),
            },
        }
    }
}

struct StyleParams {
    half_width: f32,
    half_depth: f32,
    eaves_height: f32,
    ridge_height: f32,
    plinth_height: f32,
    windows_per_side: u32,
    window_size: (f32, f32),
    door_size: (f32, f32),
}

/// Which authoritative form to bake — mirrors the battle's cover phases (v21). `Gone` draws
/// nothing and therefore has no variant here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StructureForm {
    Intact,
    /// The collapsed mound. The fraction is the CALLER's truth (`rubble_height_frac` in the
    /// terrain crate) — the heap never rises above `full_height * frac`.
    Rubble {
        height_frac: f32,
    },
}

/// A baked building: walls (plaster/stone/joinery lane) and roof, separate for the consumer's
/// palette, plus the collision footprint half-extents derived from the SAME style numbers.
#[derive(Debug, Clone)]
pub struct BakedBuilding {
    pub style: BuildingStyle,
    pub walls: GeometryMesh,
    pub roof: GeometryMesh,
    /// Collision half-extents (x, y, z): the AABB every vertex above provably stays inside.
    pub footprint_half: Vec3,
}

impl BakedBuilding {
    pub fn triangle_count(&self) -> usize {
        self.walls.triangle_count() + self.roof.triangle_count()
    }

    pub fn deterministic_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for mesh in [&self.walls, &self.roof] {
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

/// Style goldens at seed 0, Intact form (rubble is derived; its own lock is the honesty test).
pub const BUILDING_GOLDEN_HASHES: [(BuildingStyle, u64); 3] = [
    (BuildingStyle::Cottage, 0x8156_06d3_7c04_2428),
    (BuildingStyle::Barn, 0x2a90_7451_e2cd_c380),
    (BuildingStyle::Townhouse, 0xc72d_90df_09d2_f880),
];

pub fn bake_building(style: BuildingStyle, seed: u64, form: StructureForm) -> BakedBuilding {
    let params = style.params();
    let footprint_half = Vec3::new(params.half_width, params.ridge_height * 0.5, params.half_depth);
    match form {
        StructureForm::Intact => bake_intact(style, seed, &params, footprint_half),
        StructureForm::Rubble { height_frac } => {
            bake_rubble(style, seed, &params, footprint_half, height_frac.clamp(0.05, 0.9))
        }
    }
}

fn bake_intact(
    style: BuildingStyle,
    seed: u64,
    params: &StyleParams,
    footprint_half: Vec3,
) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0xB11D_0000 ^ style as u64);
    let recess = 0.08;
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    // Plinth (full footprint) then the recessed wall body up to the eaves.
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height * 0.5, 0.0),
        Vec3::new(params.half_width, params.plinth_height * 0.5, params.half_depth),
        MaterialRole::CastArmor,
    );
    let body_half_y = (params.eaves_height - params.plinth_height) * 0.5;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height + body_half_y, 0.0),
        Vec3::new(params.half_width - recess, body_half_y, params.half_depth - recess),
        MaterialRole::RolledArmor,
    );
    // Joinery sits PROUD of the recessed plaster yet inside the footprint: the door on one long
    // side, the window rhythm on both.
    let sill = params.plinth_height + 0.55;
    let (window_w, window_h) = params.window_size;
    for side in [-1.0_f32, 1.0] {
        for slot in 0..params.windows_per_side {
            let along = (slot as f32 + 0.5) / params.windows_per_side as f32 * 2.0 - 1.0;
            let jitter = rng.signed() * 0.08;
            push_box(
                &mut walls,
                &mut wall_indices,
                Vec3::new(
                    side * (params.half_width - recess * 0.5),
                    sill + window_h * 0.5,
                    (along + jitter) * (params.half_depth * 0.62),
                ),
                Vec3::new(recess * 0.45, window_h * 0.5, window_w * 0.5),
                MaterialRole::InteriorMachinery,
            );
        }
    }
    let (door_w, door_h) = params.door_size;
    let door_along = rng.signed() * params.half_depth * 0.4;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(
            params.half_width - recess * 0.5,
            params.plinth_height + door_h * 0.5,
            door_along,
        ),
        Vec3::new(recess * 0.5, door_h * 0.5, door_w * 0.5),
        MaterialRole::InteriorPrimer,
    );

    // The gable roof: two pitched planes and two gable triangles, ridge along Z.
    let mut roof = Vec::new();
    let mut roof_indices = Vec::new();
    push_gable(
        &mut roof,
        &mut roof_indices,
        params.half_width,
        params.half_depth,
        params.eaves_height,
        params.ridge_height,
    );

    BakedBuilding {
        style,
        walls: GeometryMesh::new(walls, wall_indices),
        roof: GeometryMesh::new(roof, roof_indices),
        footprint_half,
    }
}

/// The collapsed mound: a deterministic jumble of slabs and roof shards, none rising above the
/// caller's rubble ceiling — a hull still stops on it, a turret-height shot passes, exactly as
/// the sim promises.
fn bake_rubble(
    style: BuildingStyle,
    seed: u64,
    params: &StyleParams,
    footprint_half: Vec3,
    height_frac: f32,
) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0x0BB1_E000 ^ style as u64);
    let ceiling = params.ridge_height * height_frac;
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    let slabs = 7 + (rng.next() % 4) as u32;
    for _ in 0..slabs {
        let half = Vec3::new(
            0.5 + rng.unit() * params.half_width * 0.45,
            (0.18 + rng.unit() * 0.5 * ceiling).min(ceiling * 0.5),
            0.5 + rng.unit() * params.half_depth * 0.45,
        );
        let center = Vec3::new(
            rng.signed() * (params.half_width - half.x).max(0.0),
            (half.y + rng.unit() * (ceiling - 2.0 * half.y).max(0.0)).min(ceiling - half.y),
            rng.signed() * (params.half_depth - half.z).max(0.0),
        );
        push_box(&mut walls, &mut wall_indices, center, half, MaterialRole::RolledArmor);
    }
    // A few fallen roof shards keep the material story readable in the heap.
    let mut roof = Vec::new();
    let mut roof_indices = Vec::new();
    for _ in 0..3 {
        let half = Vec3::new(0.4 + rng.unit() * 0.9, 0.05, 0.5 + rng.unit() * 1.2);
        let center = Vec3::new(
            rng.signed() * (params.half_width - half.x).max(0.0),
            (ceiling * (0.4 + rng.unit() * 0.5)).clamp(half.y, ceiling - half.y),
            rng.signed() * (params.half_depth - half.z).max(0.0),
        );
        push_box(&mut roof, &mut roof_indices, center, half, MaterialRole::CastArmor);
    }
    BakedBuilding {
        style,
        walls: GeometryMesh::new(walls, wall_indices),
        roof: GeometryMesh::new(roof, roof_indices),
        footprint_half,
    }
}

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

fn push_box(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
    material: MaterialRole,
) {
    let mesh = super::world_box_mesh(center, half, material);
    let offset = vertices.len() as u32;
    vertices.extend_from_slice(mesh.vertices());
    indices.extend(mesh.indices().iter().map(|index| index + offset));
}

fn push_gable(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    half_width: f32,
    half_depth: f32,
    eaves: f32,
    ridge: f32,
) {
    let quad = |vertices: &mut Vec<GeometryVertex>,
                indices: &mut Vec<u32>,
                corners: [Vec3; 4],
                normal: Vec3| {
        let base = vertices.len() as u32;
        for corner in corners {
            vertices.push(GeometryVertex::new(
                corner,
                normal,
                MaterialRole::CastArmor,
                SmoothingGroup::hard_edges(),
            ));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    let ridge_a = Vec3::new(0.0, ridge, -half_depth);
    let ridge_b = Vec3::new(0.0, ridge, half_depth);
    for side in [-1.0_f32, 1.0] {
        let eave_a = Vec3::new(side * half_width, eaves, -half_depth);
        let eave_b = Vec3::new(side * half_width, eaves, half_depth);
        let up = Vec3::new(-side * (ridge - eaves), half_width, 0.0).normalize();
        let normal = Vec3::new(up.y * side, up.x.abs(), 0.0).normalize();
        if side < 0.0 {
            quad(vertices, indices, [eave_a, eave_b, ridge_b, ridge_a], normal);
        } else {
            quad(vertices, indices, [eave_b, eave_a, ridge_a, ridge_b], normal);
        }
    }
    // Gable triangles close both ends.
    for (z, flip) in [(-half_depth, false), (half_depth, true)] {
        let base = vertices.len() as u32;
        let normal = Vec3::new(0.0, 0.0, if flip { 1.0 } else { -1.0 });
        let corners = [
            Vec3::new(-half_width, eaves, z),
            Vec3::new(half_width, eaves, z),
            Vec3::new(0.0, ridge, z),
        ];
        for corner in corners {
            vertices.push(GeometryVertex::new(
                corner,
                normal,
                MaterialRole::RolledArmor,
                SmoothingGroup::hard_edges(),
            ));
        }
        if flip {
            indices.extend_from_slice(&[base, base + 1, base + 2]);
        } else {
            indices.extend_from_slice(&[base, base + 2, base + 1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE honesty rule, structurally: every vertex of every form stays inside the collision
    /// footprint — what blocks the shell is what blocks the eye. And rubble never rises above
    /// the caller's ceiling, exactly as the sim promises a turret-height shot a clear pass.
    #[test]
    fn every_form_stays_inside_the_footprint_and_rubble_under_its_ceiling() {
        for style in BuildingStyle::ALL {
            for (form, ceiling_frac) in [
                (StructureForm::Intact, 1.0_f32),
                (StructureForm::Rubble { height_frac: 0.28 }, 0.28),
            ] {
                let building = bake_building(style, 7, form);
                let half = building.footprint_half;
                let full_height = half.y * 2.0;
                for mesh in [&building.walls, &building.roof] {
                    for vertex in mesh.vertices() {
                        let p = vertex.position;
                        assert!(
                            p.x.abs() <= half.x + 1.0e-4 && p.z.abs() <= half.z + 1.0e-4,
                            "{style:?} {form:?}: vertex outside the footprint at {p:?}"
                        );
                        assert!(
                            p.y >= -1.0e-4 && p.y <= full_height * ceiling_frac + 1.0e-4,
                            "{style:?} {form:?}: vertex above the ceiling at {p:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn styles_bake_deterministic_on_their_goldens_and_within_budget() {
        for (style, golden) in BUILDING_GOLDEN_HASHES {
            let first = bake_building(style, 0, StructureForm::Intact);
            let second = bake_building(style, 0, StructureForm::Intact);
            assert_eq!(first.deterministic_hash(), second.deterministic_hash());
            assert!(
                (30..=400).contains(&first.triangle_count()),
                "{style:?} budget: {} tris",
                first.triangle_count()
            );
            assert_eq!(
                first.deterministic_hash(),
                golden,
                "{style:?}: the look changed — bless deliberately (0x{:016x})",
                first.deterministic_hash()
            );
        }
    }

    #[test]
    fn individuals_vary_by_seed_but_rubble_is_deterministic_too() {
        let a = bake_building(BuildingStyle::Cottage, 1, StructureForm::Intact);
        let b = bake_building(BuildingStyle::Cottage, 2, StructureForm::Intact);
        assert_ne!(a.deterministic_hash(), b.deterministic_hash(), "no two cottages alike");
        let r1 =
            bake_building(BuildingStyle::Cottage, 5, StructureForm::Rubble { height_frac: 0.3 });
        let r2 =
            bake_building(BuildingStyle::Cottage, 5, StructureForm::Rubble { height_frac: 0.3 });
        assert_eq!(r1.deterministic_hash(), r2.deterministic_hash(), "one ruin per seed");
    }
}
