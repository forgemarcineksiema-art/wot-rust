//! What the ground under a track IS — one rule, read by the picture and by the physics.
//!
//! The map's splat was never authored: `scene_build` derived it from the map's own truth (height,
//! slope, roads, the water margin, the patchwork noise) and baked it into a texture. Physics could
//! not read that texture — the render boundary forbids it — and had no ground material at all:
//! `traction` was pure geometry, so mud, cobble, a ploughed field and a wet river bank all drove
//! identically.
//!
//! Copying the classification into `physics` would have been the obvious move and the wrong one.
//! Two copies of a rule drift, and the day they disagree the game is lying about the thing it
//! promises hardest: that what you see under the track is what you feel under it. So the rule
//! lives HERE, below both, and the bake and the drive read the same answer — the same pattern
//! `map_build` already uses to keep one grounding-math implementation.
//!
//! [`GroundClassifier`] is built once per battlefield because the rule needs the heightmap's
//! statistics, which are O(all samples). Querying it is a handful of height samples.

use serde::{Deserialize, Serialize};

use crate::battlefield::{BattlefieldMap, Road, RoadSurface};
use crate::flow::FlowField;
use crate::heightmap::HeightMap;

/// How steep ground starts breaking to rock (1 - normal.y at the sampled point).
const ROCK_STEEP_START: f32 = 0.18;
/// Crest fraction of the map's height span where chalk/rock begins to break through.
const ROCK_CREST_START: f32 = 0.72;
/// Ground within this height above the waterline reads as worn wet earth.
const WATER_MARGIN_M: f32 = 0.45;
/// How much dirt full drainage wetness claims. Sized so a saturated drainage line blends
/// to grip ≈ 0.973 / rolling resistance ≈ 1.19 — a drag a driver feels, nowhere near the
/// regime the Bystra escape analysis below calls out. The water margin composes with flow
/// by MAX, never sum, so the margin band itself keeps today's exact weights.
const FLOW_DIRT_GAIN: f32 = 0.55;
/// Texel edge the render bake uses; the classification's slope term is measured at half of one,
/// so the rule has ONE sampling scale rather than one per reader.
const CLASSIFY_TEXELS: f32 = 1024.0;

/// The four ground layers, in splat channel order.
///
/// Append-only: the order is a wire/asset identity (the splat's RGBA channels), so a new surface
/// goes on the end and nothing is ever reordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GroundMaterial {
    /// Lush grass — the steppe's default footing.
    Grass,
    /// Dry straw and stubble: the same soil, looser on top.
    Straw,
    /// Worn earth: road wear and the wet margin at a waterline.
    Dirt,
    /// Chalk and broken rock on steep faces and crests.
    Rock,
}

impl GroundMaterial {
    pub const ALL: [GroundMaterial; 4] =
        [GroundMaterial::Grass, GroundMaterial::Straw, GroundMaterial::Dirt, GroundMaterial::Rock];

    /// What this surface does under a track.
    ///
    /// Deliberately modest numbers. The point of the first pass is that the materials DIFFER and
    /// that the difference comes from the same rule the eye reads — not that any one of them is
    /// finally tuned. Grass is the reference at exactly 1.0, so a map of pure grass drives
    /// bit-identically to the model before ground material existed.
    ///
    /// GRIP stays on a short leash, and the reason took three wrong guesses to pin down, so it is
    /// written out. The Bystra bot soak breaks when the wet margin loses much bite, and the
    /// mechanism is NOT what it looks like:
    ///
    /// * not the route brain's deep-water line — moving it left the failure at the identical tick
    ///   and the identical position;
    /// * not a shove from a neighbour — the hull has nobody within 9 m;
    /// * not a disabled hull sliding — its tracks are whole and nothing is destroyed.
    ///
    /// The escape IS engaged (it zeroes steer while it kills the plunge, which is exactly the
    /// frozen heading the trace shows) and simply cannot win: on a descending slick bank, reverse
    /// thrust does not overcome gravity plus the water's drag, and the hull rides down at terminal
    /// velocity. Extracting it needs the escape to steer ALONG the contour rather than straight
    /// back — a control redesign, not a constant.
    ///
    /// So the first pass carries its character in ROLLING RESISTANCE, which nothing above the
    /// drive plans against, and keeps grip inside a narrow band. The envelope opens when the
    /// escape learns to climb out sideways.
    pub fn properties(self) -> GroundProperties {
        match self {
            GroundMaterial::Grass => {
                GroundProperties { grip_scale: 1.0, rolling_resist_scale: 1.0, rut_depth_m: 0.03 }
            }
            // Loose stubble over dry soil: a little less bite, a little more drag, and it churns.
            GroundMaterial::Straw => {
                GroundProperties { grip_scale: 0.97, rolling_resist_scale: 1.15, rut_depth_m: 0.05 }
            }
            // Worn earth: the softest thing a hull crosses, and the thing that remembers it.
            GroundMaterial::Dirt => {
                GroundProperties { grip_scale: 0.95, rolling_resist_scale: 1.35, rut_depth_m: 0.09 }
            }
            // Rock bites hardest and rolls easiest, and nothing marks it.
            GroundMaterial::Rock => {
                GroundProperties { grip_scale: 1.04, rolling_resist_scale: 0.9, rut_depth_m: 0.0 }
            }
        }
    }
}

/// What a surface does to a hull driving over it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundProperties {
    /// Multiplies the terrain contact's traction — grip for drive, turn and the static hold.
    pub grip_scale: f32,
    /// Multiplies rolling resistance.
    pub rolling_resist_scale: f32,
    /// How deep a track rut this surface takes. Zero means it does not remember.
    pub rut_depth_m: f32,
}

/// The map-wide constants the classification needs, resolved once.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundClassifier {
    extent_x_m: f32,
    extent_z_m: f32,
    min_m: f32,
    span_m: f32,
    step_m: f32,
    water_level_m: Option<f32>,
    roads: Vec<Road>,
    /// Drainage truth (see [`crate::FlowField`]): computed ONCE from the authored samples
    /// at construction — flow is geology, craters never recompute it.
    flow: FlowField,
}

impl GroundClassifier {
    pub fn new(battlefield: &BattlefieldMap) -> Self {
        let heightmap = &battlefield.heightmap;
        let extent_x_m = (heightmap.width() - 1) as f32 * heightmap.cell_size_m();
        let extent_z_m = (heightmap.height() - 1) as f32 * heightmap.cell_size_m();
        let stats = heightmap.stats();
        Self {
            extent_x_m,
            extent_z_m,
            min_m: stats.min_m,
            span_m: (stats.max_m - stats.min_m).max(1.0),
            step_m: (extent_x_m / CLASSIFY_TEXELS) * 0.5,
            water_level_m: battlefield.water.map(|water| water.surface_level_m),
            roads: battlefield.roads.clone(),
            flow: FlowField::from_heightmap(heightmap),
        }
    }

    /// The drainage wetness at a world point, for readers that want the raw lane (the bake
    /// composes it into the puddle-propensity alpha). The weights below already fold it in.
    pub fn flow_wetness_at(&self, x: f32, z: f32) -> f32 {
        self.flow.wetness_at(x, z)
    }

    fn height_at(&self, heightmap: &HeightMap, x: f32, z: f32) -> f32 {
        heightmap
            .sample_height(x.clamp(0.0, self.extent_x_m), z.clamp(0.0, self.extent_z_m))
            .unwrap_or(self.min_m)
    }

    /// The macro surface normal the classification reads, sampled at the shared scale.
    pub fn macro_normal_at(&self, heightmap: &HeightMap, x: f32, z: f32) -> [f32; 3] {
        let step = self.step_m;
        let dx = self.height_at(heightmap, x + step, z) - self.height_at(heightmap, x - step, z);
        let dz = self.height_at(heightmap, x, z + step) - self.height_at(heightmap, x, z - step);
        let inv_len = 1.0 / (dx * dx + (2.0 * step) * (2.0 * step) + dz * dz).sqrt().max(1.0e-6);
        [-dx * inv_len, 2.0 * step * inv_len, -dz * inv_len]
    }

    /// Layer weights at a world point, normalized to sum to one, in [`GroundMaterial::ALL`] order.
    ///
    /// Rock breaks through on steep faces and high crests; roads wear the ground to dirt; water
    /// margins read as worn wet earth; what remains splits between lush grass and dry straw by the
    /// patchwork noise the map's character has always drifted with.
    pub fn weights_at(&self, heightmap: &HeightMap, x: f32, z: f32) -> [f32; 4] {
        let y = self.height_at(heightmap, x, z);
        let normal = self.macro_normal_at(heightmap, x, z);
        self.weights_from(y, normal[1], x, z)
    }

    /// The same rule, for a caller that has already paid for the height and the normal — the
    /// render bake computes both for its own reasons and must not pay twice.
    pub fn weights_from(&self, height_m: f32, normal_y: f32, x: f32, z: f32) -> [f32; 4] {
        let steep = (1.0 - normal_y).clamp(0.0, 1.0);
        let crest = ((height_m - self.min_m) / self.span_m - ROCK_CREST_START).max(0.0)
            / (1.0 - ROCK_CREST_START);

        // What the strongest road here is MADE OF decides which channel it claims: packed
        // farm dirt wears the ground to dirt; ballast and granite setts are STONE and take
        // the rock lane — in the picture and under the tracks (rock: grip 1.04 / rolling
        // 0.9), so a paved road stops being the slowest ground in the game. The hard
        // shoulder's feather is sharpened one extra smoothstep: stone ends where the kerb
        // does instead of bleeding a metre of gravel into the verge.
        let (road, surface) =
            strongest_road_at(&self.roads, x, z).unwrap_or((0.0, RoadSurface::Dirt));
        let (dirt_road, paved) = match surface {
            RoadSurface::Dirt => (road, 0.0),
            RoadSurface::Ballast | RoadSurface::Cobble => (0.0, road * road * (3.0 - 2.0 * road)),
        };
        let rock = ((steep - ROCK_STEEP_START).max(0.0) * 3.2 + crest * crest * 0.9 + paved)
            .clamp(0.0, 1.0);

        let margin = self
            .water_level_m
            .map(|level| (1.0 - (height_m - level).abs() / WATER_MARGIN_M).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        // Wet earth comes from the stronger of two truths: the waterline margin, or the
        // drainage the heightfield collects (docs/atmosphere-policy.md, structural ground
        // moisture). MAX, not sum — wherever the margin already dominates, the weights are
        // bit-identical to the pre-flow rule, which is the armor the Bystra escape (above)
        // stands on.
        let moisture = self.flow.wetness_at(x, z);
        let wet = (margin * 0.85).max(moisture * FLOW_DIRT_GAIN);
        let dirt = (dirt_road + wet).clamp(0.0, 1.0);

        let remaining = (1.0 - rock).max(0.0) * (1.0 - dirt).max(0.0);
        let patch = grass_patchwork_noise(x, z);
        let straw_share = ((patch - 0.5) * 2.4).clamp(0.0, 1.0);
        [remaining * (1.0 - straw_share), remaining * straw_share, dirt, rock]
    }

    /// The surface that dominates a world point — what a track is actually on.
    pub fn material_at(&self, heightmap: &HeightMap, x: f32, z: f32) -> GroundMaterial {
        let weights = self.weights_at(heightmap, x, z);
        let mut best = (GroundMaterial::Grass, weights[0]);
        for (material, weight) in GroundMaterial::ALL.into_iter().zip(weights).skip(1) {
            if weight > best.1 {
                best = (material, weight);
            }
        }
        best.0
    }

    /// Ground properties blended by the layer weights, so a surface that is half road and half
    /// grass drives like half of each instead of snapping between them.
    pub fn properties_at(&self, heightmap: &HeightMap, x: f32, z: f32) -> GroundProperties {
        let weights = self.weights_at(heightmap, x, z);
        let total: f32 = weights.iter().sum::<f32>().max(1.0e-4);
        let mut blended =
            GroundProperties { grip_scale: 0.0, rolling_resist_scale: 0.0, rut_depth_m: 0.0 };
        for (material, weight) in GroundMaterial::ALL.into_iter().zip(weights) {
            let share = weight / total;
            let properties = material.properties();
            blended.grip_scale += properties.grip_scale * share;
            blended.rolling_resist_scale += properties.rolling_resist_scale * share;
            blended.rut_depth_m += properties.rut_depth_m * share;
        }
        blended
    }
}

/// How strongly one road paints a world point: full over the inner core, feathered to nothing at
/// the edge. The render bake picks the strongest road's TONE with the same number, so the shape of
/// a road's wear and the shape of its look cannot drift apart.
pub fn road_blend(road: &Road, x: f32, z: f32) -> f32 {
    let half = road.width_m * 0.5;
    let distance = road.distance_to(x, z);
    if distance >= half {
        return 0.0;
    }
    let fade = ((half - distance) / (half * 0.45)).clamp(0.0, 1.0);
    fade * fade * (3.0 - 2.0 * fade)
}

/// The strongest road paint at a world point, 0 where no road reaches.
pub fn road_blend_at(roads: &[Road], x: f32, z: f32) -> f32 {
    roads.iter().map(|road| road_blend(road, x, z)).fold(0.0, f32::max)
}

/// The strongest road paint at a world point TOGETHER with the surface that paints it.
/// The winning blend is float-identical to [`road_blend_at`]'s fold (same set, same max),
/// so a map whose roads are all Dirt keeps a bit-identical splat — the golden hashes of
/// roadless and dirt-only maps are the lock on that.
pub fn strongest_road_at(roads: &[Road], x: f32, z: f32) -> Option<(f32, RoadSurface)> {
    let mut best: Option<(f32, RoadSurface)> = None;
    for road in roads {
        let blend = road_blend(road, x, z);
        if best.is_none_or(|(strongest, _)| blend > strongest) {
            best = Some((blend, road.surface));
        }
    }
    best
}

/// The patchwork the steppe's grass and straw drift with.
pub fn grass_patchwork_noise(x: f32, z: f32) -> f32 {
    value_noise(x / 65.0, z / 65.0) * 0.72 + value_noise(x / 19.0, z / 19.0) * 0.28
}

/// Deterministic 2D value noise in [0, 1]: hashed lattice corners, smoothstep-blended.
pub fn value_noise(x: f32, z: f32) -> f32 {
    let (x0, z0) = (x.floor(), z.floor());
    let (fx, fz) = (x - x0, z - z0);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let corner = |dx: f32, dz: f32| -> f32 {
        let (ix, iz) = ((x0 + dx) as i64, (z0 + dz) as i64);
        // splitmix64 over the packed lattice coordinates.
        let mut h = (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (iz as u64).rotate_left(32);
        h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((h ^ (h >> 31)) >> 40) as f32 / ((1u64 << 24) - 1) as f32
    };
    let top = corner(0.0, 0.0) + (corner(1.0, 0.0) - corner(0.0, 0.0)) * sx;
    let bottom = corner(0.0, 1.0) + (corner(1.0, 1.0) - corner(0.0, 1.0)) * sx;
    top + (bottom - top) * sz
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grass_is_the_reference_surface() {
        // Every scale is relative to grass, so a map of pure grass drives exactly as it did
        // before ground material existed — the replay-locked no-op the first pass depends on.
        let grass = GroundMaterial::Grass.properties();
        assert_eq!(grass.grip_scale, 1.0);
        assert_eq!(grass.rolling_resist_scale, 1.0);
    }

    #[test]
    fn the_surfaces_actually_differ_and_in_the_right_direction() {
        let dirt = GroundMaterial::Dirt.properties();
        let rock = GroundMaterial::Rock.properties();
        assert!(dirt.grip_scale < 1.0, "worn earth gives less bite than grass");
        assert!(dirt.rolling_resist_scale > 1.0, "and drags more");
        assert!(rock.grip_scale > 1.0, "rock bites hardest");
        assert!(rock.rolling_resist_scale < 1.0, "and rolls easiest");
        assert_eq!(rock.rut_depth_m, 0.0, "nothing marks rock");
        assert!(dirt.rut_depth_m > GroundMaterial::Grass.properties().rut_depth_m);
    }

    fn synthetic_battlefield(
        heightmap: crate::HeightMap,
        water_level_m: Option<f32>,
    ) -> BattlefieldMap {
        BattlefieldMap {
            id: "test".into(),
            name: "test".into(),
            size_m: heightmap.extent_m(),
            historical_basis: String::new(),
            design_notes: vec![],
            heightmap,
            water: water_level_m.map(|level| crate::WaterBody { surface_level_m: level }),
            river: None,
            spawn_zones: vec![],
            capture_zones: vec![],
            strategic_points: vec![],
            features: vec![],
            static_cover: vec![],
            scenery: vec![],
            roads: vec![],
        }
    }

    #[test]
    fn a_drainage_line_pulls_the_ground_toward_wet_earth() {
        // A tilted valley: the trough collects the whole slope, the shoulder only its own
        // strip. The rule must read more worn wet earth in the trough — this is the lock
        // on structural ground moisture reaching BOTH the splat and the drive.
        let battlefield = synthetic_battlefield(
            // A V-valley: cross-slope 0.08 beats the down-valley 0.01, so the shoulders
            // converge into the trough and the trough earns real catchment.
            crate::heightmap_from_fn(101, 5.0, |x, z| 20.0 + (x - 250.0).abs() * 0.08 - z * 0.01),
            None,
        );
        let classifier = GroundClassifier::new(&battlefield);
        let trough = classifier.weights_at(&battlefield.heightmap, 250.0, 400.0);
        let shoulder = classifier.weights_at(&battlefield.heightmap, 125.0, 400.0);
        assert!(
            trough[2] > shoulder[2] + 0.1,
            "the trough must read wetter than its shoulder (dirt {} vs {})",
            trough[2],
            shoulder[2]
        );
    }

    #[test]
    fn the_water_margin_caps_flow_instead_of_stacking_on_it() {
        // Two shoreline points at the SAME height over the waterline — one fed by a wet
        // gully, one on a plain slope. The margin term dominates both (max, not sum), so
        // their weights must be identical even though their drainage differs. This is the
        // armor the fragile Bystra escape stands on: flow never deepens the margin band.
        let battlefield = synthetic_battlefield(
            // A V-notch gully (walls 0.05) cut into a plain tilted 0.02 toward z=500: the
            // walls converge flow onto the gully axis, the plain sheds parallel strips.
            // Both probe points sit on exact grid nodes at the waterline height, so the
            // margin term saturates identically on both.
            crate::heightmap_from_fn(101, 5.0, |x, z| {
                let notch = (2.5 - (x - 125.0).abs() * 0.05).max(0.0);
                0.02 * (500.0 - z) - notch
            }),
            Some(2.0),
        );
        let classifier = GroundClassifier::new(&battlefield);
        let in_gully = (125.0, 275.0);
        let on_plain = (375.0, 400.0);
        assert!(
            classifier.flow_wetness_at(in_gully.0, in_gully.1)
                > classifier.flow_wetness_at(on_plain.0, on_plain.1),
            "the gully must actually carry more drainage for this lock to mean anything"
        );
        // Grass/straw split rides the patchwork noise and legitimately differs between the
        // two points — the lock lives in the dirt lane. Epsilon instead of bit equality:
        // two different world points never share exact f32 arithmetic, but SUM composition
        // would separate these dirt values by ~0.1, five orders above the tolerance.
        let wet = classifier.weights_at(&battlefield.heightmap, in_gully.0, in_gully.1);
        let dry = classifier.weights_at(&battlefield.heightmap, on_plain.0, on_plain.1);
        assert!(
            (wet[2] - dry[2]).abs() < 1.0e-4,
            "inside the margin band the pre-flow dirt holds ({} vs {})",
            wet[2],
            dry[2]
        );
        assert_eq!(wet[3], dry[3], "and rock is untouched by moisture");
    }

    fn straight_road(surface: RoadSurface) -> Road {
        Road {
            id: "lane".into(),
            surface,
            points: vec![[0.0, 250.0], [500.0, 250.0]],
            width_m: 8.0,
        }
    }

    fn flat_ground() -> crate::HeightMap {
        crate::heightmap_from_fn(101, 5.0, |_, _| 10.0)
    }

    #[test]
    fn a_paved_road_is_never_the_slowest_ground() {
        // Before surface routing, EVERY road classified as worn earth — grip 0.95, rolling
        // resistance 1.35 — making roads the slowest ground in the game, inverted. Stone
        // surfaces now drive as stone; a packed farm lane keeps exactly the old drive.
        let mut cobbled = synthetic_battlefield(flat_ground(), None);
        cobbled.roads = vec![straight_road(RoadSurface::Cobble)];
        let street =
            GroundClassifier::new(&cobbled).properties_at(&cobbled.heightmap, 250.0, 250.0);
        assert!(
            street.grip_scale > 1.0 && street.rolling_resist_scale < 1.0,
            "granite setts must drive as stone: {street:?}"
        );

        let mut farm = synthetic_battlefield(flat_ground(), None);
        farm.roads = vec![straight_road(RoadSurface::Dirt)];
        let lane = GroundClassifier::new(&farm).properties_at(&farm.heightmap, 250.0, 250.0);
        assert_eq!(
            (lane.grip_scale, lane.rolling_resist_scale),
            (0.95, 1.35),
            "a packed farm lane keeps exactly the worn-earth drive"
        );
    }

    #[test]
    fn stone_ends_where_the_kerb_does() {
        // The hard shoulder is sharpened one extra smoothstep: at the outer feather the
        // stone share sits BELOW the dirt share the same geometry paints — tighter, not
        // merely recolored.
        let mut cobbled = synthetic_battlefield(flat_ground(), None);
        cobbled.roads = vec![straight_road(RoadSurface::Cobble)];
        let mut farm = synthetic_battlefield(flat_ground(), None);
        farm.roads = vec![straight_road(RoadSurface::Dirt)];
        let probe = (250.0, 253.55);
        let stone =
            GroundClassifier::new(&cobbled).weights_at(&cobbled.heightmap, probe.0, probe.1);
        let earth = GroundClassifier::new(&farm).weights_at(&farm.heightmap, probe.0, probe.1);
        assert!(stone[3] > 0.0, "the feather still paints stone");
        assert!(
            stone[3] < earth[2],
            "the stone shoulder must be tighter than the dirt one ({} vs {})",
            stone[3],
            earth[2]
        );
    }

    #[test]
    fn a_road_paints_full_at_its_centre_and_nothing_past_its_edge() {
        let road = Road {
            id: "lane".into(),
            surface: crate::battlefield::RoadSurface::Dirt,
            points: vec![[0.0, 0.0], [100.0, 0.0]],
            width_m: 8.0,
        };
        assert!(road_blend(&road, 50.0, 0.0) > 0.99, "the core is full paint");
        assert_eq!(road_blend(&road, 50.0, 4.1), 0.0, "past the half width it stops");
        assert!(road_blend(&road, 50.0, 3.5) < 0.99, "and it feathers at the edge");
    }
}
