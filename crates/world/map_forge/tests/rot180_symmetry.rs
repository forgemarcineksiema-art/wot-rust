//! Rot180 fairness locks (teren W6): the half-turn symmetry is a FIRST-CLASS fair map.
//! One fixture exercises every pairing rule in the pipeline — terrain probe, road
//! expansion, town grids, scenery pairs, cover/point/zone twin hunts, the sculpt layer —
//! and the negative tests prove each gate BITES when the rotation is broken. MirrorZ
//! regression is carried by the shipped-map goldens: the generalized twin machinery
//! compiles all four maps bit-for-bit (goldens.rs), so the identity is proven, not hoped.

use map_forge::blueprint::{
    BaseSpec, Gauss2Term, GridSpec, MapBlueprint, MetaSpec, ObjectSpec, RoadSpec, ScatterRect,
    SceneryOp, SculptSpec, SpawnSpec, StrategicPointSpec, SymmetrySpec, TerrainOp, TerrainProgram,
    XCoord,
};
use map_forge::compile;

/// A minimal Rot180 document: everything comes in half-turn pairs about (150, 150).
fn rot_square() -> MapBlueprint {
    MapBlueprint {
        meta: MetaSpec {
            version: map_forge::blueprint::BLUEPRINT_VERSION,
            id: "rot_probe".into(),
            name: "Rot probe".into(),
            historical_basis: "Synthetic half-turn test map".into(),
            design_notes: Vec::new(),
        },
        grid: GridSpec { size_m: [300.0, 300.0], cell_m: 5.0, min_height_m: 0.2 },
        symmetry: Some(SymmetrySpec::Rot180),
        river: None,
        horizon: None,
        terrain: TerrainProgram {
            base: BaseSpec::Constant(5.0),
            ops: vec![
                // A rot pair of knolls plus a self-twin hill on the fixed centre.
                TerrainOp::Gauss2 {
                    apply: map_forge::blueprint::Apply::Add,
                    terms: vec![
                        Gauss2Term { x: 90.0, z: 80.0, sx: 15.0, sz: 15.0, amp: 6.0 },
                        Gauss2Term { x: 210.0, z: 220.0, sx: 15.0, sz: 15.0, amp: 6.0 },
                        Gauss2Term { x: 150.0, z: 150.0, sx: 20.0, sz: 20.0, amp: 4.0 },
                    ],
                },
            ],
        },
        sculpt: None,
        water: None,
        materials: None,
        environment: None,
        objects: vec![
            ObjectSpec::Cover {
                id: "barn_a".into(),
                name: "barn (team 1)".into(),
                kind: terrain::StaticCoverKind::FarmBuilding,
                at: [XCoord::Fixed(140.0), XCoord::Fixed(60.0)],
                half_extents_m: [4.0, 3.0, 5.0],
            },
            ObjectSpec::Cover {
                id: "barn_b".into(),
                name: "barn (team 2)".into(),
                kind: terrain::StaticCoverKind::FarmBuilding,
                at: [XCoord::Fixed(160.0), XCoord::Fixed(240.0)],
                half_extents_m: [4.0, 3.0, 5.0],
            },
            ObjectSpec::TownGrid {
                id_prefix: "row".into(),
                name_prefix: "row house".into(),
                kind: terrain::StaticCoverKind::FarmBuilding,
                columns_x_m: vec![120.0],
                row_offsets_m: vec![40.0],
                wide_half_m: [5.0, 3.0, 4.0],
                narrow_half_m: [4.0, 2.5, 3.5],
            },
        ],
        scenery: vec![
            SceneryOp::Scatter {
                seed: 4242,
                kind: terrain::SceneryKind::Rock,
                pairs: 4,
                region: ScatterRect { x: [40.0, 140.0], z: [40.0, 140.0] },
                exclude: Default::default(),
            },
            SceneryOp::Fixed {
                kind: terrain::SceneryKind::Oak,
                spots: vec![[70.0, 120.0]],
                yaw_rad: 0.4,
                scale: 1.0,
            },
        ],
        roads: vec![RoadSpec::MirroredPair {
            id_base: "lane".into(),
            surface: terrain::RoadSurface::Dirt,
            south_points: vec![[60.0, 60.0], [120.0, 90.0]],
            width_m: 5.0,
        }],
        gameplay: map_forge::blueprint::GameplaySpec {
            spawns: vec![
                SpawnSpec { team: 1, at: [60.0, 40.0], facing_yaw_rad: 0.0, radius_m: None },
                SpawnSpec {
                    team: 2,
                    at: [240.0, 260.0],
                    facing_yaw_rad: std::f32::consts::PI,
                    radius_m: None,
                },
            ],
            strategic_points: vec![
                StrategicPointSpec {
                    id: "hold_a".into(),
                    name: "hold (team 1)".into(),
                    role: terrain::StrategicRole::Observation,
                    at: [XCoord::Fixed(100.0), XCoord::Fixed(70.0)],
                    radius_m: 30.0,
                },
                StrategicPointSpec {
                    id: "hold_b".into(),
                    name: "hold (team 2)".into(),
                    role: terrain::StrategicRole::Observation,
                    at: [XCoord::Fixed(200.0), XCoord::Fixed(230.0)],
                    radius_m: 30.0,
                },
                StrategicPointSpec {
                    id: "mid".into(),
                    name: "the middle".into(),
                    role: terrain::StrategicRole::Crossing,
                    at: [XCoord::Fixed(150.0), XCoord::Fixed(150.0)],
                    radius_m: 25.0,
                },
            ],
            capture_zones: vec![map_forge::blueprint::CaptureZoneSpec {
                id: "cap_mid".into(),
                at: [150.0, 150.0],
                radius_m: 20.0,
            }],
            features: Vec::new(),
        },
    }
}

/// The whole pipeline honours the half-turn: the report comes back clean, the road twin is
/// the ROTATED polyline, the town pair rotates about the centre with identical boxes, and
/// every scenery instance has its rot twin of the same kind and scale.
#[test]
fn a_rot180_map_compiles_fair_end_to_end() {
    let blueprint = rot_square();
    let (map, report) = compile(&blueprint);
    let errors: Vec<String> =
        report.errors().map(|entry| format!("{}: {}", entry.check, entry.message)).collect();
    assert!(errors.is_empty(), "a rot-paired document compiles clean:\n{}", errors.join("\n"));

    let north = map.roads.iter().find(|road| road.id == "lane_north").expect("the twin road");
    assert_eq!(north.points, vec![[240.0, 240.0], [180.0, 210.0]], "the twin is ROTATED");

    let south = map
        .static_cover
        .iter()
        .find(|object| object.id == "row_c0_r0_south")
        .expect("the town south house");
    let north_house = map
        .static_cover
        .iter()
        .find(|object| object.id == "row_c0_r0_north")
        .expect("the town north house");
    assert_eq!([south.center[0], south.center[2]], [120.0, 110.0]);
    assert_eq!(
        [north_house.center[0], north_house.center[2]],
        [180.0, 190.0],
        "the north house is the half-turn twin, not the mirror"
    );
    assert_eq!(south.half_extents_m, north_house.half_extents_m, "fairness shares the box");

    assert!(map.scenery.len() >= 8, "the scatter and the fixed oak landed");
    assert!(map.scenery.len().is_multiple_of(2), "dressing comes in twos");
    for instance in &map.scenery {
        let twin = [300.0 - instance.position[0], 300.0 - instance.position[2]];
        assert!(
            map.scenery.iter().any(|other| {
                other.kind == instance.kind
                    && (other.position[0] - twin[0]).abs() < 0.6
                    && (other.position[2] - twin[1]).abs() < 0.6
                    && (other.scale - instance.scale).abs() < 1.0e-3
            }),
            "instance at {:?} must have a rot twin (same kind and scale)",
            instance.position
        );
    }
}

/// The terrain probe BITES: a lone knoll (no rot partner) is a symmetry Error.
#[test]
fn a_lone_knoll_breaks_the_rot_probe() {
    let mut blueprint = rot_square();
    blueprint.terrain.ops.push(TerrainOp::Gauss2 {
        apply: map_forge::blueprint::Apply::Add,
        terms: vec![Gauss2Term { x: 70.0, z: 200.0, sx: 18.0, sz: 18.0, amp: 5.0 }],
    });
    let (_, report) = compile(&blueprint);
    assert!(
        report
            .errors()
            .any(|entry| entry.check == "symmetry" && entry.message.contains("heightfield")),
        "an unpaired knoll must break the height probe"
    );
}

/// The cover twin hunt BITES and names the orphan.
#[test]
fn a_missing_cover_twin_is_named() {
    let mut blueprint = rot_square();
    blueprint
        .objects
        .retain(|object| !matches!(object, ObjectSpec::Cover { id, .. } if id == "barn_b"));
    let (_, report) = compile(&blueprint);
    assert!(
        report.errors().any(|entry| entry.check == "symmetry" && entry.message.contains("barn_a")),
        "the orphaned barn must be named by the twin hunt"
    );
}

/// The legacy river machinery is fenced off half-turn maps: its fairness algebra
/// (centerline even about the axis, mirrored banks) would certify a REFLECTED river the
/// rotation never produces. Rot180 maps author standing water instead.
#[test]
fn the_river_is_fenced_off_rot_maps() {
    let mut blueprint = rot_square();
    blueprint.river = Some(terrain::RiverSpec {
        base_x_m: 150.0,
        axis_z_m: 150.0,
        bow_sigma_m: 80.0,
        bow_amp_m: 10.0,
        wiggle_amp_m: 3.0,
        wiggle_wave_m: 90.0,
        corridor_half_width_m: 20.0,
    });
    let (_, report) = compile(&blueprint);
    assert!(
        report.errors().any(|entry| entry.message.contains("MirrorZ only")),
        "a river on a Rot180 map must be refused loudly"
    );
}

/// The sculpt layer's fairness contract rotates too: a rot-paired stroke passes, an
/// unpaired sample is refused with the twin index named.
#[test]
fn the_sculpt_layer_pairs_by_rotation() {
    let side = 61u32; // 300 m / 5 m + 1
    let index = |xi: u32, zi: u32| zi * side + xi;
    let mut blueprint = rot_square();
    blueprint.sculpt =
        Some(SculptSpec { step_m: 0.05, samples: vec![(index(10, 10), 4), (index(50, 50), 4)] });
    let (_, report) = compile(&blueprint);
    assert!(
        !report.errors().any(|entry| entry.check == "sculpt"),
        "a rot-paired stroke passes the sculpt gate"
    );

    blueprint.sculpt = Some(SculptSpec { step_m: 0.05, samples: vec![(index(10, 10), 4)] });
    let (_, report) = compile(&blueprint);
    assert!(
        report
            .errors()
            .any(|entry| entry.check == "sculpt" && entry.message.contains("no mirror twin")),
        "an unpaired sculpt sample is refused"
    );
}
