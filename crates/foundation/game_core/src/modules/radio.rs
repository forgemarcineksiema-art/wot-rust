use serde::{Deserialize, Serialize};

/// Radio: the lightest module and the most fragile. It carries NO signal range — nothing in
/// the battle reads one (team vision is shared; spotting is per-era observer range), and the
/// garage printing "700 m" off this struct was the audit's textbook dead number: a stat shown
/// where the player picks modules that the fight then ignores. If a radio mechanic ever
/// lands, its number returns WITH the mechanic, not ahead of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadioModule {
    pub name: String,
    pub mass_kg: f32,
    pub hit_points: u32,
}
