//! The map compiler: `MapBlueprint` (data) → `BattlefieldMap` (the game's runtime truth) +
//! a [`MapReport`] of contract checks. Pure and deterministic — the same document compiles
//! to the same map on any machine, which is what lets the server and every client agree on
//! a world that never crosses the wire.

use terrain::{
    BattlefieldMap, HeightMap, MapFeature, RiverSpec, Road, ScatterRegion, SceneryInstance,
    SceneryKind, SpawnZone, StaticCoverKind, StaticCoverObject, StrategicPoint, WaterBody,
    ground_position, grounded_cover, grounded_feature, grounded_point, grounded_spawn_zone,
    heightmap_from_fn, inside_any_cover, scatter_mirrored,
};

use crate::blueprint::{
    GameplaySpec, MapBlueprint, ObjectSpec, RoadSpec, SceneryOp, SymmetrySpec, XCoord,
};
use crate::ops::EvalContext;
use crate::report::{MapReport, validate_map};

#[derive(Debug, thiserror::Error)]
pub enum ForgeError {
    #[error("blueprint parse: {0}")]
    Parse(String),
    #[error("grid: {0}")]
    Grid(String),
}

impl MapBlueprint {
    pub fn from_ron(source: &str) -> Result<Self, ForgeError> {
        ron::from_str(source).map_err(|error| ForgeError::Parse(error.to_string()))
    }

    /// Serialize the canonical RON form (shortest-round-trip floats, struct names on) — the
    /// same normalizer philosophy as the vehicle blueprint exporter: a hand-edited file can
    /// always be re-canonicalized.
    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new().struct_names(true).depth_limit(4);
        ron::ser::to_string_pretty(self, pretty).expect("blueprint serialization is total")
    }
}

/// The terrain ops as EVALUATED: `RoadProfile` resolves into a `Stroke` riding its named
/// road's polyline; everything else passes through. Both the compiler's sampling closure
/// and the backdrop skirt walk THIS list, never the raw document — the resolution is the
/// construction that makes paint/profile drift impossible. An unknown road id resolves to
/// nothing here and to an Error in the report (`check_road_profiles`): a live editor
/// session must survive every keystroke.
pub(crate) fn effective_terrain_ops(
    blueprint: &MapBlueprint,
) -> std::borrow::Cow<'_, [crate::blueprint::TerrainOp]> {
    use crate::blueprint::TerrainOp;
    if !blueprint.terrain.ops.iter().any(|op| matches!(op, TerrainOp::RoadProfile(_))) {
        // The common case borrows: the backdrop walks this per sample and must not pay an
        // allocation for maps that never use RoadProfile.
        return std::borrow::Cow::Borrowed(&blueprint.terrain.ops);
    }
    let roads = expand_roads(blueprint);
    std::borrow::Cow::Owned(
        blueprint
            .terrain
            .ops
            .iter()
            .filter_map(|op| match op {
                TerrainOp::RoadProfile(spec) => {
                    roads.iter().find(|road| road.id == spec.road_id).map(|road| {
                        TerrainOp::Stroke(crate::blueprint::StrokeSpec {
                            points: road.points.clone(),
                            profile: spec.profile,
                            half_width_m: spec.half_width_m,
                            falloff_m: spec.falloff_m,
                        })
                    })
                }
                other => Some(other.clone()),
            })
            .collect(),
    )
}

/// Compile a blueprint into the runtime battlefield plus its validation report.
pub fn compile(blueprint: &MapBlueprint) -> (BattlefieldMap, MapReport) {
    let samples = blueprint.grid.samples_per_side();
    let cell = blueprint.grid.cell_m;
    let axis_z = blueprint.grid.axis_z();
    let ops = effective_terrain_ops(blueprint);
    // Per-op cull rectangles: exact compact support (strokes), `None` = never cull. The
    // skip is bitwise invisible — outside its support an op is the identity — so the
    // backdrop skirt, which evaluates the same effective list without this table, stays
    // in agreement.
    let culls: Vec<Option<[f32; 4]>> = ops.iter().map(|op| op.influence_bounds()).collect();
    let heightmap = heightmap_from_fn(samples, cell, |x, z| {
        let ctx = EvalContext { river: blueprint.river.as_ref(), axis_z };
        let mut h = blueprint.terrain.base.eval(x);
        for (op, cull) in ops.iter().zip(&culls) {
            if let Some([x0, z0, x1, z1]) = cull
                && (x < *x0 || x > *x1 || z < *z0 || z > *z1)
            {
                continue;
            }
            h = op.apply(&ctx, x, z, h);
        }
        // The sculpt layer (D1): a pointwise per-sample delta over the program, then the
        // floor clamp again — a brush must not dig below the declared minimum.
        if let Some(sculpt) = &blueprint.sculpt {
            let xi = (x / cell).round() as u32;
            let zi = (z / cell).round() as u32;
            h = (h + sculpt.delta_m_at(zi * samples as u32 + xi)).max(blueprint.grid.min_height_m);
        }
        h
    });

    let water = blueprint.water.as_ref().map(|w| WaterBody { surface_level_m: w.surface_level_m });
    let standing_water: Vec<terrain::StandingWater> = blueprint
        .water
        .as_ref()
        .map(|w| {
            w.bodies
                .iter()
                .map(|body| terrain::StandingWater {
                    rect: body.rect,
                    surface_level_m: body.surface_level_m,
                })
                .collect()
        })
        .unwrap_or_default();
    let mut static_cover = expand_objects(blueprint, &heightmap);
    let roads = expand_roads(blueprint);
    let scenery = expand_scenery(blueprint, &heightmap, &static_cover, &roads);
    // A tree is not a painting: its bole stops a shell, blocks an eye and stands in a hull's
    // way until the hull pushes it over. Deriving the boxes HERE — from the scenery the scatter
    // just produced, sized off the very wood the ladder draws (X10: every authored species, not
    // the oak alone) — is what keeps that promise honest: every authored tree gets one, a
    // mirrored pair gets mirrored trunks, and the two can never drift apart the way a
    // hand-authored box list would. They come after `expand_scenery` on purpose: a tree has no
    // business avoiding its own trunk.
    static_cover.extend(tree_trunk_cover(&scenery));
    // A field stone over the belly line is the same promise (X5): the scatter just placed it,
    // the box is the bounds of the stone it placed, and a hull meets exactly what it sees.
    static_cover.extend(boulder_cover(&scenery));
    let (spawn_zones, strategic_points, features) = expand_gameplay(blueprint, &heightmap);
    let capture_zones = blueprint
        .gameplay
        .capture_zones
        .iter()
        .map(|zone| terrain::CaptureZone {
            id: zone.id.clone(),
            center: ground_position(&heightmap, zone.at[0], zone.at[1]),
            radius_m: zone.radius_m,
        })
        .collect();

    let map = BattlefieldMap {
        id: blueprint.meta.id.clone(),
        name: blueprint.meta.name.clone(),
        size_m: blueprint.grid.size_m,
        historical_basis: blueprint.meta.historical_basis.clone(),
        design_notes: blueprint.meta.design_notes.clone(),
        heightmap,
        water,
        standing_water,
        river: blueprint.river,
        spawn_zones,
        capture_zones,
        strategic_points,
        features,
        static_cover,
        scenery,
        roads,
    };
    let report = validate_map(blueprint, &map);
    (map, report)
}

/// Resolve an authored coordinate pair: fixed, or riding the river centerline. Author
/// mistakes never panic — `RiverCenter` in the z slot or on a riverless map falls back to
/// the map's centre, and the REPORT carries the Error (`check_river_dependencies`); a live
/// editor session must survive every keystroke.
fn resolve_x(at: [XCoord; 2], river: Option<&RiverSpec>, fallback: [f32; 2]) -> [f32; 2] {
    let z = match at[1] {
        XCoord::Fixed(z) => z,
        XCoord::RiverCenter | XCoord::RiverCenterAt(_) => fallback[1],
    };
    let x = match at[0] {
        XCoord::Fixed(x) => x,
        XCoord::RiverCenter => river.map_or(fallback[0], |river| river.center_x(z)),
        XCoord::RiverCenterAt(at_z) => river.map_or(fallback[0], |river| river.center_x(at_z)),
    };
    [x, z]
}

fn expand_objects(blueprint: &MapBlueprint, heightmap: &HeightMap) -> Vec<StaticCoverObject> {
    let river = blueprint.river.as_ref();
    let axis_z = blueprint.grid.axis_z();
    let fallback = [blueprint.grid.size_m[0] * 0.5, axis_z];
    let mut out = Vec::new();
    for object in &blueprint.objects {
        match object {
            ObjectSpec::Cover { id, name, kind, at, half_extents_m, yaw_rad } => {
                out.push(grounded_cover(
                    heightmap,
                    id,
                    name,
                    *kind,
                    resolve_x(*at, river, fallback),
                    *half_extents_m,
                    *yaw_rad,
                ));
            }
            ObjectSpec::TownGrid {
                id_prefix,
                name_prefix,
                kind,
                columns_x_m,
                row_offsets_m,
                wide_half_m,
                narrow_half_m,
                annex_share,
                yaw_rad,
            } => {
                // Immersja A2.2: a town is cast house by house, not stamped from two
                // moulds on a checkerboard — see `town_grid_cells`, the one place the
                // cells are decided (the editor counts them through it). The two halves
                // stay box-for-box identical: the variety is per house, the fairness per
                // pair, exactly like the scenery wave before it.
                let symmetry = blueprint.symmetry.unwrap_or(SymmetrySpec::MirrorZ);
                // X2: the grid is one district, turned as a whole about its own centre;
                // every house wears the district's yaw and the twin the twin's. At yaw 0
                // every position is the arithmetic it was.
                let district = town_grid_frame(columns_x_m, row_offsets_m, axis_z, *yaw_rad);
                let (south_yaw, north_yaw) = if *yaw_rad == 0.0 {
                    (0.0, 0.0)
                } else {
                    (*yaw_rad, symmetry.twin_yaw(*yaw_rad))
                };
                for cell in town_grid_cells(
                    columns_x_m,
                    row_offsets_m,
                    *wide_half_m,
                    *narrow_half_m,
                    *annex_share,
                ) {
                    let (column, row) = (cell.column, cell.row);
                    let south = district.place([cell.x, axis_z - cell.row_offset]);
                    let north = symmetry.twin(south, blueprint.grid.size_m);
                    for (side, at, yaw) in
                        [("south", south, south_yaw), ("north", north, north_yaw)]
                    {
                        out.push(grounded_cover(
                            heightmap,
                            &format!("{id_prefix}_c{column}_r{row}_{side}"),
                            &format!("{name_prefix} (column {column}, row {row}, {side})"),
                            *kind,
                            at,
                            cell.half,
                            yaw,
                        ));
                        if let Some(annex) = cell.annex {
                            // Behind the house — away from the axis — sharing its rear
                            // face, shifted toward one end: an L, not a T. The twin's
                            // annex mirrors with it; a turned district turns it with the
                            // house.
                            let annex_south = district.place([
                                cell.x + annex.x_shift,
                                axis_z - cell.row_offset - (cell.half[2] + annex.half[2]),
                            ]);
                            let annex_at = if side == "south" {
                                annex_south
                            } else {
                                symmetry.twin(annex_south, blueprint.grid.size_m)
                            };
                            out.push(grounded_cover(
                                heightmap,
                                &format!("{id_prefix}_c{column}_r{row}_{side}_annex"),
                                &format!(
                                    "{name_prefix} (column {column}, row {row}, {side}, annex)"
                                ),
                                *kind,
                                annex_at,
                                annex.half,
                                yaw,
                            ));
                        }
                    }
                }
            }
        }
    }
    out
}

fn expand_roads(blueprint: &MapBlueprint) -> Vec<Road> {
    let symmetry = blueprint.symmetry.unwrap_or(SymmetrySpec::MirrorZ);
    let size_m = blueprint.grid.size_m;
    let mut out = Vec::new();
    for road in &blueprint.roads {
        match road {
            RoadSpec::Road { id, surface, points, width_m } => {
                out.push(Road {
                    id: id.clone(),
                    surface: *surface,
                    points: points.clone(),
                    width_m: *width_m,
                });
            }
            RoadSpec::MirroredPair { id_base, surface, south_points, width_m } => {
                out.push(Road {
                    id: format!("{id_base}_south"),
                    surface: *surface,
                    points: south_points.clone(),
                    width_m: *width_m,
                });
                out.push(Road {
                    id: format!("{id_base}_north"),
                    surface: *surface,
                    points: south_points.iter().map(|p| symmetry.twin(*p, size_m)).collect(),
                    width_m: *width_m,
                });
            }
        }
    }
    out
}

/// The species an instanced tree kind draws as — the one answer the map compiler's trunk box
/// (X10) and `scene_build`'s ladder must agree on (locked there: `ladder_species` says the same).
pub fn trunk_species_for(kind: SceneryKind) -> Option<world_forge::tree::TreeSpecies> {
    use world_forge::tree::TreeSpecies;
    match kind {
        SceneryKind::Oak => Some(TreeSpecies::Oak),
        SceneryKind::Poplar => Some(TreeSpecies::Poplar),
        SceneryKind::FruitTree => Some(TreeSpecies::FruitTree),
        _ => None,
    }
}

/// How far the trunk box reaches past the bole's butt radius, metres: a hand for the bark's
/// relief, inside the lock's five centimetres (wood past the butt by more than
/// `LIMB_LEAVES_M` is a limb and ends the box).
const TRUNK_BOX_SKIN_M: f32 = 0.03;

/// An authored tree's bole as a gameplay solid, one box per tree (the one program's X10).
///
/// Sized to the VISIBLE bole, because the doctrine is that a cover box IS the footprint the eye
/// reads: the box is the bole's butt radius and its first limb, measured off the very wood mesh
/// the ladder draws (`world_forge::tree::authored::bole_metrics`, per species and variant, the
/// variant from the same seed the ladder grows it from), at the instance's scale, set into the
/// ground by the ladder's own sink. The crown above it is deliberately NOT covered — leaves do not
/// stop an AP round, and a box that pretended otherwise would hand crews cover they cannot see
/// themselves taking. Until X10 only the oak earned a box, from two literals of a generator that
/// no longer ships; the poplars of Bystra, Mazurski and Orliny and the orchards' fruit trees were
/// twenty-metre boles a hull drove through.
///
/// `TreeTrunk` is the kind that says exactly this and nothing more: crushable, so a hull pushes
/// the tree over instead of parking against it forever; destructible by shells; wrecked into the
/// same stumps a felled hedgerow leaves — and alone among the kinds it bakes no box of its own,
/// because the tree's mesh is already standing there.
fn tree_trunk_cover(scenery: &[SceneryInstance]) -> Vec<StaticCoverObject> {
    // A mirrored pair (the same nonzero `seed`) draws two individuals — its own variant each
    // side, A2.1's own plant — and the map report holds every cover box to its mirror twin. So
    // a pair's two boxes are ONE box: the wider bole's radius and the lower first limb of the
    // two, which still holds each bole and still ends under each first limb (the lock says so
    // of every tree), without re-rolling a forest the owner has reviewed.
    let mut boxes: Vec<(usize, [f32; 3], [f32; 3])> = scenery
        .iter()
        .enumerate()
        .filter_map(|(index, instance)| {
            tree_trunk_box_of(instance).map(|(center, half)| (index, center, half))
        })
        .collect();
    let twin_of = |index: usize| -> Option<usize> {
        let seed = scenery[index].seed;
        if seed == 0 {
            return None;
        }
        scenery
            .iter()
            .enumerate()
            .find(|(other, instance)| *other != index && instance.seed == seed)
            .map(|(other, _)| other)
    };
    let own: std::collections::HashMap<usize, [f32; 3]> =
        boxes.iter().map(|(index, _, half)| (*index, *half)).collect();
    for (index, center, half) in &mut boxes {
        if let Some(twin) = twin_of(*index)
            && let Some(twin_half) = own.get(&twin)
        {
            let ground = center[1] - half[1];
            let half_xz = half[0].max(twin_half[0]);
            let half_y = half[1].min(twin_half[1]);
            *half = [half_xz, half_y, half_xz];
            center[1] = ground + half_y;
        }
    }
    boxes
        .into_iter()
        .map(|(index, center, half)| {
            let kind = scenery[index].kind;
            StaticCoverObject {
                id: format!("{}_trunk_{index:03}", trunk_id_stem(kind)),
                name: format!("{} trunk", trunk_id_stem(kind)),
                kind: StaticCoverKind::TreeTrunk,
                center,
                half_extents_m: half,
                yaw_rad: 0.0,
            }
        })
        .collect()
}

fn trunk_id_stem(kind: SceneryKind) -> &'static str {
    match kind {
        SceneryKind::Oak => "oak",
        SceneryKind::Poplar => "poplar",
        SceneryKind::FruitTree => "fruit",
        _ => "tree",
    }
}

/// The ladder sets every trunk into its ground by this much (`scene_build::tree_lod::TRUNK_SINK_M`,
/// locked equal there): the box starts at the ground the eye sees the bole leave.
pub const TRUNK_SINK_M: f32 = 0.35;

/// The box one tree instance earns — its centre and half extents in the world — or `None` for
/// a kind the ladder does not draw as a tree. Public so the honesty lock can ask the same
/// question of the same tree.
pub fn tree_trunk_box_of(instance: &SceneryInstance) -> Option<([f32; 3], [f32; 3])> {
    let species = trunk_species_for(instance.kind)?;
    let seed = world_forge::tree::authored::instance_tree_seed(instance.position);
    let (variant, _mirrored) = world_forge::tree::authored::variant_of_seed(seed);
    let bole = world_forge::tree::authored::bole_metrics(species, variant)?;
    let scale = instance.scale;
    let half_xz = bole.bole_radius_m * scale + TRUNK_BOX_SKIN_M;
    // The bole from the ground up to the first limb, the mesh sunk by the ladder's rule.
    let top_over_ground = (bole.first_limb_m * scale - TRUNK_SINK_M).max(0.5);
    Some((
        [instance.position[0], instance.position[1] + top_over_ground * 0.5, instance.position[2]],
        [half_xz, top_over_ground * 0.5, half_xz],
    ))
}

/// A scattered field stone taller than the fleet's belly line as a gameplay solid, one box per
/// stone (the one program's X5).
///
/// Rule 8 of `docs/map-forge-policy.md`: a SOLID scenery object stays under the belly line, and
/// one that does not is cover. The scatters plant erratics of 0.35–1.45 m, so most of them are
/// cover — and until now they were ghosts: a hull drove through a metre of granite, a shell flew
/// through it, an eye saw through it. The box is the BOUNDS of the very mesh the picture draws
/// (`world_forge::rock::rock_seed` is the one seed both bake with), scaled and turned as the
/// instance is drawn, standing from the ground the stone is set into up to its crest. Within a
/// hull's step (X4) the stone is ground the hull climbs; taller, it is the wall it looks like.
/// The box bakes nothing of its own — the stone is already standing there.
fn boulder_cover(scenery: &[SceneryInstance]) -> Vec<StaticCoverObject> {
    let belly_line_m = game_core::fleet_belly_line_m();
    scenery
        .iter()
        .filter(|instance| instance.kind == SceneryKind::Rock)
        .enumerate()
        .filter_map(|(index, instance)| {
            let (center, half) = boulder_box_of(instance, belly_line_m)?;
            Some(StaticCoverObject {
                id: format!("boulder_{index:03}"),
                name: "field stone".to_string(),
                kind: StaticCoverKind::Boulder,
                center,
                half_extents_m: half,
                yaw_rad: instance.yaw_rad,
            })
        })
        .collect()
}

/// The box one scattered stone earns — its centre and half extents in the world, its yaw the
/// instance's — or `None` for a stone that stays under `belly_line_m` (loose dressing a hull
/// drives over). Public so the honesty locks can ask the same question of the same stone.
pub fn boulder_box_of(
    instance: &SceneryInstance,
    belly_line_m: f32,
) -> Option<([f32; 3], [f32; 3])> {
    if instance.kind != SceneryKind::Rock {
        return None;
    }
    let rock = world_forge::rock::bake_rock(
        world_forge::rock::RockForm::Erratic,
        world_forge::rock::rock_seed(instance.position, instance.seed),
    );
    let bounds = rock.body.bounds()?;
    let scale = instance.scale;
    let top_m = bounds.max.y * scale;
    if top_m <= belly_line_m {
        return None;
    }
    // The stone's plan in its own frame, SYMMETRIC about the instance's origin (the further of
    // the two reaches on each axis), turned as the picture turns it — the same rotation the box
    // wears, so the box's frame is the mesh's. Symmetric so that a mirrored pair, the same stone
    // at the mirrored yaw, earns one box mirrored: the report holds every box to its twin.
    let half_x = bounds.max.x.abs().max(bounds.min.x.abs()) * scale;
    let half_z = bounds.max.z.abs().max(bounds.min.z.abs()) * scale;
    Some((
        [instance.position[0], instance.position[1] + top_m * 0.5, instance.position[2]],
        [half_x.max(0.05), top_m * 0.5, half_z.max(0.05)],
    ))
}

/// One cell of a `TownGrid`: its house's box and, when the cell drew one, its annex (B6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TownGridCell {
    pub column: usize,
    pub row: usize,
    pub x: f32,
    pub row_offset: f32,
    pub half: [f32; 3],
    pub annex: Option<TownGridAnnex>,
}

/// An annex behind a grid house: its box and its shift along the house toward one end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TownGridAnnex {
    pub half: [f32; 3],
    pub x_shift: f32,
}

/// The cells of a `TownGrid`, decided ONCE here for the compiler and the editor alike.
/// Each cell picks its mould (wide / narrow / their blend) and jitters its extents —
/// footprint ±10 %, height ±6 % (the height feeds the rubble-sightline gameplay band, so it
/// moves less) — and, for `annex_share` of the cells, grows an annex: 0.42 × 0.55 × 0.45 of
/// the house (never under 1.8 × 1.5 × 1.8 m), shifted toward one end by up to 90 % of the
/// room the house leaves. All seeded from the CANONICAL cell (column x, row offset), which
/// both mirror twins share.
pub fn town_grid_cells(
    columns_x_m: &[f32],
    row_offsets_m: &[f32],
    wide_half_m: [f32; 3],
    narrow_half_m: [f32; 3],
    annex_share: f32,
) -> Vec<TownGridCell> {
    let mut cells = Vec::with_capacity(columns_x_m.len() * row_offsets_m.len());
    for (column, &x) in columns_x_m.iter().enumerate() {
        for (row, &row_offset) in row_offsets_m.iter().enumerate() {
            let pick = terrain::position_unit(x, row_offset, 0x7061);
            let base = if pick < 0.34 {
                wide_half_m
            } else if pick < 0.67 {
                narrow_half_m
            } else {
                [
                    (wide_half_m[0] + narrow_half_m[0]) * 0.5,
                    (wide_half_m[1] + narrow_half_m[1]) * 0.5,
                    (wide_half_m[2] + narrow_half_m[2]) * 0.5,
                ]
            };
            let stretch = |axis: u64, lo: f32, hi: f32| {
                lo + terrain::position_unit(x, row_offset, axis) * (hi - lo)
            };
            let half = [
                base[0] * stretch(0x7062, 0.9, 1.1),
                base[1] * stretch(0x7063, 0.94, 1.06),
                base[2] * stretch(0x7064, 0.9, 1.1),
            ];
            let annex = if terrain::position_unit(x, row_offset, 0x7065) < annex_share {
                let annex_half = [
                    (half[0] * 0.42).max(1.8),
                    (half[1] * 0.55).max(1.5),
                    (half[2] * 0.45).max(1.8),
                ];
                let room = (half[0] - annex_half[0]).max(0.0);
                let x_shift = room * 0.9 * (stretch(0x7066, 0.0, 2.0) - 1.0);
                Some(TownGridAnnex { half: annex_half, x_shift })
            } else {
                None
            };
            cells.push(TownGridCell { column, row, x, row_offset, half, annex });
        }
    }
    cells
}

/// X2: the frame a `TownGrid` is turned in — its centre (the mean column, the mean row on
/// the south side) and its yaw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TownGridFrame {
    pub center_xz: [f32; 2],
    pub yaw_rad: f32,
}

impl TownGridFrame {
    /// An unturned cell position turned about the district's centre. At yaw 0 the position
    /// comes back untouched — the square grid's arithmetic, bit for bit.
    pub fn place(&self, unturned: [f32; 2]) -> [f32; 2] {
        if self.yaw_rad == 0.0 {
            return unturned;
        }
        let frame = terrain::CoverBox {
            center: [self.center_xz[0], 0.0, self.center_xz[1]],
            half: [0.0; 3],
            yaw_rad: self.yaw_rad,
        };
        let world =
            frame.to_world([unturned[0] - self.center_xz[0], 0.0, unturned[1] - self.center_xz[1]]);
        [world[0], world[2]]
    }
}

/// The frame a `TownGrid` turns in, decided ONCE for the compiler and any reader of the grid.
pub fn town_grid_frame(
    columns_x_m: &[f32],
    row_offsets_m: &[f32],
    axis_z: f32,
    yaw_rad: f32,
) -> TownGridFrame {
    let mean = |values: &[f32]| values.iter().sum::<f32>() / values.len().max(1) as f32;
    TownGridFrame { center_xz: [mean(columns_x_m), axis_z - mean(row_offsets_m)], yaw_rad }
}

/// How many cover boxes a `TownGrid` emits: a south/north pair per cell, plus a pair of
/// annexes for the cells that drew one.
pub fn town_grid_member_count(
    columns_x_m: &[f32],
    row_offsets_m: &[f32],
    wide_half_m: [f32; 3],
    narrow_half_m: [f32; 3],
    annex_share: f32,
) -> usize {
    town_grid_cells(columns_x_m, row_offsets_m, wide_half_m, narrow_half_m, annex_share)
        .iter()
        .map(|cell| 2 + if cell.annex.is_some() { 2 } else { 0 })
        .sum()
}

fn expand_scenery(
    blueprint: &MapBlueprint,
    heightmap: &HeightMap,
    cover: &[StaticCoverObject],
    roads: &[Road],
) -> Vec<SceneryInstance> {
    let river = blueprint.river.as_ref();
    let axis_z = blueprint.grid.axis_z();
    let symmetry = blueprint.symmetry.unwrap_or(SymmetrySpec::MirrorZ);
    let size_m = blueprint.grid.size_m;
    let twin = |x: f32, z: f32| symmetry.twin([x, z], size_m);
    let mut out = Vec::new();
    for op in &blueprint.scenery {
        match op {
            SceneryOp::Scatter { seed, kind, pairs, region, exclude } => {
                let refused = |x: f32, z: f32| exclude.excluded(river, roads, cover, axis_z, x, z);
                scatter_mirrored(
                    *seed,
                    *kind,
                    *pairs,
                    ScatterRegion { x: (region.x[0], region.x[1]), z: (region.z[0], region.z[1]) },
                    &twin,
                    &|yaw| symmetry.twin_yaw(yaw),
                    heightmap,
                    &refused,
                    &mut out,
                );
            }
            SceneryOp::Row { kind, xs, z_start_m, z_step_m, count, yaw_rad, scale } => {
                for step in 0..*count {
                    let z = z_start_m + step as f32 * z_step_m;
                    for &x in xs {
                        push_paired(
                            &mut out, heightmap, *kind, x, z, *yaw_rad, *scale, symmetry, size_m,
                        );
                    }
                }
            }
            SceneryOp::Fixed { kind, spots, yaw_rad, scale } => {
                for &[x, z] in spots {
                    push_paired(
                        &mut out, heightmap, *kind, x, z, *yaw_rad, *scale, symmetry, size_m,
                    );
                }
            }
        }
    }
    out
}

/// A planted/fixed instance plus its fairness twin — the twin position and base yaw come
/// from the map's symmetry, grounded on the heightmap. A row is a row of PLANTS, not
/// stamped clones (Immersja A2.1): each pair jitters the authored scale ±12 % (seeded from
/// the canonical position and SHARED by the twins — an Oak's trunk becomes a cover box
/// scaled by the instance, and fairness demands identical twins), and each instance grows
/// its own yaw — a lamppost's arm stays over the road within a few degrees, everything
/// else swings freely.
#[expect(clippy::too_many_arguments)]
fn push_paired(
    out: &mut Vec<SceneryInstance>,
    heightmap: &HeightMap,
    kind: terrain::SceneryKind,
    x: f32,
    z: f32,
    yaw_rad: f32,
    scale: f32,
    symmetry: SymmetrySpec,
    size_m: [f32; 2],
) {
    let scale = scale * (0.88 + terrain::position_unit(x, z, 0x5CEA) * 0.24);
    let [tx, tz] = symmetry.twin([x, z], size_m);
    let pair_seed = terrain::pair_seed_at(x, z);
    for ([xx, zz], base_yaw) in [([x, z], yaw_rad), ([tx, tz], symmetry.twin_yaw(yaw_rad))] {
        let own = terrain::position_unit(xx, zz, 0x0A17);
        let yaw = match kind {
            terrain::SceneryKind::Lamppost => base_yaw + (own - 0.5) * 0.35,
            // A stone's twin is the same stone at the mirrored yaw (X5): its box must twin.
            kind if kind.twins_as_a_solid() => base_yaw,
            _ => own * std::f32::consts::TAU,
        };
        if let Some(ground) = heightmap.sample_height(xx, zz) {
            out.push(SceneryInstance {
                kind,
                position: [xx, ground, zz],
                yaw_rad: yaw,
                scale,
                seed: pair_seed,
            });
        }
    }
}

fn expand_gameplay(
    blueprint: &MapBlueprint,
    heightmap: &HeightMap,
) -> (Vec<SpawnZone>, Vec<StrategicPoint>, Vec<MapFeature>) {
    let river = blueprint.river.as_ref();
    let fallback = [blueprint.grid.size_m[0] * 0.5, blueprint.grid.axis_z()];
    let gameplay: &GameplaySpec = &blueprint.gameplay;
    let spawns = gameplay
        .spawns
        .iter()
        .map(|spawn| match spawn.radius_m {
            None => grounded_spawn_zone(
                heightmap,
                spawn.team,
                spawn.at[0],
                spawn.at[1],
                spawn.facing_yaw_rad,
            ),
            Some(radius_m) => SpawnZone {
                team: spawn.team,
                center: ground_position(heightmap, spawn.at[0], spawn.at[1]),
                radius_m,
                facing_yaw_rad: spawn.facing_yaw_rad,
            },
        })
        .collect();
    let points = gameplay
        .strategic_points
        .iter()
        .map(|point| {
            let [x, z] = resolve_x(point.at, river, fallback);
            grounded_point(heightmap, &point.id, &point.name, point.role, x, z, point.radius_m)
        })
        .collect();
    let features = gameplay
        .features
        .iter()
        .map(|feature| {
            let [x, z] = resolve_x(feature.at, river, fallback);
            grounded_feature(
                heightmap,
                feature.kind,
                &feature.name,
                x,
                z,
                feature.radius_m,
                &feature.note,
            )
        })
        .collect();
    (spawns, points, features)
}

impl crate::blueprint::Exclusion {
    /// The refusal walk: a point is rejected when ANY rule fires. Boolean order is free —
    /// every rule is a pure predicate.
    fn excluded(
        &self,
        river: Option<&RiverSpec>,
        roads: &[Road],
        cover: &[StaticCoverObject],
        axis_z: f32,
        x: f32,
        z: f32,
    ) -> bool {
        let river_d = river.map(|river| (x - river.center_x(z)).abs());
        if let (Some(margin), Some(d)) = (self.river_margin_m, river_d) {
            let corridor = river.expect("river_d implies a river").corridor_half_width_m;
            if d < corridor + margin {
                return true;
            }
        }
        if let (Some(max), Some(d)) = (self.river_max_m, river_d) {
            let corridor = river.expect("river_d implies a river").corridor_half_width_m;
            if d > corridor + max {
                return true;
            }
        }
        if let (Some(lanes), Some(d)) = (&self.crossing_lanes, river_d) {
            let corridor = river.expect("river_d implies a river").corridor_half_width_m;
            if d < corridor + lanes.band_margin_m {
                let dz = z - axis_z;
                for [offset, half_width] in &lanes.windows {
                    if (dz.abs() - offset).abs() < *half_width {
                        return true;
                    }
                }
            }
        }
        if let Some(band) = self.axis_band_m
            && (z - axis_z).abs() < band
        {
            return true;
        }
        for [cx, cz, radius] in &self.exclusion_circles {
            if (x - cx).powi(2) + (z - cz).powi(2) < radius * radius {
                return true;
            }
        }
        if let Some(margin) = self.road_margin_m
            && roads.iter().any(|road| road.distance_to(x, z) < road.width_m * 0.5 + margin)
        {
            return true;
        }
        if let Some(margin) = self.cover_margin_m
            && inside_any_cover(cover, x, z, margin)
        {
            return true;
        }
        false
    }
}

impl SymmetrySpec {
    /// The fairness twin of a world point under this symmetry. Every pairing rule in the
    /// pipeline — road expansion, town grids, scenery pairs, the report's twin hunts, the
    /// editor's ghosts — asks THIS function, so a new symmetry lands everywhere at once
    /// instead of leaking one hardcoded mirror at a time.
    pub fn twin(&self, point: [f32; 2], size_m: [f32; 2]) -> [f32; 2] {
        match self {
            SymmetrySpec::MirrorZ => [point[0], size_m[1] - point[1]],
            SymmetrySpec::Rot180 => [size_m[0] - point[0], size_m[1] - point[1]],
        }
    }

    /// The twin's base yaw: a reflection flips the sign, a half-turn adds π.
    pub fn twin_yaw(&self, yaw_rad: f32) -> f32 {
        match self {
            SymmetrySpec::MirrorZ => -yaw_rad,
            SymmetrySpec::Rot180 => yaw_rad + std::f32::consts::PI,
        }
    }

    /// Whether a point is its OWN twin (within `tolerance_m`): the fixed line of a mirror,
    /// the fixed centre of a half-turn. The report's pairing checks exempt these.
    pub fn is_self_twin(&self, point: [f32; 2], size_m: [f32; 2], tolerance_m: f32) -> bool {
        match self {
            // The historical predicate, verbatim: within a metre of the axis LINE.
            SymmetrySpec::MirrorZ => (point[1] - size_m[1] * 0.5).abs() < tolerance_m,
            SymmetrySpec::Rot180 => {
                let twin = self.twin(point, size_m);
                (twin[0] - point[0]).abs() < tolerance_m && (twin[1] - point[1]).abs() < tolerance_m
            }
        }
    }
}
