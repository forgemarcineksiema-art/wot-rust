//! The map report: the contract checks a battlefield must pass before it may ship. Every
//! entry carries its check name, a severity, a message and — where it helps — a world
//! position, so the editor can jump the camera straight to the problem.
//!
//! The report is the editor's early warning; the gameplay-side contract tests (river
//! physics constants, battle setup) stay in `sim`/`server` as the authoritative gate.

use terrain::{BattlefieldMap, StaticCoverObject};

use crate::blueprint::{MapBlueprint, TerrainOp, XCoord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Blocks shipping: `catalog::battlefield` refuses to hand out a map with errors.
    Error,
    /// Shown to the author; shippable, but deliberate.
    Warning,
}

#[derive(Debug, Clone)]
pub struct ReportEntry {
    pub check: &'static str,
    pub severity: Severity,
    pub message: String,
    pub at: Option<[f32; 3]>,
}

#[derive(Debug, Default)]
pub struct MapReport {
    pub entries: Vec<ReportEntry>,
}

impl MapReport {
    pub fn has_errors(&self) -> bool {
        self.entries.iter().any(|entry| entry.severity == Severity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &ReportEntry> {
        self.entries.iter().filter(|entry| entry.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &ReportEntry> {
        self.entries.iter().filter(|entry| entry.severity == Severity::Warning)
    }

    fn push(
        &mut self,
        check: &'static str,
        severity: Severity,
        message: impl Into<String>,
        at: Option<[f32; 3]>,
    ) {
        self.entries.push(ReportEntry { check, severity, message: message.into(), at });
    }
}

/// Thresholds the water contract is checked against. They are parameters (not imports) so
/// `map_forge` stays a world-layer crate; the editor and tests pass the gameplay constants
/// (`sim::DROWN_DEPTH_M` = 1.5, `physics::water::FORD_MAX_DEPTH_M` = 0.9 today).
#[derive(Debug, Clone, Copy)]
pub struct WaterThresholds {
    pub drown_depth_m: f32,
    pub ford_max_depth_m: f32,
}

impl Default for WaterThresholds {
    /// Mirrors the live gameplay constants; the sim-side contract test is the authoritative
    /// lock, so a deliberate change there forces a deliberate change here.
    fn default() -> Self {
        Self { drown_depth_m: 1.5, ford_max_depth_m: 0.9 }
    }
}

/// Run every contract check against the compiled map.
pub fn validate_map(blueprint: &MapBlueprint, map: &BattlefieldMap) -> MapReport {
    let mut report = MapReport::default();
    check_grid(blueprint, &mut report);
    check_sculpt(blueprint, &mut report);
    check_river_dependencies(blueprint, &mut report);
    check_presentation(blueprint, &mut report);
    check_heightmap_sane(blueprint, map, &mut report);
    check_in_bounds(map, &mut report);
    check_cover_overlap(map, &mut report);
    check_spawns(map, &mut report);
    check_roads(map, &mut report);
    check_scenery(map, &mut report);
    if blueprint.symmetry.is_some() {
        check_symmetry(blueprint, map, &mut report);
    }
    if map.water.is_some() && map.river.is_some() {
        check_water_contract(blueprint, map, &WaterThresholds::default(), &mut report);
    }
    check_playability(blueprint, map, &WaterThresholds::default(), &mut report);
    report
}

/// The grade an average hull still climbs, per horizontal metre - the drive graph's wall.
/// The road and crossing checks use 0.5 as "drivable-ish"; the graph allows a touch more
/// (a determined climb) so the report flags genuine walls, not spirited slopes.
const CLIMB_GRADE: f32 = 0.55;

/// M7: geometry was never the point - PLAYABILITY is. A coarse drive graph over the
/// heightfield (8-neighbour steps, grade within the climb wall, water shallower than
/// drowning, outside cover boxes) must connect EVERY spawn to every strategic point and
/// capture zone. A river map's crossing windows (sills and decks) must be NAMED by
/// Crossing points - the bots' route planner reads them, not the water. A big map with a
/// starved nav skeleton warns before the bots read as lobotomized.
fn check_playability(
    blueprint: &MapBlueprint,
    map: &BattlefieldMap,
    thresholds: &WaterThresholds,
    report: &mut MapReport,
) {
    let heightmap = &map.heightmap;
    let (width, height) = (heightmap.width(), heightmap.height());
    let cell = heightmap.cell_size_m();
    let passable: Vec<bool> = (0..width * height)
        .map(|index| {
            let (xi, zi) = (index % width, index / width);
            let ground = heightmap.sample_at_index(xi, zi);
            if let Some(water) = map.water
                && water.surface_level_m - ground >= thresholds.drown_depth_m
            {
                return false;
            }
            let (x, z) = (xi as f32 * cell, zi as f32 * cell);
            !terrain::inside_any_cover(&map.static_cover, x, z, 0.3)
        })
        .collect();

    let cell_of = |position: [f32; 3]| -> usize {
        let xi = (position[0] / cell).round().clamp(0.0, (width - 1) as f32) as usize;
        let zi = (position[2] / cell).round().clamp(0.0, (height - 1) as f32) as usize;
        zi * width + xi
    };
    // A start may sit on a cell inside cover (a point on a bridge parapet) - walk to the
    // nearest passable cell within a short leash before declaring anyone boxed in.
    let start_cell = |position: [f32; 3]| -> Option<usize> {
        let origin = cell_of(position);
        let (oxi, ozi) = ((origin % width) as isize, (origin / width) as isize);
        for radius in 0..3_isize {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let (xi, zi) = (oxi + dx, ozi + dz);
                    if xi >= 0 && zi >= 0 && (xi as usize) < width && (zi as usize) < height {
                        let index = zi as usize * width + xi as usize;
                        if passable[index] {
                            return Some(index);
                        }
                    }
                }
            }
        }
        None
    };

    for spawn in &map.spawn_zones {
        let Some(start) = start_cell(spawn.center) else {
            report.push(
                "playability",
                Severity::Error,
                format!("spawn team {} is boxed in - no passable ground around it", spawn.team),
                Some(spawn.center),
            );
            continue;
        };
        let mut reached = vec![false; passable.len()];
        let mut queue = std::collections::VecDeque::from([start]);
        reached[start] = true;
        while let Some(index) = queue.pop_front() {
            let (xi, zi) = ((index % width) as isize, (index / width) as isize);
            let here = heightmap.sample_at_index(index % width, index / width);
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let (nx, nz) = (xi + dx, zi + dz);
                if nx < 0 || nz < 0 || nx as usize >= width || nz as usize >= height {
                    continue;
                }
                let next = nz as usize * width + nx as usize;
                if reached[next] || !passable[next] {
                    continue;
                }
                let there = heightmap.sample_at_index(nx as usize, nz as usize);
                let distance = cell * ((dx * dx + dz * dz) as f32).sqrt();
                if ((there - here) / distance).abs() > CLIMB_GRADE {
                    continue;
                }
                reached[next] = true;
                queue.push_back(next);
            }
        }
        let mut assert_reaches = |what: String, position: [f32; 3]| {
            let target = start_cell(position);
            if target.is_none_or(|target| !reached[target]) {
                report.push(
                    "playability",
                    Severity::Error,
                    format!("{what} is unreachable from spawn team {}", spawn.team),
                    Some(position),
                );
            }
        };
        for point in &map.strategic_points {
            assert_reaches(format!("strategic point '{}'", point.id), point.position);
        }
        for zone in &map.capture_zones {
            assert_reaches(format!("capture zone '{}'", zone.id), zone.center);
        }
    }

    // River maps: every crossing window (ford sill, deck) is NAMED by a Crossing point.
    if let Some(river) = &map.river
        && map.water.is_some()
    {
        let axis_z = blueprint.grid.axis_z();
        let mut windows: Vec<(f32, &'static str)> = Vec::new();
        for op in &blueprint.terrain.ops {
            match op {
                TerrainOp::CarveChannel { sills, .. } => {
                    windows.extend(sills.iter().map(|[z, _]| (*z, "ford sill")));
                }
                TerrainOp::Deck { dz_m, .. } => windows.push((axis_z + dz_m, "deck")),
                _ => {}
            }
        }
        for (window_z, what) in windows {
            let x = river.center_x(window_z);
            let named = map.strategic_points.iter().any(|point| {
                point.role == terrain::StrategicRole::Crossing
                    && (point.position[0] - x).powi(2) + (point.position[2] - window_z).powi(2)
                        < 40.0 * 40.0
            });
            if !named {
                let ground = map.heightmap.sample_height(x, window_z).unwrap_or(0.0);
                report.push(
                    "playability",
                    Severity::Error,
                    format!(
                        "the {what} at z {window_z:.0} has no Crossing point - the bots route \
                         by POINTS, not by water"
                    ),
                    Some([x, ground, window_z]),
                );
            }
        }
    }

    // Nav-skeleton density: below ~8 points on a real battle map the route planner starves.
    if map.size_m[0] >= 500.0 && map.strategic_points.len() < 8 {
        report.push(
            "playability",
            Severity::Warning,
            format!(
                "only {} strategic points on a {:.0} m map - the bots' route planner starves \
                 below ~8",
                map.strategic_points.len(),
                map.size_m[0]
            ),
            None,
        );
    }
}

/// The compiler samples a SQUARE grid (`samples_per_side` reads `size_m[0]`) and the mirror
/// walk steps both axes with one stride; a rectangular declaration would compile a silently
/// wrong map, so it is refused here instead. A river must also share the grid's symmetry
/// axis when the map declares one — the centerline's mirror-fairness is even about
/// `river.axis_z_m`, not about the grid.
fn check_grid(blueprint: &MapBlueprint, report: &mut MapReport) {
    let [w, d] = blueprint.grid.size_m;
    if (w - d).abs() > f32::EPSILON {
        report.push(
            "grid",
            Severity::Error,
            format!("grid must be square (got {w} x {d}) — the compiler samples one side"),
            None,
        );
    }
    if blueprint.symmetry.is_some()
        && let Some(river) = &blueprint.river
        && (river.axis_z_m - blueprint.grid.axis_z()).abs() > 1.0e-3
    {
        report.push(
            "grid",
            Severity::Error,
            format!(
                "river axis_z_m {} disagrees with the symmetry axis {} — the mirrored halves \
                 would get different water",
                river.axis_z_m,
                blueprint.grid.axis_z()
            ),
            None,
        );
    }
}

/// The sculpt layer's contract (map-editor D1): canonical form (sorted, no duplicates,
/// in range), a sane quantum, a ZERO border ring (that is what keeps the analytic
/// backdrop/apron seam exact — the skirt never reads the sculpt), and on a mirrored map a
/// sample-for-sample mirror (the editor stamps both halves by construction; a hand edit
/// that breaks it is refused here, not silently shipped).
fn check_sculpt(blueprint: &MapBlueprint, report: &mut MapReport) {
    let Some(sculpt) = &blueprint.sculpt else { return };
    let mut push = |message: String| report.push("sculpt", Severity::Error, message, None);
    if !(0.01..=0.5).contains(&sculpt.step_m) {
        push(format!("step_m {} leaves the sane 0.01..0.5 quantum band", sculpt.step_m));
    }
    let side = blueprint.grid.samples_per_side() as u32;
    let total = side * side;
    let mut previous: Option<u32> = None;
    for &(index, quanta) in &sculpt.samples {
        if index >= total {
            push(format!("sample index {index} is outside the {side}x{side} grid"));
            break;
        }
        if previous.is_some_and(|previous| previous >= index) {
            push(format!("samples are not sorted-unique at index {index} (canonical form)"));
            break;
        }
        previous = Some(index);
        let (xi, zi) = (index % side, index / side);
        if quanta != 0 && (xi == 0 || zi == 0 || xi == side - 1 || zi == side - 1) {
            push(format!(
                "sample {index} sculpts the border ring — the apron seam must stay exact"
            ));
            break;
        }
    }
    if blueprint.symmetry.is_some() {
        for &(index, quanta) in &sculpt.samples {
            let (xi, zi) = (index % side, index / side);
            let mirrored = (side - 1 - zi) * side + xi;
            if sculpt.delta_m_at(mirrored) != f32::from(quanta) * sculpt.step_m {
                report.push(
                    "sculpt",
                    Severity::Error,
                    format!(
                        "sample {index} has no mirror twin at {mirrored} — the fairness \
                         contract covers brush strokes too"
                    ),
                    None,
                );
                break;
            }
        }
    }
}

/// Everything river-relative needs a river. Evaluation is total either way (the masks
/// vanish, `RiverCenter` falls back to the map's centre — an editor session survives every
/// keystroke), so THIS is where the mistake becomes visible: each offender is an Error.
/// `RiverCenter` in the z slot is a schema misuse and errors regardless of the river.
fn check_river_dependencies(blueprint: &MapBlueprint, report: &mut MapReport) {
    let mut offend = |what: String| {
        report.push("river", Severity::Error, what, None);
    };
    let riverless = blueprint.river.is_none();
    if riverless {
        for (index, op) in blueprint.terrain.ops.iter().enumerate() {
            match op {
                TerrainOp::RiverBandAdd { .. }
                | TerrainOp::CarveChannel { .. }
                | TerrainOp::Deck { .. } => {
                    offend(format!("terrain op {index} ({op:?}) needs a river"));
                }
                TerrainOp::Relief { dampen, .. } => {
                    if dampen
                        .iter()
                        .any(|mask| matches!(mask, crate::blueprint::DampMask::RiverBand { .. }))
                    {
                        offend(format!("terrain op {index}: RiverBand damping needs a river"));
                    }
                }
                _ => {}
            }
        }
        for op in &blueprint.scenery {
            if let crate::blueprint::SceneryOp::Scatter { exclude, .. } = op
                && (exclude.river_margin_m.is_some()
                    || exclude.river_max_m.is_some()
                    || exclude.crossing_lanes.is_some())
            {
                offend("a scatter's river exclusion rules need a river".to_string());
            }
        }
    }
    let mut check_at = |what: &str, at: [XCoord; 2]| {
        if matches!(at[1], XCoord::RiverCenter | XCoord::RiverCenterAt(_)) {
            report.push(
                "river",
                Severity::Error,
                format!("{what}: RiverCenter is an x coordinate, not a z coordinate"),
                None,
            );
        }
        if riverless && matches!(at[0], XCoord::RiverCenter | XCoord::RiverCenterAt(_)) {
            report.push(
                "river",
                Severity::Error,
                format!("{what} rides the river centerline, but the map has no river"),
                None,
            );
        }
    };
    for object in &blueprint.objects {
        if let crate::blueprint::ObjectSpec::Cover { id, at, .. } = object {
            check_at(&format!("cover '{id}'"), *at);
        }
    }
    for point in &blueprint.gameplay.strategic_points {
        check_at(&format!("strategic point '{}'", point.id), point.at);
    }
    for feature in &blueprint.gameplay.features {
        check_at(&format!("feature '{}'", feature.name), feature.at);
    }
}

/// The presentation sections hold the art-direction discipline as data contracts: ground
/// albedos stay inside the policy window (vegetation saturation ≤ 0.45, and the field-patch
/// band's worst-case +17% lift keeps soil looking like soil, ≤ 0.62), strengths stay 0..1,
/// and the look list is coherent (non-empty, one look per variant, rain/wetness in 0..1).
fn check_presentation(blueprint: &MapBlueprint, report: &mut MapReport) {
    if let Some(materials) = &blueprint.materials {
        let saturation = |rgb: [f32; 3]| {
            let max = rgb[0].max(rgb[1]).max(rgb[2]);
            let min = rgb[0].min(rgb[1]).min(rgb[2]);
            if max <= 0.0 { 0.0 } else { (max - min) / max }
        };
        let luminance = |rgb: [f32; 3]| 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
        for (index, layer) in materials.layers.iter().enumerate() {
            if saturation(layer.albedo) > 0.45 + 1.0e-3 {
                report.push(
                    "materials",
                    Severity::Error,
                    format!(
                        "ground layer {index} saturation {:.3} leaves the policy window (≤ 0.45)",
                        saturation(layer.albedo)
                    ),
                    None,
                );
            }
            if layer.albedo.iter().any(|c| !(0.05..=0.7).contains(c)) {
                report.push(
                    "materials",
                    Severity::Error,
                    format!("ground layer {index} albedo {:?} leaves 0.05..0.7", layer.albedo),
                    None,
                );
            }
            // Vegetation layers (R grass, G straw) ride the field-patch band's worst-case
            // +17% plot lift; earth layers (B dirt, A rock) cut through it unchanged.
            if index < 2 {
                let lifted_max = layer.albedo.iter().map(|c| c * 1.17).fold(0.0_f32, f32::max);
                if lifted_max > 0.62 {
                    report.push(
                        "materials",
                        Severity::Error,
                        format!(
                            "ground layer {index} lifts to {lifted_max:.3} — a plot must stay soil, not neon"
                        ),
                        None,
                    );
                }
            }
            if !(0.0..=1.2).contains(&layer.detail) || !(0.0..=0.3).contains(&layer.gloss) {
                report.push(
                    "materials",
                    Severity::Error,
                    format!(
                        "ground layer {index} detail {} / gloss {} leave the ground envelope",
                        layer.detail, layer.gloss
                    ),
                    None,
                );
            }
        }
        // Value separation, not just hue: straw out-lumes grass, broken rock out-lumes dirt.
        let luma = |index: usize| luminance(materials.layers[index].albedo);
        if luma(1) <= luma(0) {
            report.push(
                "materials",
                Severity::Error,
                "dry straw must read lighter than lush grass".to_string(),
                None,
            );
        }
        if luma(3) <= luma(2) {
            report.push(
                "materials",
                Severity::Error,
                "broken rock must read lighter than worn dirt".to_string(),
                None,
            );
        }
        for (name, value) in [
            ("macro_normal_strength", materials.macro_normal_strength),
            ("field_patch_strength", materials.field_patch_strength),
        ] {
            if !(0.0..=1.0).contains(&value) {
                report.push(
                    "materials",
                    Severity::Error,
                    format!("{name} {value} is outside 0..1"),
                    None,
                );
            }
        }
    }
    if let Some(environment) = &blueprint.environment {
        if environment.looks.is_empty() {
            report.push(
                "environment",
                Severity::Error,
                "an authored environment must ship at least its default look".to_string(),
                None,
            );
        }
        for (index, look) in environment.looks.iter().enumerate() {
            if environment.looks[..index].iter().any(|other| other.variant == look.variant) {
                report.push(
                    "environment",
                    Severity::Error,
                    format!("variant {:?} is authored twice", look.variant),
                    None,
                );
            }
            for (name, value) in
                [("rain_intensity", look.rain_intensity), ("wetness", look.wetness)]
            {
                if !(0.0..=1.0).contains(&value) {
                    report.push(
                        "environment",
                        Severity::Error,
                        format!("{:?} {name} {value} is outside 0..1", look.variant),
                        None,
                    );
                }
            }
        }
    }
}

fn check_heightmap_sane(blueprint: &MapBlueprint, map: &BattlefieldMap, report: &mut MapReport) {
    for (index, &h) in map.heightmap.samples().iter().enumerate() {
        if h.is_nan() {
            let x = (index % map.heightmap.width()) as f32 * map.heightmap.cell_size_m();
            let z = (index / map.heightmap.width()) as f32 * map.heightmap.cell_size_m();
            report.push("heightmap_sane", Severity::Error, "NaN height sample", Some([x, 0.0, z]));
            return;
        }
    }
    let stats = map.heightmap.stats();
    if stats.min_m < blueprint.grid.min_height_m - 1.0e-3 {
        report.push(
            "heightmap_sane",
            Severity::Error,
            format!(
                "terrain dips to {:.3} below the declared floor {:.3}",
                stats.min_m, blueprint.grid.min_height_m
            ),
            None,
        );
    }
}

/// Everything placed must sit inside the authored rectangle.
fn check_in_bounds(map: &BattlefieldMap, report: &mut MapReport) {
    let [w, d] = map.size_m;
    let inside = |x: f32, z: f32| x >= 0.0 && x <= w && z >= 0.0 && z <= d;
    for cover in &map.static_cover {
        if !inside(
            cover.center[0] - cover.half_extents_m[0],
            cover.center[2] - cover.half_extents_m[2],
        ) || !inside(
            cover.center[0] + cover.half_extents_m[0],
            cover.center[2] + cover.half_extents_m[2],
        ) {
            report.push(
                "in_bounds",
                Severity::Error,
                format!("cover '{}' reaches outside the map", cover.id),
                Some(cover.center),
            );
        }
    }
    for point in &map.strategic_points {
        if !inside(point.position[0], point.position[2]) {
            report.push(
                "in_bounds",
                Severity::Error,
                format!("strategic point '{}' outside the map", point.id),
                Some(point.position),
            );
        }
    }
    for zone in &map.capture_zones {
        if !inside(zone.center[0], zone.center[2]) {
            report.push(
                "in_bounds",
                Severity::Error,
                format!("capture zone '{}' outside the map", zone.id),
                Some(zone.center),
            );
        }
    }
    for zone in &map.spawn_zones {
        if !inside(zone.center[0], zone.center[2]) {
            report.push(
                "in_bounds",
                Severity::Error,
                format!("spawn team {} outside the map", zone.team),
                Some(zone.center),
            );
        }
    }
    for road in &map.roads {
        for point in &road.points {
            if !inside(point[0], point[1]) {
                report.push(
                    "in_bounds",
                    Severity::Error,
                    format!("road '{}' waypoint outside the map", road.id),
                    Some([point[0], 0.0, point[1]]),
                );
            }
        }
    }
}

/// Static cover boxes must not interpenetrate (a hull deserves to know which box it hit).
fn check_cover_overlap(map: &BattlefieldMap, report: &mut MapReport) {
    let overlaps = |a: &StaticCoverObject, b: &StaticCoverObject| {
        (a.center[0] - b.center[0]).abs() < a.half_extents_m[0] + b.half_extents_m[0]
            && (a.center[2] - b.center[2]).abs() < a.half_extents_m[2] + b.half_extents_m[2]
    };
    for (i, a) in map.static_cover.iter().enumerate() {
        for b in &map.static_cover[i + 1..] {
            if overlaps(a, b) {
                report.push(
                    "cover_overlap",
                    Severity::Warning,
                    format!("cover '{}' interpenetrates '{}'", a.id, b.id),
                    Some(a.center),
                );
            }
        }
    }
}

/// Nobody deploys into a wall, into the water, or onto a cliff; both teams field equally.
fn check_spawns(map: &BattlefieldMap, report: &mut MapReport) {
    let mut team_counts = std::collections::BTreeMap::<u16, usize>::new();
    for zone in &map.spawn_zones {
        *team_counts.entry(zone.team).or_default() += 1;
        for cover in &map.static_cover {
            let dx = (zone.center[0] - cover.center[0]).abs() - cover.half_extents_m[0];
            let dz = (zone.center[2] - cover.center[2]).abs() - cover.half_extents_m[2];
            if dx.max(dz) < zone.radius_m {
                report.push(
                    "spawns",
                    Severity::Error,
                    format!("spawn team {} reaches cover '{}'", zone.team, cover.id),
                    Some(zone.center),
                );
            }
        }
        for (dx, dz) in [(0.0, 0.0), (-25.0, -25.0), (25.0, -25.0), (-25.0, 25.0), (25.0, 25.0)] {
            let x = zone.center[0] + dx;
            let z = zone.center[2] + dz;
            if let Some(h) = map.heightmap.sample_height(x, z) {
                if let Some(water) = map.water
                    && water.depth_over(h) > 0.0
                {
                    report.push(
                        "spawns",
                        Severity::Error,
                        format!("spawn team {} ground is wet at ({x}, {z})", zone.team),
                        Some([x, h, z]),
                    );
                }
                if let Some(ahead) = map.heightmap.sample_height(x, z + 10.0)
                    && ((ahead - h) / 10.0).abs() >= 0.2
                {
                    report.push(
                        "spawns",
                        Severity::Warning,
                        format!("spawn team {} approach is steep at ({x}, {z})", zone.team),
                        Some([x, h, z]),
                    );
                }
            }
        }
    }
    let mut counts = team_counts.values();
    if counts.next() != counts.next() {
        report.push("spawns", Severity::Error, "teams do not field equally".to_string(), None);
    }
}

/// Roads stay on the map and drivable-ish: no 5 m step along the polyline exceeds a 0.5 grade.
fn check_roads(map: &BattlefieldMap, report: &mut MapReport) {
    for road in &map.roads {
        for pair in road.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            let steps = (length / 5.0).ceil().max(1.0) as u32;
            let mut previous: Option<f32> = None;
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                let x = a[0] + (b[0] - a[0]) * t;
                let z = a[1] + (b[1] - a[1]) * t;
                let Some(h) = map.heightmap.sample_height(x, z) else { continue };
                if let Some(previous) = previous {
                    let grade = ((h - previous) / 5.0).abs();
                    if grade >= 0.5 {
                        report.push(
                            "roads",
                            Severity::Warning,
                            format!("road '{}' hits a {grade:.2} grade", road.id),
                            Some([x, h, z]),
                        );
                    }
                }
                previous = Some(h);
            }
        }
    }
}

/// Dressing obeys its own rules: grounded, out of cover, and (when mirrored) in pairs.
fn check_scenery(map: &BattlefieldMap, report: &mut MapReport) {
    let [w, d] = map.size_m;
    for instance in &map.scenery {
        let [x, _, z] = instance.position;
        if !(0.0..=w).contains(&x) || !(0.0..=d).contains(&z) {
            report.push(
                "scenery",
                Severity::Warning,
                format!("{:?} grows outside the map", instance.kind),
                Some(instance.position),
            );
        }
        if map.static_cover.iter().any(|c| {
            (x - c.center[0]).abs() < c.half_extents_m[0]
                && (z - c.center[2]).abs() < c.half_extents_m[2]
        }) {
            report.push(
                "scenery",
                Severity::Warning,
                format!("{:?} grows through a cover footprint", instance.kind),
                Some(instance.position),
            );
        }
    }
}

/// The fairness contract: heightfield mirrors within 1 mm, spawns/points/cover/scenery are
/// on-axis or come in mirror twins (the legacy tests' tolerances, kept).
fn check_symmetry(blueprint: &MapBlueprint, map: &BattlefieldMap, report: &mut MapReport) {
    let Some(symmetry) = blueprint.symmetry else { return };
    let axis_z = blueprint.grid.axis_z();
    let size_z = blueprint.grid.size_m[1];
    let mut max_delta = 0.0_f32;
    let mut worst = None;
    for zi in 0..=50 {
        for xi in 0..=50 {
            let x = xi as f32 * (size_z / 50.0);
            let z = zi as f32 * (size_z / 50.0);
            let here = map.heightmap.sample_height(x, z).unwrap_or(f32::NAN);
            let mirrored =
                map.heightmap.sample_height(x, symmetry.mirror_z(z, axis_z)).unwrap_or(f32::NAN);
            let delta = (here - mirrored).abs();
            if delta > max_delta {
                max_delta = delta;
                worst = Some([x, here, z]);
            }
        }
    }
    if max_delta >= 1.0e-3 {
        report.push(
            "symmetry",
            Severity::Error,
            format!("heightfield mirror broke: max delta {max_delta}"),
            worst,
        );
    }

    let has_twin = |center: [f32; 3], half: Option<[f32; 3]>, others: &[StaticCoverObject]| {
        others.iter().any(|other| {
            (other.center[0] - center[0]).abs() < 1.0
                && (other.center[2] - symmetry.mirror_z(center[2], axis_z)).abs() < 1.0
                && half.is_none_or(|h| other.half_extents_m == h)
        })
    };
    for cover in &map.static_cover {
        if (cover.center[2] - axis_z).abs() < 1.0 {
            continue;
        }
        if !has_twin(cover.center, Some(cover.half_extents_m), &map.static_cover) {
            report.push(
                "symmetry",
                Severity::Error,
                format!("cover '{}' has no mirror twin", cover.id),
                Some(cover.center),
            );
        }
    }
    for point in &map.strategic_points {
        if (point.position[2] - axis_z).abs() < 1.0 {
            continue;
        }
        let twin = map.strategic_points.iter().any(|other| {
            other.role == point.role
                && (other.position[0] - point.position[0]).abs() < 1.0
                && (other.position[2] - symmetry.mirror_z(point.position[2], axis_z)).abs() < 1.0
        });
        if !twin {
            report.push(
                "symmetry",
                Severity::Error,
                format!("strategic point '{}' has no mirror twin", point.id),
                Some(point.position),
            );
        }
    }
    for zone in &map.capture_zones {
        if (zone.center[2] - axis_z).abs() < 1.0 {
            continue;
        }
        let twin = map.capture_zones.iter().any(|other| {
            (other.center[0] - zone.center[0]).abs() < 1.0
                && (other.center[2] - symmetry.mirror_z(zone.center[2], axis_z)).abs() < 1.0
        });
        if !twin {
            report.push(
                "symmetry",
                Severity::Error,
                format!("capture zone '{}' has no mirror twin", zone.id),
                Some(zone.center),
            );
        }
    }
    if !map.scenery.len().is_multiple_of(2) {
        report.push("symmetry", Severity::Warning, "mirrored dressing comes in twos", None);
    }
}

/// The river's gameplay contract against the physics thresholds: the current drowns between
/// the crossings, the sills stay fordable, the decks stand clear, no puddles escape the
/// corridor, and every crossing approach is drivable.
fn check_water_contract(
    blueprint: &MapBlueprint,
    map: &BattlefieldMap,
    thresholds: &WaterThresholds,
    report: &mut MapReport,
) {
    let Some(water) = map.water else { return };
    let Some(river) = map.river else { return };
    let axis_z = blueprint.grid.axis_z();

    // Crossing windows come from the blueprint itself: ford sills (3σ skirts) and decks.
    let mut windows: Vec<(f32, f32)> = Vec::new();
    let mut deck_checks: Vec<(f32, f32, f32)> = Vec::new(); // (dz, half_length, freeboard)
    for op in &blueprint.terrain.ops {
        match op {
            TerrainOp::CarveChannel { sills, .. } => {
                for [sill_z, sill_sigma] in sills {
                    windows.push((sill_z - axis_z, 3.0 * sill_sigma));
                }
            }
            TerrainOp::Deck { dz_m, half_width_m, half_length_m, .. } => {
                windows.push((*dz_m, half_width_m + 19.0));
                let freeboard = if *half_width_m >= 6.0 { 1.0 } else { 0.3 };
                deck_checks.push((*dz_m, *half_length_m, freeboard));
            }
            _ => {}
        }
    }
    let in_crossing_window =
        |z: f32| windows.iter().any(|(dz, half)| (z - axis_z - dz).abs() < *half);

    let mut z = 30.0_f32;
    while z <= blueprint.grid.size_m[1] - 30.0 {
        if !in_crossing_window(z) {
            let x = river.center_x(z);
            if let Some(ground) = map.heightmap.sample_height(x, z) {
                let depth = water.depth_over(ground);
                if depth < thresholds.drown_depth_m + 0.2 {
                    report.push(
                        "water_contract",
                        Severity::Error,
                        format!("mid-channel at z {z} is only {depth:.2} m deep — must drown"),
                        Some([x, ground, z]),
                    );
                }
            }
        }
        z += 5.0;
    }

    for op in &blueprint.terrain.ops {
        if let TerrainOp::CarveChannel { sills, .. } = op {
            for [sill_z, _] in sills {
                let x = river.center_x(*sill_z);
                if let Some(ground) = map.heightmap.sample_height(x, *sill_z) {
                    let depth = water.depth_over(ground);
                    if !(0.4..=thresholds.ford_max_depth_m).contains(&depth) {
                        report.push(
                            "water_contract",
                            Severity::Error,
                            format!("ford sill at z {sill_z} is {depth:.2} m deep — off the band"),
                            Some([x, ground, *sill_z]),
                        );
                    }
                }
            }
        }
    }

    for (dz, half_length, freeboard) in deck_checks {
        let z = axis_z + dz;
        let x = river.center_x(z);
        let mut dx = -half_length;
        while dx <= half_length {
            if let Some(h) = map.heightmap.sample_height(x + dx, z)
                && h < water.surface_level_m + freeboard
            {
                report.push(
                    "water_contract",
                    Severity::Error,
                    format!("crossing deck dips to {h:.2} at dz {dz}, dx {dx}"),
                    Some([x + dx, h, z]),
                );
            }
            dx += 2.0;
        }
    }

    let cell = map.heightmap.cell_size_m();
    for zi in 0..map.heightmap.height() {
        for xi in 0..map.heightmap.width() {
            let h = map.heightmap.sample_at_index(xi, zi);
            if water.depth_over(h) > 0.0 {
                let x = xi as f32 * cell;
                let z = zi as f32 * cell;
                let d = (x - river.center_x(z)).abs();
                if d > river.corridor_half_width_m + 3.0 {
                    report.push(
                        "water_contract",
                        Severity::Error,
                        format!("accidental puddle at ({x}, {z})"),
                        Some([x, h, z]),
                    );
                }
            }
        }
    }

    // Every crossing approach is drivable: no 5 m step exceeds the 0.5 climb wall.
    for (dz, _) in &windows {
        let z = axis_z + dz;
        let center_x = river.center_x(z);
        if let Some(mut previous) = map.heightmap.sample_height(center_x - 45.0, z) {
            let mut dx = -40.0_f32;
            while dx <= 45.0 {
                if let Some(h) = map.heightmap.sample_height(center_x + dx, z) {
                    let grade = ((h - previous) / 5.0).abs();
                    if grade >= 0.5 {
                        report.push(
                            "water_contract",
                            Severity::Error,
                            format!("crossing at z {z} has a {grade:.2} grade at dx {dx}"),
                            Some([center_x + dx, h, z]),
                        );
                    }
                    previous = h;
                }
                dx += 5.0;
            }
        }
    }
}
