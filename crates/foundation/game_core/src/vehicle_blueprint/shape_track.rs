//! The running-gear shape data, split from `mod.rs` for the file budget: the wrapped belt, road
//! wheels, idler/sprocket placement, and the return rollers of layouts that carry their top run.
/// Running-gear shape: the wrapped track belt, road wheels, drive sprocket, and idler.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrackShape {
    pub center_x: f32,
    pub belt_half_thickness: f32,
    pub top_y: f32,
    pub bottom_y: f32,
    pub wheel_radius: f32,
    pub wheel_count: usize,
    pub wheel_first_z: f32,
    pub wheel_last_z: f32,
    pub end_radius: f32,
    /// |Z| of the idler (front) and drive-sprocket (rear) axles. Beyond `wheel_last_z` on the
    /// T-54, which carries separate end wheels the belt ramps up to; equal to the wheel-span end
    /// for vehicles whose belt still wraps at the outermost road wheels (the stadium loop).
    pub end_z: f32,
    /// Y of the idler/sprocket axles. Raised above the road-wheel axle line on the T-54; equal to
    /// the axle line for the stadium wrap.
    pub end_y: f32,
    pub inner_x: f32,
    pub outer_x: f32,
    pub segments: usize,
    /// Explicit hull-local Z of each road-wheel axle, when the layout is irregular (the T-54's
    /// signature wider first/second gap). `None` spreads `wheel_count` wheels evenly between
    /// `wheel_first_z` and `wheel_last_z`. Read through [`TrackShape::wheel_stations`] — the one
    /// source both the physics contact footprint and the rendered running gear place wheels by.
    ///
    /// The `&'static` keeps [`TrackShape`] `Copy` for every consumer; the RON loader
    /// deserializes an owned list and `Box::leak`s it — bounded to one small slice per vehicle
    /// kind per process, parsed once behind a `OnceLock` (see `source.rs`).
    #[serde(deserialize_with = "leak_wheel_stations")]
    pub wheel_stations: Option<&'static [f32]>,
    /// Return rollers carrying the top run (`0` for layouts whose top run rests on the road
    /// wheels, like the T-54 family). The IS family runs three small rollers per side.
    pub return_rollers: usize,
    /// Radius of one return roller (ignored when `return_rollers` is 0).
    pub roller_radius: f32,
    /// Schachtellaufwerk: how far INBOARD every odd-indexed road wheel sits relative to the even
    /// row. `0` is the ordinary single file; a large offset (≈ a wheel's width) reads as the
    /// Tiger's interleaved double row, a small one as the Tiger II/Panther overlapped stagger.
    /// Presentation only — the contact footprint and belt stay on the shared centreline.
    pub overlap_inner_dx: f32,
    /// Half-width of one road-wheel disc face. Narrow cast discs (IS family ~0.14) vs broad
    /// pressed wheels (T-54 0.18); interleaved rows must be thin enough to clear each other.
    pub wheel_half_width: f32,
    /// Half-width of one track shoe plate — the belt band the links span.
    pub link_half_width: f32,
    /// Authored links per side (the documented shoe count), or `None` to derive from the loop
    /// length at the standard shoe pitch.
    pub link_count: Option<usize>,
    /// Top-run droop between supports: a rollered run stays taut (~0.012), a run resting on
    /// the road wheels sags visibly (~0.05).
    pub top_sag_m: f32,
    /// Spoke count of the road-wheel face (12 for the IS family's cast wheels, 6 typical).
    pub wheel_spokes: usize,
    /// Which end the toothed drive sprocket lives at. `false` (default) = rear drive — the
    /// Soviet family, the T-34 and the Centurion. `true` = FRONT drive: the whole German line
    /// (Tiger I/II, Jagdtiger, Panther II) drives from the transmission at the bow, teeth on
    /// the forward end wheel — photo-confirmed on all four (model-logic audit #16).
    #[serde(default)]
    pub drive_front: bool,
    /// The track-shoe pattern the link unit mesh stamps — per FAMILY, from the photos (audit
    /// #14: one shoe design used to run on every vehicle across three nations).
    #[serde(default)]
    pub shoe_pattern: ShoePattern,
    /// The road-wheel FACE construction (the other half of audit #14): openwork spokes read
    /// Soviet; the German line runs bolted steel-dish wheels; the Centurion a bolted dish
    /// under a rubber tire.
    #[serde(default)]
    pub wheel_face: WheelFace,
}

/// Track-shoe families. The pitch/width still come from the numeric fields; this picks the
/// PATTERN a shoe reads as up close.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ShoePattern {
    /// Soviet small-pitch OMSh (T-54 family, IS-3): flat plate, twin inner guide pads,
    /// pin bars at the joints.
    #[default]
    Omsh,
    /// German Kgs 63/725 double-pin (Tiger I/II, Jagdtiger, Panther II): wide plate with one
    /// TALL centre guide horn riding between the interleaved wheels, transverse grousers.
    Kgs,
    /// The T-34's stamped "waffle" plate: ribbed outer face under a low broad horn.
    Waffle,
    /// The Centurion's cast shoe: TWIN spaced guide horns and a heavy transverse bar.
    BritishCast,
}

/// Road-wheel face families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum WheelFace {
    /// Openwork spoke/rib casting over a recessed web (T-54 starfish, IS twelve-rib,
    /// T-34 spoked) — arm count from `wheel_spokes`.
    #[default]
    Openwork,
    /// German late-war steel-rimmed wheel: a bolted conical dish, steel tire band.
    SteelDish,
    /// Bolted dish under a rubber tire (Centurion).
    RubberDish,
}

/// Deserialize `Option<Vec<f32>>` into the `Copy`-preserving `Option<&'static [f32]>` by
/// leaking the vector. Bounded: at most one small slice per vehicle kind, once per process
/// (the loader caches the parsed blueprint in a `OnceLock`).
fn leak_wheel_stations<'de, D>(deserializer: D) -> Result<Option<&'static [f32]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let stations: Option<Vec<f32>> = serde::Deserialize::deserialize(deserializer)?;
    Ok(stations.map(|list| &*list.leak()))
}

impl TrackShape {
    /// Hull-local Z of each road-wheel axle, front positive: the explicit stations when authored,
    /// otherwise an even spread between the first and last wheel.
    pub fn wheel_stations(&self) -> Vec<f32> {
        if let Some(stations) = self.wheel_stations {
            return stations.to_vec();
        }
        match self.wheel_count {
            0 => Vec::new(),
            1 => vec![self.wheel_first_z],
            count => {
                let step = (self.wheel_last_z - self.wheel_first_z) / (count - 1) as f32;
                (0..count).map(|i| self.wheel_first_z + step * i as f32).collect()
            }
        }
    }
}
