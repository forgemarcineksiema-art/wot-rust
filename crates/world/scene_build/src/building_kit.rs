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
use world_forge::building_kit::{BuildingPlan, KitPart, TintLane, plan_building};

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

/// The mesh handle of one part.
pub fn kit_mesh_handle(part: KitPart) -> MeshHandle {
    MeshHandle(BUILDING_MESH_BASE + part.index() as u32)
}

/// Whether a handle is one of the kit's.
pub fn is_kit_mesh(handle: MeshHandle) -> bool {
    (BUILDING_MESH_BASE..BUILDING_MESH_BASE + KitPart::all().len() as u32).contains(&handle.0)
}

/// Every part's mesh, ready to register once per renderer.
pub fn kit_meshes() -> Vec<(MeshHandle, MeshAsset)> {
    KitPart::all().into_iter().map(|part| (kit_mesh_handle(part), kit_mesh_asset(part))).collect()
}

/// One part as scene geometry: the material decodes to colour, gloss and surface role the
/// way the bake decodes it (`world_material::to_scene`), with the wall and the roof left WHITE
/// and tint-weighted so the instance tint paints them.
fn kit_mesh_asset(part: KitPart) -> MeshAsset {
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
    if !matches!(
        cover.kind,
        terrain::StaticCoverKind::FarmBuilding | terrain::StaticCoverKind::CityBuilding
    ) {
        return None;
    }
    let half = Vec3::from_array(cover.half_extents_m);
    let style = crate::battlefield::derived_building_style(&cover.id, half);
    plan_building(
        style,
        cover_seed(&cover.id).wrapping_add(salt.wrapping_mul(0x9E37_79B9_7F4A_7C15)),
        half,
    )
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
            let mut plan = kit_plan_for_cover(cover)?;
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
                    if let Some(rerolled) = kit_plan_for_cover_salted(cover, salt) {
                        plan = rerolled;
                    }
                    salt += 1;
                }
                planned.insert(cell, plan.signature);
            }
            let center = Vec3::from_array(cover.center);
            let half = Vec3::from_array(cover.half_extents_m);
            let (wall, roof, _) = crate::battlefield::building_palette(&cover.id);
            let age = plan.signature.age_tint();
            let wall = [wall[0] * age, wall[1] * age, wall[2] * age];
            let objects = plan
                .placements
                .iter()
                .map(|placement| RenderObject {
                    tank_id: None,
                    mesh: kit_mesh_handle(placement.part),
                    material: MaterialHandle(0),
                    transform: (Mat4::from_translation(center) * placement.transform)
                        .to_cols_array_2d(),
                    tint: match placement.tint {
                        TintLane::Wall => wall,
                        TintLane::Roof => roof,
                        TintLane::Absolute => [1.0, 1.0, 1.0],
                    },
                    dither: [0.0, 1.0],
                })
                .collect();
            Some(PlacedBuilding {
                cover: index,
                center,
                radius: half.length() + world_forge::building_kit::SCENERY_REACH_M,
                plan,
                objects,
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
    let mut objects = Vec::new();
    for building in cache.placed(battlefield) {
        if cover_states.get(building.cover).copied().unwrap_or(0) != 0 {
            continue;
        }
        if !eye.sees(building.center, building.radius) {
            continue;
        }
        objects.extend_from_slice(&building.objects);
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
    use world_forge::building_kit::{SCENERY_REACH_M, Signature};

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
                for object in &building.objects {
                    let mesh = &meshes[&object.mesh];
                    let transform = Mat4::from_cols_array_2d(&object.transform);
                    for vertex in mesh.vertices() {
                        let p = transform.transform_point3(Vec3::from_array(vertex.position))
                            - building.center;
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
        let fewer = building_frame_objects(
            &battlefield,
            &states,
            crate::tree_lod::TreeEye::at(Vec3::new(500.0, 3.0, 500.0)),
            &mut cache,
        );
        assert!(kit_object_count(&fewer) < count, "a collapsed house leaves the frame");
    }
}
