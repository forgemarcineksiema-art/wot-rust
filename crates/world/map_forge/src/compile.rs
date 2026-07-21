//! The map compiler: `MapBlueprint` (data) → `BattlefieldMap` (the game's runtime truth) +
//! a [`MapReport`] of contract checks. Pure and deterministic — the same document compiles
//! to the same map on any machine, which is what lets the server and every client agree on
//! a world that never crosses the wire.

use terrain::{
    BattlefieldMap, HeightMap, MapFeature, RiverSpec, Road, ScatterRegion, SceneryInstance,
    SpawnZone, StaticCoverObject, StrategicPoint, WaterBody, ground_position, grounded_cover,
    grounded_feature, grounded_point, grounded_spawn_zone, heightmap_from_fn, inside_any_cover,
    scatter_mirrored,
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

/// Compile a blueprint into the runtime battlefield plus its validation report.
pub fn compile(blueprint: &MapBlueprint) -> (BattlefieldMap, MapReport) {
    let samples = blueprint.grid.samples_per_side();
    let cell = blueprint.grid.cell_m;
    let axis_z = blueprint.grid.axis_z();
    let heightmap = heightmap_from_fn(samples, cell, |x, z| {
        let ctx = EvalContext { river: blueprint.river.as_ref(), axis_z };
        let mut h = blueprint.terrain.base.eval(x);
        for op in &blueprint.terrain.ops {
            h = op.apply(&ctx, x, z, h);
        }
        h
    });

    let water = blueprint.water.map(|w| WaterBody { surface_level_m: w.surface_level_m });
    let static_cover = expand_objects(blueprint, &heightmap);
    let roads = expand_roads(blueprint);
    let scenery = expand_scenery(blueprint, &heightmap, &static_cover, &roads);
    let (spawn_zones, strategic_points, features) = expand_gameplay(blueprint, &heightmap);

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
        for [cx, cz, radius] in &self.spawn_circles {
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
