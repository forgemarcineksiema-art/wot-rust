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
    /// The hand-off under the battle-outcome banner (H20): Enter continues at once, the banner
    /// hands off by itself after three seconds; G still opens the garage.
    pub const OUTCOME_CONTINUE_HINT: &str = "ENTER \u{b7} CONTINUE";
    /// The dead crew's strip (H19): who it rides with, and the keys that step through.
    pub const SPECTATING: &str = "SPECTATING";
    pub const SPECTATE_KEYS: &str = "< >";
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
    /// The ammunition panel's band while the loader swaps the round (H6).
    pub const AMMO_SWITCHING: &str = "SWITCHING";
    /// Millimetres, after a penetration figure (H6).
    pub const MILLIMETRES: &str = "MM";
    /// The sixth-sense lamp (H13).
    pub const SPOTTED_LAMP: &str = "SPOTTED";
    /// The visibility budget line (H14): „SEEN FROM 308 M · MOVING".
    pub const SEEN_FROM: &str = "SEEN FROM";
    pub const BUDGET_STILL: &str = "STILL";
    pub const BUDGET_MOVING: &str = "MOVING";
    pub const BUDGET_FIRED: &str = "FIRED";
    pub const SECONDS_UNIT: &str = "S";
    /// The hit log's words for the causes without a shell (H8).
    pub const HIT_RAM: &str = "RAM";
    pub const HIT_SPLASH: &str = "SPLASH";
    pub const HIT_IMPACT: &str = "IMPACT";
    pub const HIT_DROWNED: &str = "DROWNED";
    /// The hit log's armour zones (H8), `game_core::ArmorZone` order.
    pub const ZONE_UPPER_GLACIS: &str = "UPPER GLACIS";
    pub const ZONE_LOWER_PLATE: &str = "LOWER PLATE";
    pub const ZONE_HULL_SIDE: &str = "HULL SIDE";
    pub const ZONE_HULL_REAR: &str = "HULL REAR";
    pub const ZONE_TURRET_FRONT: &str = "TURRET FRONT";
    pub const ZONE_MANTLET: &str = "MANTLET";
    pub const ZONE_TURRET_SIDE: &str = "TURRET SIDE";
    pub const ZONE_TURRET_REAR: &str = "TURRET REAR";
    pub const ZONE_ROOF: &str = "ROOF";
    pub const ZONE_LEFT_TRACK: &str = "LEFT TRACK";
    pub const ZONE_RIGHT_TRACK: &str = "RIGHT TRACK";
    pub const ZONE_SKIRT: &str = "SKIRT";
    pub const ZONE_HULL_DECK: &str = "HULL DECK";
    pub const ZONE_CUPOLA: &str = "CUPOLA";
    pub const ZONE_GLACIS_PORT: &str = "GLACIS PORT";
    /// The hit log's module names (H8), `game_core::ModuleSlot` order.
    pub const MODULE_ENGINE: &str = "ENGINE";
    pub const MODULE_SUSPENSION: &str = "SUSPENSION";
    pub const MODULE_TURRET: &str = "TURRET RING";
    pub const MODULE_GUN: &str = "GUN";
    pub const MODULE_AMMO_RACK: &str = "AMMO RACK";
    pub const MODULE_RADIO: &str = "RADIO";
    /// The command wheel's words (H16), `net::TeamCommand` order.
    pub const CMD_ATTACK: &str = "ATTACK";
    pub const CMD_HELP: &str = "HELP";
    pub const CMD_RELOADING: &str = "RELOADING";
    pub const CMD_AFFIRMATIVE: &str = "AFFIRMATIVE";
    pub const CMD_NEGATIVE: &str = "NEGATIVE";
    pub const CMD_BACK_TO_BASE: &str = "BACK TO BASE";
    pub const CMD_FOLLOW_ME: &str = "FOLLOW ME";
    pub const CMD_PING: &str = "PING";
    /// The wheel's counter and its refusal (H16): „COMMANDS 3/5", „REFUSED · WAIT 12 S".
    pub const WHEEL_COMMANDS: &str = "COMMANDS";
    pub const WHEEL_REFUSED: &str = "REFUSED";
    pub const WHEEL_WAIT: &str = "WAIT";
    /// The kill feed's word between the killer and the wreck (H3).
    pub const KILL_WORD: &str = "DESTROYED";
    /// The connection readout (H18): „RTT 48 MS · SNAP 32 MS", or „LOCAL" for the local host.
    pub const NET_RTT: &str = "RTT";
    pub const NET_SNAPSHOT: &str = "SNAP";
    pub const NET_LOCAL: &str = "LOCAL";
    pub const MILLISECONDS: &str = "MS";

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
        OUTCOME_CONTINUE_HINT,
        SPECTATING,
        SPECTATE_KEYS,
        PAUSE_TITLE,
        PAUSE_EXIT_TO_GARAGE,
        PAUSE_STAY,
        FIRE_LAMP,
        RACK_FUZE,
        RADIO_OUT,
        AMMO_SWITCHING,
        MILLIMETRES,
        SPOTTED_LAMP,
        SEEN_FROM,
        BUDGET_STILL,
        BUDGET_MOVING,
        BUDGET_FIRED,
        SECONDS_UNIT,
        HIT_RAM,
        HIT_SPLASH,
        HIT_IMPACT,
        HIT_DROWNED,
        ZONE_UPPER_GLACIS,
        ZONE_LOWER_PLATE,
        ZONE_HULL_SIDE,
        ZONE_HULL_REAR,
        ZONE_TURRET_FRONT,
        ZONE_MANTLET,
        ZONE_TURRET_SIDE,
        ZONE_TURRET_REAR,
        ZONE_ROOF,
        ZONE_LEFT_TRACK,
        ZONE_RIGHT_TRACK,
        ZONE_SKIRT,
        ZONE_HULL_DECK,
        ZONE_CUPOLA,
        ZONE_GLACIS_PORT,
        MODULE_ENGINE,
        MODULE_SUSPENSION,
        MODULE_TURRET,
        MODULE_GUN,
        MODULE_AMMO_RACK,
        MODULE_RADIO,
        CMD_ATTACK,
        CMD_HELP,
        CMD_RELOADING,
        CMD_AFFIRMATIVE,
        CMD_NEGATIVE,
        CMD_BACK_TO_BASE,
        CMD_FOLLOW_ME,
        CMD_PING,
        WHEEL_COMMANDS,
        WHEEL_REFUSED,
        WHEEL_WAIT,
        KILL_WORD,
        NET_RTT,
        NET_SNAPSHOT,
        NET_LOCAL,
        MILLISECONDS,
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
