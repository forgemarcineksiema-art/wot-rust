//! The road tool and the water panel (M6). Roads are render-only polylines painted into
//! the splat — the editor collects clicked waypoints, previews a draped ribbon, and
//! commits ONE `RoadSpec` through `apply_edit` (a `MirroredPair` on fair maps: the twin is
//! not optional). Water is the document's single standing level (a deliberate known
//! limit); the panel tunes it in quarter-metre quanta and the viewport tints every wet
//! cell by its GAMEPLAY class — fordable, marginal, drowning — against the same
//! thresholds the report checks.

use map_forge::WaterThresholds;
use map_forge::blueprint::{MapBlueprint, RoadSpec, WaterSpec};
use renderer_api::SceneVertex;
use terrain::{HeightMap, RoadSurface};

/// The pending road: clicked waypoints (snapped to the metre) plus its knobs.
#[derive(Debug, Clone)]
pub struct PendingRoad {
    pub points: Vec<[f32; 2]>,
    pub surface: RoadSurface,
    pub width_m: f32,
}

impl Default for PendingRoad {
    fn default() -> Self {
        Self { points: Vec::new(), surface: RoadSurface::Dirt, width_m: 4.0 }
    }
}

impl PendingRoad {
    pub fn add_point(&mut self, at: [f32; 2]) {
        self.points.push([at[0].round(), at[1].round()]);
    }

    pub fn pop_point(&mut self) {
        self.points.pop();
    }

    pub fn cycle_surface(&mut self) {
        self.surface = match self.surface {
            RoadSurface::Dirt => RoadSurface::Ballast,
            RoadSurface::Ballast => RoadSurface::Cobble,
            RoadSurface::Cobble => RoadSurface::Dirt,
        };
    }

    pub fn adjust_width(&mut self, steps: f32) {
        self.width_m = (self.width_m + steps * 0.5).clamp(2.0, 8.0);
    }

    /// Commit into the document: a `MirroredPair` on a fair map (the northern twin is the
    /// contract, not a choice), a plain `Road` otherwise. `None` under two points — a road
    /// is a line, not a dot.
    pub fn committed(&self, blueprint: &MapBlueprint) -> Option<(RoadSpec, String)> {
        if self.points.len() < 2 {
            return None;
        }
        let id = unique_road_id(blueprint);
        let summary = format!(
            "road {id}: {} points, {:?}, {:.1} m wide",
            self.points.len(),
            self.surface,
            self.width_m
        );
        let spec = if blueprint.symmetry.is_some() {
            RoadSpec::MirroredPair {
                id_base: id,
                surface: self.surface,
                south_points: self.points.clone(),
                width_m: self.width_m,
            }
        } else {
            RoadSpec::Road {
                id,
                surface: self.surface,
                points: self.points.clone(),
                width_m: self.width_m,
            }
        };
        Some((spec, summary))
    }
}

fn unique_road_id(blueprint: &MapBlueprint) -> String {
    let mut highest = 0_u32;
    for road in &blueprint.roads {
        let id = match road {
            RoadSpec::Road { id, .. } => id,
            RoadSpec::MirroredPair { id_base, .. } => id_base,
        };
        if let Some(number) = id.strip_prefix("road_").and_then(|number| number.parse::<u32>().ok())
        {
            highest = highest.max(number);
        }
    }
    format!("road_{}", highest + 1)
}

/// The preview ribbon: the pending polyline draped over the (live) ground as a matte
/// strip, plus a small pad on every waypoint. The REAL paint arrives with the commit's
/// splat rebake; this is the surveyor's chalk line.
pub fn ribbon_mesh(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    heightmap: &HeightMap,
    road: &PendingRoad,
) {
    const RIBBON: [f32; 3] = [0.52, 0.44, 0.30];
    let ground = |x: f32, z: f32| heightmap.sample_height(x, z).unwrap_or(0.0) + 0.22;
    for pair in road.points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = (dx * dx + dz * dz).sqrt();
        if length < 0.5 {
            continue;
        }
        let steps = (length / 4.0).ceil() as u32;
        let (nx, nz) = (-dz / length, dx / length);
        let half = road.width_m * 0.5;
        let base = vertices.len() as u32;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let (x, z) = (a[0] + dx * t, a[1] + dz * t);
            for side in [-1.0_f32, 1.0] {
                let (px, pz) = (x + nx * half * side, z + nz * half * side);
                vertices.push(SceneVertex::new([px, ground(px, pz), pz], [0.0, 1.0, 0.0], RIBBON));
            }
        }
        for step in 0..steps {
            let row = base + step * 2;
            indices.extend_from_slice(&[row, row + 1, row + 3, row, row + 3, row + 2]);
        }
    }
    for point in &road.points {
        crate::markers::probe_marker(
            vertices,
            indices,
            glam::Vec3::new(point[0], ground(point[0], point[1]) - 0.1, point[1]),
            0.8,
        );
    }
}

/// Set (or create) the standing-water level, snapped to the quarter metre.
pub fn set_water_level(blueprint: &mut MapBlueprint, level_m: f32) -> String {
    let level_m = (level_m * 4.0).round() / 4.0;
    blueprint.water = Some(WaterSpec { surface_level_m: level_m, bodies: Vec::new() });
    format!("water level {level_m:.2} m")
}

/// The gameplay class of a wet cell, against the SAME thresholds the report checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthClass {
    /// A hull wades through (≤ ford max).
    Fordable,
    /// Deeper than a safe ford, not yet a drowning — the marginal band.
    Marginal,
    /// The current drowns (≥ drown depth).
    Drowning,
}

pub fn classify_depth(depth_m: f32, thresholds: &WaterThresholds) -> DepthClass {
    if depth_m >= thresholds.drown_depth_m {
        DepthClass::Drowning
    } else if depth_m > thresholds.ford_max_depth_m {
        DepthClass::Marginal
    } else {
        DepthClass::Fordable
    }
}

/// The live depth tint: one flat quad per WET heightmap cell at the water surface, colored
/// by gameplay class — ford green, marginal amber, drowning red. This is how a ford is
/// PLACED: sculpt the sill until the band under the crossing turns green.
pub fn depth_tint_mesh(
    heightmap: &HeightMap,
    water: terrain::WaterView<'_>,
    thresholds: &WaterThresholds,
) -> (Vec<SceneVertex>, Vec<u32>) {
    const FORD_GREEN: [f32; 3] = [0.30, 0.58, 0.34];
    const MARGINAL_AMBER: [f32; 3] = [0.82, 0.62, 0.20];
    const DROWNING_RED: [f32; 3] = [0.66, 0.22, 0.16];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let cell = heightmap.cell_size_m();
    for zi in 0..heightmap.height() {
        for xi in 0..heightmap.width() {
            let ground = heightmap.sample_at_index(xi, zi);
            let (x, z) = (xi as f32 * cell, zi as f32 * cell);
            let Some(level) = water.level_at(x, z) else { continue };
            let lift = level + 0.06;
            let depth = level - ground;
            if depth <= 0.0 {
                continue;
            }
            let color = match classify_depth(depth, thresholds) {
                DepthClass::Fordable => FORD_GREEN,
                DepthClass::Marginal => MARGINAL_AMBER,
                DepthClass::Drowning => DROWNING_RED,
            };
            let half = cell * 0.5;
            let base = vertices.len() as u32;
            for (px, pz) in [
                (x - half, z - half),
                (x + half, z - half),
                (x + half, z + half),
                (x - half, z + half),
            ] {
                vertices.push(SceneVertex::new([px, lift, pz], [0.0, 1.0, 0.0], color));
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use map_forge::blueprint::SymmetrySpec;

    #[test]
    fn a_committed_road_is_mirrored_on_fair_maps_and_plain_otherwise() {
        let document = crate::EditorDocument::new_scratch();
        let mut pending = PendingRoad::default();
        pending.add_point([60.4, 80.7]);
        pending.add_point([120.0, 140.0]);
        pending.add_point([200.0, 150.0]);
        let (spec, summary) = pending.committed(document.blueprint()).expect("commits");
        assert!(summary.contains("road_1"));
        let RoadSpec::Road { points, .. } = &spec else { panic!("plain road without symmetry") };
        assert_eq!(points[0], [60.0, 81.0], "waypoints snap to the metre");

        let mut fair = document.blueprint().clone();
        fair.symmetry = Some(SymmetrySpec::MirrorZ);
        fair.roads.push(spec);
        let (mirrored, _) = pending.committed(&fair).expect("commits");
        let RoadSpec::MirroredPair { id_base, .. } = &mirrored else {
            panic!("fair maps commit the twin pair")
        };
        assert_eq!(id_base, "road_2", "ids stay unique across both spec shapes");

        // Compiled: the pair expands into _south and _north.
        let mut with_road = fair.clone();
        with_road.roads.push(mirrored);
        let (map, _) = map_forge::compile(&with_road);
        assert!(map.roads.iter().any(|road| road.id == "road_2_south"));
        assert!(map.roads.iter().any(|road| road.id == "road_2_north"));

        let mut dot = PendingRoad::default();
        dot.add_point([50.0, 50.0]);
        assert!(dot.committed(document.blueprint()).is_none(), "a road is a line, not a dot");
    }

    /// The surface cycle walks every authored surface exactly once and returns home — a new
    /// `RoadSurface` variant that forgets the editor shows up here, not in a playtest.
    #[test]
    fn the_surface_cycle_visits_every_surface_and_returns() {
        let mut pending = PendingRoad::default();
        let start = pending.surface;
        let mut seen = vec![start];
        loop {
            pending.cycle_surface();
            if pending.surface == start {
                break;
            }
            assert!(!seen.contains(&pending.surface), "the cycle must not trap a subset");
            seen.push(pending.surface);
            assert!(seen.len() <= 8, "the cycle must return to the start");
        }
        assert!(seen.contains(&RoadSurface::Cobble), "the city street surface is reachable");
        assert!(seen.contains(&RoadSurface::Ballast));
    }

    #[test]
    fn the_depth_tint_speaks_the_gameplay_thresholds() {
        let thresholds = WaterThresholds::default();
        assert_eq!(classify_depth(0.5, &thresholds), DepthClass::Fordable);
        assert_eq!(classify_depth(1.2, &thresholds), DepthClass::Marginal);
        assert_eq!(classify_depth(1.6, &thresholds), DepthClass::Drowning);

        // A bowl under a 5 m water line: the rim wades, the middle drowns.
        let heightmap = terrain::heightmap_from_fn(21, 5.0, |x, z| {
            let d = ((x - 50.0).powi(2) + (z - 50.0).powi(2)).sqrt();
            2.0 + (d / 50.0) * 4.0
        });
        let water = terrain::WaterView {
            table: Some(terrain::WaterBody { surface_level_m: 5.0 }),
            sheets: &[],
        };
        let (vertices, indices) = depth_tint_mesh(&heightmap, water, &thresholds);
        assert!(!vertices.is_empty());
        assert!(indices.len().is_multiple_of(3));
        assert!(vertices.iter().all(|vertex| (vertex.position[1] - 5.06).abs() < 1.0e-3));
        let has = |color: [f32; 3]| vertices.iter().any(|vertex| vertex.color == color);
        assert!(has([0.66, 0.22, 0.16]), "the bowl's middle drowns");
        assert!(has([0.30, 0.58, 0.34]), "the rim is fordable");
    }

    #[test]
    fn the_water_level_snaps_and_creates_the_spec() {
        let mut document = crate::EditorDocument::new_scratch();
        document.apply_edit(|blueprint| {
            set_water_level(blueprint, 3.13);
        });
        assert_eq!(document.blueprint().water.as_ref().unwrap().surface_level_m, 3.25);
        document.apply_edit(|blueprint| blueprint.water = None);
        assert!(document.blueprint().water.is_none());
    }
}
