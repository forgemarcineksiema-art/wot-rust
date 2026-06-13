//! A lightweight crew model: a fixed roster of five roles with a single shared proficiency. There
//! is no economy/XP yet — proficiency is just a dial that nudges the assembled stats so a better
//! trained crew reloads and aims a touch faster. [`Crew::apply`] is the single bridge from crew to
//! [`crate::TankSpec`], applied after the modules assemble.

use serde::{Deserialize, Serialize};

use crate::TankSpec;

/// The classic five tank crew roles. Presentational roster; every role shares one proficiency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrewRole {
    Commander,
    Gunner,
    Driver,
    Loader,
    RadioOperator,
}

impl CrewRole {
    pub const ALL: [CrewRole; 5] = [
        CrewRole::Commander,
        CrewRole::Gunner,
        CrewRole::Driver,
        CrewRole::Loader,
        CrewRole::RadioOperator,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CrewRole::Commander => "Commander",
            CrewRole::Gunner => "Gunner",
            CrewRole::Driver => "Driver",
            CrewRole::Loader => "Loader",
            CrewRole::RadioOperator => "Radio Op",
        }
    }
}

/// The crew aboard a vehicle. `proficiency` is a single 0.5..=1.0 dial shared by all roles.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crew {
    proficiency: f32,
}

impl Crew {
    pub const MIN_PROFICIENCY: f32 = 0.5;
    pub const MAX_PROFICIENCY: f32 = 1.0;

    pub fn new(proficiency: f32) -> Self {
        Self { proficiency: proficiency.clamp(Self::MIN_PROFICIENCY, Self::MAX_PROFICIENCY) }
    }

    pub fn proficiency(self) -> f32 {
        self.proficiency
    }

    pub fn roles() -> [CrewRole; 5] {
        CrewRole::ALL
    }

    /// Scale the gun handling by crew quality: a fully trained crew (1.0) keeps the gun's rated
    /// reload and aim time; a green crew (0.5) is up to ~15% slower. Monotonic — more proficiency
    /// never makes the tank worse.
    pub fn apply(self, spec: &mut TankSpec) {
        let penalty = 1.0 + (1.0 - self.proficiency) * 0.30;
        spec.gun.reload_seconds *= penalty;
        spec.gun.aim_time_seconds *= penalty;
    }
}

impl Default for Crew {
    fn default() -> Self {
        Self { proficiency: 0.85 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VehicleKind;

    #[test]
    fn proficiency_is_clamped_to_the_trained_band() {
        assert_eq!(Crew::new(5.0).proficiency(), Crew::MAX_PROFICIENCY);
        assert_eq!(Crew::new(0.0).proficiency(), Crew::MIN_PROFICIENCY);
    }

    #[test]
    fn better_crew_never_reloads_slower() {
        let base = VehicleKind::TigerII.spec();
        let mut green = base.clone();
        let mut elite = base.clone();
        Crew::new(0.5).apply(&mut green);
        Crew::new(1.0).apply(&mut elite);
        assert!(elite.gun.reload_seconds <= green.gun.reload_seconds);
        assert!(elite.gun.aim_time_seconds <= green.gun.aim_time_seconds);
        // A fully trained crew leaves the rated values untouched.
        assert!((elite.gun.reload_seconds - base.gun.reload_seconds).abs() < 1.0e-6);
    }
}
