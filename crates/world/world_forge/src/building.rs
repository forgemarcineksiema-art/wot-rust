//! The Eastern-European building generator (Inna Liga B3) — the content track's flagship. A
//! building is a STYLE plus a seed, baked in two authoritative FORMS that mirror the battle's
//! v21 cover phases: Intact and Rubble (Gone draws nothing, so it needs no mesh). The honesty
//! rule is structural: every vertex of every form stays inside the collision footprint the
//! caller derives from the SAME numbers — what blocks the shell is what blocks the eye. The
//! rubble ceiling fraction comes IN from the caller (the terrain crate owns
//! `rubble_height_frac`), so the single source of truth never forks.

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, SmoothingGroup};

use crate::WorldMaterial;

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
    /// The Kamienna church (B4 cz.2): a steep-roofed nave with a west tower and pyramid
    /// spire - the tallest silhouette in town, readable across the river.
    Church,
    /// The windmill (B4 cz.2): an eight-sided tapered body under a conical cap. No sails -
    /// the honesty rule keeps every vertex inside the collision footprint, and the footprint
    /// is the tower.
    Windmill,
    /// The city tenement (urban-map program PR-08): a three-storey masonry block — tall
    /// plinth, string courses marking the floors, a regular window grid on both street
    /// fronts, a paired entrance, and a shallow-pitched roof. The urban core's brick.
    Tenement,
    /// The factory hall (urban-map program PR-09): a long tall brick hall — sparse high
    /// windows over a working wall, a big gable-end doorway under a stone lintel, and a
    /// glazed clerestory band riding the ridge under its flat industrial cap.
    FactoryHall,
}

impl BuildingStyle {
    pub const ALL: [BuildingStyle; 7] = [
        BuildingStyle::Cottage,
        BuildingStyle::Barn,
        BuildingStyle::Townhouse,
        BuildingStyle::Church,
        BuildingStyle::Windmill,
        BuildingStyle::Tenement,
        BuildingStyle::FactoryHall,
    ];

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
            // ridge_height is the SPIRE TOP: the footprint must contain the whole silhouette.
            BuildingStyle::Church => StyleParams {
                half_width: 3.6,
                half_depth: 6.4,
                eaves_height: 4.4,
                ridge_height: 11.0,
                plinth_height: 0.6,
                windows_per_side: 3,
                window_size: (0.5, 1.3),
                door_size: (0.9, 1.7),
            },
            // ridge_height is the cap peak; the body tapers inside half_width.
            BuildingStyle::Windmill => StyleParams {
                half_width: 3.0,
                half_depth: 3.0,
                eaves_height: 6.2,
                ridge_height: 8.6,
                plinth_height: 0.5,
                windows_per_side: 1,
                window_size: (0.45, 0.6),
                door_size: (0.8, 1.5),
            },
            // Three storeys of masonry under a shallow roof: eaves ~10.4 m puts each floor
            // near the civic 3.2 m, and the low ridge keeps the skyline a wall, not a barn.
            BuildingStyle::Tenement => StyleParams {
                half_width: 4.6,
                half_depth: 6.0,
                eaves_height: 10.4,
                ridge_height: 12.0,
                plinth_height: 0.7,
                windows_per_side: 4,
                window_size: (0.65, 1.0),
                door_size: (0.85, 1.3),
            },
            // A working span, not a house: one tall volume, high sills so machines line the
            // walls, and a door a loaded wagon clears. ridge_height caps the clerestory.
            BuildingStyle::FactoryHall => StyleParams {
                half_width: 6.5,
                half_depth: 11.0,
                eaves_height: 7.2,
                ridge_height: 9.0,
                plinth_height: 0.6,
                windows_per_side: 5,
                window_size: (0.8, 1.4),
                door_size: (2.6, 3.4),
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
pub const BUILDING_GOLDEN_HASHES: [(BuildingStyle, u64); 7] = [
    (BuildingStyle::Cottage, 0x8156_06d3_7c04_2428),
    (BuildingStyle::Barn, 0x2a90_7451_e2cd_c380),
    (BuildingStyle::Townhouse, 0xc72d_90df_09d2_f880),
    (BuildingStyle::Church, 0xe6b4_6a85_bda2_598b),
    (BuildingStyle::Windmill, 0xeb28_4dff_02e0_1929),
    (BuildingStyle::Tenement, 0x5cb8_be3a_8953_2a48),
    (BuildingStyle::FactoryHall, 0x71cb_9750_5e35_0fe4),
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
    match style {
        BuildingStyle::Church => return bake_church(seed, params, footprint_half),
        BuildingStyle::Windmill => return bake_windmill(seed, params, footprint_half),
        BuildingStyle::Tenement => return bake_tenement(seed, params, footprint_half),
        BuildingStyle::FactoryHall => return bake_factory_hall(seed, params, footprint_half),
        _ => {}
    }
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
        WorldMaterial::PlinthStone,
    );
    let body_half_y = (params.eaves_height - params.plinth_height) * 0.5;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height + body_half_y, 0.0),
        Vec3::new(params.half_width - recess, body_half_y, params.half_depth - recess),
        WorldMaterial::Wall,
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
                WorldMaterial::WindowGlass,
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
        WorldMaterial::PlankDoor,
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

/// The church (B4 cz.2): the nave reuses the cottage grammar (plinth, recessed body, tall
/// windows), the tower rises at the -Z end past the nave ridge and closes with a pyramid
/// spire. All inside the footprint; the spire tip IS the footprint ceiling.
fn bake_church(seed: u64, params: &StyleParams, footprint_half: Vec3) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0xC44C_0000);
    let recess = 0.08;
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    let nave_ridge = params.eaves_height + 2.2;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height * 0.5, 0.0),
        Vec3::new(params.half_width, params.plinth_height * 0.5, params.half_depth),
        WorldMaterial::PlinthStone,
    );
    let body_half_y = (params.eaves_height - params.plinth_height) * 0.5;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height + body_half_y, 0.0),
        Vec3::new(params.half_width - recess, body_half_y, params.half_depth - recess),
        WorldMaterial::Wall,
    );
    let (window_w, window_h) = params.window_size;
    let sill = params.plinth_height + 0.9;
    for side in [-1.0_f32, 1.0] {
        for slot in 0..params.windows_per_side {
            let along = (slot as f32 + 0.5) / params.windows_per_side as f32 * 2.0 - 1.0;
            let jitter = rng.signed() * 0.05;
            push_box(
                &mut walls,
                &mut wall_indices,
                Vec3::new(
                    side * (params.half_width - recess * 0.5),
                    sill + window_h * 0.5,
                    (along + jitter) * (params.half_depth * 0.45) + params.half_depth * 0.25,
                ),
                Vec3::new(recess * 0.45, window_h * 0.5, window_w * 0.5),
                WorldMaterial::WindowGlass,
            );
        }
    }
    // The tower: square shaft at the -Z end, rising past the nave ridge.
    let tower_half = (params.half_width - recess) * 0.62;
    let tower_top = params.ridge_height - 2.4;
    let tower_z = -params.half_depth + tower_half + recess;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, tower_top * 0.5, tower_z),
        Vec3::new(tower_half, tower_top * 0.5, tower_half),
        WorldMaterial::Wall,
    );
    let (door_w, door_h) = params.door_size;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height + door_h * 0.5, tower_z - tower_half + 0.02),
        Vec3::new(door_w * 0.5, door_h * 0.5, recess * 0.5),
        WorldMaterial::PlankDoor,
    );

    let mut roof = Vec::new();
    let mut roof_indices = Vec::new();
    push_gable(
        &mut roof,
        &mut roof_indices,
        params.half_width,
        params.half_depth,
        params.eaves_height,
        nave_ridge,
    );
    push_pyramid(
        &mut roof,
        &mut roof_indices,
        Vec3::new(0.0, tower_top, tower_z),
        tower_half,
        params.ridge_height - tower_top,
    );
    BakedBuilding {
        style: BuildingStyle::Church,
        walls: GeometryMesh::new(walls, wall_indices),
        roof: GeometryMesh::new(roof, roof_indices),
        footprint_half,
    }
}

/// The tenement (urban-map PR-08): three storeys of masonry read as FLOORS — the plinth,
/// then a window row per storey between string courses, under a shallow roof. The mechanical
/// logic is civic, not rural: floors land near 3.2 m, windows align in a grid (the per-slot
/// jitter is a hand-set tolerance, not a scatter), the entrance is a pair, and the courses
/// carry the masonry story across the whole street front.
fn bake_tenement(seed: u64, params: &StyleParams, footprint_half: Vec3) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0x7E4E_0000);
    let recess = 0.1;
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    // Tall stone plinth, then the recessed masonry body up to the eaves.
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height * 0.5, 0.0),
        Vec3::new(params.half_width, params.plinth_height * 0.5, params.half_depth),
        WorldMaterial::PlinthStone,
    );
    let body_half_y = (params.eaves_height - params.plinth_height) * 0.5;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height + body_half_y, 0.0),
        Vec3::new(params.half_width - recess, body_half_y, params.half_depth - recess),
        WorldMaterial::Wall,
    );
    // Three storeys between plinth and eaves; a stone string course marks each floor line.
    const STOREYS: u32 = 3;
    let storey_h = (params.eaves_height - params.plinth_height) / STOREYS as f32;
    for course in 1..STOREYS {
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(0.0, params.plinth_height + storey_h * course as f32, 0.0),
            Vec3::new(params.half_width - recess * 0.3, 0.07, params.half_depth - recess * 0.3),
            WorldMaterial::PlinthStone,
        );
    }
    // The window grid: one row per storey on BOTH street fronts, aligned in slots with a
    // hand-set jitter per pane. Glass sits proud of the recessed render, inside the footprint.
    let (window_w, window_h) = params.window_size;
    for storey in 0..STOREYS {
        let sill = params.plinth_height + storey_h * storey as f32 + 0.85;
        for side in [-1.0_f32, 1.0] {
            for slot in 0..params.windows_per_side {
                let along = (slot as f32 + 0.5) / params.windows_per_side as f32 * 2.0 - 1.0;
                let jitter = rng.signed() * 0.05;
                push_box(
                    &mut walls,
                    &mut wall_indices,
                    Vec3::new(
                        side * (params.half_width - recess * 0.5),
                        sill + window_h * 0.5,
                        (along + jitter) * (params.half_depth * 0.7),
                    ),
                    Vec3::new(recess * 0.45, window_h * 0.5, window_w * 0.5),
                    WorldMaterial::WindowGlass,
                );
            }
        }
    }
    // The paired entrance on the +X street front: two doors flanking the middle bay.
    let (door_w, door_h) = params.door_size;
    for door_side in [-1.0_f32, 1.0] {
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(
                params.half_width - recess * 0.5,
                params.plinth_height + door_h * 0.5,
                door_side * params.half_depth * 0.35,
            ),
            Vec3::new(recess * 0.5, door_h * 0.5, door_w * 0.5),
            WorldMaterial::PlankDoor,
        );
    }

    // The shallow-pitched roof: the skyline stays a masonry wall, not a barn.
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
        style: BuildingStyle::Tenement,
        walls: GeometryMesh::new(walls, wall_indices),
        roof: GeometryMesh::new(roof, roof_indices),
        footprint_half,
    }
}

/// The factory hall (urban-map PR-09): one working span. The mechanical story — high sills
/// because machine lines own the walls below, a gable-end doorway a loaded wagon clears
/// under its stone lintel, and the glazed clerestory band riding the ridge (the hall's real
/// daylight) under a flat industrial cap. Halls stand by NAME (`factory` in the id): the
/// proportion heuristic never invents one.
fn bake_factory_hall(seed: u64, params: &StyleParams, footprint_half: Vec3) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0xFAC7_0000);
    let recess = 0.1;
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height * 0.5, 0.0),
        Vec3::new(params.half_width, params.plinth_height * 0.5, params.half_depth),
        WorldMaterial::PlinthStone,
    );
    let body_half_y = (params.eaves_height - params.plinth_height) * 0.5;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height + body_half_y, 0.0),
        Vec3::new(params.half_width - recess, body_half_y, params.half_depth - recess),
        WorldMaterial::Wall,
    );
    // Sparse HIGH windows on both long walls: the machine lines own the wall below the
    // sills, so the glass starts at shoulder-of-the-hall height.
    let (window_w, window_h) = params.window_size;
    let sill = params.eaves_height - window_h - 1.1;
    for side in [-1.0_f32, 1.0] {
        for slot in 0..params.windows_per_side {
            let along = (slot as f32 + 0.5) / params.windows_per_side as f32 * 2.0 - 1.0;
            let jitter = rng.signed() * 0.05;
            push_box(
                &mut walls,
                &mut wall_indices,
                Vec3::new(
                    side * (params.half_width - recess * 0.5),
                    sill + window_h * 0.5,
                    (along + jitter) * (params.half_depth * 0.78),
                ),
                Vec3::new(recess * 0.45, window_h * 0.5, window_w * 0.5),
                WorldMaterial::WindowGlass,
            );
        }
    }
    // The wagon door on the +Z gable end, under a stone lintel band; a worker door on the
    // +X long wall near the corner.
    let (door_w, door_h) = params.door_size;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height + door_h * 0.5, params.half_depth - recess * 0.5),
        Vec3::new(door_w * 0.5, door_h * 0.5, recess * 0.5),
        WorldMaterial::PlankDoor,
    );
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height + door_h + 0.25, params.half_depth - recess * 0.5),
        Vec3::new(door_w * 0.5 + 0.35, 0.25, recess * 0.5),
        WorldMaterial::PlinthStone,
    );
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(
            params.half_width - recess * 0.5,
            params.plinth_height + 1.05,
            -params.half_depth * 0.72,
        ),
        Vec3::new(recess * 0.5, 1.05, 0.55),
        WorldMaterial::PlankDoor,
    );

    // The roof story: the main gable stops short of the ridge cap, the glazed clerestory
    // band rides the ridge line, and a flat slab caps it — industrial, not domestic.
    let gable_top = params.ridge_height - 1.0;
    let mut roof = Vec::new();
    let mut roof_indices = Vec::new();
    push_gable(
        &mut roof,
        &mut roof_indices,
        params.half_width,
        params.half_depth,
        params.eaves_height,
        gable_top,
    );
    let clerestory_half = Vec3::new(1.9, 0.45, params.half_depth * 0.72);
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, gable_top - 0.1 + clerestory_half.y, 0.0),
        clerestory_half,
        WorldMaterial::WindowGlass,
    );
    push_box(
        &mut roof,
        &mut roof_indices,
        Vec3::new(0.0, gable_top - 0.1 + clerestory_half.y * 2.0 + 0.09, 0.0),
        Vec3::new(clerestory_half.x + 0.25, 0.1, clerestory_half.z + 0.25),
        WorldMaterial::Roof,
    );
    BakedBuilding {
        style: BuildingStyle::FactoryHall,
        walls: GeometryMesh::new(walls, wall_indices),
        roof: GeometryMesh::new(roof, roof_indices),
        footprint_half,
    }
}

/// The windmill (B4 cz.2): an eight-sided body tapering toward the cap, a door at the base,
/// and a conical cap. Sails are deliberately absent - the collision box is the tower, and
/// the honesty rule keeps the eye and the shell agreeing.
fn bake_windmill(seed: u64, params: &StyleParams, footprint_half: Vec3) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0x3311_7700);
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height * 0.5, 0.0),
        Vec3::new(params.half_width, params.plinth_height * 0.5, params.half_depth),
        WorldMaterial::PlinthStone,
    );
    let base_r = params.half_width - 0.15;
    let top_r = base_r * 0.62;
    push_taper(
        &mut walls,
        &mut wall_indices,
        params.plinth_height,
        params.eaves_height,
        base_r,
        top_r,
        // The windmill body is timber-clad — dark boards, not render.
        WorldMaterial::Timber,
    );
    let (door_w, door_h) = params.door_size;
    let door_angle = rng.unit() * std::f32::consts::TAU;
    let (dsin, dcos) = door_angle.sin_cos();
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(
            dcos * (base_r - 0.45),
            params.plinth_height + door_h * 0.5,
            dsin * (base_r - 0.45),
        ),
        Vec3::new(door_w * 0.5, door_h * 0.5, door_w * 0.5),
        WorldMaterial::PlankDoor,
    );

    let mut roof = Vec::new();
    let mut roof_indices = Vec::new();
    push_cone(&mut roof, &mut roof_indices, params.eaves_height, params.ridge_height, top_r + 0.18);
    BakedBuilding {
        style: BuildingStyle::Windmill,
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
        push_box(&mut walls, &mut wall_indices, center, half, WorldMaterial::Wall);
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
        push_box(&mut roof, &mut roof_indices, center, half, WorldMaterial::Roof);
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
    material: WorldMaterial,
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
                WorldMaterial::Roof.carrier(),
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
                WorldMaterial::Wall.carrier(),
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

/// A four-sided pyramid roof (the church spire): apex over the base centre.
fn push_pyramid(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    base_center: Vec3,
    half: f32,
    height: f32,
) {
    let apex = base_center + Vec3::new(0.0, height, 0.0);
    let corners = [
        base_center + Vec3::new(-half, 0.0, -half),
        base_center + Vec3::new(half, 0.0, -half),
        base_center + Vec3::new(half, 0.0, half),
        base_center + Vec3::new(-half, 0.0, half),
    ];
    for face in 0..4 {
        let a = corners[face];
        let b = corners[(face + 1) % 4];
        let normal = outward_normal((b - a).cross(apex - a), base_center, (a + b) * 0.5);
        let base = vertices.len() as u32;
        for corner in [a, b, apex] {
            vertices.push(GeometryVertex::new(
                corner,
                normal,
                WorldMaterial::Roof.carrier(),
                SmoothingGroup::hard_edges(),
            ));
        }
        if normal.dot((b - a).cross(apex - a)) >= 0.0 {
            indices.extend_from_slice(&[base, base + 1, base + 2]);
        } else {
            indices.extend_from_slice(&[base, base + 2, base + 1]);
        }
    }
}

/// An eight-sided tapered drum (the windmill body), open top and bottom (the plinth and cap
/// close them visually).
fn push_taper(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    bottom_y: f32,
    top_y: f32,
    bottom_r: f32,
    top_r: f32,
    material: WorldMaterial,
) {
    let material = material.carrier();
    const SIDES: usize = 8;
    for side in 0..SIDES {
        let a0 = side as f32 / SIDES as f32 * std::f32::consts::TAU;
        let a1 = (side + 1) as f32 / SIDES as f32 * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let p00 = Vec3::new(c0 * bottom_r, bottom_y, s0 * bottom_r);
        let p10 = Vec3::new(c1 * bottom_r, bottom_y, s1 * bottom_r);
        let p11 = Vec3::new(c1 * top_r, top_y, s1 * top_r);
        let p01 = Vec3::new(c0 * top_r, top_y, s0 * top_r);
        let raw = (p10 - p00).cross(p01 - p00);
        let normal = outward_normal(raw, Vec3::ZERO, (p00 + p10) * 0.5);
        let base = vertices.len() as u32;
        for corner in [p00, p10, p11, p01] {
            vertices.push(GeometryVertex::new(
                corner,
                normal,
                material,
                SmoothingGroup::hard_edges(),
            ));
        }
        if normal.dot(raw) >= 0.0 {
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
}

/// An eight-sided conical cap (the windmill roof).
fn push_cone(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    base_y: f32,
    apex_y: f32,
    radius: f32,
) {
    const SIDES: usize = 8;
    let apex = Vec3::new(0.0, apex_y, 0.0);
    for side in 0..SIDES {
        let a0 = side as f32 / SIDES as f32 * std::f32::consts::TAU;
        let a1 = (side + 1) as f32 / SIDES as f32 * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let p0 = Vec3::new(c0 * radius, base_y, s0 * radius);
        let p1 = Vec3::new(c1 * radius, base_y, s1 * radius);
        let raw = (p1 - p0).cross(apex - p0);
        let normal = outward_normal(raw, Vec3::ZERO, (p0 + p1) * 0.5);
        let base = vertices.len() as u32;
        for corner in [p0, p1, apex] {
            vertices.push(GeometryVertex::new(
                corner,
                normal,
                WorldMaterial::Roof.carrier(),
                SmoothingGroup::hard_edges(),
            ));
        }
        if normal.dot(raw) >= 0.0 {
            indices.extend_from_slice(&[base, base + 1, base + 2]);
        } else {
            indices.extend_from_slice(&[base, base + 2, base + 1]);
        }
    }
}

/// Flip a radial face normal to point AWAY from the body's axis: the winding conventions of
/// the radial helpers are easier to keep honest by construction than by case analysis.
fn outward_normal(raw: Vec3, center: Vec3, at: Vec3) -> Vec3 {
    let normal = raw.normalize();
    let outward = Vec3::new(at.x - center.x, 0.0, at.z - center.z);
    if normal.dot(outward) < 0.0 { -normal } else { normal }
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
