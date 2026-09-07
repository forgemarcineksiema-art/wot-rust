//! The building kit on the battlefield (the one program's B3): the dwellings are drawn as
//! INSTANCES of a small catalogue of parts, not as merged triangles in the static buffer.
//!
//! `world_forge::building_kit` plans a building from its collision box and its id's seed; this
//! module turns the plan into `RenderObject`s in world space — one per placed part, tinted by
//! the building's wall or roof tone — caches them per map, and submits the standing ones the
//! eye can see every frame through the same instanced path the tree ladder rides. The static
//! bake (`battlefield::append_building`) draws nothing for a kit-dressed building; the rubble
//! and the gone states keep their bakes.
//!
//! Mesh handles: the catalogue owns the block at [`BUILDING_MESH_BASE`], one handle per
//! `KitPart` in catalogue order, BELOW `SHADOWLESS_DRESSING_MESH_BASE` so every part casts in
//! both shadow cascades and takes the SSAO prepass.

use glam::{Mat4, Vec3};
use renderer_api::{MaterialHandle, MeshAsset, MeshHandle, RenderObject, SceneVertex};
use world_forge::WorldMaterial;
use world_forge::building_kit::{
    BuildingPlan, Cladding, KitPart, Placement, TintLane, collapse_placements, plan_building_with,
    plan_ruin,
};

/// The base of the kit's mesh-handle block. Below the tree ladder's block and the shadowless
/// base: a house must cast a shadow.
pub const BUILDING_MESH_BASE: u32 = 0xFED0_0000;
const _: () = assert!(BUILDING_MESH_BASE + 0x1000 < 0xFEE0_0000);
const _: () = assert!(BUILDING_MESH_BASE < renderer_api::SHADOWLESS_DRESSING_MESH_BASE);

/// The stone tone the kit's trim (plinths, bands, sills, chimneys) wears. One tone for the
/// kit today: the trim shares an instance with the wall it sits in, and an instance has one
/// tint — the wall's. Per-building stone returns with B4's coursing, as a surface term.
const KIT_STONE: ([f32; 3], f32) = ([0.45, 0.42, 0.36], 0.17);
/// The roof's gloss lane; the roof TONE is the instance tint.
const KIT_ROOF_GLOSS: f32 = 0.25;

/// The mesh handle of one part in one cladding (B4): the catalogue twice over, plaster then
/// brick — a wall vertex's surface role is baked into the mesh.
pub fn kit_mesh_handle(part: KitPart, cladding: Cladding) -> MeshHandle {
    let block = KitPart::all().len() as u32;
    let cladding_offset = match cladding {
        Cladding::Plaster => 0,
        Cladding::Brick => block,
    };
    MeshHandle(BUILDING_MESH_BASE + cladding_offset + part.index() as u32)
}

/// Whether a handle is one of the kit's.
pub fn is_kit_mesh(handle: MeshHandle) -> bool {
    let block = KitPart::all().len() as u32 * Cladding::ALL.len() as u32;
    (BUILDING_MESH_BASE..BUILDING_MESH_BASE + block).contains(&handle.0)
}

/// Every part's mesh in every cladding, ready to register once per renderer.
pub fn kit_meshes() -> Vec<(MeshHandle, MeshAsset)> {
    let mut meshes = Vec::new();
    for cladding in Cladding::ALL {
        for part in KitPart::all() {
            meshes.push((kit_mesh_handle(part, cladding), kit_mesh_asset(part, cladding)));
        }
    }
    meshes
}

/// The wall tones a brick dwelling wears (its instance tint): red, brown, yellow brick.
const BRICK_TONES: [[f32; 3]; 3] = [[0.52, 0.34, 0.26], [0.44, 0.30, 0.24], [0.58, 0.48, 0.32]];

/// One part as scene geometry: the material decodes to colour, gloss and surface role the
/// way the bake decodes it (`world_material::to_scene`), with the wall and the roof left WHITE
/// and tint-weighted so the instance tint paints them.
fn kit_mesh_asset(part: KitPart, cladding: Cladding) -> MeshAsset {
    let mesh = part.mesh();
    let palette = crate::world_material::Palette {
        wall: [1.0, 1.0, 1.0],
        roof: [1.0, 1.0, 1.0],
        roof_gloss: KIT_ROOF_GLOSS,
        stone: KIT_STONE,
    };
    let vertices = mesh
        .vertices()
        .iter()
        .map(|vertex| {
            let material = WorldMaterial::from_carrier(vertex.material);
            let (color, gloss, role) = crate::world_material::to_scene(material, palette);
            // B4: the cladding decides a wall vertex's role; the reveals carry their baked
            // shade in the colour (the instance tint multiplies it).
            let role = if material == WorldMaterial::Wall && cladding == Cladding::Brick {
                renderer_api::surface_role::BRICK
            } else {
                role
            };
            let shade = vertex.surface_shade;
            let color = [color[0] * shade, color[1] * shade, color[2] * shade];
            let mut scene = SceneVertex::surfaced(
                vertex.position.to_array(),
                vertex.normal.normalize_or_zero().to_array(),
                color,
                gloss,
            )
            .with_surface(role);
            scene.tint_weight = if matches!(material, WorldMaterial::Wall | WorldMaterial::Roof) {
                1.0
            } else {
                0.0
            };
            scene
        })
        .collect();
    MeshAsset::new(vertices, mesh.indices().to_vec())
}

/// The seed a building's id names — the same FNV walk the bake and the palettes use.
pub fn cover_seed(id: &str) -> u64 {
    let mut seed = 0xcbf2_9ce4_8422_2325_u64;
    for byte in id.bytes() {
        seed ^= u64::from(byte);
        seed = seed.wrapping_mul(0x0100_0000_01b3);
    }
    seed
}

/// The kit's plan for a cover box, if the kit dresses it (a dwelling whose box carries a
/// plan); the landmarks and any box the grammar cannot fit stay on the authored bake.
pub fn kit_plan_for_cover(cover: &terrain::StaticCoverObject) -> Option<BuildingPlan> {
    kit_plan_for_cover_salted(cover, 0)
}

/// [`kit_plan_for_cover`] with a salt on the seed: the street planner re-rolls a house
/// whose whole signature its neighbour already wears (see [`place_buildings`]).
pub fn kit_plan_for_cover_salted(
    cover: &terrain::StaticCoverObject,
    salt: u64,
) -> Option<BuildingPlan> {
    kit_plan_for_cover_blind(cover, salt, [false; 4])
}

/// [`kit_plan_for_cover_salted`] with the party walls the map gives this box (B6).
pub fn kit_plan_for_cover_blind(
    cover: &terrain::StaticCoverObject,
    salt: u64,
    blind: [bool; 4],
) -> Option<BuildingPlan> {
    if !matches!(
        cover.kind,
        terrain::StaticCoverKind::FarmBuilding | terrain::StaticCoverKind::CityBuilding
    ) {
        return None;
    }
    let half = Vec3::from_array(cover.half_extents_m);
    let style = crate::battlefield::derived_building_style(&cover.id, half);
    plan_building_with(
        style,
        cover_seed(&cover.id).wrapping_add(salt.wrapping_mul(0x9E37_79B9_7F4A_7C15)),
        half,
        blind,
    )
}

/// Which facades of `cover` (+X, −X, +Z, −Z) touch another dwelling's box — a party wall
/// (B6): the two boxes share the plane within 6 cm and overlap along it by a metre.
pub fn party_walls(
    cover: &terrain::StaticCoverObject,
    all: &[terrain::StaticCoverObject],
) -> [bool; 4] {
    let dwelling = |c: &terrain::StaticCoverObject| {
        matches!(
            c.kind,
            terrain::StaticCoverKind::FarmBuilding | terrain::StaticCoverKind::CityBuilding
        )
    };
    let mut blind = [false; 4];
    if !dwelling(cover) {
        return blind;
    }
    let h = Vec3::from_array(cover.half_extents_m);
    // X1: neighbours are read in this box's own frame; a neighbour turned differently shares
    // no plane with it.
    let frame = terrain::CoverBox::of(cover);
    for other in
        all.iter().filter(|o| dwelling(o) && o.id != cover.id && o.yaw_rad == cover.yaw_rad)
    {
        let oh = Vec3::from_array(other.half_extents_m);
        let local = frame.to_local(other.center);
        let dx = local[0];
        let dz = local[2];
        let along_z_overlap = (h.z + oh.z) - dz.abs() >= 1.0;
        let along_x_overlap = (h.x + oh.x) - dx.abs() >= 1.0;
        if (dx.abs() - (h.x + oh.x)).abs() <= 0.06 && along_z_overlap {
            blind[if dx > 0.0 { 0 } else { 1 }] = true;
        }
        if (dz.abs() - (h.z + oh.z)).abs() <= 0.06 && along_x_overlap {
            blind[if dz > 0.0 { 2 } else { 3 }] = true;
        }
    }
    blind
}

/// The grid cell a `TownGrid` id names — (column, row, south side) — or `None` for an
/// authored box.
pub fn grid_cell(id: &str) -> Option<(i32, i32, bool)> {
    let c = id.find("_c")?;
    let r = id[c..].find("_r")? + c;
    let col: i32 = id[c + 2..r].parse().ok()?;
    let mut tail = id[r + 2..].split('_');
    let row: i32 = tail.next()?.parse().ok()?;
    let south = tail.next()? == "south";
    Some((col, row, south))
}

/// Whether the kit dresses this box (so the static bake draws nothing for it standing).
pub fn kit_dresses(cover: &terrain::StaticCoverObject) -> bool {
    kit_plan_for_cover(cover).is_some()
}

/// One building's objects in world space, ready to submit.
#[derive(Debug, Clone)]
pub struct PlacedBuilding {
    /// Index into `static_cover`.
    pub cover: usize,
    pub center: Vec3,
    /// The box's bounding-sphere radius (plus the scenery reach), for the eye's cone.
    pub radius: f32,
    pub plan: BuildingPlan,
    pub objects: Vec<RenderObject>,
    /// B5: the ruin's objects, drawn in phase 1 over the static bake's mound.
    pub ruin_objects: Vec<RenderObject>,
    /// The tints the objects were built with — the collapse (Z10) builds its pieces the same.
    pub wall_tint: [f32; 3],
    pub roof_tint: [f32; 3],
    /// X1: the box's yaw the objects were turned by.
    pub yaw_rad: f32,
}

/// One kit placement as the frame draws it, in world space.
fn kit_object(
    placement: &Placement,
    center: Vec3,
    yaw_rad: f32,
    wall: [f32; 3],
    roof: [f32; 3],
    cladding: Cladding,
) -> RenderObject {
    // X1: the plan is laid in the box's own frame and turned with it.
    let placed = if yaw_rad == 0.0 {
        Mat4::from_translation(center) * placement.transform
    } else {
        Mat4::from_translation(center) * Mat4::from_rotation_y(yaw_rad) * placement.transform
    };
    RenderObject {
        tank_id: None,
        mesh: kit_mesh_handle(placement.part, cladding),
        material: MaterialHandle(0),
        transform: placed.to_cols_array_2d(),
        tint: match placement.tint {
            TintLane::Wall => wall,
            TintLane::Roof => roof,
            TintLane::Absolute => [1.0, 1.0, 1.0],
        },
        dither: [0.0, 1.0],
    }
}

/// A kit building coming down (Z10): its cover index and how far through the fall it is
/// (0 standing, 1 the ruin).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildingCollapse {
    pub cover: usize,
    pub progress: f32,
}

/// The per-map cache of placed buildings.
#[derive(Debug, Clone, Default)]
pub struct KitCache {
    map: Option<(String, Vec<PlacedBuilding>)>,
}

impl KitCache {
    /// The placed buildings of `battlefield`, built once per map.
    pub fn placed(&mut self, battlefield: &terrain::BattlefieldMap) -> &[PlacedBuilding] {
        if self.map.as_ref().is_none_or(|(id, _)| *id != battlefield.id) {
            self.map = Some((battlefield.id.clone(), place_buildings(battlefield)));
        }
        &self.map.as_ref().expect("built above").1
    }
}

/// Every kit-dressed building of the map, placed. The STREET planner: a grid house whose
/// whole signature a neighbour (a column or a row apart, the same side) already wears is
/// re-rolled with a salt, up to a few times — a street is not one house repeated, and the
/// seed alone collides on a grid of forty-eight.
pub fn place_buildings(battlefield: &terrain::BattlefieldMap) -> Vec<PlacedBuilding> {
    let mut planned: std::collections::HashMap<
        (i32, i32, bool),
        world_forge::building_kit::Signature,
    > = Default::default();
    battlefield
        .static_cover
        .iter()
        .enumerate()
        .filter_map(|(index, cover)| {
            let blind = party_walls(cover, &battlefield.static_cover);
            let mut plan = kit_plan_for_cover_blind(cover, 0, blind)?;
            if let Some(cell) = grid_cell(&cover.id) {
                let neighbours = [
                    (cell.0 - 1, cell.1, cell.2),
                    (cell.0 + 1, cell.1, cell.2),
                    (cell.0, cell.1 - 1, cell.2),
                    (cell.0, cell.1 + 1, cell.2),
                ];
                let mut salt = 1;
                while neighbours.iter().any(|n| planned.get(n) == Some(&plan.signature))
                    && salt <= 6
                {
                    if let Some(rerolled) = kit_plan_for_cover_blind(cover, salt, blind) {
                        plan = rerolled;
                    }
                    salt += 1;
                }
                planned.insert(cell, plan.signature);
            }
            let center = Vec3::from_array(cover.center);
            let half = Vec3::from_array(cover.half_extents_m);
            let (wall, roof, _) = crate::battlefield::building_palette(&cover.id);
            let wall = match plan.signature.cladding {
                Cladding::Plaster => wall,
                Cladding::Brick => BRICK_TONES[(cover_seed(&cover.id) >> 16) as usize % 3],
            };
            let age = plan.signature.age_tint();
            let wall = [wall[0] * age, wall[1] * age, wall[2] * age];
            let cladding = plan.signature.cladding;
            let yaw_rad = cover.yaw_rad;
            let to_object = |placement: &Placement| {
                kit_object(placement, center, yaw_rad, wall, roof, cladding)
            };
            let objects = plan.placements.iter().map(to_object).collect();
            // B5: the ruin, under the sim's rubble height for this kind of box.
            let ceiling = half.y * 2.0 * cover.kind.rubble_height_frac();
            let style = crate::battlefield::derived_building_style(&cover.id, half);
            let ruin_objects = plan_ruin(style, cover_seed(&cover.id), half, ceiling)
                .map(|placements| placements.iter().map(to_object).collect())
                .unwrap_or_default();
            Some(PlacedBuilding {
                cover: index,
                center,
                radius: half.length() + world_forge::building_kit::SCENERY_REACH_M,
                plan,
                objects,
                ruin_objects,
                wall_tint: wall,
                roof_tint: roof,
                yaw_rad: cover.yaw_rad,
            })
        })
        .collect()
}

/// The frame's building objects: every standing kit building the eye can see. A building
/// whose box is in phase 1 (rubble) or 2 (gone) draws nothing here — the static bake carries
/// its mound.
pub fn building_frame_objects(
    battlefield: &terrain::BattlefieldMap,
    cover_states: &[u8],
    eye: crate::tree_lod::TreeEye,
    cache: &mut KitCache,
) -> Vec<RenderObject> {
    building_frame_objects_collapsing(battlefield, cover_states, &[], eye, cache)
}

/// [`building_frame_objects`] with the collapses in progress (Z10): a building in phase 1
/// whose fall is still running draws the ruin AND the standing pieces coming down over it —
/// `collapse_placements` at the fall's progress; the fall over, the ruin alone, byte for
/// byte what a plain phase-1 frame draws.
pub fn building_frame_objects_collapsing(
    battlefield: &terrain::BattlefieldMap,
    cover_states: &[u8],
    collapses: &[BuildingCollapse],
    eye: crate::tree_lod::TreeEye,
    cache: &mut KitCache,
) -> Vec<RenderObject> {
    let mut objects = Vec::new();
    for building in cache.placed(battlefield) {
        if !eye.sees(building.center, building.radius) {
            continue;
        }
        match cover_states.get(building.cover).copied().unwrap_or(0) {
            0 => objects.extend_from_slice(&building.objects),
            // B5: the ruin with form over the bake's mound — and, Z10, the walls still coming
            // down over it while the fall runs.
            1 => {
                objects.extend_from_slice(&building.ruin_objects);
                let falling = collapses
                    .iter()
                    .find(|collapse| collapse.cover == building.cover && collapse.progress < 1.0);
                if let Some(collapse) = falling {
                    let cover = &battlefield.static_cover[building.cover];
                    let half = Vec3::from_array(cover.half_extents_m);
                    let cladding = building.plan.signature.cladding;
                    let pieces = collapse_placements(
                        &building.plan,
                        cover_seed(&cover.id),
                        half,
                        collapse.progress,
                    );
                    objects.extend(pieces.iter().map(|piece| {
                        kit_object(
                            piece,
                            building.center,
                            building.yaw_rad,
                            building.wall_tint,
                            building.roof_tint,
                            cladding,
                        )
                    }));
                }
            }
            _ => {}
        }
    }
    objects
}

/// How many kit objects the frame carries.
pub fn kit_object_count(objects: &[RenderObject]) -> usize {
    objects.iter().filter(|object| is_kit_mesh(object.mesh)).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use world_forge::building_kit::{SCENERY_REACH_M, Signature, plan_building};

    const SHIPPED: [terrain::MapId; 4] = [
        terrain::MapId::ProkhorovkaHill252_2,
        terrain::MapId::BystraValley,
        terrain::MapId::Ostrogorsk,
        terrain::MapId::OrlinyPereval,
    ];

    /// The doctrine, on every shipped map: a placed part's vertices stay inside the box
    /// they dress, except the scenery-class reach past a face (never above the top).
    #[test]
    fn every_placed_part_stays_inside_its_box_on_every_shipped_map() {
        let meshes: std::collections::HashMap<MeshHandle, MeshAsset> =
            kit_meshes().into_iter().collect();
        for map in SHIPPED {
            let battlefield = map_forge::battlefield(map);
            let placed = place_buildings(&battlefield);
            assert!(!placed.is_empty(), "{map:?}: the kit dresses no building");
            for building in &placed {
                let cover = &battlefield.static_cover[building.cover];
                let half = Vec3::from_array(cover.half_extents_m);
                // X2: a turned box is measured in its own frame.
                let frame = terrain::CoverBox::of(cover);
                for object in &building.objects {
                    let mesh = &meshes[&object.mesh];
                    let transform = Mat4::from_cols_array_2d(&object.transform);
                    for vertex in mesh.vertices() {
                        let world = transform.transform_point3(Vec3::from_array(vertex.position));
                        let p = Vec3::from_array(frame.to_local(world.to_array()));
                        let over = (p.abs() - half).max(Vec3::ZERO);
                        assert!(
                            over.x <= SCENERY_REACH_M + 1e-3 && over.z <= SCENERY_REACH_M + 1e-3,
                            "{}: a part reaches {over} past its box",
                            cover.id
                        );
                        assert!(
                            over.y <= 1e-3,
                            "{}: a part rises {} over its box",
                            cover.id,
                            over.y
                        );
                    }
                }
            }
        }
    }

    /// B6's number: a street is not one house repeated. On the Bystra and Ostrogorsk grids
    /// no two neighbours (a column or a row apart) share a whole signature, and no signature
    /// owns more than a third of a grid.
    #[test]
    fn grid_neighbours_never_share_a_whole_signature() {
        for map in [terrain::MapId::BystraValley, terrain::MapId::Ostrogorsk] {
            let battlefield = map_forge::battlefield(map);
            let placed = place_buildings(&battlefield);
            let grid: Vec<((i32, i32, bool), &str, Signature)> = placed
                .iter()
                .filter_map(|b| {
                    let cover = &battlefield.static_cover[b.cover];
                    grid_cell(&cover.id).map(|cell| (cell, cover.id.as_str(), b.plan.signature))
                })
                .collect();
            assert!(grid.len() >= 8, "{map:?}: a grid to judge ({})", grid.len());
            for (i, (cell, id, signature)) in grid.iter().enumerate() {
                for (other_cell, other_id, other) in grid.iter().skip(i + 1) {
                    let adjacent = cell.2 == other_cell.2
                        && (cell.0 - other_cell.0).abs() + (cell.1 - other_cell.1).abs() == 1;
                    if adjacent && signature == other {
                        panic!("{map:?}: neighbours {id} and {other_id} are the same house");
                    }
                }
            }
            let mut histogram: std::collections::HashMap<Signature, usize> = Default::default();
            for (_, _, signature) in &grid {
                *histogram.entry(*signature).or_default() += 1;
            }
            let (top, count) =
                histogram.iter().max_by_key(|(_, count)| **count).expect("non-empty");
            assert!(
                *count as f32 <= grid.len() as f32 * 0.34,
                "{map:?}: {top:?} owns {count} of {} houses",
                grid.len()
            );
            let roofs: std::collections::HashSet<_> =
                grid.iter().map(|(_, _, s)| (s.pitch, s.ridge_across)).collect();
            let storeys: std::collections::HashSet<_> =
                grid.iter().map(|(_, _, s)| s.storeys).collect();
            assert!(
                roofs.len() >= 3 && storeys.len() >= 2,
                "{map:?}: roofs {roofs:?} storeys {storeys:?}"
            );
            // B1: the roofs' MASSING varies too — a street of gables is a street of one roof.
            let forms: std::collections::HashSet<_> = grid.iter().map(|(_, _, s)| s.roof).collect();
            assert!(forms.len() >= 2, "{map:?}: roof forms {forms:?}");
            // B6: a street is not detached rectangles — a fifth of the grid houses at least
            // are L-shapes with an annex behind (data: the grid's `annex_share`), and every
            // such pair shares a blind party wall.
            let attached = grid.iter().filter(|(_, _, s)| s.attached).count();
            assert!(
                attached * 5 >= grid.len(),
                "{map:?}: {attached} attached houses of {}",
                grid.len()
            );
            // B4: a street is not one cladding either — brick among the plaster.
            let brick = grid.iter().filter(|(_, _, s)| s.cladding == Cladding::Brick).count();
            assert!(
                brick * 8 >= grid.len() && brick * 2 <= grid.len(),
                "{map:?}: {brick} brick houses of {}",
                grid.len()
            );
        }
    }

    /// The kit reads as buildings: a town house carries windows and a door, a barn a portal,
    /// and the parts name their surfaces down the lane.
    #[test]
    fn kit_buildings_carry_windows_doors_portals_and_named_surfaces() {
        use renderer_api::surface_role;
        let bystra = map_forge::battlefield(terrain::MapId::BystraValley);
        let house = bystra
            .static_cover
            .iter()
            .find(|c| c.id.starts_with("town_house"))
            .expect("Bystra's grid");
        let plan = kit_plan_for_cover(house).expect("a kit house");
        let windows =
            plan.placements.iter().filter(|p| matches!(p.part, KitPart::WindowBay { .. })).count();
        let doors =
            plan.placements.iter().filter(|p| matches!(p.part, KitPart::DoorBay { .. })).count();
        assert!(windows >= 4 && doors == 1, "windows {windows} doors {doors}");
        // A barn: no shipped box derives the Barn style today (the farm boxes are 3.5 m
        // tall and derive Townhouse), so the portal is read off a barn-proportioned box.
        let plan = plan_building(
            world_forge::building::BuildingStyle::Barn,
            cover_seed("some_barn"),
            Vec3::new(8.0, 2.8, 4.4),
        )
        .expect("a kit barn");
        assert!(
            plan.placements.iter().any(|p| p.part == KitPart::PortalBay),
            "the barn has its portal"
        );
        let mut roles = std::collections::HashSet::new();
        for (_, mesh) in kit_meshes() {
            for vertex in mesh.vertices() {
                roles.insert((vertex.surface * 100.0).round() as i32);
                if vertex.color == crate::world_material::WINDOW.0 {
                    assert_eq!(vertex.surface, surface_role::LEGACY, "glass takes no treatment");
                    assert_eq!(vertex.tint_weight, 0.0, "glass is never tinted");
                }
            }
        }
        for role in [
            surface_role::PLASTER,
            surface_role::BRICK,
            surface_role::SLATE,
            surface_role::PLANK,
            surface_role::DRESSED_STONE,
        ] {
            assert!(roles.contains(&((role * 100.0).round() as i32)), "role {role} is on the kit");
        }
    }

    /// The instance budget the kit spends on the densest town, recorded: the objects the
    /// whole of Ostrogorsk submits with nothing culled. The town's static buffer is the
    /// lighter for it (`battlefield` locks that count).
    #[test]
    fn the_densest_town_stays_inside_the_kit_instance_budget() {
        const OSTROGORSK_KIT_OBJECT_CEILING: usize = 9_000;
        let battlefield = map_forge::battlefield(terrain::MapId::Ostrogorsk);
        let mut cache = KitCache::default();
        let objects = building_frame_objects(
            &battlefield,
            &[],
            crate::tree_lod::TreeEye::at(Vec3::new(500.0, 3.0, 500.0)),
            &mut cache,
        );
        let count = kit_object_count(&objects);
        println!(
            "OSTROGORSK KIT: {count} objects over {} buildings",
            cache.placed(&battlefield).len()
        );
        assert!(count > 0 && count <= OSTROGORSK_KIT_OBJECT_CEILING, "{count} kit objects");
        // A rubble box draws nothing from the kit.
        let mut states = vec![0u8; battlefield.static_cover.len()];
        let first = cache.placed(&battlefield)[0].cover;
        states[first] = 1;
        // Z10: while the fall runs the ruin AND the falling pieces draw; at 0 the pieces are
        // the standing house; the fall over, the frame is the plain phase-1 frame byte for byte.
        let eye = crate::tree_lod::TreeEye::at(Vec3::new(500.0, 3.0, 500.0));
        let ruin_alone = building_frame_objects(&battlefield, &states, eye, &mut cache);
        let at = |progress: f32, cache: &mut KitCache| {
            building_frame_objects_collapsing(
                &battlefield,
                &states,
                &[BuildingCollapse { cover: first, progress }],
                eye,
                cache,
            )
        };
        let standing_count = cache.placed(&battlefield)[0].objects.len();
        let ruin_count = cache.placed(&battlefield)[0].ruin_objects.len();
        let falling = at(0.5, &mut cache);
        assert_eq!(falling.len(), ruin_alone.len() + standing_count, "the ruin and the pieces");
        assert!(ruin_count > 0 && standing_count > 0);
        let start = at(0.0, &mut cache);
        let standing = &cache.placed(&battlefield)[0].objects;
        assert!(
            standing.iter().all(|object| start.contains(object)),
            "at 0 the standing house is what falls"
        );
        assert_eq!(at(1.0, &mut cache), ruin_alone, "the fall over: the ruin, byte for byte");
        let fewer = building_frame_objects(
            &battlefield,
            &states,
            crate::tree_lod::TreeEye::at(Vec3::new(500.0, 3.0, 500.0)),
            &mut cache,
        );
        assert!(kit_object_count(&fewer) < count, "a collapsed house leaves the frame");
        // ...and its ruin (B5) takes its place, under the mound's top and inside the box.
        let placed = cache.placed(&battlefield);
        let building = &placed[0];
        assert!(!building.ruin_objects.is_empty(), "the ruin stands");
        let meshes: std::collections::HashMap<MeshHandle, MeshAsset> =
            kit_meshes().into_iter().collect();
        let cover = &battlefield.static_cover[building.cover];
        let half = Vec3::from_array(cover.half_extents_m);
        let ceiling = half.y * 2.0 * cover.kind.rubble_height_frac();
        for object in &building.ruin_objects {
            let transform = Mat4::from_cols_array_2d(&object.transform);
            for vertex in meshes[&object.mesh].vertices() {
                let p =
                    transform.transform_point3(Vec3::from_array(vertex.position)) - building.center;
                assert!(
                    p.y + half.y <= ceiling + 1e-3,
                    "{}: the ruin rises over the mound",
                    cover.id
                );
                let over = (p.abs() - half).max(Vec3::ZERO);
                assert!(over.x <= 0.41 && over.z <= 0.41, "{}: the ruin leaves its box", cover.id);
            }
        }
    }
}
