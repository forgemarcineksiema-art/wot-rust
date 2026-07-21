//! The map blueprint: a map as DATA. One RON document is the single source of truth for a
//! battlefield — grid, terrain program, river, water, objects, roads and gameplay layout —
//! compiled deterministically by [`crate::compile`]. The editor authors these documents; the
//! game never ships a map across the wire, only agrees on which blueprint to compile.
//!
//! Schema rules:
//! - every position is world meters on the authored grid (`x` grows east, `z` grows north);
//! - `dz` values inside ops are relative to the map's symmetry axis (`size_m[1] / 2`);
//! - op ORDER is the design ("decks applied last" is visible in the document, not a comment).

use game_core::WeatherVariant;
use serde::{Deserialize, Serialize};
use terrain::{MapFeatureKind, RoadSurface, SceneryKind, StaticCoverKind, StrategicRole};

/// One authored map document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapBlueprint {
    pub meta: MetaSpec,
    pub grid: GridSpec,
    /// The fairness contract, when the map has one: everything mirrors across the axis.
    #[serde(default)]
    pub symmetry: Option<SymmetrySpec>,
    /// The river centerline, when the map has one (see `terrain::RiverSpec`).
    #[serde(default)]
    pub river: Option<terrain::RiverSpec>,
    /// The horizon enclosure for the render-only backdrop skirt; `None` means the terrain
    /// program simply continues (an open steppe).
    #[serde(default)]
    pub horizon: Option<HorizonSpec>,
    pub terrain: TerrainProgram,
    /// The freeform sculpt layer (map-editor D1): brush strokes consolidated into ONE
    /// quantized delta grid over the terrain program. The document holds the full terrain
    /// truth; stroke-by-stroke history lives only in the editor session.
    #[serde(default)]
    pub sculpt: Option<SculptSpec>,
    #[serde(default)]
    pub water: Option<WaterSpec>,
    /// The ground's material palette (render-only). `None` falls back to the neutral
    /// steppe set — a map that cares authors its own.
    #[serde(default)]
    pub materials: Option<GroundMaterialsSpec>,
    /// The map's looks: which weather variants it ships and what each one means for the
    /// eyes (render-only; the server reads only the variant list). `None` means the single
    /// hazy-noon default.
    #[serde(default)]
    pub environment: Option<EnvironmentSpec>,
    #[serde(default)]
    pub objects: Vec<ObjectSpec>,
    #[serde(default)]
    pub scenery: Vec<SceneryOp>,
    #[serde(default)]
    pub roads: Vec<RoadSpec>,
    pub gameplay: GameplaySpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetaSpec {
    /// Blueprint schema version. Additive sections stay `serde(default)`; this bumps only
    /// on a BREAKING document change, so the editor knows when a file needs migrating.
    #[serde(default = "default_blueprint_version")]
    pub version: u32,
    pub id: String,
    pub name: String,
    pub historical_basis: String,
    #[serde(default)]
    pub design_notes: Vec<String>,
}

/// The current blueprint schema version (and the default for documents that predate the
/// field).
pub const BLUEPRINT_VERSION: u32 = 1;

fn default_blueprint_version() -> u32 {
    BLUEPRINT_VERSION
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridSpec {
    pub size_m: [f32; 2],
    pub cell_m: f32,
    /// Final clamp: terrain never dips below this (keeps banks dry-edged and sampling sane).
    pub min_height_m: f32,
}

impl GridSpec {
    pub fn samples_per_side(&self) -> usize {
        (self.size_m[0] / self.cell_m).round() as usize + 1
    }

    /// The symmetry axis on z (`size_m[1] / 2`); every mirrored thing references it.
    pub fn axis_z(&self) -> f32 {
        self.size_m[1] * 0.5
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymmetrySpec {
    /// Mirror across `z = axis_z` (positions and yaw), the WoT-like north/south fairness.
    MirrorZ,
}

/// Standing water (see `terrain::WaterBody`); depth is `level − ground` by construction.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaterSpec {
    pub surface_level_m: f32,
}

/// The hills that close the horizon beyond the map's edge, for the render-only backdrop
/// skirt: the terrain program continues past the boundary and blends outward into rolling
/// ground that closes the view — except along the river's course, whose exit gap stays open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HorizonSpec {
    /// Base elevation of the enclosing hills.
    pub hills_base_m: f32,
    /// Rolling waves composing the enclosure: `(amp, x_freq, z_freq, x_phase)` evaluated as
    /// `amp · sin(x·x_freq + x_phase) · cos(z·z_freq)` for the first entry, then plain
    /// `amp · cos(x·x_freq)` / `amp · sin(z·z_freq)` additions.
    pub swell: [f32; 4],
    pub x_roll: [f32; 2],
    pub z_roll: [f32; 2],
    /// The outward blend: full interior until `closure_start_m` past the border, fully
    /// enclosed after `closure_start + closure_span`.
    pub closure_start_m: f32,
    pub closure_span_m: f32,
    /// The river's exit corridor stays open: a band mask around the centerline.
    pub river_gap_half_m: f32,
    pub river_gap_falloff_m: f32,
}

// --- Terrain program -------------------------------------------------------------------

/// The heightfield recipe: a base surface, then ordered structural ops.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainProgram {
    pub base: BaseSpec,
    #[serde(default)]
    pub ops: Vec<TerrainOp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BaseSpec {
    /// A flat starting elevation.
    Constant(f32),
    /// Piecewise ease steps along x: `base + Σ delta · smoothstep01((x − start) / span)`,
    /// applied left to right (a valley cross-section, an upland staircase).
    SlopeEases { base: f32, eases: Vec<[f32; 3]> },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Apply {
    Add,
    Subtract,
}

/// One structural terrain op. Evaluation mirrors the historical generators' arithmetic
/// exactly (sums before application, masks in document order), so a blueprint reproduces a
/// legacy map bit for bit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TerrainOp {
    /// Rolling relief: `h += (Σ terms) · (Π (1 − strength·mask))`. `z` enters through even
    /// functions of `dz`, keeping the mirror contract.
    Relief { terms: Vec<ReliefTerm>, dampen: Vec<DampMask> },
    /// A group of 2D Gaussians summed, then added to (or subtracted from) the terrain —
    /// hills, knolls, bowls.
    Gauss2 { apply: Apply, terms: Vec<Gauss2Term> },
    /// A group of 1D Gaussians along one axis, summed then applied — ditches, lowlands.
    Gauss1 { axis: MapAxis, apply: Apply, terms: Vec<Gauss1Term> },
    /// A ridge along the symmetry axis with crossing gates opened in it:
    /// `h += g1(z) · (1 − min(1, Σ g1(x at gates)))` — the railway embankment pattern.
    RidgeGated { center: f32, sigma: f32, amp: f32, gates: Vec<Gauss1Term> },
    /// Hull-down crest+shelf pair, localized in z: per window,
    /// `(g1(x, crest) − g1(x, shelf)) · g1(z, window)` — a hull masks behind the crest while
    /// the turret works over it.
    CrestShelf {
        crest_x: f32,
        shelf_x: f32,
        sigma_x: f32,
        crest_h: f32,
        shelf_cut: f32,
        /// `(z, sigma_z)` pairs locating each terrace along the front.
        windows: Vec<[f32; 2]>,
    },
    /// Flatten toward an explicit ramp under a rectangular band mask — a town bench whose
    /// streets hold one honest grade.
    FlattenToRamp {
        mask: RectBand,
        ramp_base_m: f32,
        ramp_from_x_m: f32,
        ramp_slope: f32,
        strength: f32,
    },
    /// Blend toward a constant elevation under a Gaussian mask — a quarry floor, a pond bed.
    FlattenToGauss { target_m: f32, x: f32, z: f32, sx: f32, sz: f32 },
    /// Add a river-relative band (negative deltas carve floodplain dips).
    RiverBandAdd { half_width_m: f32, falloff_m: f32, delta_m: f32 },
    /// Carve the river channel to an explicit bed target `water − depth(z)`: the depth
    /// profile IS the design — drowning-deep everywhere except the ford sills.
    CarveChannel {
        half_width_m: f32,
        falloff_m: f32,
        water_level_m: f32,
        channel_depth_m: f32,
        sill_depth_m: f32,
        /// Ford sills as `(z, sigma)` Gaussian lifts toward `sill_depth_m`.
        sills: Vec<[f32; 2]>,
    },
    /// Raise a crossing deck above the water, applied after the carve so it always clears:
    /// `h = lerp(h, max(deck, h), band(dz) · band(river_d))`.
    Deck {
        dz_m: f32,
        deck_m: f32,
        half_width_m: f32,
        half_length_m: f32,
        width_falloff_m: f32,
        length_falloff_m: f32,
    },
    /// Final floor clamp (from `grid.min_height_m`; explicit so the document reads in order).
    ClampMin(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapAxis {
    X,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ReliefTerm {
    /// `amp · sin(x / wave)`
    SinX { amp: f32, wave: f32 },
    /// `amp · cos(dz / wave)`
    CosDz { amp: f32, wave: f32 },
    /// `amp · sin(x · x_freq) · cos(dz · dz_freq)`
    SinXCosDz { amp: f32, x_freq: f32, dz_freq: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DampMask {
    /// Relief fades across a linear x ramp (`(outer − x) / (outer − inner)` clamped) —
    /// flatten the open killzone, keep the rolling steppe.
    LinearRampX { inner_m: f32, outer_m: f32, strength: f32 },
    /// Relief fades inside a rectangular band (town bench): `band(x) · band(dz)`.
    TownRect { rect: RectBand, strength: f32 },
    /// Relief fades near the river corridor: `band(river_d)`.
    RiverBand { half_width_m: f32, falloff_m: f32, strength: f32 },
}

/// A rectangular band mask: `band(x − x_center, x_half, x_fall) · band(dz, dz_half, dz_fall)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RectBand {
    pub x_center_m: f32,
    pub x_half_m: f32,
    pub x_falloff_m: f32,
    pub dz_half_m: f32,
    pub dz_falloff_m: f32,
}

/// `(x, z, sigma_x, sigma_z, amplitude)` — one 2D Gaussian hill/bowl term.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Gauss2Term {
    pub x: f32,
    pub z: f32,
    pub sx: f32,
    pub sz: f32,
    pub amp: f32,
}

/// `(center, sigma, amplitude)` — one 1D Gaussian term along its op's axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Gauss1Term {
    pub center: f32,
    pub sigma: f32,
    pub amp: f32,
}

// --- Sculpt layer (map-editor D1) --------------------------------------------------------

/// Brush-sculpted height deltas over the terrain program: sparse, quantized, canonical.
/// Applied AFTER every terrain op and re-clamped to `grid.min_height_m`; evaluation stays
/// pointwise (the delta is a per-sample lookup), so compile purity holds. The BORDER ring
/// must stay zero — that is what keeps the analytic backdrop/apron seam exact even though
/// the skirt never reads the sculpt (a report Error enforces it). On a mirror-symmetric
/// map the layer must mirror sample-for-sample (report Error; the editor stamps both
/// halves by construction).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SculptSpec {
    /// Metres per quantum — brush edits snap to this, so a stroke is deterministic data,
    /// never an accumulation of float noise.
    pub step_m: f32,
    /// Non-zero samples only, as `(row-major heightmap index, quanta)`, sorted by index
    /// with no duplicates (the canonical form; a report Error otherwise). Small strokes
    /// stay a readable diff — a contiguous block of indices in one region.
    pub samples: Vec<(u32, i16)>,
}

impl SculptSpec {
    /// The delta in metres at a sample index (0 where unsculpted).
    pub fn delta_m_at(&self, index: u32) -> f32 {
        match self.samples.binary_search_by_key(&index, |(sample, _)| *sample) {
            Ok(found) => f32::from(self.samples[found].1) * self.step_m,
            Err(_) => 0.0,
        }
    }
}

// --- Ground materials (render-only) ------------------------------------------------------

/// The map's ground palette: the four splat layers plus the two macro strengths. Plain data
/// (`map_forge` is renderer-free); `scene_build` binds it to `renderer_api`'s
/// `TerrainMaterialSet`. The art-direction ground window (vegetation saturation ≤ 0.45,
/// worst-case field-patch lift ≤ 0.62) is a report check.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GroundMaterialsSpec {
    /// Splat channel order: R = lush grass, G = dry straw, B = worn dirt, A = rock/chalk.
    pub layers: [GroundLayerSpec; 4],
    /// 0..1: how far ground lighting leans from the vertex normal toward the baked macro
    /// normal.
    pub macro_normal_strength: f32,
    /// 0..1: the field-patch macro band — worked land is a quilt of plots, not one lawn.
    pub field_patch_strength: f32,
}

/// One ground layer: linear albedo inside the art-direction window, detail-octave
/// amplitude, and the specular lane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GroundLayerSpec {
    pub albedo: [f32; 3],
    pub detail: f32,
    pub gloss: f32,
}

// --- Environment (render-only) -----------------------------------------------------------

/// The looks a map ships: one entry per supported weather variant, each a **named lighting
/// preset plus sparse overrides** — never thirty raw knobs. The FIRST look is the map's
/// default and the fallback for any unauthored variant; the server's `supported_weather`
/// reads exactly this list (in order — the order feeds the seeded weather roll).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentSpec {
    pub looks: Vec<LookSpec>,
}

/// What one weather variant means for the eyes on this map.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LookSpec {
    pub variant: WeatherVariant,
    /// The base lighting profile, by name; `scene_build` owns what each name looks like.
    pub preset: LightingPreset,
    /// The renderer's clear colour behind the gradient sky (f64: it feeds the surface clear
    /// colour directly, so the document's literals survive bit-exact).
    pub sky_rgb: [f64; 3],
    /// Rain streak density 0..1; 0 disables the rain pass.
    #[serde(default)]
    pub rain_intensity: f32,
    /// World wetness 0..1: darkens albedo, sharpens finishes, pools sheen on flat ground.
    #[serde(default)]
    pub wetness: f32,
    /// Sparse overrides on top of the preset — the authored exceptions, not a knob farm.
    #[serde(default)]
    pub overrides: LightingOverrides,
}

/// The named lighting presets — the VOCABULARY (code owns what a name means, the blueprint
/// owns which map wears it; same split as the terrain ops).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightingPreset {
    /// The hazy high-noon battlefield default.
    HazyNoon,
    /// A clear afternoon with honest shadows.
    ClearAfternoon,
    /// A low western sun raking long shadows — the golden hour.
    GoldenEvening,
    /// A dry lead-grey lid: flat light, no sun disc worth the name.
    LeadOvercast,
    /// Squall lines: dark, wet, rain-streaked.
    RainSqualls,
    /// Ground fog in first light.
    DawnFog,
}

/// Sparse per-look overrides on the preset's `SceneLighting`. `None` keeps the preset's
/// value; the report bounds what IS set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct LightingOverrides {
    #[serde(default)]
    pub fog_density: Option<f32>,
    #[serde(default)]
    pub exposure: Option<f32>,
    #[serde(default)]
    pub cloud_coverage_bias: Option<f32>,
    #[serde(default)]
    pub cloud_opacity: Option<f32>,
    #[serde(default)]
    pub saturation: Option<f32>,
}

// --- Objects ----------------------------------------------------------------------------

/// A world x coordinate: fixed, or following the river centerline (crossing markers ride the
/// meander — edit the river and the markers follow).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum XCoord {
    Fixed(f32),
    /// `river.center_x(z)` at THIS point's own z.
    RiverCenter,
    /// `river.center_x(z)` at the GIVEN z (a bridge parapet rides the line at the bridge's
    /// center, not at its own z).
    RiverCenterAt(f32),
}

/// Static content placed on the battlefield, in document order (the compiled lists keep
/// that order — determinism and readable diffs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ObjectSpec {
    /// One grounded cover box (buildings, walls, wrecks, tree lines, fences).
    Cover {
        id: String,
        name: String,
        kind: StaticCoverKind,
        at: [XCoord; 2],
        half_extents_m: [f32; 3],
    },
    /// A mirrored town block grid: for each column x and row offset, a south/north pair of
    /// houses whose sizes alternate by grid parity — a town skyline, not a barracks.
    TownGrid {
        id_prefix: String,
        name_prefix: String,
        kind: StaticCoverKind,
        columns_x_m: Vec<f32>,
        row_offsets_m: Vec<f32>,
        wide_half_m: [f32; 3],
        narrow_half_m: [f32; 3],
    },
}

// --- Scenery ----------------------------------------------------------------------------

/// Refusal rules for a scatter: a point is rejected when ANY rule fires.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Exclusion {
    /// Reject inside the river corridor inflated by this margin.
    #[serde(default)]
    pub river_margin_m: Option<f32>,
    /// Reject OUTSIDE the river corridor inflated by this margin (bank-loving plants).
    #[serde(default)]
    pub river_max_m: Option<f32>,
    /// Crossing approach lanes stay clear: inside the corridor inflated by `band_margin_m`,
    /// reject the listed `(|dz offset|, half width)` windows.
    #[serde(default)]
    pub crossing_lanes: Option<CrossingLanes>,
    /// Reject a band around the symmetry axis (a railbed and its shoulders stay bare).
    #[serde(default)]
    pub axis_band_m: Option<f32>,
    /// Spawn circles `(x, z, radius)` stay clear.
    #[serde(default)]
    pub spawn_circles: Vec<[f32; 3]>,
    /// Reject within `road.width/2 + margin` of any road.
    #[serde(default)]
    pub road_margin_m: Option<f32>,
    /// Reject inside any cover footprint inflated by this margin.
    #[serde(default)]
    pub cover_margin_m: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossingLanes {
    pub band_margin_m: f32,
    pub windows: Vec<[f32; 2]>,
}

/// Render-only dressing ops, in document order. Everything is seeded and mirrored, so the
/// map looks as fair as it plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SceneryOp {
    /// A mirrored seeded scatter in a rectangle on the southern half; accepted points emit
    /// their northern twin (`terrain::scatter_mirrored` does the walk).
    Scatter {
        seed: u64,
        kind: SceneryKind,
        pairs: usize,
        region: ScatterRect,
        #[serde(default)]
        exclude: Exclusion,
    },
    /// A planted row (windbreaks are planted, not grown): `count` steps of `z_step` from
    /// `z_start`, each x in `xs`, mirrored with fixed yaw.
    Row {
        kind: SceneryKind,
        xs: Vec<f32>,
        z_start_m: f32,
        z_step_m: f32,
        count: u32,
        yaw_rad: f32,
        scale: f32,
    },
    /// Fixed spots (town gardens, authored accents), mirrored with fixed yaw.
    Fixed { kind: SceneryKind, spots: Vec<[f32; 2]>, yaw_rad: f32, scale: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScatterRect {
    pub x: [f32; 2],
    pub z: [f32; 2],
}

// --- Roads -------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RoadSpec {
    /// One painted polyline (render-only, see `terrain::Road`).
    Road { id: String, surface: RoadSurface, points: Vec<[f32; 2]>, width_m: f32 },
    /// A southern polyline emitted with its exact northern twin (`_south` / `_north`).
    MirroredPair {
        id_base: String,
        surface: RoadSurface,
        south_points: Vec<[f32; 2]>,
        width_m: f32,
    },
}

// --- Gameplay layout ---------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GameplaySpec {
    #[serde(default)]
    pub spawns: Vec<SpawnSpec>,
    #[serde(default)]
    pub strategic_points: Vec<StrategicPointSpec>,
    #[serde(default)]
    pub features: Vec<FeatureSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnSpec {
    pub team: u16,
    pub at: [f32; 2],
    pub facing_yaw_rad: f32,
    /// Deployment radius; the WoT-like default is 55 m.
    #[serde(default)]
    pub radius_m: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategicPointSpec {
    pub id: String,
    pub name: String,
    pub role: StrategicRole,
    pub at: [XCoord; 2],
    pub radius_m: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureSpec {
    pub kind: MapFeatureKind,
    pub name: String,
    pub at: [XCoord; 2],
    pub radius_m: f32,
    pub note: String,
}
