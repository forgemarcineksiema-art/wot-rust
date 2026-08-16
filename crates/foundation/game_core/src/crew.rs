//! A lightweight crew model: a fixed roster of five roles, fully trained.
//!
//! Proficiency is PINNED to 1.0 (user decision 2026-08-14, Hala v4 W1). The old 0.5..=1.0
//! dial was a free, dominated choice that punished not knowing about it — +4.5% reload/aim
//! at the 0.85 default while bots played unpenalized — which is the exact opposite of the
//! creed "progression is proof, never power". The type and [`Crew::apply`] stay as the seam
//! a future crew system would use; the band collapsed to the pin, so every old save and
//! every call site migrates by clamp.

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

/// The crew aboard a vehicle — fully trained, always (see the module doc for the decision).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crew {
    proficiency: f32,
}

impl Crew {
    /// The band is the pin: both bounds are 1.0, so `new` migrates every historical value
    /// (old saves carried 0.5..=1.0) up by clamp and no code path can reopen the dial
    /// without moving these constants — which is the deliberate, test-visible act.
    pub const MIN_PROFICIENCY: f32 = 1.0;
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

    /// The crew→spec bridge. At the 1.0 pin the penalty is exactly 1.0 and the rated values
    /// pass through untouched — the formula stays as the seam a future crew system plugs
    /// into, and the pin test below is what makes it provably inert today.
    pub fn apply(self, spec: &mut TankSpec) {
        let penalty = 1.0 + (1.0 - self.proficiency) * 0.30;
        spec.gun.reload_seconds *= penalty;
        spec.gun.aim_time_seconds *= penalty;
    }
}

impl Default for Crew {
    fn default() -> Self {
        Self { proficiency: 1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VehicleKind;

    /// The W1 pin, locked from both ends: every input collapses to 1.0 (old saves carried
    /// 0.5..=1.0 and migrate up by clamp), and the crew bridge leaves the rated gun values
    /// untouched — the screen's numbers ARE the battle's numbers, for humans and bots alike.
    #[test]
    fn the_crew_is_pinned_fully_trained() {
        for historical in [0.0, 0.5, 0.85, 1.0, 5.0] {
            assert_eq!(Crew::new(historical).proficiency(), 1.0, "pin migrates {historical}");
        }
        assert_eq!(Crew::default().proficiency(), 1.0);

        let base = VehicleKind::TigerII.spec();
        let mut crewed = base.clone();
        Crew::default().apply(&mut crewed);
        assert!((crewed.gun.reload_seconds - base.gun.reload_seconds).abs() < 1.0e-6);
        assert!((crewed.gun.aim_time_seconds - base.gun.aim_time_seconds).abs() < 1.0e-6);
    }
}
