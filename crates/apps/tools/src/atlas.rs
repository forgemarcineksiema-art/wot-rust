//! The terrain atlas: top-down instrument renders of a shipped map, for terrain review.
//!
//! Every layer is measured through the game's OWN resolution path, never a private copy of
//! the rule: heights come from `HeightMap::sample_height` (the sim's contact/LOS lane),
//! ground truth from `terrain::GroundClassifier` (the one rule the splat bake and the drive
//! both read), drivability from `game_core::MAX_CLIMB_GRADE` / `ROAD_COMFORT_GRADE`, water
//! bands from the physics wading constants, and every sight line from `sim::line_of_sight`
//! — the exact eye the spotting recompute uses, over the map's BORN cover state (a ruin is
//! already rubble at tick zero). An atlas that measured with its own math would be a second
//! implementation waiting to drift; this one is a camera pointed at the real rules.
//!
//! Layers per map (written as PNG by the `map-atlas` CLI command):
//!   * `form` — hypsometric tint x hillshade + contours + standing water;
//!   * `ground` — the four splat layers as the classifier blends them, wetness-darkened;
//!   * `drive` — grade classes (comfort / hard / wall) + the wading-to-drowning bands +
//!     cover footprints (solid vs crushable);
//!   * `tactical` — hillshade base + roads, cover by kind, scenery, spawns, strategic
//!     points, capture zones;
//!   * `exposure_from_south` / `exposure_from_north` — for every standable cell, the
//!     fraction of a stratified observer fleet on the enemy half that has `sim` LOS to a
//!     T-54's turret and hull sample points: white = unseen, red ramp = seen, blue = the
//!     hull-down band (turret engages, hull masked).
//!
//! Plus [`MapAtlasStats`]: the numbers the per-map dossier claims should be checkable
//! against (drivable share, water bands, hull-down share, engagement-distance profile).

use physics::water::{FORD_MAX_DEPTH_M, WADE_DRAG_START_M};
use terrain::{
    BattlefieldMap, GroundClassifier, HeightMap, RoadSurface, StaticCoverKind, StaticCoverObject,
    born_cover_phase_byte,
};

/// One RGB raster. The atlas draws in plain 8-bit RGB — layers are review instruments, not
/// assets, and an alpha channel would only invite compositing nobody does.
pub struct Raster {
    pub width: usize,
    pub height: usize,
    pub px: Vec<[u8; 3]>,
}

impl Raster {
    fn new(width: usize, height: usize, fill: [u8; 3]) -> Self {
        Self { width, height, px: vec![fill; width * height] }
    }

    fn put(&mut self, x: i32, y: i32, color: [u8; 3]) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.px[y as usize * self.width + x as usize] = color;
        }
    }

    fn blend(&mut self, x: i32, y: i32, color: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let index = y as usize * self.width + x as usize;
        let base = self.px[index];
        let a = alpha.clamp(0.0, 1.0);
        self.px[index] = [
            (base[0] as f32 * (1.0 - a) + color[0] as f32 * a) as u8,
            (base[1] as f32 * (1.0 - a) + color[1] as f32 * a) as u8,
            (base[2] as f32 * (1.0 - a) + color[2] as f32 * a) as u8,
        ];
    }

    fn disc(&mut self, cx: i32, cy: i32, radius: i32, color: [u8; 3], alpha: f32) {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= radius * radius {
                    self.blend(cx + dx, cy + dy, color, alpha);
                }
            }
        }
    }

    fn circle(&mut self, cx: i32, cy: i32, radius: i32, color: [u8; 3], alpha: f32) {
        let steps = (radius * 8).max(16);
        for step in 0..steps {
            let angle = step as f32 / steps as f32 * std::f32::consts::TAU;
            let x = cx + (angle.cos() * radius as f32).round() as i32;
            let y = cy + (angle.sin() * radius as f32).round() as i32;
            self.blend(x, y, color, alpha);
        }
    }

    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 3], alpha: f32) {
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.blend(x, y, color, alpha);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn text(&mut self, x: i32, y: i32, text: &str, color: [u8; 3]) {
        let mut cursor = x;
        for ch in text.chars() {
            let glyph = glyph_rows(ch.to_ascii_uppercase());
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0b1_0000 >> col) != 0 {
                        self.put(cursor + col, y + row as i32, color);
                    }
                }
            }
            cursor += 6;
        }
    }

    /// Encode as an 8-bit RGB PNG.
    pub fn to_png_bytes(&self) -> anyhow::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, self.width as u32, self.height as u32);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(self.px.as_flattened())?;
        }
        Ok(bytes)
    }
}

/// A 5x7 glyph per character the atlas labels with; unknown characters render as a box.
#[rustfmt::skip]
fn glyph_rows(ch: char) -> [u8; 7] {
    match ch {
        'A' => [0x0E,0x11,0x11,0x1F,0x11,0x11,0x11],
        'B' => [0x1E,0x11,0x11,0x1E,0x11,0x11,0x1E],
        'C' => [0x0E,0x11,0x10,0x10,0x10,0x11,0x0E],
        'D' => [0x1E,0x11,0x11,0x11,0x11,0x11,0x1E],
        'E' => [0x1F,0x10,0x10,0x1E,0x10,0x10,0x1F],
        'F' => [0x1F,0x10,0x10,0x1E,0x10,0x10,0x10],
        'G' => [0x0E,0x11,0x10,0x17,0x11,0x11,0x0E],
        'H' => [0x11,0x11,0x11,0x1F,0x11,0x11,0x11],
        'I' => [0x0E,0x04,0x04,0x04,0x04,0x04,0x0E],
        'J' => [0x07,0x02,0x02,0x02,0x02,0x12,0x0C],
        'K' => [0x11,0x12,0x14,0x18,0x14,0x12,0x11],
        'L' => [0x10,0x10,0x10,0x10,0x10,0x10,0x1F],
        'M' => [0x11,0x1B,0x15,0x15,0x11,0x11,0x11],
        'N' => [0x11,0x19,0x15,0x13,0x11,0x11,0x11],
        'O' => [0x0E,0x11,0x11,0x11,0x11,0x11,0x0E],
        'P' => [0x1E,0x11,0x11,0x1E,0x10,0x10,0x10],
        'Q' => [0x0E,0x11,0x11,0x11,0x15,0x12,0x0D],
        'R' => [0x1E,0x11,0x11,0x1E,0x14,0x12,0x11],
        'S' => [0x0F,0x10,0x10,0x0E,0x01,0x01,0x1E],
        'T' => [0x1F,0x04,0x04,0x04,0x04,0x04,0x04],
        'U' => [0x11,0x11,0x11,0x11,0x11,0x11,0x0E],
        'V' => [0x11,0x11,0x11,0x11,0x11,0x0A,0x04],
        'W' => [0x11,0x11,0x11,0x15,0x15,0x15,0x0A],
        'X' => [0x11,0x11,0x0A,0x04,0x0A,0x11,0x11],
        'Y' => [0x11,0x11,0x0A,0x04,0x04,0x04,0x04],
        'Z' => [0x1F,0x01,0x02,0x04,0x08,0x10,0x1F],
        '0' => [0x0E,0x11,0x13,0x15,0x19,0x11,0x0E],
        '1' => [0x04,0x0C,0x04,0x04,0x04,0x04,0x0E],
        '2' => [0x0E,0x11,0x01,0x02,0x04,0x08,0x1F],
        '3' => [0x1E,0x01,0x01,0x0E,0x01,0x01,0x1E],
        '4' => [0x02,0x06,0x0A,0x12,0x1F,0x02,0x02],
        '5' => [0x1F,0x10,0x1E,0x01,0x01,0x11,0x0E],
        '6' => [0x0E,0x10,0x10,0x1E,0x11,0x11,0x0E],
        '7' => [0x1F,0x01,0x02,0x04,0x08,0x08,0x08],
        '8' => [0x0E,0x11,0x11,0x0E,0x11,0x11,0x0E],
        '9' => [0x0E,0x11,0x11,0x0F,0x01,0x01,0x0E],
        '-' => [0x00,0x00,0x00,0x1F,0x00,0x00,0x00],
        '_' => [0x00,0x00,0x00,0x00,0x00,0x00,0x1F],
        '.' => [0x00,0x00,0x00,0x00,0x00,0x0C,0x0C],
        ' ' => [0x00; 7],
        _ => [0x1F,0x11,0x11,0x11,0x11,0x11,0x1F],
    }
}

// --- the shared measuring vocabulary -------------------------------------------------------

/// The grade classes the drive layer paints, split exactly on the game's two constants:
/// a road's comfort promise and the hull's climb authority.
const GRADE_COMFORT: f32 = game_core::ROAD_COMFORT_GRADE;
const GRADE_WALL: f32 = game_core::MAX_CLIMB_GRADE;

/// Worst 5 m-step grade at a world point — the same step the map contracts scan drivability
/// with ("worst 5 m-step grade"), measured through `sample_height` like everything else.
fn step_grade(heightmap: &HeightMap, x: f32, z: f32) -> f32 {
    const STEP: f32 = 5.0;
    let here = height_clamped(heightmap, x, z);
    let mut worst = 0.0f32;
    for (dx, dz) in [(STEP, 0.0), (-STEP, 0.0), (0.0, STEP), (0.0, -STEP)] {
        let there = height_clamped(heightmap, x + dx, z + dz);
        worst = worst.max((there - here).abs() / STEP);
    }
    worst
}

/// `sample_height` with edge clamping: the atlas probes pixel centres right up to the map
/// border, and the closed-interval sampler answers `None` only past the edge.
fn height_clamped(heightmap: &HeightMap, x: f32, z: f32) -> f32 {
    let [ex, ez] = heightmap.extent_m();
    heightmap.sample_height(x.clamp(0.0, ex), z.clamp(0.0, ez)).unwrap_or(0.0)
}

/// Hillshade factor from a north-west sun, sampled at the terrain's own half-cell scale.
fn hillshade(heightmap: &HeightMap, x: f32, z: f32) -> f32 {
    let step = heightmap.cell_size_m() * 0.5;
    let dx = height_clamped(heightmap, x + step, z) - height_clamped(heightmap, x - step, z);
    let dz = height_clamped(heightmap, x, z + step) - height_clamped(heightmap, x, z - step);
    let normal_len = (dx * dx + (2.0 * step) * (2.0 * step) + dz * dz).sqrt().max(1.0e-6);
    let normal = [-dx / normal_len, 2.0 * step / normal_len, -dz / normal_len];
    // Sun from the north-west (-x, +z in world; the raster draws north = +z at the top),
    // 45 degrees up.
    let light = [-0.5, std::f32::consts::FRAC_1_SQRT_2, 0.5];
    let dot = normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2];
    0.55 + dot.max(0.0) * 0.6
}

fn shade(color: [u8; 3], factor: f32) -> [u8; 3] {
    [
        (color[0] as f32 * factor).clamp(0.0, 255.0) as u8,
        (color[1] as f32 * factor).clamp(0.0, 255.0) as u8,
        (color[2] as f32 * factor).clamp(0.0, 255.0) as u8,
    ]
}

fn atlas_lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}

/// Deterministic splitmix64 — seeded sampling for the engagement profile.
fn mix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn atlas_unit(state: &mut u64) -> f32 {
    (mix64(state) >> 40) as f32 / ((1u64 << 24) - 1) as f32
}

/// The frame every layer shares: world → pixel with north (+z) at the top of the image.
struct Frame {
    res_m: f32,
    width: usize,
    height: usize,
}

impl Frame {
    fn for_map(map: &BattlefieldMap, res_m: f32) -> Self {
        Self {
            res_m,
            width: (map.size_m[0] / res_m).round() as usize,
            height: (map.size_m[1] / res_m).round() as usize,
        }
    }

    fn world_at(&self, px: usize, py: usize) -> (f32, f32) {
        let x = (px as f32 + 0.5) * self.res_m;
        let z = (self.height - 1 - py) as f32 * self.res_m + 0.5 * self.res_m;
        (x, z)
    }

    fn pixel_at(&self, x: f32, z: f32) -> (i32, i32) {
        let px = (x / self.res_m - 0.5).round() as i32;
        let py = self.height as i32 - 1 - (z / self.res_m - 0.5).round() as i32;
        (px, py)
    }
}

/// The map's cover in its BORN phase, as boxes the eye meets at tick zero: a born ruin that
/// leaves rubble becomes its mound (per-kind height fraction, seated on the ground), a born
/// ruin that goes clear disappears, everything else stands as authored. The same birth rule
/// the sim and every baker read (`terrain::born_cover_phase_byte`).
fn born_cover_boxes(map: &BattlefieldMap) -> Vec<StaticCoverObject> {
    map.static_cover
        .iter()
        .filter_map(|object| match born_cover_phase_byte(object) {
            0 => Some(object.clone()),
            1 => {
                let ground = object.center[1] - object.half_extents_m[1];
                let half_height = object.half_extents_m[1] * object.kind.rubble_height_frac();
                let mut mound = object.clone();
                mound.center[1] = ground + half_height;
                mound.half_extents_m[1] = half_height;
                Some(mound)
            }
            _ => None,
        })
        .collect()
}

// --- layers --------------------------------------------------------------------------------

/// Hypsometric tint x hillshade, 1 m contours (5 m emphasized), standing water by depth.
pub fn form_layer(map: &BattlefieldMap, res_m: f32) -> Raster {
    let frame = Frame::for_map(map, res_m);
    let heightmap = &map.heightmap;
    let stats = heightmap.stats();
    let span = stats.range_m().max(1.0);
    let mut raster = Raster::new(frame.width, frame.height, [0, 0, 0]);
    let ramp: [(f32, [u8; 3]); 5] = [
        (0.0, [74, 112, 72]),
        (0.35, [125, 148, 82]),
        (0.6, [172, 152, 96]),
        (0.8, [152, 122, 92]),
        (1.0, [206, 200, 190]),
    ];
    for py in 0..frame.height {
        for px in 0..frame.width {
            let (x, z) = frame.world_at(px, py);
            let h = height_clamped(heightmap, x, z);
            let t = (h - stats.min_m) / span;
            let mut color = ramp[ramp.len() - 1].1;
            for pair in ramp.windows(2) {
                if t <= pair[1].0 {
                    color = atlas_lerp_rgb(
                        pair[0].1,
                        pair[1].1,
                        (t - pair[0].0) / (pair[1].0 - pair[0].0),
                    );
                    break;
                }
            }
            let mut color = shade(color, hillshade(heightmap, x, z));
            // Contours: compare the height BAND of this pixel against its +x / +z pixel
            // neighbours; a band change draws the line once, emphasized every 5 m.
            let band = |v: f32| v.floor() as i32;
            let h_right = height_clamped(heightmap, x + res_m, z);
            let h_up = height_clamped(heightmap, x, z + res_m);
            if band(h) != band(h_right) || band(h) != band(h_up) {
                let major = band(h.max(h_right).max(h_up)) % 5 == 0;
                let alpha = if major { 0.5 } else { 0.22 };
                color = atlas_lerp_rgb(color, [40, 32, 24], alpha);
            }
            if let Some(water) = &map.water {
                let depth = water.depth_over(h);
                if depth > 0.0 {
                    let deep = (depth / sim::DROWN_DEPTH_M).clamp(0.0, 1.0);
                    color = atlas_lerp_rgb(
                        color,
                        atlas_lerp_rgb([90, 140, 190], [16, 44, 108], deep),
                        0.85,
                    );
                }
            }
            raster.px[py * frame.width + px] = color;
        }
    }
    raster
}

/// The classifier's four splat layers blended exactly by its weights, darkened by drainage
/// wetness — what the eye reads under the look, and what the tracks feel.
pub fn ground_layer(map: &BattlefieldMap, classifier: &GroundClassifier, res_m: f32) -> Raster {
    let frame = Frame::for_map(map, res_m);
    let heightmap = &map.heightmap;
    let mut raster = Raster::new(frame.width, frame.height, [0, 0, 0]);
    let palette: [[u8; 3]; 4] = [
        [88, 118, 66],   // grass
        [168, 148, 94],  // straw
        [122, 98, 72],   // dirt
        [142, 142, 138], // rock
    ];
    for py in 0..frame.height {
        for px in 0..frame.width {
            let (x, z) = frame.world_at(px, py);
            let weights = classifier.weights_at(heightmap, x, z);
            let total: f32 = weights.iter().sum::<f32>().max(1.0e-4);
            let mut rgb = [0.0f32; 3];
            for (color, weight) in palette.iter().zip(weights) {
                for (channel, value) in rgb.iter_mut().zip(color) {
                    *channel += *value as f32 * (weight / total);
                }
            }
            let wet = classifier.flow_wetness_at(x, z);
            let factor = hillshade(heightmap, x, z) * (1.0 - wet * 0.35);
            let mut color = shade([rgb[0] as u8, rgb[1] as u8, rgb[2] as u8], factor);
            if let Some(water) = &map.water {
                let depth = water.depth_over(height_clamped(heightmap, x, z));
                if depth > 0.0 {
                    color = atlas_lerp_rgb(color, [30, 70, 140], 0.8);
                }
            }
            raster.px[py * frame.width + px] = color;
        }
    }
    raster
}

/// Drive-layer classes, exported so the locking tests speak the same colors the tool paints.
pub const DRIVE_COMFORT: [u8; 3] = [104, 162, 92];
pub const DRIVE_HARD: [u8; 3] = [224, 172, 62];
pub const DRIVE_WALL: [u8; 3] = [142, 32, 32];
pub const DRIVE_SPLASH: [u8; 3] = [152, 200, 214];
pub const DRIVE_FORD: [u8; 3] = [74, 142, 202];
pub const DRIVE_DEEP: [u8; 3] = [42, 92, 172];
pub const DRIVE_DROWN: [u8; 3] = [18, 46, 110];
pub const DRIVE_SOLID_COVER: [u8; 3] = [24, 24, 28];
pub const DRIVE_CRUSHABLE_COVER: [u8; 3] = [110, 104, 96];

/// The drivability class at one world point, before hillshading — the testable core of the
/// drive layer. Water wins over grade (a drowned wall is water first), matching how a crew
/// reads the map: blue means "the water decides here".
pub fn drive_class(map: &BattlefieldMap, x: f32, z: f32) -> [u8; 3] {
    let h = height_clamped(&map.heightmap, x, z);
    if let Some(water) = &map.water {
        let depth = water.depth_over(h);
        if depth >= sim::DROWN_DEPTH_M {
            return DRIVE_DROWN;
        }
        if depth >= FORD_MAX_DEPTH_M {
            return DRIVE_DEEP;
        }
        if depth >= WADE_DRAG_START_M {
            return DRIVE_FORD;
        }
        if depth > 0.0 {
            return DRIVE_SPLASH;
        }
    }
    let grade = step_grade(&map.heightmap, x, z);
    if grade >= GRADE_WALL {
        DRIVE_WALL
    } else if grade >= GRADE_COMFORT {
        DRIVE_HARD
    } else {
        DRIVE_COMFORT
    }
}

/// Grade classes + water bands + cover footprints + roads: where a hull can actually go.
pub fn drive_layer(map: &BattlefieldMap, res_m: f32) -> Raster {
    let frame = Frame::for_map(map, res_m);
    let mut raster = Raster::new(frame.width, frame.height, [0, 0, 0]);
    for py in 0..frame.height {
        for px in 0..frame.width {
            let (x, z) = frame.world_at(px, py);
            let class = drive_class(map, x, z);
            raster.px[py * frame.width + px] =
                shade(class, hillshade(&map.heightmap, x, z).clamp(0.7, 1.1));
        }
    }
    draw_roads(&mut raster, &frame, map, 0.85);
    for object in &map.static_cover {
        let color =
            if object.kind.is_crushable() { DRIVE_CRUSHABLE_COVER } else { DRIVE_SOLID_COVER };
        fill_box(&mut raster, &frame, object, color, 1.0);
    }
    raster
}

/// Hillshade base + every authored gameplay element: the map card.
pub fn tactical_layer(map: &BattlefieldMap, res_m: f32) -> Raster {
    let frame = Frame::for_map(map, res_m);
    let heightmap = &map.heightmap;
    let mut raster = Raster::new(frame.width, frame.height, [0, 0, 0]);
    for py in 0..frame.height {
        for px in 0..frame.width {
            let (x, z) = frame.world_at(px, py);
            let mut color = shade([196, 192, 184], hillshade(heightmap, x, z));
            if let Some(water) = &map.water {
                let depth = water.depth_over(height_clamped(heightmap, x, z));
                if depth > 0.0 {
                    color = atlas_lerp_rgb(color, [40, 90, 160], 0.85);
                }
            }
            raster.px[py * frame.width + px] = color;
        }
    }
    draw_roads(&mut raster, &frame, map, 1.0);
    for instance in &map.scenery {
        let color = match instance.kind {
            terrain::SceneryKind::Rock => [104, 100, 96],
            terrain::SceneryKind::Bush => [96, 132, 72],
            terrain::SceneryKind::Lamppost | terrain::SceneryKind::DebrisHeap => [80, 76, 72],
            _ => [44, 108, 52],
        };
        let (px, py) = frame.pixel_at(instance.position[0], instance.position[2]);
        raster.disc(px, py, 1, color, 0.9);
    }
    for object in &map.static_cover {
        let color = match object.kind {
            StaticCoverKind::FarmBuilding => [152, 78, 48],
            StaticCoverKind::CityBuilding => [70, 70, 82],
            StaticCoverKind::RailCover => [84, 84, 92],
            StaticCoverKind::TreeLine => [30, 92, 42],
            StaticCoverKind::Wreck => [58, 46, 40],
            StaticCoverKind::WoodenFence => [188, 158, 92],
            StaticCoverKind::StoneWall => [158, 122, 92],
            StaticCoverKind::TreeTrunk => [92, 62, 32],
        };
        fill_box(&mut raster, &frame, object, color, 1.0);
    }
    for zone in &map.capture_zones {
        let (px, py) = frame.pixel_at(zone.center[0], zone.center[2]);
        let radius = (zone.radius_m / frame.res_m) as i32;
        raster.circle(px, py, radius, [255, 70, 70], 0.95);
        raster.circle(px, py, radius - 1, [255, 70, 70], 0.55);
        raster.text(px - 8, py - 3, "CAP", [255, 70, 70]);
    }
    for point in &map.strategic_points {
        let (px, py) = frame.pixel_at(point.position[0], point.position[2]);
        let radius = (point.radius_m / frame.res_m) as i32;
        let (label, color) = match point.role {
            terrain::StrategicRole::HighGround => ("H", [232, 190, 30]),
            terrain::StrategicRole::Crossing => ("C", [30, 190, 232]),
            terrain::StrategicRole::Observation => ("O", [240, 240, 240]),
            terrain::StrategicRole::HullDown => ("D", [110, 150, 255]),
            terrain::StrategicRole::FlankRoute => ("F", [210, 130, 255]),
        };
        raster.circle(px, py, radius, color, 0.6);
        raster.text(px - 2, py - 3, label, color);
    }
    for spawn in &map.spawn_zones {
        let (px, py) = frame.pixel_at(spawn.center[0], spawn.center[2]);
        let radius = (spawn.radius_m / frame.res_m) as i32;
        let color = if spawn.team == 1 { [50, 120, 230] } else { [230, 70, 50] };
        raster.circle(px, py, radius, color, 1.0);
        raster.circle(px, py, radius - 1, color, 0.6);
        raster.text(px - 5, py - 3, if spawn.team == 1 { "S1" } else { "S2" }, color);
    }
    raster.text(4, 4, map.id.as_str(), [20, 20, 20]);
    raster
}

fn draw_roads(raster: &mut Raster, frame: &Frame, map: &BattlefieldMap, alpha: f32) {
    for road in &map.roads {
        let color = match road.surface {
            RoadSurface::Dirt => [154, 124, 90],
            RoadSurface::Ballast => [128, 124, 118],
            RoadSurface::Cobble => [106, 106, 118],
        };
        let width_px = (road.width_m / frame.res_m * 0.5).max(1.0) as i32;
        for pair in road.points.windows(2) {
            let (x0, y0) = frame.pixel_at(pair[0][0], pair[0][1]);
            let (x1, y1) = frame.pixel_at(pair[1][0], pair[1][1]);
            for offset_x in -width_px / 2..=width_px / 2 {
                for offset_y in -width_px / 2..=width_px / 2 {
                    raster.line(
                        x0 + offset_x,
                        y0 + offset_y,
                        x1 + offset_x,
                        y1 + offset_y,
                        color,
                        alpha,
                    );
                }
            }
        }
    }
}

fn fill_box(
    raster: &mut Raster,
    frame: &Frame,
    object: &StaticCoverObject,
    color: [u8; 3],
    alpha: f32,
) {
    let (x0, z0) =
        (object.center[0] - object.half_extents_m[0], object.center[2] - object.half_extents_m[2]);
    let (x1, z1) =
        (object.center[0] + object.half_extents_m[0], object.center[2] + object.half_extents_m[2]);
    let (px0, py1) = frame.pixel_at(x0, z0);
    let (px1, py0) = frame.pixel_at(x1, z1);
    for py in py0..=py1 {
        for px in px0..=px1 {
            raster.blend(px, py, color, alpha);
        }
    }
}

// --- exposure ------------------------------------------------------------------------------

/// The benchmark eye and target geometry: the T-54's spec, exactly as the sim reads it.
/// The observer's eye is the hitbox top (`sim::spotting::observer_eye`), the target samples
/// are the hull centre and the turret top (`sim::spotting::target_points`).
struct FleetGeometry {
    eye_m: f32,
    hull_m: f32,
    turret_m: f32,
    view_range_m: f32,
}

impl FleetGeometry {
    fn benchmark() -> Self {
        let spec = game_core::TankSpec::t54_1951();
        let hitbox = &spec.hitbox;
        Self {
            eye_m: hitbox.center_y_m + hitbox.half_height_m,
            hull_m: hitbox.center_y_m,
            turret_m: hitbox.center_y_m + hitbox.half_height_m,
            view_range_m: spec.view_range_m(),
        }
    }
}

/// One cell's exposure verdict toward one observer fleet.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ExposureClass {
    /// No standable hull here: climb wall or water past the ford band.
    NotStandable,
    /// No observer sees either sample point: a masked approach.
    Hidden,
    /// The turret top is seen but the hull centre is not: the hull-down band.
    HullDown,
    /// Both sample points are seen by at least one observer.
    Exposed,
}

pub struct ExposureField {
    pub cell_m: f32,
    pub cols: usize,
    pub rows: usize,
    pub class: Vec<ExposureClass>,
    /// Fraction of the observer fleet with LOS to the turret sample, per cell.
    pub turret_seen: Vec<f32>,
    pub observer_count: usize,
}

impl ExposureField {
    fn index_at(&self, x: f32, z: f32) -> usize {
        let col = ((x / self.cell_m) as usize).min(self.cols - 1);
        let row = ((z / self.cell_m) as usize).min(self.rows - 1);
        row * self.cols + col
    }

    pub fn class_at(&self, x: f32, z: f32) -> ExposureClass {
        self.class[self.index_at(x, z)]
    }

    pub fn turret_seen_at(&self, x: f32, z: f32) -> f32 {
        self.turret_seen[self.index_at(x, z)]
    }
}

/// Where a hull can stand for the exposure sweep: under the climb wall and inside the
/// wading band (a hull glued to a 0.55 face or parked in drowning water holds no position).
fn standable(map: &BattlefieldMap, x: f32, z: f32) -> bool {
    if step_grade(&map.heightmap, x, z) >= GRADE_WALL {
        return false;
    }
    if let Some(water) = &map.water {
        let depth = water.depth_over(height_clamped(&map.heightmap, x, z));
        if depth >= FORD_MAX_DEPTH_M {
            return false;
        }
    }
    true
}

/// Sweep the whole map against a stratified observer fleet standing on one half.
///
/// `from_north` picks the observer half: north is `z > axis` (the raster's top). Observers
/// stand `observer_step_m` apart on standable ground at the benchmark eye height; every
/// grid cell's two target samples are tested with `sim::line_of_sight` over the map's
/// born cover, inside the observer's view range. The turret ray is measured first and the
/// hull ray only behind a seen turret: with ground-standing cover a lower line to the same
/// eye is blocked at least as often, so an unseen turret implies an unseen hull.
pub fn exposure_field(
    map: &BattlefieldMap,
    from_north: bool,
    cell_m: f32,
    observer_step_m: f32,
) -> ExposureField {
    let geometry = FleetGeometry::benchmark();
    let cover = born_cover_boxes(map);
    let heightmap = &map.heightmap;
    let axis_z = map.size_m[1] * 0.5;

    let mut observers: Vec<glam::Vec3> = Vec::new();
    let margin = 30.0;
    let (z_from, z_to) = if from_north {
        (axis_z + margin, map.size_m[1] - margin)
    } else {
        (margin, axis_z - margin)
    };
    let mut z = z_from;
    while z <= z_to {
        let mut x = margin;
        while x <= map.size_m[0] - margin {
            if standable(map, x, z) {
                observers.push(glam::Vec3::new(
                    x,
                    height_clamped(heightmap, x, z) + geometry.eye_m,
                    z,
                ));
            }
            x += observer_step_m;
        }
        z += observer_step_m;
    }

    let cols = (map.size_m[0] / cell_m).round() as usize;
    let rows = (map.size_m[1] / cell_m).round() as usize;
    let mut class = vec![ExposureClass::NotStandable; cols * rows];
    let mut turret_seen = vec![0.0f32; cols * rows];
    for row in 0..rows {
        for col in 0..cols {
            let x = (col as f32 + 0.5) * cell_m;
            let z = (row as f32 + 0.5) * cell_m;
            let index = row * cols + col;
            if !standable(map, x, z) {
                continue;
            }
            let ground = height_clamped(heightmap, x, z);
            let turret = glam::Vec3::new(x, ground + geometry.turret_m, z);
            let hull = glam::Vec3::new(x, ground + geometry.hull_m, z);
            let mut turret_hits = 0usize;
            let mut hull_seen = false;
            for eye in &observers {
                if eye.distance(turret) > geometry.view_range_m {
                    continue;
                }
                if sim::line_of_sight(Some(heightmap), &cover, *eye, turret) {
                    turret_hits += 1;
                    if !hull_seen && sim::line_of_sight(Some(heightmap), &cover, *eye, hull) {
                        hull_seen = true;
                    }
                }
            }
            turret_seen[index] = turret_hits as f32 / observers.len().max(1) as f32;
            class[index] = if turret_hits == 0 {
                ExposureClass::Hidden
            } else if hull_seen {
                ExposureClass::Exposed
            } else {
                ExposureClass::HullDown
            };
        }
    }
    ExposureField { cell_m, cols, rows, class, turret_seen, observer_count: observers.len() }
}

/// Render an exposure field over the map's relief: white = hidden, red ramp = exposed (by
/// how much of the fleet sees the turret), blue = the hull-down band, slate = not standable.
pub fn exposure_layer(map: &BattlefieldMap, field: &ExposureField, res_m: f32) -> Raster {
    let frame = Frame::for_map(map, res_m);
    let mut raster = Raster::new(frame.width, frame.height, [0, 0, 0]);
    for py in 0..frame.height {
        for px in 0..frame.width {
            let (x, z) = frame.world_at(px, py);
            let seen = field.turret_seen_at(x, z);
            let color = match field.class_at(x, z) {
                ExposureClass::NotStandable => [70, 74, 82],
                ExposureClass::Hidden => [238, 238, 228],
                ExposureClass::HullDown => {
                    atlas_lerp_rgb([120, 156, 240], [50, 84, 210], (seen * 3.0).min(1.0))
                }
                ExposureClass::Exposed => {
                    atlas_lerp_rgb([242, 210, 160], [200, 34, 26], (seen * 2.5).min(1.0))
                }
            };
            let mut color = shade(color, hillshade(&map.heightmap, x, z).clamp(0.75, 1.1));
            if let Some(water) = &map.water {
                let depth = water.depth_over(height_clamped(&map.heightmap, x, z));
                if depth >= FORD_MAX_DEPTH_M {
                    color = atlas_lerp_rgb(color, [18, 46, 110], 0.8);
                }
            }
            raster.px[py * frame.width + px] = color;
        }
    }
    for object in &map.static_cover {
        fill_box(&mut raster, &frame, object, [30, 30, 34], 0.9);
    }
    for spawn in &map.spawn_zones {
        let (px, py) = frame.pixel_at(spawn.center[0], spawn.center[2]);
        let color = if spawn.team == 1 { [50, 120, 230] } else { [230, 70, 50] };
        raster.circle(px, py, (spawn.radius_m / frame.res_m) as i32, color, 1.0);
    }
    raster
}

// --- stats ---------------------------------------------------------------------------------

/// The sim-vs-render ground parity: `sample_height` (the lane contact, shell traces and
/// LOS resolve through) measured directly against the two planes the render mesh draws for
/// each cell (`scene_build::terrain_scene_mesh_full`, fixed anti-diagonal split). Since
/// the surface-parity fix the sampler stands on those planes by construction and this
/// residual is exactly zero; the instrument keeps measuring it so any second surface
/// definition that ever creeps back in is a red number, not a comment. "What blocks the
/// shell blocks the eye" is a promise about THIS residual.
pub struct MeshParityStats {
    pub worst_m: f32,
    pub worst_at: (f32, f32),
    /// Share of cells whose worst divergence exceeds 5 cm / 15 cm.
    pub over_5cm_share: f32,
    pub over_15cm_share: f32,
}

pub fn mesh_parity(heightmap: &HeightMap) -> MeshParityStats {
    let mut worst_m = 0.0f32;
    let mut worst_at = (0.0, 0.0);
    let mut over_5 = 0usize;
    let mut over_15 = 0usize;
    let mut cells = 0usize;
    let cell = heightmap.cell_size_m();
    for z0 in 0..heightmap.height() - 1 {
        for x0 in 0..heightmap.width() - 1 {
            let h00 = heightmap.sample_at_index(x0, z0);
            let h10 = heightmap.sample_at_index(x0 + 1, z0);
            let h01 = heightmap.sample_at_index(x0, z0 + 1);
            let h11 = heightmap.sample_at_index(x0 + 1, z0 + 1);
            // One probe per triangle of the CHOSEN split, the drawn plane evaluated with
            // the same float expressions the sampler uses, so parity reads exactly 0.0
            // when sim and mesh agree.
            let (pa, pb, plane_a, plane_b) =
                if terrain::cell_splits_on_main_diagonal(h00, h10, h01, h11) {
                    (
                        (0.75, 0.25),
                        (0.25, 0.75),
                        h00 + 0.75 * (h10 - h00) + 0.25 * (h11 - h10),
                        h00 + 0.75 * (h01 - h00) + 0.25 * (h11 - h01),
                    )
                } else {
                    (
                        (0.25, 0.25),
                        (0.75, 0.75),
                        h00 + 0.25 * (h10 - h00) + 0.25 * (h01 - h00),
                        h11 + 0.25 * (h01 - h11) + 0.25 * (h10 - h11),
                    )
                };
            let sample_a = heightmap
                .sample_height((x0 as f32 + pa.0) * cell, (z0 as f32 + pa.1) * cell)
                .unwrap_or(plane_a);
            let sample_b = heightmap
                .sample_height((x0 as f32 + pb.0) * cell, (z0 as f32 + pb.1) * cell)
                .unwrap_or(plane_b);
            let divergence = (sample_a - plane_a).abs().max((sample_b - plane_b).abs());
            cells += 1;
            if divergence > 0.05 {
                over_5 += 1;
            }
            if divergence > 0.15 {
                over_15 += 1;
            }
            if divergence > worst_m {
                worst_m = divergence;
                worst_at = (
                    (x0 as f32 + 0.5) * heightmap.cell_size_m(),
                    (z0 as f32 + 0.5) * heightmap.cell_size_m(),
                );
            }
        }
    }
    let total = cells.max(1) as f32;
    MeshParityStats {
        worst_m,
        worst_at,
        over_5cm_share: over_5 as f32 / total,
        over_15cm_share: over_15 as f32 / total,
    }
}

pub struct ExposureShare {
    pub observers: usize,
    pub hidden: f32,
    pub hull_down: f32,
    pub exposed: f32,
}

pub struct MapAtlasStats {
    pub id: String,
    pub height_min_m: f32,
    pub height_max_m: f32,
    pub grade_comfort_share: f32,
    pub grade_hard_share: f32,
    pub grade_wall_share: f32,
    pub water_share: f32,
    pub cover_counts: Vec<(StaticCoverKind, usize)>,
    pub scenery_count: usize,
    pub from_south: ExposureShare,
    pub from_north: ExposureShare,
    /// Clear-LOS share of sampled standable pairs, by distance band (upper edges in m).
    pub engagement_bands: Vec<(f32, f32)>,
    pub mesh_parity: MeshParityStats,
}

fn exposure_share(field: &ExposureField) -> ExposureShare {
    let mut hidden = 0usize;
    let mut hull_down = 0usize;
    let mut exposed = 0usize;
    let mut standable = 0usize;
    for class in &field.class {
        match class {
            ExposureClass::NotStandable => {}
            ExposureClass::Hidden => {
                hidden += 1;
                standable += 1;
            }
            ExposureClass::HullDown => {
                hull_down += 1;
                standable += 1;
            }
            ExposureClass::Exposed => {
                exposed += 1;
                standable += 1;
            }
        }
    }
    let total = standable.max(1) as f32;
    ExposureShare {
        observers: field.observer_count,
        hidden: hidden as f32 / total,
        hull_down: hull_down as f32 / total,
        exposed: exposed as f32 / total,
    }
}

/// The engagement profile: over seeded random standable pairs, how often a turret-height
/// sight line is clear, per distance band — the map's fingerprint of long vs short fights.
fn engagement_bands(map: &BattlefieldMap, cover: &[StaticCoverObject]) -> Vec<(f32, f32)> {
    let geometry = FleetGeometry::benchmark();
    let heightmap = &map.heightmap;
    let bands = [100.0, 200.0, 300.0, 400.0, 550.0];
    let mut clear = vec![0usize; bands.len()];
    let mut total = vec![0usize; bands.len()];
    let mut state = 0xA71A_5EEDu64;
    let mut sampled = 0usize;
    let mut attempts = 0usize;
    while sampled < 6000 && attempts < 120_000 {
        attempts += 1;
        let ax = 20.0 + atlas_unit(&mut state) * (map.size_m[0] - 40.0);
        let az = 20.0 + atlas_unit(&mut state) * (map.size_m[1] - 40.0);
        let bx = 20.0 + atlas_unit(&mut state) * (map.size_m[0] - 40.0);
        let bz = 20.0 + atlas_unit(&mut state) * (map.size_m[1] - 40.0);
        let distance = ((ax - bx).powi(2) + (az - bz).powi(2)).sqrt();
        let Some(band) = bands.iter().position(|&edge| distance <= edge) else {
            continue;
        };
        if !standable(map, ax, az) || !standable(map, bx, bz) {
            continue;
        }
        sampled += 1;
        total[band] += 1;
        let eye = glam::Vec3::new(ax, height_clamped(heightmap, ax, az) + geometry.eye_m, az);
        let target = glam::Vec3::new(bx, height_clamped(heightmap, bx, bz) + geometry.turret_m, bz);
        if sim::line_of_sight(Some(heightmap), cover, eye, target) {
            clear[band] += 1;
        }
    }
    bands
        .iter()
        .zip(clear.iter().zip(total))
        .map(|(&edge, (&clear, total))| (edge, clear as f32 / total.max(1) as f32))
        .collect()
}

pub fn atlas_stats(
    map: &BattlefieldMap,
    from_south: &ExposureField,
    from_north: &ExposureField,
) -> MapAtlasStats {
    let stats = map.heightmap.stats();
    let mut comfort = 0usize;
    let mut hard = 0usize;
    let mut wall = 0usize;
    let mut water = 0usize;
    let mut total = 0usize;
    let step = 5.0;
    let mut z = 2.5;
    while z < map.size_m[1] {
        let mut x = 2.5;
        while x < map.size_m[0] {
            total += 1;
            let class = drive_class(map, x, z);
            if class == DRIVE_WALL {
                wall += 1;
            } else if class == DRIVE_HARD {
                hard += 1;
            } else if class == DRIVE_COMFORT {
                comfort += 1;
            } else {
                water += 1;
            }
            x += step;
        }
        z += step;
    }
    let mut cover_counts: Vec<(StaticCoverKind, usize)> = Vec::new();
    for kind in StaticCoverKind::ALL {
        let count = map.static_cover.iter().filter(|object| object.kind == kind).count();
        if count > 0 {
            cover_counts.push((kind, count));
        }
    }
    let denominator = total.max(1) as f32;
    MapAtlasStats {
        id: map.id.clone(),
        height_min_m: stats.min_m,
        height_max_m: stats.max_m,
        grade_comfort_share: comfort as f32 / denominator,
        grade_hard_share: hard as f32 / denominator,
        grade_wall_share: wall as f32 / denominator,
        water_share: water as f32 / denominator,
        cover_counts,
        scenery_count: map.scenery.len(),
        from_south: exposure_share(from_south),
        from_north: exposure_share(from_north),
        engagement_bands: engagement_bands(map, &born_cover_boxes(map)),
        mesh_parity: mesh_parity(&map.heightmap),
    }
}

impl MapAtlasStats {
    pub fn markdown_section(&self) -> String {
        let mut out = format!(
            "## {}\n\n\
             | metric | value |\n| --- | --- |\n\
             | height span | {:.1} - {:.1} m ({:.1} m relief) |\n\
             | grade comfort (<{:.2}) | {:.1}% |\n\
             | grade hard ({:.2}-{:.2}) | {:.1}% |\n\
             | grade wall (>={:.2}) | {:.1}% |\n\
             | standing water | {:.1}% |\n\
             | scenery instances | {} |\n",
            self.id,
            self.height_min_m,
            self.height_max_m,
            self.height_max_m - self.height_min_m,
            GRADE_COMFORT,
            self.grade_comfort_share * 100.0,
            GRADE_COMFORT,
            GRADE_WALL,
            self.grade_hard_share * 100.0,
            GRADE_WALL,
            self.grade_wall_share * 100.0,
            self.water_share * 100.0,
            self.scenery_count,
        );
        for (kind, count) in &self.cover_counts {
            out.push_str(&format!("| cover {kind:?} | {count} |\n"));
        }
        out.push_str(&format!(
            "| sim-vs-mesh ground parity | worst {:.2} m at ({:.0}, {:.0}); >5 cm on {:.1}% \
             of cells, >15 cm on {:.1}% |\n",
            self.mesh_parity.worst_m,
            self.mesh_parity.worst_at.0,
            self.mesh_parity.worst_at.1,
            self.mesh_parity.over_5cm_share * 100.0,
            self.mesh_parity.over_15cm_share * 100.0,
        ));
        out.push_str(&format!(
            "\n| exposure (standable cells) | hidden | hull-down | exposed |\n\
             | --- | --- | --- | --- |\n\
             | seen from the north half ({} eyes) | {:.1}% | {:.1}% | {:.1}% |\n\
             | seen from the south half ({} eyes) | {:.1}% | {:.1}% | {:.1}% |\n",
            self.from_north.observers,
            self.from_north.hidden * 100.0,
            self.from_north.hull_down * 100.0,
            self.from_north.exposed * 100.0,
            self.from_south.observers,
            self.from_south.hidden * 100.0,
            self.from_south.hull_down * 100.0,
            self.from_south.exposed * 100.0,
        ));
        out.push_str("\n| engagement band | clear-LOS share |\n| --- | --- |\n");
        let mut previous = 0.0;
        for (edge, share) in &self.engagement_bands {
            out.push_str(&format!("| {previous:.0}-{edge:.0} m | {:.1}% |\n", share * 100.0));
            previous = *edge;
        }
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrain::MapId;

    /// The drive layer's classes are the game's own numbers on the maps' own promises:
    /// Orliny's massif FACE is a wall while its Dolina gate and the crest walk both drive;
    /// Bystra's south ford sits in the wading band while the channel between crossings
    /// drowns. These are the dossier claims, checked through the same `drive_class` the PNG
    /// paints — and the first probe of this test caught the author aiming at the crest line
    /// itself, which is drivable BY DESIGN (the crest walk is the signature mechanic).
    #[test]
    fn the_drive_layer_speaks_the_maps_own_contracts() {
        let orliny = map_forge::battlefield(MapId::OrlinyPereval);
        assert_eq!(drive_class(&orliny, 350.0, 470.0), DRIVE_WALL, "the massif face is a wall");
        assert_eq!(drive_class(&orliny, 200.0, 500.0), DRIVE_COMFORT, "the Dolina col drives");
        assert_eq!(drive_class(&orliny, 350.0, 500.0), DRIVE_COMFORT, "the crest walk drives");

        let bystra = map_forge::battlefield(MapId::BystraValley);
        let ford_x = bystra.river.expect("the Bystra has a river").center_x(320.0);
        assert_eq!(drive_class(&bystra, ford_x, 320.0), DRIVE_FORD, "the south ford wades");
        let channel_x = bystra.river.expect("river").center_x(430.0);
        assert_eq!(
            drive_class(&bystra, channel_x, 430.0),
            DRIVE_DROWN,
            "the channel between crossings drowns"
        );
    }

    /// The exposure classes on a synthetic profile with a known answer: observers stand on
    /// a north plateau behind a scarp, the south plain carries a low lip that masks a hull
    /// but not a turret, and a deep trench behind the lip is dead ground. Open plain =
    /// Exposed, behind the lip = HullDown, trench floor = Hidden. This locks the
    /// instrument's SEMANTICS on geometry computed by hand; per-map stories stay in the
    /// map contracts.
    #[test]
    fn exposure_classes_read_a_single_ridge_correctly() {
        let gauss = |d: f32, sigma: f32| (-(d / sigma).powi(2) * 0.5).exp();
        let smooth = |t: f32| {
            let t = t.clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let heightmap = terrain::heightmap_from_fn(101, 5.0, |_, z| {
            10.0 + 2.0 * smooth((z - 278.0) / 9.0) + 2.6 * gauss(z - 225.0, 5.0)
                - 6.0 * gauss(z - 180.0, 6.0)
        });
        let map = BattlefieldMap {
            id: "ridge-test".into(),
            name: "ridge test".into(),
            size_m: heightmap.extent_m(),
            historical_basis: String::new(),
            design_notes: vec![],
            heightmap,
            water: None,
            river: None,
            spawn_zones: vec![],
            capture_zones: vec![],
            strategic_points: vec![],
            features: vec![],
            static_cover: vec![],
            scenery: vec![],
            roads: vec![],
        };
        let field = exposure_field(&map, true, 10.0, 60.0);
        assert!(field.observer_count > 10, "the plateau fields a real observer fleet");
        assert_eq!(
            field.class_at(250.0, 265.0),
            ExposureClass::Exposed,
            "the open plain in front of the lip is seen hull and all"
        );
        assert_eq!(
            field.class_at(250.0, 215.0),
            ExposureClass::HullDown,
            "behind the lip the turret engages while the hull is masked"
        );
        assert_eq!(
            field.class_at(250.0, 185.0),
            ExposureClass::Hidden,
            "the trench floor is dead ground"
        );
    }

    /// The same sweep pointed at Prokhorovka's anti-tank balka: the dossier promises full
    /// defilade from the embankment line, so from the whole north half the ditch floor must
    /// never read as hull-exposed.
    #[test]
    fn the_balka_reads_as_cover_in_the_exposure_sweep() {
        let map = map_forge::battlefield(MapId::ProkhorovkaHill252_2);
        let field = exposure_field(&map, true, 10.0, 90.0);
        let ditch = field.class_at(520.0, 384.0);
        assert_ne!(ditch, ExposureClass::Exposed, "the balka masks the hull (got {ditch:?})");
    }

    /// The surface-parity lock: on every shipped map, `sample_height` and the planes the
    /// render mesh draws are the SAME surface — the residual the atlas once measured at
    /// up to 0.48 m reads exactly zero, and stays a red number if a second surface
    /// definition ever creeps back in.
    #[test]
    fn the_sim_stands_on_the_drawn_ground() {
        for &id in MapId::SHIPPED {
            let map = map_forge::battlefield(id);
            let parity = mesh_parity(&map.heightmap);
            assert_eq!(
                parity.worst_m, 0.0,
                "{}: sim and mesh disagree by {} m at {:?}",
                map.id, parity.worst_m, parity.worst_at
            );
        }
    }

    /// Every shipped map renders every layer at review resolution without panicking, and the
    /// PNG encoder accepts the raster — the CLI's whole pipeline, minus the filesystem.
    #[test]
    fn every_shipped_map_renders_and_encodes() {
        for &id in MapId::SHIPPED {
            let map = map_forge::battlefield(id);
            let classifier = GroundClassifier::new(&map);
            for raster in [
                form_layer(&map, 4.0),
                ground_layer(&map, &classifier, 4.0),
                drive_layer(&map, 4.0),
                tactical_layer(&map, 4.0),
            ] {
                assert_eq!(raster.width, 250);
                assert_eq!(raster.height, 250);
                let png = raster.to_png_bytes().expect("png encodes");
                assert!(png.len() > 1000, "a real image, not a stub");
            }
        }
    }
}
