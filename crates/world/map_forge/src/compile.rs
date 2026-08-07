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

    let water = blueprint.water.map(|w| WaterBody { surface_level_m: w.surface_level_m });
    let mut static_cover = expand_objects(blueprint, &heightmap);
    let roads = expand_roads(blueprint);
    let scenery = expand_scenery(blueprint, &heightmap, &static_cover, &roads);
    // An oak is not a painting: its trunk stops a shell, blocks an eye and stands in a
    // hull's way until the hull pushes it over. Deriving the boxes HERE — from the scenery the
    // scatter just produced — is what keeps that promise honest: every authored tree gets one,
    // a mirrored pair gets mirrored trunks, and the two can never drift apart the way a
    // hand-authored box list would. They come after `expand_scenery` on purpose: a tree has no
    // business avoiding its own trunk.
    static_cover.extend(oak_trunk_cover(&scenery));
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
            ObjectSpec::Cover { id, name, kind, at, half_extents_m } => {
                out.push(grounded_cover(
                    heightmap,
                    id,
                    name,
                    *kind,
                    resolve_x(*at, river, fallback),
                    *half_extents_m,
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
            } => {
                for (column, &x) in columns_x_m.iter().enumerate() {
                    for (row, &row_offset) in row_offsets_m.iter().enumerate() {
                        let half =
                            if (column + row) % 2 == 0 { wide_half_m } else { narrow_half_m };
                        for (side, sign) in [("south", -1.0_f32), ("north", 1.0_f32)] {
                            out.push(grounded_cover(
                                heightmap,
                                &format!("{id_prefix}_c{column}_r{row}_{side}"),
                                &format!("{name_prefix} (column {column}, row {row}, {side})"),
                                *kind,
                                [x, axis_z + sign * row_offset],
                                *half,
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
    let size_z = blueprint.grid.size_m[1];
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
                    points: south_points.iter().map(|p| [p[0], size_z - p[1]]).collect(),
                    width_m: *width_m,
                });
            }
        }
    }
    out
}

/// The procedural oak's trunk as a gameplay solid, one box per authored tree.
///
/// Sized to the VISIBLE trunk, because the doctrine is that a cover box IS the footprint the
/// eye reads: a mature procedural oak (`world_forge::tree`) stands ~17–18 m with a ~1.0 m
/// butt, and the column below the crown is what a hull meets and a shell stops in. The canopy
/// above it is deliberately NOT covered — leaves do not stop an AP round, and a box that
/// pretended otherwise would hand crews cover they cannot see themselves taking.
///
/// `TreeTrunk` is the kind that says exactly this and nothing more: crushable, so a hull pushes
/// the tree over instead of parking against it forever; destructible by shells; wrecked into the
/// same stumps a felled hedgerow leaves — and alone among the kinds it bakes no box of its own,
/// because the tree's mesh is already standing there.
fn oak_trunk_cover(scenery: &[SceneryInstance]) -> Vec<StaticCoverObject> {
    /// Half-width of the trunk at chest height on an unscaled tree, metres. The procedural
    /// oak's `trunk_radius` is 0.52 m; 0.6 covers the butt at chest height with a small
    /// margin for the deterministic lean.
    const TRUNK_HALF_M: f32 = 0.6;
    /// Half-height of the covered column: the clear bole under the first limbs (the oak's
    /// limbs start at ~55% of the 9.2 m trunk, i.e. ~5 m).
    const TRUNK_HALF_HEIGHT_M: f32 = 5.0;
    /// The procedural oak bakes at mature scale already — the only factor is the instance's
    /// own scatter scale. (The retired imported oak needed a 1.7 species fix; the procedural
    /// species table IS 1:1.)
    const SPECIES_SCALE: f32 = 1.0;

    scenery
        .iter()
        .filter(|instance| instance.kind == SceneryKind::Oak)
        .enumerate()
        .map(|(index, instance)| {
            let scale = instance.scale * SPECIES_SCALE;
            let half_height = TRUNK_HALF_HEIGHT_M * scale;
            StaticCoverObject {
                id: format!("oak_trunk_{index:03}"),
                name: "oak trunk".to_string(),
                kind: StaticCoverKind::TreeTrunk,
                center: [
                    instance.position[0],
                    instance.position[1] + half_height,
                    instance.position[2],
                ],
                half_extents_m: [TRUNK_HALF_M * scale, half_height, TRUNK_HALF_M * scale],
            }
        })
        .collect()
}

fn expand_scenery(
    blueprint: &MapBlueprint,
    heightmap: &HeightMap,
    cover: &[StaticCoverObject],
    roads: &[Road],
) -> Vec<SceneryInstance> {
    let river = blueprint.river.as_ref();
    let axis_z = blueprint.grid.axis_z();
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
                    axis_z,
                    heightmap,
                    &refused,
                    &mut out,
                );
            }
            SceneryOp::Row { kind, xs, z_start_m, z_step_m, count, yaw_rad, scale } => {
                for step in 0..*count {
                    let z = z_start_m + step as f32 * z_step_m;
                    for &x in xs {
                        push_mirrored(&mut out, heightmap, *kind, x, z, *yaw_rad, *scale, axis_z);
                    }
                }
            }
            SceneryOp::Fixed { kind, spots, yaw_rad, scale } => {
                for &[x, z] in spots {
                    push_mirrored(&mut out, heightmap, *kind, x, z, *yaw_rad, *scale, axis_z);
                }
            }
        }
    }
    out
}

/// A planted/fixed instance plus its northern twin — mirrors position and yaw across the
/// axis, grounded on the heightmap.
#[allow(clippy::too_many_arguments)]
fn push_mirrored(
    out: &mut Vec<SceneryInstance>,
    heightmap: &HeightMap,
    kind: terrain::SceneryKind,
    x: f32,
    z: f32,
    yaw_rad: f32,
    scale: f32,
    axis_z: f32,
) {
    for (zz, yaw) in [(z, yaw_rad), (axis_z * 2.0 - z, -yaw_rad)] {
        if let Some(ground) = heightmap.sample_height(x, zz) {
            out.push(SceneryInstance { kind, position: [x, ground, zz], yaw_rad: yaw, scale });
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
    /// The mirror image of z under this symmetry (`axis_z` comes from the grid).
    pub(crate) fn mirror_z(&self, z: f32, axis_z: f32) -> f32 {
        match self {
            SymmetrySpec::MirrorZ => axis_z * 2.0 - z,
        }
    }
}
