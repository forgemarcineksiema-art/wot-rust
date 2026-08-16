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
use crate::shape::Rng;

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
                storeys: 1,
                windows_per_side: 2,
                window_size: (0.85, 1.1),
                door_size: (0.9, 1.95),
            },
            BuildingStyle::Barn => StyleParams {
                half_width: 4.4,
                half_depth: 7.0,
                eaves_height: 3.4,
                ridge_height: 5.4,
                plinth_height: 0.35,
                storeys: 1,
                windows_per_side: 1,
                window_size: (0.6, 0.7),
                door_size: (2.4, 2.6),
            },
            BuildingStyle::Townhouse => StyleParams {
                half_width: 3.8,
                half_depth: 5.2,
                eaves_height: 5.6,
                ridge_height: 7.6,
                plinth_height: 0.55,
                storeys: 2,
                windows_per_side: 3,
                window_size: (0.9, 1.4),
                door_size: (0.9, 2.0),
            },
            // ridge_height is the SPIRE TOP: the footprint must contain the whole silhouette.
            BuildingStyle::Church => StyleParams {
                half_width: 3.6,
                half_depth: 6.4,
                eaves_height: 4.4,
                ridge_height: 11.0,
                plinth_height: 0.6,
                storeys: 1,
                windows_per_side: 3,
                window_size: (0.8, 2.2),
                door_size: (1.4, 2.4),
            },
            // ridge_height is the cap peak; the body tapers inside half_width.
            BuildingStyle::Windmill => StyleParams {
                half_width: 3.0,
                half_depth: 3.0,
                eaves_height: 6.2,
                ridge_height: 8.6,
                plinth_height: 0.5,
                storeys: 1,
                windows_per_side: 1,
                window_size: (0.6, 0.8),
                door_size: (0.9, 1.95),
            },
            // Three storeys of masonry under a shallow roof: eaves ~10.4 m puts each floor
            // near the civic 3.2 m, and the low ridge keeps the skyline a wall, not a barn.
            BuildingStyle::Tenement => StyleParams {
                half_width: 4.6,
                half_depth: 6.0,
                eaves_height: 10.4,
                ridge_height: 12.0,
                plinth_height: 0.7,
                storeys: 3,
                windows_per_side: 4,
                window_size: (0.95, 1.7),
                door_size: (1.3, 2.4),
            },
            // A working span, not a house: one tall volume, high sills so machines line the
            // walls, and a door a loaded wagon clears. ridge_height caps the clerestory.
            BuildingStyle::FactoryHall => StyleParams {
                half_width: 6.5,
                half_depth: 11.0,
                eaves_height: 7.2,
                ridge_height: 9.0,
                plinth_height: 0.6,
                storeys: 1,
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
    /// How many floors the wall carries (Immersja A1.3): canonical styles keep their
    /// authored count; a sized bake derives it from the target height at the civic
    /// ~3 m floor pitch, so a 19 m elevator head house gets five storeys of windows
    /// instead of three stretched ones.
    storeys: u32,
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
/// Blessed 2026-08-16 (Immersja A1.1): openings raised to real-world absolutes — the
/// 2026-08-03 audit measured doors/windows 30-45 % short, which is why an 11 m tenement
/// read as a maquette. FactoryHall was already true and its hash is UNCHANGED, proving the
/// wave touched nothing else. Canonical footprints did not move (map goldens untouched).
pub const BUILDING_GOLDEN_HASHES: [(BuildingStyle, u64); 7] = [
    (BuildingStyle::Cottage, 0xddc3_1870_16a5_036a),
    (BuildingStyle::Barn, 0x9ee8_9ec1_cb17_2858),
    (BuildingStyle::Townhouse, 0x8786_05a6_615f_a9f6),
    (BuildingStyle::Church, 0xef7e_f288_7997_c4c6),
    (BuildingStyle::Windmill, 0x9702_54dc_8c3c_74bd),
    (BuildingStyle::Tenement, 0xffe8_ca8e_053c_8dc0),
    (BuildingStyle::FactoryHall, 0xc2a7_6240_a862_1b1a),
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

/// Bake AT the blueprint's size instead of stretching a canonical bake into it (Immersja
/// A1.2). The per-axis AABB stretch scaled the OPENINGS with the box — a wide tenement got
/// wide windows instead of more windows, and the same style wore a 0.92 m window on one
/// building and a 1.58 m one on another, which is exactly how a city becomes a maquette.
/// Here the facade layout is computed in world units: openings keep their real-world
/// absolute sizes and the COUNT comes from the wall's length (the canonical facade's
/// rhythm carried to the target). Every vertex provably stays inside `target_half` — the
/// honesty contract is unchanged, only the stretch is gone.
pub fn bake_building_sized(
    style: BuildingStyle,
    seed: u64,
    form: StructureForm,
    target_half: Vec3,
) -> BakedBuilding {
    let params = sized_params(style, target_half);
    match form {
        StructureForm::Intact => bake_intact(style, seed, &params, target_half),
        StructureForm::Rubble { height_frac } => {
            bake_rubble(style, seed, &params, target_half, height_frac.clamp(0.05, 0.9))
        }
    }
}

/// The style table re-derived for a target box: masses follow the box, the roof keeps the
/// style's proportion, openings keep their ABSOLUTE size, and the window count follows the
/// wall length at the canonical rhythm. Degenerate boxes degrade gracefully — an opening
/// that cannot fit its band shrinks and then disappears rather than ever leaving the box.
fn sized_params(style: BuildingStyle, target_half: Vec3) -> StyleParams {
    let canonical = style.params();
    let half_width = target_half.x.max(0.3);
    let half_depth = target_half.z.max(0.3);
    let ridge_height = (target_half.y * 2.0).max(0.8);
    let eaves_height = ridge_height * (canonical.eaves_height / canonical.ridge_height);
    // The plinth is a real-world course, not a proportion — absolute, but never a third of
    // a genuinely tiny wall.
    let plinth_height = canonical.plinth_height.min(eaves_height * 0.3);

    // The storey ladder (Immersja A1.3): the multi-floor styles derive their floor count
    // from the target height at the civic ~3 m pitch, so a 19 m head house earns five
    // storeys of windows instead of three stretched ones. On the canonical boxes the
    // formula reproduces the authored counts exactly (townhouse 2, tenement 3).
    let storeys = match style {
        BuildingStyle::Townhouse | BuildingStyle::Tenement => {
            (((eaves_height - plinth_height) / 3.0).round() as u32).clamp(1, 8)
        }
        _ => 1,
    };
    let storey_h = (eaves_height - plinth_height) / storeys as f32;

    // Window height: absolute, clamped into the band its style cuts it from (each arm
    // mirrors the bake fn's own sill formula).
    let (window_w, window_h) = canonical.window_size;
    let band_headroom = match style {
        BuildingStyle::Cottage => eaves_height - plinth_height - 0.55 - 0.15,
        BuildingStyle::Barn => eaves_height - plinth_height - 1.8 - 0.15,
        BuildingStyle::Townhouse => storey_h - 0.8 - 0.15,
        BuildingStyle::Church => eaves_height - plinth_height - 0.9 - 0.2,
        BuildingStyle::Windmill => eaves_height - plinth_height - 0.9 - 0.15,
        BuildingStyle::Tenement => storey_h - 0.85 - 0.15,
        BuildingStyle::FactoryHall => eaves_height - plinth_height - 1.2,
    };
    let window_h = window_h.min(band_headroom.max(0.0));

    // Window count: the canonical facade's rhythm (meters of wall per window along the
    // ridge axis) carried to the target length, capped so glass never crowds the piers out
    // and a budget cap keeps a city block from baking a curtain wall.
    let rhythm = (canonical.half_depth * 2.0) / canonical.windows_per_side.max(1) as f32;
    let by_rhythm = ((half_depth * 2.0) / rhythm).round() as u32;
    let by_fit = ((half_depth - 0.1) * 2.0 * 0.6 / window_w.max(0.1)).floor() as u32;
    let windows_per_side =
        if window_h < 0.3 { 0 } else { by_rhythm.clamp(1, 12).min(by_fit.max(1)) };

    // Door: absolute, clamped under the band it is cut into.
    let (door_w, door_h) = canonical.door_size;
    let door_headroom = match style {
        BuildingStyle::Townhouse => storey_h - 0.3,
        BuildingStyle::Tenement => storey_h - 0.3,
        _ => eaves_height - plinth_height - 0.25,
    };
    let door_h = door_h.min(door_headroom.max(0.3));
    let door_w = door_w.min(half_depth * 0.8).min(half_width * 0.8);

    StyleParams {
        half_width,
        half_depth,
        eaves_height,
        ridge_height,
        plinth_height,
        storeys,
        windows_per_side,
        window_size: (window_w, window_h),
        door_size: (door_w, door_h),
    }
}

fn bake_intact(
    style: BuildingStyle,
    seed: u64,
    params: &StyleParams,
    footprint_half: Vec3,
) -> BakedBuilding {
    match style {
        BuildingStyle::Cottage => bake_cottage(seed, params, footprint_half),
        BuildingStyle::Barn => bake_barn(seed, params, footprint_half),
        BuildingStyle::Townhouse => bake_townhouse(seed, params, footprint_half),
        BuildingStyle::Church => bake_church(seed, params, footprint_half),
        BuildingStyle::Windmill => bake_windmill(seed, params, footprint_half),
        BuildingStyle::Tenement => bake_tenement(seed, params, footprint_half),
        BuildingStyle::FactoryHall => bake_factory_hall(seed, params, footprint_half),
    }
}

/// A doorway cut through a facade leaf: the leaf is built as two pierced runs flanking the
/// opening, and this fills the gap — the recessed door leaf, its lintel and the wall above.
/// `run_lo`/`run_hi` are the z bounds of the opening in leaf-local coordinates.
#[allow(clippy::too_many_arguments)]
fn push_doorway(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    side: f32,
    face: f32,
    thickness: f32,
    base: f32,
    top: f32,
    door_half_w: f32,
    door_h: f32,
    z: f32,
    trim: WorldMaterial,
) {
    let half_x = thickness * 0.5;
    let cx = side * (face - half_x);
    // The wall above the doorway, from the door head to the run's top.
    push_box(
        vertices,
        indices,
        Vec3::new(cx, (base + door_h + top) * 0.5, z),
        Vec3::new(half_x, (top - base - door_h) * 0.5, door_half_w),
        WorldMaterial::Wall,
    );
    // The lintel over the door, a finger proud of the face.
    push_box(
        vertices,
        indices,
        Vec3::new(side * (face + 0.015), base + door_h + 0.05, z),
        Vec3::new(0.045, 0.05, door_half_w + 0.09),
        trim,
    );
    // The door leaf itself, recessed into the opening.
    push_face(
        vertices,
        indices,
        [
            Vec3::new(side * (face - 0.07), base, z - door_half_w + 0.03),
            Vec3::new(side * (face - 0.07), base, z + door_half_w - 0.03),
            Vec3::new(side * (face - 0.07), base + door_h, z + door_half_w - 0.03),
            Vec3::new(side * (face - 0.07), base + door_h, z - door_half_w + 0.03),
        ],
        Vec3::X * side,
        WorldMaterial::PlankDoor,
    );
}

/// One pierced run of a rural long wall: `windows` slots between `run_lo` and `run_hi`
/// (absolute facade z), jittered inside their bays so no two cottages share a facade.
#[allow(clippy::too_many_arguments)]
fn rural_wall_run(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    rng: &mut Rng,
    side: f32,
    face: f32,
    thickness: f32,
    run_lo: f32,
    run_hi: f32,
    base: f32,
    top: f32,
    sill: f32,
    head: f32,
    windows: u32,
    half_w: f32,
    band: WorldMaterial,
    glazing: WorldMaterial,
    trim: WorldMaterial,
) {
    let span_half = (run_hi - run_lo) * 0.5;
    if windows == 0 || span_half < half_w + 0.2 {
        // Too narrow to pierce honestly — a solid pier.
        push_box(
            vertices,
            indices,
            Vec3::new(side * (face - thickness * 0.5), (base + top) * 0.5, (run_lo + run_hi) * 0.5),
            Vec3::new(thickness * 0.5, (top - base) * 0.5, span_half),
            WorldMaterial::Wall,
        );
        return;
    }
    let mid = (run_lo + run_hi) * 0.5;
    let usable = span_half - half_w - 0.2;
    let slots: Vec<(f32, f32)> = (0..windows)
        .map(|slot| {
            let along = (slot as f32 + 0.5) / windows as f32 * 2.0 - 1.0;
            let jitter = rng.signed() * 0.06;
            (mid + (along + jitter) * usable, half_w)
        })
        .collect();
    push_pierced_wall(
        vertices,
        indices,
        WallSpec {
            side,
            face,
            thickness,
            span_lo: run_lo,
            span_hi: run_hi,
            base,
            top,
            sill,
            head,
            band,
        },
        &slots,
    );
    for &(z, hw) in &slots {
        push_window(vertices, indices, side, face, z, sill, head, hw, glazing, trim);
    }
}

/// The cottage (Fasada 2.0, Świat 2.0 PR 4): the village house — a low leaf pierced by one
/// row of true window openings under a timber bressumer, plank-framed panes a hand inside
/// the wall, and the doorway cut through the street leaf (two runs carry the wall around
/// it). The dressing is sawn timber, not the town's stone.
fn bake_cottage(seed: u64, params: &StyleParams, footprint_half: Vec3) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0xC077_0000);
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height * 0.5, 0.0),
        Vec3::new(params.half_width, params.plinth_height * 0.5, params.half_depth),
        WorldMaterial::PlinthStone,
    );
    let face = params.half_width - 0.1;
    let leaf_depth = params.half_depth - 0.1;
    let (window_w, window_h) = params.window_size;
    let half_w = window_w * 0.5;
    let sill = (params.plinth_height + 0.55).min(params.eaves_height);
    let head = (sill + window_h).min(params.eaves_height);
    // The street leaf splits around the doorway; the back leaf runs full length. Window
    // counts follow the run's LENGTH at the facade's rhythm (Immersja A1.2): a longer izba
    // earns more windows, the windows themselves never grow.
    let (door_w, door_h) = params.door_size;
    let door_half = door_w * 0.5;
    let door_z = rng.signed() * leaf_depth * 0.3;
    let per_run = |len: f32| {
        ((params.windows_per_side as f32 * len / (leaf_depth * 2.0)).round() as u32)
            .max(params.windows_per_side.min(1))
    };
    for (side, runs) in [
        (-1.0_f32, [(-leaf_depth, leaf_depth, params.windows_per_side), (0.0, 0.0, 0)]),
        (
            1.0,
            [
                (-leaf_depth, door_z - door_half, per_run(door_z - door_half + leaf_depth)),
                (door_z + door_half, leaf_depth, per_run(leaf_depth - door_z - door_half)),
            ],
        ),
    ] {
        for &(run_lo, run_hi, windows) in &runs {
            if run_hi - run_lo < 0.3 {
                continue;
            }
            rural_wall_run(
                &mut walls,
                &mut wall_indices,
                &mut rng,
                side,
                face,
                0.18,
                run_lo,
                run_hi,
                params.plinth_height,
                params.eaves_height,
                sill,
                head,
                windows,
                half_w,
                WorldMaterial::Timber,
                WorldMaterial::WindowGlass,
                WorldMaterial::Timber,
            );
        }
    }
    push_doorway(
        &mut walls,
        &mut wall_indices,
        1.0,
        face,
        0.18,
        params.plinth_height,
        params.eaves_height,
        door_half,
        door_h,
        door_z,
        WorldMaterial::Timber,
    );
    // The gable ends close the shell.
    let body_half_y = (params.eaves_height - params.plinth_height) * 0.5;
    for end in [-1.0_f32, 1.0] {
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(0.0, params.plinth_height + body_half_y, end * (leaf_depth - 0.09)),
            Vec3::new(face, body_half_y, 0.09),
            WorldMaterial::Wall,
        );
    }

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
        style: BuildingStyle::Cottage,
        walls: GeometryMesh::new(walls, wall_indices),
        roof: GeometryMesh::new(roof, roof_indices),
        footprint_half,
    }
}

/// The barn (Fasada 2.0, Świat 2.0 PR 4): a working barn has no glass — its openings are
/// a high shuttered slit per long wall (the hay loft breathes) and a true wagon portal in
/// EACH gable end, the door leaf recessed behind timber jambs and a bressumer.
fn bake_barn(seed: u64, params: &StyleParams, footprint_half: Vec3) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0xBA27_0000);
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height * 0.5, 0.0),
        Vec3::new(params.half_width, params.plinth_height * 0.5, params.half_depth),
        WorldMaterial::PlinthStone,
    );
    let face = params.half_width - 0.1;
    let leaf_depth = params.half_depth - 0.1;
    let (window_w, window_h) = params.window_size;
    let sill = (params.plinth_height + 1.8).min(params.eaves_height);
    let head = (sill + window_h).min(params.eaves_height);
    for side in [-1.0_f32, 1.0] {
        rural_wall_run(
            &mut walls,
            &mut wall_indices,
            &mut rng,
            side,
            face,
            0.18,
            -leaf_depth,
            leaf_depth,
            params.plinth_height,
            params.eaves_height,
            sill,
            head,
            params.windows_per_side,
            window_w * 0.5,
            WorldMaterial::Timber,
            WorldMaterial::PlankDoor,
            WorldMaterial::Timber,
        );
    }
    // Both gable ends carry a wagon portal: side piers, the wall above the doorway, a
    // bressumer and jamb stones, and the door leaf recessed into the opening.
    let (door_w, door_h) = params.door_size;
    let door_half = door_w * 0.5;
    for end in [-1.0_f32, 1.0] {
        let z_face = end * leaf_depth;
        let z_wall = end * (leaf_depth - 0.09);
        for door_side in [-1.0_f32, 1.0] {
            let pier_half = (face - door_half) * 0.5;
            push_box(
                &mut walls,
                &mut wall_indices,
                Vec3::new(
                    door_side * (door_half + pier_half),
                    (params.plinth_height + params.eaves_height) * 0.5,
                    z_wall,
                ),
                Vec3::new(pier_half, (params.eaves_height - params.plinth_height) * 0.5, 0.09),
                WorldMaterial::Wall,
            );
            push_box(
                &mut walls,
                &mut wall_indices,
                Vec3::new(
                    door_side * (door_half + 0.07),
                    params.plinth_height + door_h * 0.5 + 0.06,
                    z_face - end * 0.03,
                ),
                Vec3::new(0.07, door_h * 0.5 + 0.06, 0.06),
                WorldMaterial::Timber,
            );
        }
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(
                0.0,
                params.plinth_height
                    + door_h
                    + (params.eaves_height - params.plinth_height - door_h) * 0.5,
                z_wall,
            ),
            Vec3::new(door_half, (params.eaves_height - params.plinth_height - door_h) * 0.5, 0.09),
            WorldMaterial::Wall,
        );
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(0.0, params.plinth_height + door_h + 0.06, z_face - end * 0.02),
            Vec3::new(door_half + 0.12, 0.06, 0.05),
            WorldMaterial::Timber,
        );
        push_face(
            &mut walls,
            &mut wall_indices,
            [
                Vec3::new(-door_half + 0.04, params.plinth_height, z_face - end * 0.16),
                Vec3::new(door_half - 0.04, params.plinth_height, z_face - end * 0.16),
                Vec3::new(door_half - 0.04, params.plinth_height + door_h, z_face - end * 0.16),
                Vec3::new(-door_half + 0.04, params.plinth_height + door_h, z_face - end * 0.16),
            ],
            Vec3::Z * end,
            WorldMaterial::PlankDoor,
        );
    }

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
        style: BuildingStyle::Barn,
        walls: GeometryMesh::new(walls, wall_indices),
        roof: GeometryMesh::new(roof, roof_indices),
        footprint_half,
    }
}

/// The townhouse (Fasada 2.0, Świat 2.0 PR 4): the village's masonry house — two storeys,
/// each a pierced leaf under a dressed-stone band with stone-framed recessed panes, and the
/// street doorway cut through the ground floor. A humbler grammar than the city's, in the
/// same language.
fn bake_townhouse(seed: u64, params: &StyleParams, footprint_half: Vec3) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0x7041_0000);
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height * 0.5, 0.0),
        Vec3::new(params.half_width, params.plinth_height * 0.5, params.half_depth),
        WorldMaterial::PlinthStone,
    );
    let face = params.half_width - 0.1;
    let leaf_depth = params.half_depth - 0.1;
    let (window_w, window_h) = params.window_size;
    let half_w = window_w * 0.5;
    let (door_w, door_h) = params.door_size;
    let door_half = door_w * 0.5;
    let door_z = rng.signed() * leaf_depth * 0.35;
    let storeys = params.storeys.max(1);
    let storey_h = (params.eaves_height - params.plinth_height) / storeys as f32;
    for side in [-1.0_f32, 1.0] {
        for storey in 0..storeys {
            let floor = params.plinth_height + storey_h * storey as f32;
            let sill = (floor + 0.8).min(floor + storey_h);
            let head = (sill + window_h).min(floor + storey_h);
            if side > 0.0 && storey == 0 {
                // The street ground floor splits around the doorway: each run carries its
                // length's share of the facade rhythm (Immersja A1.2).
                for &(run_lo, run_hi) in
                    &[(-leaf_depth, door_z - door_half), (door_z + door_half, leaf_depth)]
                {
                    let share = ((params.windows_per_side as f32 * (run_hi - run_lo)
                        / (leaf_depth * 2.0))
                        .round() as u32)
                        .max(params.windows_per_side.min(1));
                    rural_wall_run(
                        &mut walls,
                        &mut wall_indices,
                        &mut rng,
                        side,
                        face,
                        0.18,
                        run_lo,
                        run_hi,
                        floor,
                        floor + storey_h,
                        sill,
                        head,
                        share,
                        half_w,
                        WorldMaterial::PlinthStone,
                        WorldMaterial::WindowGlass,
                        WorldMaterial::PlinthStone,
                    );
                }
            } else {
                rural_wall_run(
                    &mut walls,
                    &mut wall_indices,
                    &mut rng,
                    side,
                    face,
                    0.18,
                    -leaf_depth,
                    leaf_depth,
                    floor,
                    floor + storey_h,
                    sill,
                    head,
                    params.windows_per_side,
                    half_w,
                    WorldMaterial::PlinthStone,
                    WorldMaterial::WindowGlass,
                    WorldMaterial::PlinthStone,
                );
            }
        }
    }
    push_doorway(
        &mut walls,
        &mut wall_indices,
        1.0,
        face,
        0.18,
        params.plinth_height,
        params.plinth_height + storey_h,
        door_half,
        door_h,
        door_z,
        WorldMaterial::PlinthStone,
    );
    // The gable ends close the shell.
    let body_half_y = (params.eaves_height - params.plinth_height) * 0.5;
    for end in [-1.0_f32, 1.0] {
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(0.0, params.plinth_height + body_half_y, end * (leaf_depth - 0.09)),
            Vec3::new(face, body_half_y, 0.09),
            WorldMaterial::Wall,
        );
    }

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
        style: BuildingStyle::Townhouse,
        walls: GeometryMesh::new(walls, wall_indices),
        roof: GeometryMesh::new(roof, roof_indices),
        footprint_half,
    }
}

/// The church (Fasada 2.0, Świat 2.0 PR 4): the nave's side walls are pierced by tall
/// stone-framed windows, buttress strips stand on the east corners, and the tower's bell
/// stage is BUILT — four corner piers with a true opening on each face, louvres recessed
/// behind them — under the pyramid spire. All inside the footprint; the spire tip IS the
/// footprint ceiling.
fn bake_church(seed: u64, params: &StyleParams, footprint_half: Vec3) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0xC44C_0000);
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    // The additive silhouette constants are CLAMPED for sized bakes (Immersja A1.2): a
    // church squeezed into a shed-sized box keeps its whole silhouette inside the box
    // instead of pushing the bell floor underground. The canonical box never touches any
    // of these clamps, so the canonical golden is bit-identical.
    let nave_ridge = (params.eaves_height + 2.2).min(params.ridge_height);
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height * 0.5, 0.0),
        Vec3::new(params.half_width, params.plinth_height * 0.5, params.half_depth),
        WorldMaterial::PlinthStone,
    );
    let face = params.half_width - 0.1;
    let leaf_depth = params.half_depth - 0.1;
    // The tower: square shaft at the -Z end, rising past the nave ridge to the bell floor.
    let tower_half = face * 0.62;
    let tower_top =
        (params.ridge_height - 2.4).max(params.eaves_height).min(params.ridge_height - 0.2);
    let tower_z = -leaf_depth + tower_half;
    let tower_front = tower_z - tower_half; // the -Z face of the tower
    let nave_lo = tower_z + tower_half; // the nave's side leaves start at the tower
    // The shaft is solid only UP TO the bell floor — above it the corner piers carry the
    // cap band and the faces are true openings.
    let bell_lo = (tower_top - 1.9).max(params.plinth_height);
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, bell_lo * 0.5, tower_z),
        Vec3::new(tower_half, bell_lo * 0.5, tower_half),
        WorldMaterial::Wall,
    );
    // The nave's side leaves: tall stone-framed windows in true openings, from the tower
    // face to the east gable.
    let (window_w, window_h) = params.window_size;
    let sill = (params.plinth_height + 0.9).min(params.eaves_height);
    let head = (sill + window_h).min(params.eaves_height);
    for side in [-1.0_f32, 1.0] {
        rural_wall_run(
            &mut walls,
            &mut wall_indices,
            &mut rng,
            side,
            face,
            0.18,
            nave_lo,
            leaf_depth,
            params.plinth_height,
            params.eaves_height,
            sill,
            head,
            params.windows_per_side,
            window_w * 0.5,
            WorldMaterial::PlinthStone,
            WorldMaterial::WindowGlass,
            WorldMaterial::PlinthStone,
        );
        // The buttress strip on each east corner, plinth to eaves, slightly proud.
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(
                side * (face + 0.03),
                (params.plinth_height + params.eaves_height) * 0.5,
                leaf_depth - 0.22,
            ),
            Vec3::new(0.05, (params.eaves_height - params.plinth_height) * 0.5, 0.22),
            WorldMaterial::PlinthStone,
        );
    }
    // The east gable closes the nave.
    let body_half_y = (params.eaves_height - params.plinth_height) * 0.5;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height + body_half_y, leaf_depth - 0.09),
        Vec3::new(face, body_half_y, 0.09),
        WorldMaterial::Wall,
    );
    // The west door in the tower face: recessed leaf, stone jambs and lintel.
    let (door_w, door_h) = params.door_size;
    let door_half = door_w * 0.5;
    for door_side in [-1.0_f32, 1.0] {
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(
                door_side * (door_half + 0.09),
                params.plinth_height + door_h * 0.5 + 0.07,
                tower_front - 0.03,
            ),
            Vec3::new(0.09, door_h * 0.5 + 0.07, 0.06),
            WorldMaterial::PlinthStone,
        );
    }
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height + door_h + 0.07, tower_front - 0.03),
        Vec3::new(door_half + 0.14, 0.07, 0.06),
        WorldMaterial::PlinthStone,
    );
    push_face(
        &mut walls,
        &mut wall_indices,
        [
            Vec3::new(-door_half + 0.03, params.plinth_height, tower_front + 0.07),
            Vec3::new(door_half - 0.03, params.plinth_height, tower_front + 0.07),
            Vec3::new(door_half - 0.03, params.plinth_height + door_h, tower_front + 0.07),
            Vec3::new(-door_half + 0.03, params.plinth_height + door_h, tower_front + 0.07),
        ],
        -Vec3::Z,
        WorldMaterial::PlankDoor,
    );
    // The bell stage: four corner piers carry the cap band; each face is a TRUE opening
    // with the louvre board recessed behind it.
    let pier_off = tower_half - 0.18;
    for px in [-1.0_f32, 1.0] {
        for pz in [-1.0_f32, 1.0] {
            push_box(
                &mut walls,
                &mut wall_indices,
                Vec3::new(px * pier_off, (bell_lo + tower_top) * 0.5, tower_z + pz * pier_off),
                Vec3::new(0.18, (tower_top - bell_lo) * 0.5, 0.18),
                WorldMaterial::Wall,
            );
        }
    }
    let opening_half = pier_off - 0.18;
    for face_side in [-1.0_f32, 1.0] {
        // The ±X faces.
        push_face(
            &mut walls,
            &mut wall_indices,
            [
                Vec3::new(face_side * (tower_half - 0.07), bell_lo + 0.1, tower_z - opening_half),
                Vec3::new(face_side * (tower_half - 0.07), bell_lo + 0.1, tower_z + opening_half),
                Vec3::new(
                    face_side * (tower_half - 0.07),
                    tower_top - 0.15,
                    tower_z + opening_half,
                ),
                Vec3::new(
                    face_side * (tower_half - 0.07),
                    tower_top - 0.15,
                    tower_z - opening_half,
                ),
            ],
            Vec3::X * face_side,
            WorldMaterial::PlankDoor,
        );
        // The ±Z faces.
        push_face(
            &mut walls,
            &mut wall_indices,
            [
                Vec3::new(-opening_half, bell_lo + 0.1, tower_z + face_side * (tower_half - 0.07)),
                Vec3::new(opening_half, bell_lo + 0.1, tower_z + face_side * (tower_half - 0.07)),
                Vec3::new(
                    opening_half,
                    tower_top - 0.15,
                    tower_z + face_side * (tower_half - 0.07),
                ),
                Vec3::new(
                    -opening_half,
                    tower_top - 0.15,
                    tower_z + face_side * (tower_half - 0.07),
                ),
            ],
            Vec3::Z * face_side,
            WorldMaterial::PlankDoor,
        );
    }
    // The cap band the piers carry.
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, tower_top - 0.075, tower_z),
        Vec3::new(tower_half, 0.075, tower_half),
        WorldMaterial::PlinthStone,
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

/// The tenement (Fasada 2.0, Świat 2.0 PR 3): three storeys of masonry read as FLOORS, and
/// the facade is BUILT, not painted — each street front is a leaf pierced by true window
/// openings (apron, dressed-stone lintel band, piers), the pane sits a hand INSIDE the leaf
/// behind a stone frame and mullion cross, a sill ledge stands proud, corner lesenes and an
/// eaves cornice carry the civic order, and a string course marks each floor line. The
/// mechanical logic is civic, not rural: floors land near 3.2 m, windows align in a grid
/// (the per-slot jitter is a hand-set tolerance, not a scatter), and the entrance is a pair.
fn bake_tenement(seed: u64, params: &StyleParams, footprint_half: Vec3) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0x7E4E_0000);
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    // Tall stone plinth (the full footprint), then the pierced leaves above it.
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height * 0.5, 0.0),
        Vec3::new(params.half_width, params.plinth_height * 0.5, params.half_depth),
        WorldMaterial::PlinthStone,
    );
    // The recess is the leaf's room: the outer plane stands 0.1 m inside the footprint, and
    // every proud mark (sills, lesenes, courses, cornice) stays inside that allowance.
    let face = params.half_width - 0.1;
    let leaf_depth = params.half_depth - 0.1;
    let storeys = params.storeys.max(1);
    let storey_h = (params.eaves_height - params.plinth_height) / storeys as f32;
    let (window_w, window_h) = params.window_size;
    let half_w = window_w * 0.5;
    // The window grid: one row per storey on BOTH street fronts, aligned in slots with a
    // hand-set jitter per bay. The SAME slots cut the leaf and take the window assembly.
    for side in [-1.0_f32, 1.0] {
        for storey in 0..storeys {
            let floor = params.plinth_height + storey_h * storey as f32;
            let sill = (floor + 0.85).min(floor + storey_h);
            let head = (sill + window_h).min(floor + storey_h);
            let slots: Vec<(f32, f32)> = (0..params.windows_per_side)
                .map(|slot| {
                    let along = (slot as f32 + 0.5) / params.windows_per_side as f32 * 2.0 - 1.0;
                    let jitter = rng.signed() * 0.05;
                    ((along + jitter) * (params.half_depth * 0.7), half_w)
                })
                .collect();
            push_pierced_wall(
                &mut walls,
                &mut wall_indices,
                WallSpec {
                    side,
                    face,
                    thickness: 0.2,
                    span_lo: -leaf_depth,
                    span_hi: leaf_depth,
                    base: floor,
                    top: floor + storey_h,
                    sill,
                    head,
                    band: WorldMaterial::PlinthStone,
                },
                &slots,
            );
            for &(z, hw) in &slots {
                push_window(
                    &mut walls,
                    &mut wall_indices,
                    side,
                    face,
                    z,
                    sill,
                    head,
                    hw,
                    WorldMaterial::WindowGlass,
                    WorldMaterial::PlinthStone,
                );
            }
        }
    }
    // The gable ends close the shell (no openings — the party walls of the row).
    let body_half_y = (params.eaves_height - params.plinth_height) * 0.5;
    for end in [-1.0_f32, 1.0] {
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(0.0, params.plinth_height + body_half_y, end * (leaf_depth - 0.1)),
            Vec3::new(face, body_half_y, 0.1),
            WorldMaterial::Wall,
        );
    }
    // A stone string course marks each floor line, wrapping the leaves.
    for course in 1..storeys {
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(0.0, params.plinth_height + storey_h * course as f32, 0.0),
            Vec3::new(params.half_width - 0.03, 0.07, params.half_depth - 0.03),
            WorldMaterial::PlinthStone,
        );
    }
    // Corner lesenes: the shallow piers of the civic order, plinth to eaves, prouder than
    // the courses so the storey lines read as running INTO them.
    for side in [-1.0_f32, 1.0] {
        for end in [-1.0_f32, 1.0] {
            push_box(
                &mut walls,
                &mut wall_indices,
                Vec3::new(
                    side * (face + 0.04),
                    params.plinth_height + body_half_y,
                    end * (leaf_depth - 0.24),
                ),
                Vec3::new(0.04, body_half_y, 0.24),
                WorldMaterial::PlinthStone,
            );
        }
    }
    // The eaves cornice: the facade's top line, on all four faces.
    for side in [-1.0_f32, 1.0] {
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(side * (face + 0.04), params.eaves_height - 0.11, 0.0),
            Vec3::new(0.04, 0.11, params.half_depth - 0.03),
            WorldMaterial::PlinthStone,
        );
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(0.0, params.eaves_height - 0.11, side * (leaf_depth + 0.04)),
            Vec3::new(face - 0.06, 0.11, 0.04),
            WorldMaterial::PlinthStone,
        );
    }
    // The paired entrance on the +X street front: two doors under their own lintels,
    // standing proud of the ground-storey's piers.
    let (door_w, door_h) = params.door_size;
    for door_side in [-1.0_f32, 1.0] {
        let door_z = door_side * params.half_depth * 0.35;
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(face + 0.05, params.plinth_height + door_h * 0.5, door_z),
            Vec3::new(0.05, door_h * 0.5, door_w * 0.5),
            WorldMaterial::PlankDoor,
        );
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(face + 0.04, params.plinth_height + door_h + 0.09, door_z),
            Vec3::new(0.05, 0.09, door_w * 0.5 + 0.14),
            WorldMaterial::PlinthStone,
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

/// The factory hall (Fasada 2.0, Świat 2.0 PR 3): one working span, and its order is BUILT —
/// full-height pilaster strips carry the long walls between bays, the high windows sit in
/// true openings under a dressed-stone lintel band (machine lines own the wall below the
/// sills), the wagon doorway on the gable end is a real portal a loaded wagon clears, and
/// the glazed clerestory band rides the ridge under its flat industrial cap with a steel-sash
/// rhythm. Halls stand by NAME (`factory` in the id): the proportion heuristic never invents
/// one.
fn bake_factory_hall(seed: u64, params: &StyleParams, footprint_half: Vec3) -> BakedBuilding {
    let mut rng = Rng(seed ^ 0xFAC7_0000);
    let mut walls = Vec::new();
    let mut wall_indices = Vec::new();
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, params.plinth_height * 0.5, 0.0),
        Vec3::new(params.half_width, params.plinth_height * 0.5, params.half_depth),
        WorldMaterial::PlinthStone,
    );
    // The recess is the leaf's room (see the tenement): outer plane 0.1 m inside the
    // footprint, proud marks inside that allowance.
    let face = params.half_width - 0.1;
    let leaf_depth = params.half_depth - 0.1;
    let (window_w, window_h) = params.window_size;
    let half_w = window_w * 0.5;
    let sill = (params.eaves_height - window_h - 1.1).max(params.plinth_height);
    let head = (sill + window_h).min(params.eaves_height);
    // Both long walls: the working apron below the sills, the stone lintel band above the
    // heads, leaf piers between the bays — then full-height pilaster strips standing proud
    // over every pier, the brick order of a real hall.
    for side in [-1.0_f32, 1.0] {
        let slots: Vec<(f32, f32)> = (0..params.windows_per_side)
            .map(|slot| {
                let along = (slot as f32 + 0.5) / params.windows_per_side as f32 * 2.0 - 1.0;
                let jitter = rng.signed() * 0.05;
                ((along + jitter) * (params.half_depth * 0.78), half_w)
            })
            .collect();
        push_pierced_wall(
            &mut walls,
            &mut wall_indices,
            WallSpec {
                side,
                face,
                thickness: 0.2,
                span_lo: -leaf_depth,
                span_hi: leaf_depth,
                base: params.plinth_height,
                top: params.eaves_height,
                sill,
                head,
                band: WorldMaterial::PlinthStone,
            },
            &slots,
        );
        for &(z, hw) in &slots {
            push_window(
                &mut walls,
                &mut wall_indices,
                side,
                face,
                z,
                sill,
                head,
                hw,
                WorldMaterial::WindowGlass,
                WorldMaterial::PlinthStone,
            );
        }
        // Pilaster strips: one over each gap, plinth to eaves, 12 cm proud of the leaf.
        let mut edge = -leaf_depth;
        for &(z, hw) in &slots {
            let gap_mid = (edge + z - hw) * 0.5;
            let gap_half = (z - hw - edge) * 0.5;
            if gap_half > 0.3 {
                push_box(
                    &mut walls,
                    &mut wall_indices,
                    Vec3::new(
                        side * (face + 0.03),
                        (params.plinth_height + params.eaves_height) * 0.5,
                        gap_mid,
                    ),
                    Vec3::new(0.06, (params.eaves_height - params.plinth_height) * 0.5, 0.26),
                    WorldMaterial::Wall,
                );
            }
            edge = z + hw;
        }
        let last_mid = (edge + leaf_depth) * 0.5;
        if (leaf_depth - edge) * 0.5 > 0.3 {
            push_box(
                &mut walls,
                &mut wall_indices,
                Vec3::new(
                    side * (face + 0.03),
                    (params.plinth_height + params.eaves_height) * 0.5,
                    last_mid,
                ),
                Vec3::new(0.06, (params.eaves_height - params.plinth_height) * 0.5, 0.26),
                WorldMaterial::Wall,
            );
        }
    }
    // The gable ends: the -Z end is a plain leaf; the +Z end carries the wagon portal —
    // side piers, a stone lintel over the clear span, and the door leaf recessed behind it.
    let end_z = leaf_depth;
    let (door_w, door_h) = params.door_size;
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(0.0, (params.plinth_height + params.eaves_height) * 0.5, -(end_z - 0.1)),
        Vec3::new(face, (params.eaves_height - params.plinth_height) * 0.5, 0.1),
        WorldMaterial::Wall,
    );
    let door_half = door_w * 0.5;
    for door_side in [-1.0_f32, 1.0] {
        // The pier beside the opening, and its dressed-stone jamb standing slightly proud.
        let pier_half = (face - door_half) * 0.5;
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(
                door_side * (door_half + pier_half),
                (params.plinth_height + params.eaves_height) * 0.5,
                end_z - 0.1,
            ),
            Vec3::new(pier_half, (params.eaves_height - params.plinth_height) * 0.5, 0.1),
            WorldMaterial::Wall,
        );
        push_box(
            &mut walls,
            &mut wall_indices,
            Vec3::new(
                door_side * (door_half + 0.16),
                params.plinth_height + door_h * 0.5 + 0.08,
                end_z + 0.01,
            ),
            Vec3::new(0.16, door_h * 0.5 + 0.08, 0.08),
            WorldMaterial::PlinthStone,
        );
    }
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(
            0.0,
            params.plinth_height
                + door_h
                + (params.eaves_height - params.plinth_height - door_h) * 0.5,
            end_z - 0.1,
        ),
        Vec3::new(door_half, (params.eaves_height - params.plinth_height - door_h) * 0.5, 0.1),
        WorldMaterial::PlinthStone,
    );
    // The door leaf itself, recessed into the portal; a worker door on the +X wall under a
    // stone canopy near the corner.
    push_face(
        &mut walls,
        &mut wall_indices,
        [
            Vec3::new(-door_half + 0.05, params.plinth_height, end_z - 0.18),
            Vec3::new(door_half - 0.05, params.plinth_height, end_z - 0.18),
            Vec3::new(door_half - 0.05, params.plinth_height + door_h, end_z - 0.18),
            Vec3::new(-door_half + 0.05, params.plinth_height + door_h, end_z - 0.18),
        ],
        Vec3::Z,
        WorldMaterial::PlankDoor,
    );
    // Worker door + canopy: absolute joinery clamped into the box for sized bakes
    // (Immersja A1.2) — the canonical span never touches the clamps.
    let worker_h = 1.05_f32.min((params.eaves_height - params.plinth_height) * 0.5);
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(face + 0.03, params.plinth_height + worker_h, -params.half_depth * 0.72),
        Vec3::new(0.05, worker_h, 0.55_f32.min(params.half_depth * 0.25)),
        WorldMaterial::PlankDoor,
    );
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(
            face + 0.04,
            (params.plinth_height + 2.2).min(params.eaves_height - 0.06),
            -params.half_depth * 0.72,
        ),
        Vec3::new(0.05, 0.06, 0.75_f32.min(params.half_depth * 0.27)),
        WorldMaterial::PlinthStone,
    );

    // The roof story: the main gable stops short of the ridge cap, the glazed clerestory
    // band rides the ridge line with a steel-sash rhythm, and a flat slab caps it.
    let gable_top = (params.ridge_height - 1.0).max(params.eaves_height);
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
    let clerestory_half =
        Vec3::new(1.9_f32.min(params.half_width * 0.9), 0.45, params.half_depth * 0.72);
    push_box(
        &mut walls,
        &mut wall_indices,
        Vec3::new(
            0.0,
            (gable_top - 0.1 + clerestory_half.y).min(params.ridge_height - clerestory_half.y),
            0.0,
        ),
        clerestory_half,
        WorldMaterial::WindowGlass,
    );
    // The sash bars: dark joinery rhythm across both glazed faces of the band.
    for side in [-1.0_f32, 1.0] {
        let pane_x = side * (clerestory_half.x + 0.012);
        let bars = 9;
        for bar in 0..bars {
            let z = -clerestory_half.z + (bar as f32 + 0.5) / bars as f32 * clerestory_half.z * 2.0;
            push_face(
                &mut walls,
                &mut wall_indices,
                [
                    Vec3::new(pane_x, gable_top - 0.06, z - 0.035),
                    Vec3::new(pane_x, gable_top - 0.06, z + 0.035),
                    Vec3::new(
                        pane_x,
                        (gable_top - 0.18 + clerestory_half.y * 2.0).min(params.ridge_height),
                        z + 0.035,
                    ),
                    Vec3::new(
                        pane_x,
                        (gable_top - 0.18 + clerestory_half.y * 2.0).min(params.ridge_height),
                        z - 0.035,
                    ),
                ],
                Vec3::X * side,
                WorldMaterial::PlankDoor,
            );
        }
    }
    push_box(
        &mut roof,
        &mut roof_indices,
        Vec3::new(
            0.0,
            (gable_top - 0.1 + clerestory_half.y * 2.0 + 0.09).min(params.ridge_height - 0.1),
            0.0,
        ),
        Vec3::new(
            (clerestory_half.x + 0.25).min(params.half_width),
            0.1,
            (clerestory_half.z + 0.25).min(params.half_depth),
        ),
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
        // Sized bakes (Immersja A1.2) can hand this a box SMALLER than a slab's old floor
        // size — clamp every slab into the box; the canonical footprints never hit the
        // clamps, so the canonical goldens are untouched.
        let half = Vec3::new(
            (0.5 + rng.unit() * params.half_width * 0.45).min(params.half_width * 0.92),
            (0.18 + rng.unit() * 0.5 * ceiling).min(ceiling * 0.5),
            (0.5 + rng.unit() * params.half_depth * 0.45).min(params.half_depth * 0.92),
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
        let half = Vec3::new(
            (0.4 + rng.unit() * 0.9).min(params.half_width * 0.92),
            0.05,
            (0.5 + rng.unit() * 1.2).min(params.half_depth * 0.92),
        );
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

/// A single-sided material quad: corners in either winding, the indices are chosen to face
/// `normal`. Glass panes and joinery strips — the cheapest honest mark; the wall's depth
/// comes from the pierced-leaf boxes, not from thickness these marks do not need.
/// One four-corner face of a structure, indexed and flat-shaded. Named for the world it builds:
/// the HUD's `ui_kit::push_quad` is a 2D triangle pair with no indices and no normal, and two
/// unrelated helpers sharing one name is how an edit reaches the wrong one.
fn push_face(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    corners: [Vec3; 4],
    normal: Vec3,
    material: WorldMaterial,
) {
    let base = vertices.len() as u32;
    for corner in corners {
        vertices.push(GeometryVertex::new(
            corner,
            normal,
            material.carrier(),
            SmoothingGroup::hard_edges(),
        ));
    }
    if (corners[1] - corners[0]).cross(corners[3] - corners[0]).dot(normal) >= 0.0 {
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
}

/// The geometry of one pierced facade leaf (Fasada 2.0): an X-facing wall plane whose
/// openings are TRUE holes — the leaf is built around them, so the recessed pane behind
/// reads as depth, never as a painted rectangle.
struct WallSpec {
    /// Which X face the leaf stands on (-1 / +1).
    side: f32,
    /// The leaf's outer plane (|x|).
    face: f32,
    /// The leaf's thickness, reaching inward from `face`.
    thickness: f32,
    /// The leaf's run along Z (absolute facade coordinates — a doorway splits a facade
    /// into two runs that do not sit about the origin).
    span_lo: f32,
    span_hi: f32,
    /// The leaf's vertical span.
    base: f32,
    top: f32,
    /// The openings' vertical span within it.
    sill: f32,
    head: f32,
    /// The lintel band's material: dressed stone in the town, a timber bressumer in the
    /// village.
    band: WorldMaterial,
}

/// Build one pierced leaf: the apron below the sills, the lintel band above the heads (the
/// "pas nadproży" — one continuous band per row, not per window), and the piers between the
/// openings. `slots` are (z centre, half width) in absolute facade coordinates, sorted along
/// the run and strictly inside it.
fn push_pierced_wall(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    spec: WallSpec,
    slots: &[(f32, f32)],
) {
    let half_x = spec.thickness * 0.5;
    let cx = spec.side * (spec.face - half_x);
    let span_mid = (spec.span_lo + spec.span_hi) * 0.5;
    let span_half = (spec.span_hi - spec.span_lo) * 0.5;
    if spec.sill > spec.base {
        push_box(
            vertices,
            indices,
            Vec3::new(cx, (spec.base + spec.sill) * 0.5, span_mid),
            Vec3::new(half_x, (spec.sill - spec.base) * 0.5, span_half),
            WorldMaterial::Wall,
        );
    }
    if spec.top > spec.head {
        push_box(
            vertices,
            indices,
            Vec3::new(cx, (spec.head + spec.top) * 0.5, span_mid),
            Vec3::new(half_x, (spec.top - spec.head) * 0.5, span_half),
            spec.band,
        );
    }
    let mut pier = |edge: f32, next: f32| {
        let gap_half = (next - edge) * 0.5;
        push_box(
            vertices,
            indices,
            Vec3::new(cx, (spec.sill + spec.head) * 0.5, edge + gap_half),
            Vec3::new(half_x, (spec.head - spec.sill) * 0.5, gap_half),
            WorldMaterial::Wall,
        );
    };
    let mut edge = spec.span_lo;
    for &(z, half_w) in slots {
        if z - half_w > edge {
            pier(edge, z - half_w);
        }
        edge = z + half_w;
    }
    if edge < spec.span_hi {
        pier(edge, spec.span_hi);
    }
}

/// One honest window behind its opening: the pane recessed a hand into the leaf, a frame
/// (jambs and head — the sill ledge is the bottom rail) and a mullion cross on the pane, the
/// sill ledge standing proud of the face. All inside the recess the leaf leaves in the
/// footprint. `glazing` fills the opening (glass in the house, a plank shutter in the barn),
/// `trim` dresses it (dressed stone in the town, sawn timber in the village).
#[allow(clippy::too_many_arguments)]
fn push_window(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    side: f32,
    face: f32,
    z: f32,
    sill: f32,
    head: f32,
    half_w: f32,
    glazing: WorldMaterial,
    trim: WorldMaterial,
) {
    let pane_x = side * (face - 0.09);
    let out = Vec3::X * side;
    // The pane, a hand inside the leaf.
    push_face(
        vertices,
        indices,
        [
            Vec3::new(pane_x, sill, z - half_w),
            Vec3::new(pane_x, sill, z + half_w),
            Vec3::new(pane_x, head, z + half_w),
            Vec3::new(pane_x, head, z - half_w),
        ],
        out,
        glazing,
    );
    // The frame: two jambs and the head rail, on the pane and a finger proud of it.
    let frame_x = pane_x + side * 0.012;
    for jamb in [-1.0_f32, 1.0] {
        push_face(
            vertices,
            indices,
            [
                Vec3::new(frame_x, sill, z + jamb * (half_w - 0.05)),
                Vec3::new(frame_x, sill, z + jamb * half_w),
                Vec3::new(frame_x, head, z + jamb * half_w),
                Vec3::new(frame_x, head, z + jamb * (half_w - 0.05)),
            ],
            out,
            trim,
        );
    }
    push_face(
        vertices,
        indices,
        [
            Vec3::new(frame_x, head - 0.05, z - half_w),
            Vec3::new(frame_x, head - 0.05, z + half_w),
            Vec3::new(frame_x, head, z + half_w),
            Vec3::new(frame_x, head, z - half_w),
        ],
        out,
        trim,
    );
    // The mullion cross: one vertical, one horizontal at the meeting rail's height.
    push_face(
        vertices,
        indices,
        [
            Vec3::new(frame_x, sill, z - 0.018),
            Vec3::new(frame_x, sill, z + 0.018),
            Vec3::new(frame_x, head, z + 0.018),
            Vec3::new(frame_x, head, z - 0.018),
        ],
        out,
        trim,
    );
    let meeting = sill + (head - sill) * 0.55;
    push_face(
        vertices,
        indices,
        [
            Vec3::new(frame_x, meeting - 0.018, z - half_w),
            Vec3::new(frame_x, meeting - 0.018, z + half_w),
            Vec3::new(frame_x, meeting + 0.018, z + half_w),
            Vec3::new(frame_x, meeting + 0.018, z - half_w),
        ],
        out,
        trim,
    );
    // The sill ledge, a touch wider than the opening, proud of the face.
    push_box(
        vertices,
        indices,
        Vec3::new(side * (face + 0.02), sill - 0.025, z),
        Vec3::new(0.06, 0.045, half_w + 0.13),
        trim,
    );
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

    /// Per-style triangle budget (Fasada 2.0, Świat 2.0 PR 3/PR 4): the shared envelope
    /// stays 400 — a style raises ONLY its own number, with the frame measurement recorded
    /// in the PR that raised it. The two urban styles carry true openings, dressed-stone
    /// trim, sills and the cornice: ~1.3k (Tenement) and ~0.8k (FactoryHall), locked with
    /// headroom at 1500 against the Ostrogorsk frame numbers. PR 4 raised the two bigger
    /// village styles: Townhouse bakes 644 (two pierced storeys + the doorway runs), the
    /// Church 464 (pierced nave + the open bell stage); Cottage (320) and Barn (310) fit
    /// the shared envelope and keep it.
    fn triangle_budget(style: BuildingStyle) -> std::ops::RangeInclusive<usize> {
        match style {
            BuildingStyle::Tenement | BuildingStyle::FactoryHall => 30..=1500,
            BuildingStyle::Townhouse => 30..=800,
            BuildingStyle::Church => 30..=600,
            _ => 30..=400,
        }
    }

    #[test]
    fn styles_bake_deterministic_on_their_goldens_and_within_budget() {
        for (style, golden) in BUILDING_GOLDEN_HASHES {
            let first = bake_building(style, 0, StructureForm::Intact);
            let second = bake_building(style, 0, StructureForm::Intact);
            assert_eq!(first.deterministic_hash(), second.deterministic_hash());
            assert!(
                triangle_budget(style).contains(&first.triangle_count()),
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

    /// Fasada 2.0's whole point, locked: on the urban styles every pane sits INSIDE the
    /// pierced leaf — no glass stands proud of the wall plane (the painted-rectangle look
    /// this wave retired) — and the facade carries real dressed stone: sills, lintel bands,
    /// frames, lesenes.
    #[test]
    fn urban_glass_is_recessed_into_a_pierced_leaf() {
        for (style, half_width) in
            [(BuildingStyle::Tenement, 4.6_f32), (BuildingStyle::FactoryHall, 6.5)]
        {
            let building = bake_building(style, 0, StructureForm::Intact);
            let face = half_width - 0.1;
            let mut panes = 0usize;
            let mut stone = 0usize;
            for vertex in building.walls.vertices() {
                match WorldMaterial::from_carrier(vertex.material) {
                    WorldMaterial::WindowGlass => {
                        panes += 1;
                        assert!(
                            vertex.position.x.abs() <= face - 0.04,
                            "{style:?}: pane proud of the wall plane at {:?}",
                            vertex.position
                        );
                    }
                    WorldMaterial::PlinthStone => stone += 1,
                    _ => {}
                }
            }
            assert!(panes >= 40, "{style:?}: a facade carries its panes, got {panes} verts");
            assert!(stone >= 200, "{style:?}: the trim is real geometry, got {stone} verts");
        }
    }

    /// The village styles keep the same promise (Fasada 2.0, PR 4): every pane sits INSIDE
    /// its pierced leaf, and the barn — a working building — carries no glass at all, only
    /// plank shutters and doors behind true openings.
    #[test]
    fn village_glass_is_recessed_and_the_barn_has_none() {
        for (style, half_width, min_panes) in [
            (BuildingStyle::Cottage, 3.4_f32, 16usize),
            (BuildingStyle::Townhouse, 3.8, 44),
            (BuildingStyle::Church, 3.6, 24),
        ] {
            let building = bake_building(style, 0, StructureForm::Intact);
            let face = half_width - 0.1;
            let mut panes = 0usize;
            for vertex in building.walls.vertices() {
                if WorldMaterial::from_carrier(vertex.material) == WorldMaterial::WindowGlass {
                    panes += 1;
                    assert!(
                        vertex.position.x.abs() <= face - 0.04,
                        "{style:?}: pane proud of the wall plane at {:?}",
                        vertex.position
                    );
                }
            }
            assert!(panes >= min_panes, "{style:?}: a facade carries its panes, got {panes}");
        }
        let barn = bake_building(BuildingStyle::Barn, 0, StructureForm::Intact);
        let glass = barn
            .walls
            .vertices()
            .iter()
            .filter(|v| WorldMaterial::from_carrier(v.material) == WorldMaterial::WindowGlass)
            .count();
        assert_eq!(glass, 0, "a barn's openings take shutters, not glass");
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

    /// Immersja A1.1: openings are the only absolute numbers the eye can read scale from,
    /// so they carry REAL-WORLD minima — a person's door clears a person (>= 1.9 m for
    /// every inhabited style), the barn portal clears a loaded wagon, and dwelling windows
    /// are windows, not slits. The 2026-08-03 audit measured the whole table 30-45 % short;
    /// this lock keeps it from ever shrinking back into the maquette.
    #[test]
    fn opening_sizes_carry_real_world_minima() {
        let inhabited = [
            BuildingStyle::Cottage,
            BuildingStyle::Townhouse,
            BuildingStyle::Church,
            BuildingStyle::Windmill,
            BuildingStyle::Tenement,
            BuildingStyle::FactoryHall,
        ];
        for style in inhabited {
            let params = style.params();
            let (door_w, door_h) = params.door_size;
            assert!(door_h >= 1.9, "{style:?}: a person's door is {door_h} m tall");
            assert!(door_w >= 0.85, "{style:?}: a person's door is {door_w} m wide");
        }
        let barn = BuildingStyle::Barn.params();
        assert!(
            barn.door_size.0 >= 2.2 && barn.door_size.1 >= 2.4,
            "the barn portal must clear a loaded wagon, got {:?}",
            barn.door_size
        );
        for style in [BuildingStyle::Cottage, BuildingStyle::Townhouse, BuildingStyle::Tenement] {
            let (window_w, window_h) = style.params().window_size;
            assert!(
                window_w >= 0.8 && window_h >= 1.0,
                "{style:?}: a dwelling window is a window, not a slit — got {window_w}x{window_h}"
            );
        }
        // The bigger openings still live inside their walls: every door fits under the
        // eaves band it is cut into, so honesty never trades against realism.
        for style in BuildingStyle::ALL {
            let params = style.params();
            assert!(
                params.plinth_height + params.door_size.1 < params.eaves_height,
                "{style:?}: the door head must stay under the eaves"
            );
        }
    }

    /// Immersja A1.2, the wave's promise locked three ways: (1) baked AT SIZE, every vertex
    /// of every style, form and target box still lives inside that box — the honesty
    /// contract without the stretch that used to buy it; (2) the bake is deterministic per
    /// (style, seed, target); (3) a longer wall earns MORE windows while the window itself
    /// never grows, and the door never grows either — the eye's absolute yardsticks hold.
    #[test]
    fn sized_bakes_stay_honest_and_tile_openings_by_count() {
        let targets = [
            Vec3::new(1.4, 1.1, 1.8),  // a shed barely worth a door
            Vec3::new(3.4, 2.3, 4.6),  // canonical-ish village mass
            Vec3::new(5.5, 5.5, 8.0),  // an Ostrogorsk tenement box (pre-rotated)
            Vec3::new(6.0, 9.5, 6.0),  // the elevator head house
            Vec3::new(9.0, 6.0, 14.0), // the factory hall span (pre-rotated)
            Vec3::new(0.9, 1.6, 7.0),  // a sliver: long, thin, low
        ];
        for style in BuildingStyle::ALL {
            for &target in &targets {
                for form in [StructureForm::Intact, StructureForm::Rubble { height_frac: 0.3 }] {
                    let building = bake_building_sized(style, 11, form, target);
                    let again = bake_building_sized(style, 11, form, target);
                    assert_eq!(
                        building.deterministic_hash(),
                        again.deterministic_hash(),
                        "{style:?} {form:?} {target:?}: one building per (style, seed, target)"
                    );
                    let ceiling_frac = match form {
                        StructureForm::Intact => 1.0,
                        StructureForm::Rubble { height_frac } => height_frac,
                    };
                    let full_height = target.y * 2.0;
                    for mesh in [&building.walls, &building.roof] {
                        for vertex in mesh.vertices() {
                            let p = vertex.position;
                            assert!(
                                p.x.abs() <= target.x + 1.0e-4 && p.z.abs() <= target.z + 1.0e-4,
                                "{style:?} {form:?} {target:?}: vertex outside the box at {p:?}"
                            );
                            assert!(
                                p.y >= -1.0e-4 && p.y <= full_height * ceiling_frac + 1.0e-4,
                                "{style:?} {form:?} {target:?}: vertex above the ceiling at {p:?}"
                            );
                        }
                    }
                }
            }
        }

        // The tiling law, on the params the bake consumes: double the wall, same window,
        // same door, more windows.
        for style in [BuildingStyle::Tenement, BuildingStyle::Cottage, BuildingStyle::FactoryHall] {
            let narrow = sized_params(style, Vec3::new(4.6, 6.0, 6.0));
            let wide = sized_params(style, Vec3::new(4.6, 6.0, 12.0));
            assert_eq!(
                narrow.window_size, wide.window_size,
                "{style:?}: the window must NOT grow with the wall"
            );
            assert_eq!(
                narrow.door_size, wide.door_size,
                "{style:?}: the door must NOT grow with the wall"
            );
            assert!(
                wide.windows_per_side > narrow.windows_per_side,
                "{style:?}: the longer wall must earn more windows ({} vs {})",
                wide.windows_per_side,
                narrow.windows_per_side
            );
        }
    }

    /// Immersja A1.3: floors follow the target height at the civic ~3 m pitch — an 11 m
    /// tenement carries three storeys, the 19 m elevator head house five — and the pitch
    /// never leaves the band a real staircase would accept. The windows stay absolute
    /// through it all, and the canonical boxes reproduce their authored counts through
    /// the same formula (townhouse 2, tenement 3), which is why the canonical goldens
    /// never moved.
    #[test]
    fn storeys_follow_height_at_a_civic_pitch() {
        for (target, expected) in [
            (Vec3::new(5.5, 5.5, 8.0), 3_u32), // an Ostrogorsk tenement box
            (Vec3::new(6.0, 9.5, 6.0), 5),     // the elevator head house
            (Vec3::new(4.6, 6.0, 6.0), 3),     // the canonical tenement height
            (Vec3::new(4.6, 12.0, 6.0), 7),    // a hypothetical tower still walks stairs
        ] {
            let params = sized_params(BuildingStyle::Tenement, target);
            assert_eq!(params.storeys, expected, "{target:?}");
            let pitch = (params.eaves_height - params.plinth_height) / params.storeys as f32;
            assert!(
                (2.3..=3.9).contains(&pitch),
                "a staircase must accept the pitch, got {pitch} at {target:?}"
            );
            assert_eq!(
                params.window_size,
                BuildingStyle::Tenement.params().window_size,
                "the tall building's windows stay the human size at {target:?}"
            );
        }
        let town = sized_params(BuildingStyle::Townhouse, Vec3::new(3.8, 3.8, 5.2));
        assert_eq!(town.storeys, 2, "the canonical townhouse keeps its two floors");
    }

    /// The frame-budget fence for the sized path: the biggest half-extents actually
    /// authored on the four maps (pre-rotated as the scene bakes them). A style that
    /// outgrows its ceiling must bring a new frame measurement, not a bigger number.
    #[test]
    fn the_largest_authored_boxes_stay_inside_their_triangle_ceilings() {
        for (style, target, ceiling) in [
            (BuildingStyle::FactoryHall, Vec3::new(9.0, 6.0, 14.0), 2_600_usize),
            (BuildingStyle::Tenement, Vec3::new(5.5, 5.5, 8.0), 2_600),
            (BuildingStyle::Tenement, Vec3::new(6.0, 9.5, 6.0), 2_600),
            (BuildingStyle::Church, Vec3::new(6.5, 7.0, 5.0), 1_000),
        ] {
            let tris =
                bake_building_sized(style, 0, StructureForm::Intact, target).triangle_count();
            assert!(
                tris <= ceiling,
                "{style:?} at {target:?} bakes {tris} tris over its {ceiling} ceiling"
            );
        }
    }
}
