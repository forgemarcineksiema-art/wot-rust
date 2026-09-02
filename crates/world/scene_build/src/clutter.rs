//! The world's small change: everything scattered over a map that is NOT a plant.
//!
//! Skały 1.0 (Świat 2.0 PR 8) split this out of `foliage`, which had grown to hold a lamppost
//! and a masonry spill under a module named after leaves. What lives here is the scenery
//! DISPATCH plus the non-vegetal kinds; `foliage` keeps the species bake it is named for.
//!
//! Two things changed with the split, both of them about stone:
//!
//! - a rock is baked by `world_forge::rock` instead of being one rotated cuboid (D6), and
//! - it is coloured by the MAP, not by a constant here. The rock/chalk splat layer of a map's
//!   ground palette is the one place that answers "what stone is this map made of", so a boulder
//!   now matches the cliff the terrain already paints behind it. Four maps used to share one
//!   grey — and that grey was Bystra's.

use glam::{Mat3, Vec3};
use renderer_api::{SceneVertex, surface_role};
use terrain::{BattlefieldMap, SceneryInstance, SceneryKind};

use crate::foliage::{push_frustum, push_impostor_tree};
use crate::tank_mesh::push_oriented_box;

/// The stone a map is built from: linear albedo plus the specular lane, straight off its ground
/// palette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoneTone {
    pub albedo: [f32; 3],
    pub gloss: f32,
}

impl StoneTone {
    /// The fallback for a map with no authored palette (test fixtures, the scratch blueprint).
    /// Deliberately the tone every rock used to wear, so an unpalletted map does not move.
    pub const NEUTRAL: StoneTone = StoneTone { albedo: [0.42, 0.40, 0.37], gloss: 0.16 };

    /// The stone THIS map is made of — splat channel A (rock/chalk) of its ground palette.
    /// Shipped maps today: Orliny pale granite, Bystra warm river stone, Ostrogorsk cool
    /// rubble-stone, Prokhorovka chalk break.
    pub fn of_map(battlefield: &BattlefieldMap) -> Self {
        let Some(blueprint) = map_forge::cached_blueprint_by_id(&battlefield.id) else {
            return Self::NEUTRAL;
        };
        let Some(materials) = blueprint.materials else {
            return Self::NEUTRAL;
        };
        let rock = materials.layers[ROCK_SPLAT_LAYER];
        Self { albedo: rock.albedo, gloss: rock.gloss }
    }
}

/// Splat channel order is `R = lush grass, G = dry straw, B = worn dirt, A = rock/chalk`
/// (`map_forge::blueprint::GroundMaterialsSpec`). The stone is the last one.
const ROCK_SPLAT_LAYER: usize = 3;

/// Bake one scenery instance into the statics mesh at full (near) detail.
pub fn push_scenery_instance(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
    stone: StoneTone,
) {
    match instance.kind {
        // Every planted TREE species left the statics bake (the oak in hero-flora phase 2,
        // the rest in Inny Poziom F7): they draw from the instanced mesh path with runtime LOD
        // (`crate::tree_lod`), so baking them here too would draw every tree twice — once at
        // full detail regardless of distance, which is the cost the ladder exists to end. The
        // arm and `tree_lod::ladder_species` are held to the same answer by
        // `the_near_bake_skips_exactly_the_ladder_species`.
        SceneryKind::Oak
        | SceneryKind::Poplar
        | SceneryKind::Willow
        | SceneryKind::FruitTree
        | SceneryKind::Pine
        | SceneryKind::Bush => {}
        // Retired imported kinds (procedural-only decision, Świat 2.0) bake to NOTHING: falling
        // through to the painted far frusta would resurrect the retired silhouette at close
        // range.
        SceneryKind::FloraTree | SceneryKind::FloraPine | SceneryKind::FloraBush => {}
        SceneryKind::Rock => {
            push_rock(vertices, indices, instance, stone, world_forge::rock::RockForm::Erratic)
        }
        SceneryKind::DebrisHeap => push_debris_heap(vertices, indices, instance, stone),
        SceneryKind::Lamppost => push_scenery_instance_far(vertices, indices, instance, stone),
    }
}

/// The masonry spill, transformed and dressed. Its vertices carry the world's OWN material
/// vocabulary, decoded through the single function the building facades use — so the plaster,
/// the tile and the timber wear the same treatments here as they do on the wall they fell off.
fn push_debris_heap(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
    stone: StoneTone,
) {
    let base = Vec3::from_array(instance.position);
    let seed =
        instance.position[0].to_bits() as u64 ^ ((instance.position[2].to_bits() as u64) << 32);
    let spill = world_forge::spill::bake_debris_spill(seed);
    let rotation = Mat3::from_rotation_y(instance.yaw_rad);
    // Debris has no per-building palette to inherit — it is authored street dressing, not the
    // wreck of a named house — so walls and roofs take the material's own PBR-lite defaults.
    // The dressed stone alone is the MAP's, so a broken sill matches the town's own stone.
    let palette = crate::world_material::Palette {
        wall: world_forge::WorldMaterial::Wall.albedo(),
        roof: world_forge::WorldMaterial::Roof.albedo(),
        roof_gloss: 1.0 - world_forge::WorldMaterial::Roof.roughness(),
        stone: (stone.albedo, stone.gloss),
    };
    let start = vertices.len() as u32;
    for vertex in spill.vertices() {
        let position = base + rotation * (vertex.position * instance.scale);
        let normal = (rotation * vertex.normal).normalize_or_zero();
        let (color, gloss, role) = crate::world_material::to_scene(
            world_forge::WorldMaterial::from_carrier(vertex.material),
            palette,
        );
        vertices.push(
            SceneVertex::surfaced(position.to_array(), normal.to_array(), color, gloss)
                .with_surface(role),
        );
    }
    indices.extend(spill.indices().iter().map(|index| index + start));
}

/// The far representation: the original flat-shaded frusta. The backdrop ring calls this
/// directly — at its distances the baked crown and the painted cone are the same picture, and
/// the ring has thousands of instances.
pub fn push_scenery_instance_far(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
    stone: StoneTone,
) {
    let base = Vec3::from_array(instance.position);
    let s = instance.scale;
    let yaw = instance.yaw_rad;
    match instance.kind {
        // A far PLANT is its impostor (Inny Poziom F1): the same crossed quads over the same
        // species sprite the instanced ladder draws past 150 m, baked here at the instance
        // transform. The painted frustum kit that used to answer this arm is deleted.
        SceneryKind::Oak
        | SceneryKind::Poplar
        | SceneryKind::Willow
        | SceneryKind::FruitTree
        | SceneryKind::Bush
        | SceneryKind::Pine => push_impostor_tree(vertices, indices, instance),
        // Retired imported kinds (procedural-only, Świat 2.0): they draw nothing anywhere — not
        // here, not in the near bake. The variants stay in the enum (append-only wire identity)
        // but are never authored; this arm only exists so the match is total.
        SceneryKind::FloraTree | SceneryKind::FloraPine | SceneryKind::FloraBush => {}
        SceneryKind::Rock => {
            // The backdrop ring's stone: the pre-forge cuboid, kept deliberately. At the
            // distances this path serves, a hundred triangles of bedding relief and twelve of
            // box read as the same speck.
            let start = vertices.len();
            push_oriented_box(
                vertices,
                indices,
                base + Vec3::Y * 0.45 * s,
                Vec3::new(0.9, 0.5, 0.7) * s,
                Mat3::from_rotation_y(yaw),
                stone.albedo,
            );
            for vertex in &mut vertices[start..] {
                vertex.gloss = stone.gloss;
            }
        }
        SceneryKind::Lamppost => {
            // Cast iron (urban-map PR-11): the tapering column, the short arm reaching over
            // the roadway along the instance yaw, and the warm glass head under it.
            // Immersja A4.1: 4 m read as a garden lantern; a street column is 6.5 m.
            push_frustum(vertices, indices, base, 0.09 * s, 0.055 * s, 6.5 * s, IRON);
            let reach = Vec3::new(yaw.sin(), 0.0, yaw.cos());
            let arm_root = base + Vec3::Y * 6.5 * s;
            let start = vertices.len();
            push_oriented_box(
                vertices,
                indices,
                arm_root + reach * 0.35 * s + Vec3::Y * 0.06 * s,
                Vec3::new(0.045 * s, 0.045 * s, 0.42 * s),
                Mat3::from_rotation_y(yaw),
                IRON.0,
            );
            push_oriented_box(
                vertices,
                indices,
                arm_root + reach * 0.72 * s - Vec3::Y * 0.12 * s,
                Vec3::new(0.11 * s, 0.16 * s, 0.11 * s),
                Mat3::from_rotation_y(yaw),
                LAMP_GLASS.0,
            );
            for vertex in &mut vertices[start..] {
                vertex.gloss = if vertex.color == LAMP_GLASS.0 { LAMP_GLASS.1 } else { IRON.1 };
            }
        }
        SceneryKind::DebrisHeap => {
            // A knee-high masonry spill (urban-map PR-11): a low mound with two tumbled
            // slabs. Deliberately under bush height — dressing, never implied cover.
            push_frustum(vertices, indices, base, 0.95 * s, 0.3 * s, 0.42 * s, RUBBLE);
            let start = vertices.len();
            push_oriented_box(
                vertices,
                indices,
                base + Vec3::new(0.45 * s, 0.14 * s, -0.2 * s),
                Vec3::new(0.3 * s, 0.12 * s, 0.2 * s),
                Mat3::from_rotation_y(yaw + 0.5),
                RUBBLE.0,
            );
            push_oriented_box(
                vertices,
                indices,
                base + Vec3::new(-0.4 * s, 0.1 * s, 0.3 * s),
                Vec3::new(0.24 * s, 0.09 * s, 0.17 * s),
                Mat3::from_rotation_y(yaw - 0.8),
                RUBBLE.0,
            );
            for vertex in &mut vertices[start..] {
                vertex.gloss = RUBBLE.1;
            }
        }
    }
}

/// A baked stone — one erratic, or a whole talus drift — transformed and coloured into the static
/// scene mesh. The seed comes from the instance's position bits, the same trick the trees use, so
/// a scatter never repeats a stone yet every bake of the map is identical.
fn push_rock(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
    stone: StoneTone,
    form: world_forge::rock::RockForm,
) {
    let base = Vec3::from_array(instance.position);
    let seed =
        instance.position[0].to_bits() as u64 ^ ((instance.position[2].to_bits() as u64) << 32);
    let rock = world_forge::rock::bake_rock(form, seed);
    let rotation = Mat3::from_rotation_y(instance.yaw_rad);
    let start = vertices.len() as u32;
    for vertex in rock.body.vertices() {
        let position = base + rotation * (vertex.position * instance.scale);
        let normal = (rotation * vertex.normal).normalize_or_zero();
        vertices.push(
            SceneVertex::surfaced(
                position.to_array(),
                normal.to_array(),
                stone.albedo,
                stone.gloss,
            )
            .with_surface(surface_role::ROCK_FACE),
        );
    }
    indices.extend(rock.body.indices().iter().map(|index| index + start));
}

/// Cast iron and its warm glass head (urban-map PR-11); masonry spill for the debris heap.
const IRON: ([f32; 3], f32) = ([0.16, 0.17, 0.18], 0.25);
const LAMP_GLASS: ([f32; 3], f32) = ([0.75, 0.68, 0.45], 0.5);
const RUBBLE: ([f32; 3], f32) = ([0.44, 0.40, 0.35], 0.07);

/// The BELLY LINE: the lowest ground clearance in the fleet, metres.
///
/// The honesty doctrine's rule for scenery is written for the eye — "anything that reads as cover
/// must be a cover box". Stone forces a second rule, written for the belly: a SOLID object taller
/// than this is something a hull would touch, so driving through it on the way past is a lie
/// whether or not it reads as cover. Loose dressing (grass, brush, a rubble spill) is exempt by
/// construction — a tank drives through loose material, and that is why a knee-high bush is
/// honest at 1.05 m while a knee-high boulder is not.
///
/// Derived, never copied: [`tests::the_belly_line_is_the_fleet_measurement`] walks every shipped
/// blueprint and fails the moment the fleet goes lower. It is the T-34-85's 0.40 m today, not the
/// benchmark T-54's 0.425.
///
/// Enforced against `SceneryKind::Rock` when the big stones move to the cover tiers that block
/// (`StaticCoverKind::Boulder` / `Outcrop`); until then it is the measurement those PRs cut to.
pub const BELLY_LINE_M: f32 = 0.40;

#[cfg(test)]
mod tests {
    use game_core::{VehicleBlueprint, VehicleKind};
    use terrain::MapId;

    use super::*;

    fn erratic(position: [f32; 3], scale: f32) -> SceneryInstance {
        SceneryInstance { kind: SceneryKind::Rock, position, yaw_rad: 0.7, scale }
    }

    fn bake(instance: &SceneryInstance, stone: StoneTone) -> (Vec<SceneVertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        push_scenery_instance(&mut vertices, &mut indices, instance, stone);
        (vertices, indices)
    }

    /// The number the honesty rule stands on is a MEASUREMENT of the fleet, not a constant
    /// somebody typed twice. A vehicle with a lower belly moves the rule, and this fails first.
    #[test]
    fn the_belly_line_is_the_fleet_measurement() {
        let lowest = VehicleKind::ALL
            .iter()
            .filter_map(|kind| VehicleBlueprint::for_vehicle(*kind))
            .map(|blueprint| blueprint.hull.belly_y)
            .fold(f32::INFINITY, f32::min);
        assert!(lowest.is_finite(), "the fleet has at least one blueprint");
        assert!(
            (lowest - BELLY_LINE_M).abs() < 1.0e-6,
            "the fleet's lowest belly is now {lowest} m, not {BELLY_LINE_M} m — the honesty \
             threshold for solid scenery moved with it"
        );
    }

    /// D6's closer: a rock is geometry now, not a cuboid. Twelve triangles was the tell.
    #[test]
    fn a_rock_is_built_not_boxed() {
        let (vertices, indices) = bake(&erratic([10.0, 3.0, 10.0], 1.0), StoneTone::NEUTRAL);
        assert!(
            indices.len() / 3 > 40,
            "a baked field stone is real geometry, not a box: {} tris",
            indices.len() / 3
        );
        assert!(indices.len().is_multiple_of(3));
        assert!(indices.iter().all(|index| (*index as usize) < vertices.len()));
        assert!(
            vertices.iter().all(|vertex| (vertex.surface - surface_role::ROCK_FACE).abs() < 0.01),
            "every rock vertex names its surface, or the shader shades it as nothing"
        );
    }

    /// Deterministic per position, and no two stones in a scatter are the same stone.
    #[test]
    fn rocks_are_deterministic_per_position_and_never_clones() {
        let here = bake(&erratic([10.0, 3.0, 10.0], 1.0), StoneTone::NEUTRAL);
        let again = bake(&erratic([10.0, 3.0, 10.0], 1.0), StoneTone::NEUTRAL);
        assert_eq!(here.0.len(), again.0.len(), "the scene bake is deterministic");
        assert!(
            here.0.iter().zip(&again.0).all(|(a, b)| a.position == b.position),
            "the scene bake is deterministic vertex for vertex"
        );
        let elsewhere = bake(&erratic([50.0, 3.0, 90.0], 1.0), StoneTone::NEUTRAL);
        let sum = |mesh: &(Vec<SceneVertex>, Vec<u32>)| {
            mesh.0.iter().map(|v| u64::from(v.position[1].to_bits())).sum::<u64>()
        };
        assert_ne!(sum(&here), sum(&elsewhere), "two stones at different spots are different");
    }

    /// The seam killer: a stone reaches BELOW the ground it stands on, at every authored scale.
    /// The cuboid it replaces sank 5 cm at scale 1.0 and read as laid on the turf.
    #[test]
    fn a_rock_sits_in_the_ground_it_stands_on() {
        for scale in [0.8, 1.0, 1.3] {
            let ground = 3.0_f32;
            let (vertices, _) = bake(&erratic([10.0, ground, 10.0], scale), StoneTone::NEUTRAL);
            let lowest = vertices.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
            assert!(
                lowest < ground - 0.05 * scale,
                "scale {scale} rests on the turf: lowest {lowest} vs ground {ground}"
            );
            let top = vertices.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max) - ground;
            assert!((0.3 * scale..=1.6 * scale).contains(&top), "scale {scale} height {top}");
        }
    }

    /// The spill wears the materials of the building it fell from, so the scene shader dresses
    /// each in its own treatment — plaster blotches, roof courses, plank grain. The defect this
    /// replaces was three lumps of ONE grey in a town built of four materials.
    #[test]
    fn the_debris_heap_is_made_of_the_building_it_fell_from() {
        let instance = SceneryInstance {
            kind: SceneryKind::DebrisHeap,
            position: [12.0, 2.0, 8.0],
            yaw_rad: 1.1,
            scale: 1.0,
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        push_scenery_instance(&mut vertices, &mut indices, &instance, StoneTone::NEUTRAL);
        assert!(!indices.is_empty(), "a debris heap draws something");
        for (role, name) in [
            (surface_role::PLASTER, "rendered masonry"),
            (surface_role::SLATE, "roof tile"),
            (surface_role::PLANK, "snapped timber"),
        ] {
            assert!(
                vertices.iter().any(|v| (v.surface - role).abs() < 0.01),
                "the spill carries no {name} — it is a grey pile again"
            );
        }
        // Honest-blockers rule: knee-high, whatever the seed and scale.
        let top = vertices.iter().map(|v| v.position[1] - 2.0).fold(f32::MIN, f32::max);
        assert!(top <= world_forge::spill::SPILL_HEIGHT_CAP_M + 1.0e-4, "spill stands {top} m");
    }

    /// Per-map identity: the boulders are made of the stone the map's own splat paints. A map
    /// with no palette keeps the tone every rock used to wear, so nothing moves by accident.
    #[test]
    fn each_map_carries_its_own_stone() {
        let tones: Vec<StoneTone> = MapId::SHIPPED
            .iter()
            .map(|id| {
                let blueprint = map_forge::blueprint_for(*id);
                let materials = blueprint.materials.expect("a shipped map authors its palette");
                let rock = materials.layers[ROCK_SPLAT_LAYER];
                StoneTone { albedo: rock.albedo, gloss: rock.gloss }
            })
            .collect();
        assert!(tones.len() >= 4, "four shipped maps");
        for (index, tone) in tones.iter().enumerate() {
            assert!(
                tones.iter().skip(index + 1).all(|other| other.albedo != tone.albedo),
                "two shipped maps are made of the same stone: {tone:?}"
            );
            // Art-direction rule 2: the ground plane is muted. Stone is the palest thing on it
            // and still lives inside the window.
            let max = tone.albedo.iter().copied().fold(f32::MIN, f32::max);
            let min = tone.albedo.iter().copied().fold(f32::MAX, f32::min);
            assert!(max <= 0.62, "stone brighter than the ground envelope: {tone:?}");
            assert!((max - min) / max.max(1.0e-4) <= 0.45, "stone out of the saturation window");
        }
    }
}
