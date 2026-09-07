use serde::{Deserialize, Serialize};

/// Powerplant. Drives `engine_power_kw`; when destroyed the vehicle loses drive (and may
/// catch fire). Mass adds to the assembled combat weight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineModule {
    pub name: String,
    pub power_kw: f32,
    pub mass_kg: f32,
    pub hit_points: u32,
    /// Probability (0.0..1.0) that a destroying engine hit starts a fire.
    pub fire_chance: f32,
    /// The transmission behind this engine (GDD §4 „krzywa momentu × przełożenie, automatyczna
    /// skrzynia z punktami zmiany biegów”; the one program's J6). Data, never a knob: how many
    /// ratios and where first gear tops out. `serde(default)` keeps older fixtures on a five-speed.
    #[serde(default)]
    pub gearbox: Gearbox,
}

/// A tank's gearbox as DATA. The ratios step geometrically from first gear's top speed to the
/// hull's, which is how a real box is laid out (each gear a fixed multiple of the last); the
/// drive (`physics::engine`) reads the torque curve through the ratio the hull is in, so a hull
/// launches hard in first, "dociąga" as the revs climb, and loses a beat at every shift — the
/// shape §4 asked for, with nothing to tune but the two numbers here.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Gearbox {
    /// Number of forward ratios (1..=8).
    pub gears: u8,
    /// Where first gear tops out, as a fraction of the hull's top speed.
    pub first_gear_top_speed_fraction: f32,
}

impl Gearbox {
    pub const MAX_GEARS: usize = 8;

    /// A five-speed with first gear at 22 % of the top speed — the post-war medium's box.
    pub const FIVE_SPEED: Self = Self { gears: 5, first_gear_top_speed_fraction: 0.22 };

    /// How many ratios the box really has (clamped to the range the drive handles).
    pub fn gear_count(&self) -> usize {
        (self.gears as usize).clamp(1, Self::MAX_GEARS)
    }

    /// The step between consecutive ratios: the geometric factor that carries first gear's top
    /// speed to the hull's over `gears − 1` steps.
    pub fn ratio_step(&self) -> f32 {
        let gears = self.gear_count();
        if gears == 1 {
            return 1.0;
        }
        let first = self.first_gear_top_speed_fraction.clamp(0.05, 0.95);
        (1.0 / first).powf(1.0 / (gears as f32 - 1.0))
    }

    /// Top speed of gear `index` (0-based) as a fraction of the hull's top speed.
    pub fn top_speed_fraction(&self, index: usize) -> f32 {
        let gears = self.gear_count();
        if gears == 1 || index + 1 >= gears {
            return 1.0;
        }
        let first = self.first_gear_top_speed_fraction.clamp(0.05, 0.95);
        (first * self.ratio_step().powi(index as i32)).min(1.0)
    }
}

impl Default for Gearbox {
    fn default() -> Self {
        Self::FIVE_SPEED
    }
}
