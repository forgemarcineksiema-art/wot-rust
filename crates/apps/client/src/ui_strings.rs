//! Every user-facing UI string lives here, grouped by screen, so the copy is auditable in one
//! place and future localization is a table swap instead of a literal hunt. UI copy only: entity
//! identity stays data-side (vehicle short names in `game_core::VehicleKind::short_name`, nation labels in
//! `game_core::Nation::label`, crew role labels in `game_core::CrewRole::label`) — those name
//! things, they don't phrase them. Every string is checked against the baked faces (Latin
//! Extended-A since the interface program's F3), so a Polish word is a word and not a row of tofu.

/// Garage screen copy.
pub(crate) mod garage {
    /// The big red commit-to-battle button.
    pub const BATTLE: &str = "BATTLE";
    /// Top-bar tab naming the default hangar view.
    pub const TAB_GARAGE: &str = "GARAGE";
    /// Top-bar tab toggling the tech-tree view.
    pub const TAB_TECH_TREE: &str = "TECH TREE";
    /// Left crew panel header.
    pub const CREW: &str = "CREW";
    /// Right stats panel header.
    pub const VEHICLE: &str = "VEHICLE";
    /// Tech-tree close button.
    pub const BACK: &str = "BACK";

    /// Stat-row units (value formatting stays at the call site; the unit is copy).
    pub const UNIT_KILOWATTS: &str = "kW";
    pub const UNIT_KMH: &str = "km/h";
    pub const UNIT_DEGREES_PER_S: &str = "d/s";
    pub const UNIT_MILLIMETERS: &str = "mm";
    pub const UNIT_SECONDS: &str = "s";
    /// Milliradians — the unit the whole aiming promise is written in (no +-25% roll, a gun that
    /// groups where it is pointed). It belongs on the screen where the gun is chosen.
    pub const UNIT_MRAD: &str = "mrad";

    /// Every string of this module, for the coverage lock (`every_ui_string_constant_is_listed_in_all` counts it).
    #[cfg(test)]
    pub const ALL: &[&str] = &[
        BATTLE,
        TAB_GARAGE,
        TAB_TECH_TREE,
        CREW,
        VEHICLE,
        BACK,
        UNIT_KILOWATTS,
        UNIT_KMH,
        UNIT_DEGREES_PER_S,
        UNIT_MILLIMETERS,
        UNIT_SECONDS,
        UNIT_MRAD,
    ];
}

/// Battle HUD copy.
pub(crate) mod battle {
    /// Unit tag beside the bottom-left speed readout.
    pub const SPEED_UNIT: &str = "KM/H";
    /// Unit tag beside the target-distance readout at the reticle.
    pub const DISTANCE_UNIT: &str = "M";
    /// The arc's refusal in words (Inny Poziom A3): which end of the gun arc bit.
    pub const ARC_LIMIT_DEPRESSION: &str = "DEPRESSION LIMIT";
    pub const ARC_LIMIT_ELEVATION: &str = "ELEVATION LIMIT";
    /// A fixed casemate stopped on its hull line (Inny Poziom A4).
    pub const ARC_LIMIT_TRAVERSE: &str = "TRAVERSE LIMIT";
    /// The outcome a landed shot prints when it dealt no damage (Inny Poziom A6) — never a "0".
    pub const HIT_RICOCHET: &str = "RICOCHET";
    pub const HIT_SHATTER: &str = "SHATTER";
    pub const HIT_TRACKED: &str = "TRACKED";
    pub const HIT_NO_PEN: &str = "NO PEN";
    pub const HIT_PEN: &str = "PEN";
    /// Prefix of the sniper magnification readout ("X6.9", WT-style).
    pub const ZOOM_PREFIX: &str = "X";
    /// Center banner after the player's team wins a local battle.
    pub const VICTORY: &str = "VICTORY";
    /// Center banner after the player's team is eliminated.
    pub const DEFEAT: &str = "DEFEAT";
    /// Center banner after a mutual wipe or the battle clock running out.
    pub const DRAW: &str = "DRAW";
    /// Center banner when the remote authority becomes unreachable during a live battle.
    pub const CONNECTION_LOST: &str = "CONNECTION LOST";
    /// The host closed an ended match, but its result word did not arrive.
    pub const BATTLE_OVER: &str = "BATTLE OVER";
    /// Kill confirmation line under the reticle.
    pub const TARGET_DESTROYED: &str = "TARGET DESTROYED";
    /// Way-out hint under the battle-outcome banner (G opens the garage; Battle deploys fresh).
    pub const RETURN_TO_GARAGE_HINT: &str = "G - RETURN TO GARAGE";
    /// Header of the ESC modal. Phrased as the question being asked, so neither button has to
    /// repeat the stakes.
    pub const PAUSE_TITLE: &str = "LEAVE BATTLE?";
    /// The ESC modal's destructive choice.
    pub const PAUSE_EXIT_TO_GARAGE: &str = "EXIT TO GARAGE";
    /// The ESC modal's dismiss choice; names what happens, not the key that does it.
    pub const PAUSE_STAY: &str = "STAY IN BATTLE";
    /// The damage panel's fire lamp (H17).
    pub const FIRE_LAMP: &str = "FIRE";
    /// The damage panel's ammunition-rack fuze, followed by the seconds (H17).
    pub const RACK_FUZE: &str = "RACK";
    /// The damage panel's word for a dead radio (H4).
    pub const RADIO_OUT: &str = "RADIO OUT";

    /// Every string of this module, for the coverage lock (`every_ui_string_constant_is_listed_in_all` counts it).
    #[cfg(test)]
    pub const ALL: &[&str] = &[
        SPEED_UNIT,
        DISTANCE_UNIT,
        ARC_LIMIT_DEPRESSION,
        ARC_LIMIT_ELEVATION,
        ARC_LIMIT_TRAVERSE,
        HIT_RICOCHET,
        HIT_SHATTER,
        HIT_TRACKED,
        HIT_NO_PEN,
        HIT_PEN,
        ZOOM_PREFIX,
        VICTORY,
        DEFEAT,
        DRAW,
        CONNECTION_LOST,
        BATTLE_OVER,
        TARGET_DESTROYED,
        RETURN_TO_GARAGE_HINT,
        PAUSE_TITLE,
        PAUSE_EXIT_TO_GARAGE,
        PAUSE_STAY,
        FIRE_LAMP,
        RACK_FUZE,
        RADIO_OUT,
    ];
}

/// OS window title; the selected vehicle's display name is appended after a dash.
pub(crate) const WINDOW_TITLE: &str = "WOT Rust Prototype";

#[cfg(test)]
mod tests {
    use super::{battle, garage};
    use crate::hud::font::{POLISH_LETTERS, Style, atlas};

    /// Every string the client can print bakes in the label face, the value face and the
    /// banner face — a glyph the atlas lacks renders as a tofu box, and a box in a banner is a
    /// bug the eye finds first. The Polish alphabet rides along: the first localisation this
    /// atlas owes.
    #[test]
    fn every_ui_string_is_covered_by_both_faces() {
        let font = atlas();
        let faces = [Style::LABEL, Style::VALUE, Style::BANNER];
        let mut examined = 0usize;
        for s in garage::ALL.iter().chain(battle::ALL.iter()) {
            assert!(!s.is_empty(), "UI strings must not be empty");
            for ch in s.chars() {
                for style in faces {
                    assert!(font.covers(style, ch), "{style:?} cannot set {ch:?} of {s:?}");
                }
                examined += 1;
            }
        }
        for ch in POLISH_LETTERS.chars() {
            for style in faces {
                assert!(font.covers(style, ch), "{style:?} cannot set the Polish letter {ch:?}");
            }
            examined += 1;
        }
        assert!(examined > 200, "the lock examined {examined} characters — too few");
        assert!(garage::ALL.len() >= 12 && battle::ALL.len() >= 20, "the ALL lists shrank");
    }
}
