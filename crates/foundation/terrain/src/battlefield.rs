use serde::{Deserialize, Serialize};

use crate::HeightMap;
use crate::river::RiverSpec;
use crate::scenery::SceneryInstance;
use crate::water::WaterBody;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategicRole {
    HighGround,
    Crossing,
    Observation,
    HullDown,
    FlankRoute,
}

/// A capture zone: data first (map-editor M7) — the sim's capture rules arrive with their
/// own program; until then the zone is authored truth the editor, minimap and report see.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureZone {
    pub id: String,
    /// World position, grounded on the heightmap at authoring time.
    pub center: [f32; 3],
    pub radius_m: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategicPoint {
    pub id: String,
    pub name: String,
    pub role: StrategicRole,
    pub position: [f32; 3],
    pub radius_m: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnZone {
    pub team: u16,
    pub center: [f32; 3],
    pub radius_m: f32,
    pub facing_yaw_rad: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapFeatureKind {
    Hill,
    RailEmbankment,
    AntiTankDitch,
    Lowland,
    Farm,
    TreeLine,
    Ridge,
    Crossing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapFeature {
    pub kind: MapFeatureKind,
    pub name: String,
    pub center: [f32; 3],
    pub radius_m: f32,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticCoverKind {
    FarmBuilding,
    RailCover,
    TreeLine,
    Wreck,
    /// A wooden farm fence: waist-high posts and rails. Flimsy — one shell clears a span, and
    /// a hull simply drives THROUGH it (crushable), which is the entire point: the world
    /// reacts like matter, not like invisible walls (Fizyczny Świat P10). Appended last so
    /// baked map assets keep their discriminants.
    WoodenFence,
    /// A masonry city building (urban-map program PR-06): brick and plaster over a metre of
    /// wall, far tougher than a timber farm building, and it collapses into the same honest
    /// rubble mound. Durability is stated by the KIND in the map document, never inferred
    /// from box proportions. Appended after WoodenFence — the order is frozen.
    CityBuilding,
    /// A brick garden/compound wall: a shell breaches it, and a 30 t hull drives THROUGH it
    /// (crushable). Destroyed or crushed it goes fully clear — never a hull-blocking mound;
    /// the breach dressing is presentation, not collision.
    StoneWall,
    /// The bole of a single hero tree (hero-flora phase 3): the metre-and-a-half of bark a
    /// hull meets, a shell stops in and an eye cannot see past. It is the one kind that bakes
    /// NO box of its own — the tree's mesh IS its visual, and drawing a second solid around
    /// it would put a green pillar inside the trunk the player is looking at. Everything else
    /// it inherits from a hedgerow: crushable (a hull pushes the tree over), destructible,
    /// and its wreckage is the same stumps-and-fallen-trunk dressing. Appended last — the
    /// order is frozen.
    TreeTrunk,
    /// A granite crag (teren W3b, Atlas audit): the honest BIG rock the scenery rules could
    /// never carry — solid scenery must stay under the 0.40 m belly line, and a
    /// metre-and-more of stone that reads as cover MUST be cover. Indestructible like a
    /// rail embankment (shellfire does not demolish granite), never crushable, and the bake
    /// composes faceted blocks strictly inside the collision box: what stops the shell is
    /// what the eye sees. Appended last — the order is frozen.
    Crag,
    /// A freestanding stone watchtower (teren W3b, Orliny col landmark): a Svan-style
    /// battered masonry shaft. Destructible like the masonry it is — and its rubble
    /// fraction is tuned so the FELLED tower's stump lands in the hull-down band
    /// (hides a hull, ducks the turret line): the landmark falls into a fighting
    /// position instead of a wall. Appended after Crag — the order is frozen.
    StoneTower,
}

impl StaticCoverKind {
    /// Every kind of cover a map may place. Append-only: the compiled map blueprints store these.
    ///
    /// Locked variant-by-variant against the declaration by `quality`, not by counting: a
    /// length assertion cannot tell a forgotten variant from a shorter enum.
    pub const ALL: [StaticCoverKind; 10] = [
        StaticCoverKind::FarmBuilding,
        StaticCoverKind::RailCover,
        StaticCoverKind::TreeLine,
        StaticCoverKind::Wreck,
        StaticCoverKind::WoodenFence,
        StaticCoverKind::CityBuilding,
        StaticCoverKind::StoneWall,
        StaticCoverKind::TreeTrunk,
        StaticCoverKind::Crag,
        StaticCoverKind::StoneTower,
    ];

    /// Structural health before the object is destroyed; `None` is indestructible (rail
    /// embankments, pre-placed decorative wrecks — they never take damage or change phase).
    pub fn max_health(self) -> Option<u32> {
        match self {
            StaticCoverKind::FarmBuilding => Some(600),
            // Masonry over timber: a city block soaks 2.5 farm buildings' worth of shellfire
            // before it comes down (urban-map doctrine decision 2).
            StaticCoverKind::CityBuilding => Some(1500),
            StaticCoverKind::TreeLine => Some(120),
            // A mature oak's bole is not a hedgerow: it takes real shellfire to fell, and a
            // hull leans on it before it goes over.
            StaticCoverKind::TreeTrunk => Some(240),
            // A garden wall is bricks, not a bunker: a couple of shells open it.
            StaticCoverKind::StoneWall => Some(150),
            // Any shell sweeps a fence span away (the kinetic chip already deals 80).
            StaticCoverKind::WoodenFence => Some(40),
            // A freestanding tower is less structure than a city block but thicker-walled
            // than a farmhouse: 1.5 farm buildings' worth of shellfire fells it.
            StaticCoverKind::StoneTower => Some(900),
            // A knocked-out hull is steel, not scenery (Inny Poziom Z3): shellfire opens its
            // superstructure and brings it down to a hull-line mound, then to scrap. It was
            // indestructible forever — "pre-placed decorative" — on maps where the wrecks are
            // a third of the cover.
            StaticCoverKind::Wreck => Some(500),
            StaticCoverKind::RailCover | StaticCoverKind::Crag => None,
        }
    }

    /// A hull driving through at speed flattens it (hedgerows/tree lines, fences — and a
    /// brick garden wall under 30 t of tank). Buildings and rail embankments do not crush —
    /// a shell has to bring them down.
    pub fn is_crushable(self) -> bool {
        matches!(
            self,
            StaticCoverKind::TreeLine
                | StaticCoverKind::TreeTrunk
                | StaticCoverKind::WoodenFence
                | StaticCoverKind::StoneWall
        )
    }

    /// When destroyed, a building slumps into a rubble mound that still blocks hulls; foliage
    /// simply vanishes. `true` = leaves a (lowered) blocking mound, `false` = goes fully clear.
    /// A StoneWall deliberately leaves NO mound: a breached wall is a door, not a speed bump.
    pub fn leaves_rubble(self) -> bool {
        matches!(
            self,
            StaticCoverKind::FarmBuilding
                | StaticCoverKind::CityBuilding
                | StaticCoverKind::StoneTower
                // A shelled wreck keeps its hull as a mound (Inny Poziom Z3).
                | StaticCoverKind::Wreck
        )
    }

    /// The fraction of its original height a rubble mound keeps: low enough that a turret-height
    /// shot clears it, tall enough to still stop a hull. The fraction is PER KIND because the
    /// promise is absolute, not proportional: 0.4 of a farm building is a 2.4 m pile a turret
    /// works over, but 0.4 of an 11 m tenement would be a 4.4 m wall that buries the
    /// destruction-opens-the-map promise — so masonry blocks keep their mound under turret
    /// eyes (urban-map PR-14, locked by `tests/ostrogorsk_urban.rs`).
    pub fn rubble_height_frac(self) -> f32 {
        match self {
            StaticCoverKind::CityBuilding => 0.18,
            // A watchtower stands nearly twice a tenement, so its fraction drops further:
            // 0.11 of the 20 m Orliny tower is a 2.2 m stump — under the benchmark turret
            // line (2.53 m), above the hull-down floor (1.49 m). The felled landmark
            // becomes a fighting mound, locked by `sim`'s Orliny tests.
            StaticCoverKind::StoneTower => 0.11,
            // A shelled wreck keeps its hull: the turret and the superstructure go, and a
            // 2.7 m hulk becomes a 1.2 m mound — still a thing to sit behind, no longer a
            // thing to hide a turret behind.
            StaticCoverKind::Wreck => 0.45,
            _ => 0.4,
        }
    }
}

/// The phase byte a cover object is BORN in (urban-map program PR-07, doctrine decision 4):
/// an id containing `"ruin"` spawns already collapsed — rubble (1) if its kind leaves rubble,
/// gone (2) otherwise — while indestructible kinds ignore the tag (a "ruined" rail cover is
/// still just a rail cover; a "ruined" wreck, steel since Z3, is born as its mound). The bytes
/// speak the shared phase encoding every consumer
/// already reads (0 intact / 1 rubble / 2 gone), so the rule lives HERE, below both the sim
/// and every renderer-side baker, and all of them agree at birth.
pub fn born_cover_phase_byte(object: &StaticCoverObject) -> u8 {
    if !object.id.contains("ruin") || object.kind.max_health().is_none() {
        return 0;
    }
    if object.kind.leaves_rubble() { 1 } else { 2 }
}

/// One born-phase byte per cover object, index-aligned — what a pristine battle starts from.
pub fn initial_cover_phase_bytes(cover: &[StaticCoverObject]) -> Vec<u8> {
    cover.iter().map(born_cover_phase_byte).collect()
}

/// The heading a felled tree went down along (the one program's Z8), as the one wire byte:
/// 256 steps around the compass from +X toward +Z (a degree and a half each), so every client
/// and the wreckage bake lay the trunk the way the authority decided — along the crusher's
/// heading, or along the shell's flight.
pub fn fall_heading_byte(heading_rad: f32) -> u8 {
    let turns = heading_rad / std::f32::consts::TAU;
    let unit = turns - turns.floor();
    ((unit * 256.0).round() as u32 % 256) as u8
}

/// The heading a fall byte encodes, in radians from +X toward +Z.
pub fn fall_heading_rad(byte: u8) -> f32 {
    f32::from(byte) / 256.0 * std::f32::consts::TAU
}

/// The material of a building's walls (the one program's Z9, §13.4): what a shell has to get
/// through, in the same thickness table a spaced screen uses. The KIND names it; a landmark's
/// id refines it (a church or a tower is stone though its box is a city block's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallMaterial {
    Timber,
    Brick,
    Stone,
}

impl WallMaterial {
    /// The wall's thickness — the slab a sight line or a shell meets once a segment is open.
    pub fn thickness_m(self) -> f32 {
        match self {
            WallMaterial::Timber => 0.25,
            WallMaterial::Brick => 0.45,
            WallMaterial::Stone => 0.90,
        }
    }

    /// One wall segment's structural budget, scored against a shell's cover damage
    /// (`sim::cover_damage_hp`): two 57 mm HE rounds open a timber segment, one 152 mm; a
    /// brick segment takes five of the small ones; stone yields only to the large charge.
    pub fn segment_health(self) -> u32 {
        match self {
            WallMaterial::Timber => 200,
            WallMaterial::Brick => 500,
            WallMaterial::Stone => 900,
        }
    }
}

/// The wall material of a building box, `None` for everything that is not a building.
pub fn wall_material(object: &StaticCoverObject) -> Option<WallMaterial> {
    match object.kind {
        StaticCoverKind::FarmBuilding => Some(WallMaterial::Timber),
        StaticCoverKind::CityBuilding => {
            Some(if object.id.contains("church") || object.id.contains("tower") {
                WallMaterial::Stone
            } else {
                WallMaterial::Brick
            })
        }
        StaticCoverKind::StoneTower => Some(WallMaterial::Stone),
        _ => None,
    }
}

/// Wall SEGMENTS (Z9, §13.4 „zniszczenie per segment ściany, nie per budynek"): each facade of
/// a building is cut into runs of about `SEGMENT_RUN_M` (B5's ruin cuts the same 3–5), at most
/// `SEGMENTS_PER_FACADE`, and each segment carries its own state in two bits of the packed
/// `SEGMENT_BYTES`: 0 whole, 1 damaged, 2 ruin (an opening from the sill up), 3 rubble (a lip
/// at the foot). Facades count as `CoverScar.face` does — 0 +X, 1 -X, 2 +Z, 3 -Z — and run
/// along +Z for the X faces and +X for the Z faces, the way the scar's `u` counts.
pub const SEGMENT_RUN_M: f32 = 4.0;
pub const SEGMENTS_PER_FACADE: usize = 5;
/// Segment slots per building: four facades of `SEGMENTS_PER_FACADE`.
pub const SEGMENT_SLOTS: usize = 4 * SEGMENTS_PER_FACADE;
/// Two bits per slot, packed.
pub const SEGMENT_BYTES: usize = SEGMENT_SLOTS.div_ceil(4);
pub const SEGMENT_WHOLE: u8 = 0;
pub const SEGMENT_DAMAGED: u8 = 1;
pub const SEGMENT_RUIN: u8 = 2;
pub const SEGMENT_RUBBLE: u8 = 3;
/// The sill an opening keeps standing, and the lip a rubble segment keeps.
pub const RUIN_SILL_M: f32 = 0.6;
pub const RUBBLE_LIP_M: f32 = 0.3;

/// One building's segment states, packed.
pub type SegmentStates = [u8; SEGMENT_BYTES];

/// Every segment of a building that has come down.
pub const SEGMENTS_ALL_RUBBLE: SegmentStates = [0xFF; SEGMENT_BYTES];

/// The run of one facade of a box.
pub fn facade_run_m(object: &StaticCoverObject, facade: usize) -> f32 {
    if facade < 2 { 2.0 * object.half_extents_m[2] } else { 2.0 * object.half_extents_m[0] }
}

/// How many segments a facade of `run_m` is cut into.
pub fn facade_segments(run_m: f32) -> usize {
    ((run_m / SEGMENT_RUN_M).round().max(1.0) as usize).min(SEGMENTS_PER_FACADE)
}

/// Which segment of a facade of `run_m` a point `u` (−1..1 along the run, as `CoverScar.u_q`
/// counts it) falls in.
pub fn segment_at(run_m: f32, u: f32) -> usize {
    let n = facade_segments(run_m);
    (((u + 1.0) * 0.5).clamp(0.0, 0.999_9) * n as f32) as usize
}

pub fn segment_state(packed: &SegmentStates, facade: usize, segment: usize) -> u8 {
    let bit = (facade * SEGMENTS_PER_FACADE + segment) * 2;
    (packed[bit / 8] >> (bit % 8)) & 3
}

pub fn set_segment_state(packed: &mut SegmentStates, facade: usize, segment: usize, state: u8) {
    let bit = (facade * SEGMENTS_PER_FACADE + segment) * 2;
    packed[bit / 8] = (packed[bit / 8] & !(3 << (bit % 8))) | ((state & 3) << (bit % 8));
}

/// Whether any segment is OPEN (ruin or rubble): the box is no longer one solid to the eye.
pub fn segments_opened(packed: &SegmentStates) -> bool {
    (0..4).any(|facade| {
        (0..SEGMENTS_PER_FACADE)
            .any(|segment| segment_state(packed, facade, segment) >= SEGMENT_RUIN)
    })
}

/// The slabs a building with an opening blocks with (Z9): a hollow of four walls, each
/// segment its own box of the material's thickness — full height while whole or damaged, the
/// sill's height where ruined, the lip's where rubble — under one roof slab. What the eye and
/// the shell meet is the wall that is there; a line through two openings sees clean through.
/// The hull never enters (§13.4): movement keeps the whole box.
pub fn opened_building_boxes(
    object: &StaticCoverObject,
    packed: &SegmentStates,
) -> Vec<StaticCoverObject> {
    let Some(material) = wall_material(object) else {
        return vec![object.clone()];
    };
    let t = material.thickness_m();
    let c = object.center;
    let h = object.half_extents_m;
    let floor = c[1] - h[1];
    let top = c[1] + h[1];
    let mut boxes = Vec::with_capacity(SEGMENT_SLOTS + 1);
    let slab = |id: String, center: [f32; 3], half: [f32; 3]| StaticCoverObject {
        id,
        name: object.name.clone(),
        kind: object.kind,
        center,
        half_extents_m: half,
    };
    boxes.push(slab(
        format!("{}#roof", object.id),
        [c[0], top - t * 0.5, c[2]],
        [h[0], t * 0.5, h[2]],
    ));
    for facade in 0..4 {
        let run = facade_run_m(object, facade);
        let n = facade_segments(run);
        let seg = run / n as f32;
        for s in 0..n {
            let height = match segment_state(packed, facade, s) {
                SEGMENT_RUIN => RUIN_SILL_M,
                SEGMENT_RUBBLE => RUBBLE_LIP_M,
                _ => top - floor,
            };
            let along = -run * 0.5 + seg * (s as f32 + 0.5);
            let y = floor + height * 0.5;
            let (center, half) = match facade {
                0 => ([c[0] + h[0] - t * 0.5, y, c[2] + along], [t * 0.5, height * 0.5, seg * 0.5]),
                1 => ([c[0] - h[0] + t * 0.5, y, c[2] + along], [t * 0.5, height * 0.5, seg * 0.5]),
                2 => ([c[0] + along, y, c[2] + h[2] - t * 0.5], [seg * 0.5, height * 0.5, t * 0.5]),
                _ => ([c[0] + along, y, c[2] - h[2] + t * 0.5], [seg * 0.5, height * 0.5, t * 0.5]),
            };
            boxes.push(slab(format!("{}#f{facade}s{s}", object.id), center, half));
        }
    }
    boxes
}

/// What a road is paved with — picks the painted tone and finish on the terrain mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoadSurface {
    /// Packed farm dirt: pale earth worn into the grass.
    Dirt,
    /// Railway ballast: the crushed-stone bed under the rails.
    Ballast,
    /// Granite setts: the paved city street (urban-map program PR-05). Append-only, like
    /// every content enum — the variant order never changes.
    Cobble,
}

impl RoadSurface {
    /// Every road surface a map may lay. Append-only: the compiled map blueprints store these.
    ///
    /// Locked variant-by-variant against the declaration by `quality`, not by counting: a
    /// length assertion cannot tell a forgotten variant from a shorter enum.
    pub const ALL: [RoadSurface; 3] =
        [RoadSurface::Dirt, RoadSurface::Ballast, RoadSurface::Cobble];
}

/// A road: a polyline in world XZ painted onto the terrain — and DRIVEN. No collision and
/// no concealment change, but the surface routes the ground rule (teren A2): Ballast and
/// Cobble claim the rock lane (grip 1.04 / rolling 0.9), Dirt wears the ground to worn
/// earth (0.95 / 1.35), and physics reads those scales on the deterministic path — locked
/// by `a_paved_road_is_never_the_slowest_ground`. A road is sim input, not decoration:
/// moving one retunes every drive across it. (This doc used to say "render-only, no
/// movement bonus", which had been false since the stone lane landed.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Road {
    pub id: String,
    pub surface: RoadSurface,
    /// Polyline waypoints in world XZ, walked in order.
    pub points: Vec<[f32; 2]>,
    /// Full painted width; the tone feathers out over the outer half.
    pub width_m: f32,
}

impl Road {
    /// Distance from `(x, z)` to the nearest point on the polyline (the shared
    /// [`crate::polyline_distance`] walk — behavior-identical to the historical inline math).
    pub fn distance_to(&self, x: f32, z: f32) -> f32 {
        crate::sculpt::polyline_distance(&self.points, x, z)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StaticCoverObject {
    pub id: String,
    pub name: String,
    pub kind: StaticCoverKind,
    pub center: [f32; 3],
    pub half_extents_m: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattlefieldMap {
    pub id: String,
    pub name: String,
    pub size_m: [f32; 2],
    pub historical_basis: String,
    pub design_notes: Vec<String>,
    pub heightmap: HeightMap,
    /// The map's global water table, if any (see [`WaterBody`]): depth anywhere is
    /// `water.depth_over(heightmap height)`. `None` is a dry map; `serde(default)` keeps
    /// pre-water baked assets deserializing.
    #[serde(default)]
    pub water: Option<WaterBody>,
    /// Bounded standing-water sheets over their own tables (teren W6) — mountain tarns,
    /// paired lakes at different altitudes. Consumers resolve the COMPLETE water through
    /// [`crate::WaterField`] (`water_field()`), never one channel alone. `serde(default)`
    /// keeps single-table baked assets deserializing.
    #[serde(default)]
    pub standing_water: Vec<crate::StandingWater>,
    /// The map's river centerline as data, if any (see [`RiverSpec`]): the carve, the water
    /// mesh, the minimap, bot water probes and the backdrop skirt all follow the same line.
    /// `serde(default)` keeps pre-river baked assets deserializing.
    #[serde(default)]
    pub river: Option<RiverSpec>,
    pub spawn_zones: Vec<SpawnZone>,
    /// Authored capture zones (`serde(default)` keeps pre-M7 baked assets deserializing).
    #[serde(default)]
    pub capture_zones: Vec<CaptureZone>,
    pub strategic_points: Vec<StrategicPoint>,
    pub features: Vec<MapFeature>,
    pub static_cover: Vec<StaticCoverObject>,
    /// Render-only dressing (trees, rocks): never collides, never blocks shells or sight.
    /// `serde(default)` keeps pre-scenery baked assets deserializing.
    #[serde(default)]
    pub scenery: Vec<SceneryInstance>,
    /// Roads painted onto the terrain AND fed to the ground rule — a paved lane drives as
    /// stone (see [`Road`]). `serde(default)` keeps pre-road baked assets deserializing.
    #[serde(default)]
    pub roads: Vec<Road>,
}

impl BattlefieldMap {
    pub fn feature(&self, kind: MapFeatureKind, name: &str) -> Option<&MapFeature> {
        self.features.iter().find(|feature| feature.kind == kind && feature.name.contains(name))
    }

    /// The map's COMPLETE water as one resolvable field — the door every consumer walks
    /// (sim, physics, meshes, minimap, report): reading `water` alone misses the sheets.
    pub fn water_field(&self) -> crate::WaterField {
        crate::WaterField { table: self.water, sheets: self.standing_water.clone() }
    }

    /// The zero-clone borrow of the same field, for per-frame callers (reticle sweeps,
    /// minimap rebuilds).
    pub fn water_view(&self) -> crate::WaterView<'_> {
        crate::WaterView { table: self.water, sheets: &self.standing_water }
    }
}

#[cfg(test)]
mod born_phase_tests {
    use super::*;

    /// Z9: two bits per segment, a facade cut into ~4 m runs, and an opened building is a
    /// hollow of slabs — every slab inside the authored box, the ruined one sill-high, the
    /// rubble one lip-high, the whole ones the full height, one roof over them all.
    #[test]
    fn wall_segments_pack_two_bits_and_an_opened_building_is_a_hollow_of_slabs() {
        let mut packed = [0u8; SEGMENT_BYTES];
        set_segment_state(&mut packed, 0, 0, SEGMENT_DAMAGED);
        assert!(!segments_opened(&packed), "damage is not an opening");
        set_segment_state(&mut packed, 2, 1, SEGMENT_RUIN);
        set_segment_state(&mut packed, 3, 2, SEGMENT_RUBBLE);
        assert_eq!(segment_state(&packed, 0, 0), SEGMENT_DAMAGED);
        assert_eq!(segment_state(&packed, 2, 1), SEGMENT_RUIN);
        assert_eq!(segment_state(&packed, 3, 2), SEGMENT_RUBBLE);
        assert_eq!(segment_state(&packed, 2, 0), SEGMENT_WHOLE);
        assert_eq!(segment_state(&packed, 1, 3), SEGMENT_WHOLE);
        assert!(segments_opened(&packed));
        assert!(segments_opened(&SEGMENTS_ALL_RUBBLE));

        assert_eq!(facade_segments(8.0), 2);
        assert_eq!(facade_segments(12.0), 3);
        assert_eq!(facade_segments(2.0), 1);
        assert_eq!(facade_segments(40.0), SEGMENTS_PER_FACADE);
        assert_eq!(segment_at(12.0, -0.9), 0);
        assert_eq!(segment_at(12.0, 0.1), 1);
        assert_eq!(segment_at(12.0, 0.9), 2);
        assert_eq!(segment_at(12.0, 1.0), 2, "the far end stays in the last segment");

        let barn = StaticCoverObject {
            id: "barn".into(),
            name: "barn".into(),
            kind: StaticCoverKind::FarmBuilding,
            center: [10.0, 2.0, -5.0],
            half_extents_m: [6.0, 2.0, 4.0],
        };
        assert_eq!(wall_material(&barn), Some(WallMaterial::Timber));
        let slabs = opened_building_boxes(&barn, &packed);
        // +X and -X run 8 m: 2 segments each; +Z and -Z run 12 m: 3 each; one roof.
        assert_eq!(slabs.len(), 1 + 2 + 2 + 3 + 3);
        for slab in &slabs {
            for axis in 0..3 {
                let reach =
                    (slab.center[axis] - barn.center[axis]).abs() + slab.half_extents_m[axis];
                assert!(reach <= barn.half_extents_m[axis] + 1e-4, "{} inside the box", slab.id);
            }
        }
        let by_id = |id: &str| slabs.iter().find(|s| s.id == format!("barn#{id}")).expect(id);
        assert_eq!(by_id("f0s0").half_extents_m[1], 2.0, "a whole segment is the full wall");
        assert_eq!(by_id("f2s1").half_extents_m[1] * 2.0, RUIN_SILL_M, "a ruin keeps its sill");
        assert!((by_id("f2s1").center[1] - RUIN_SILL_M * 0.5).abs() < 1e-5);
        assert_eq!(by_id("f3s2").half_extents_m[1] * 2.0, RUBBLE_LIP_M, "rubble keeps a lip");
        assert_eq!(by_id("f0s0").half_extents_m[0] * 2.0, WallMaterial::Timber.thickness_m());
        assert_eq!(by_id("roof").half_extents_m[0], 6.0);
        let stone = StaticCoverObject {
            id: "ostrogorsk_church".into(),
            kind: StaticCoverKind::CityBuilding,
            ..barn.clone()
        };
        assert_eq!(wall_material(&stone), Some(WallMaterial::Stone), "a church is stone");
    }

    fn object(id: &str, kind: StaticCoverKind) -> StaticCoverObject {
        StaticCoverObject {
            id: id.to_string(),
            name: id.to_string(),
            kind,
            center: [0.0, 2.0, 0.0],
            half_extents_m: [4.0, 2.0, 3.0],
        }
    }

    /// The birth rule (urban-map PR-07): "ruin" in the id collapses a destructible object at
    /// birth - to rubble if its kind leaves rubble, to gone otherwise - and indestructible
    /// kinds ignore the tag entirely.
    #[test]
    fn ruin_ids_are_born_collapsed_by_their_kind() {
        assert_eq!(
            born_cover_phase_byte(&object("tenement_ruin_a", StaticCoverKind::CityBuilding)),
            1
        );
        assert_eq!(born_cover_phase_byte(&object("ruined_barn", StaticCoverKind::FarmBuilding)), 1);
        assert_eq!(born_cover_phase_byte(&object("yard_wall_ruin", StaticCoverKind::StoneWall)), 2);
        assert_eq!(born_cover_phase_byte(&object("ruined_hedge", StaticCoverKind::TreeLine)), 2);
        assert_eq!(born_cover_phase_byte(&object("ruined_wreck", StaticCoverKind::Wreck)), 1);
        assert_eq!(born_cover_phase_byte(&object("ruin_wall", StaticCoverKind::RailCover)), 0);
        assert_eq!(born_cover_phase_byte(&object("tenement_a", StaticCoverKind::CityBuilding)), 0);
        assert_eq!(
            initial_cover_phase_bytes(&[
                object("tenement_a", StaticCoverKind::CityBuilding),
                object("tenement_ruin_b", StaticCoverKind::CityBuilding),
            ]),
            vec![0, 1]
        );
    }
}
