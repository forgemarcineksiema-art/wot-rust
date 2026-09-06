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
    pub const UNIT_KILOWATTS_PER_TONNE: &str = "kW/t";
    pub const UNIT_HIT_POINTS: &str = "hp";
    /// The VEHICLE column's labels (G1): every row says what it is.
    pub const STAT_HIT_POINTS: &str = "HIT POINTS";
    pub const STAT_ARMOUR: &str = "ARMOUR HULL / TURRET";
    pub const STAT_EFFECTIVE_FRONT: &str = "EFFECTIVE FRONT @ 0\u{b0}";
    pub const STAT_POWER: &str = "ENGINE";
    pub const STAT_POWER_PER_TONNE: &str = "POWER / TONNE";
    pub const STAT_TOP_SPEED: &str = "TOP SPEED";
    pub const STAT_TRAVERSE: &str = "TURRET TRAVERSE";
    pub const STAT_PENETRATION: &str = "PENETRATION @ 100 M";
    pub const STAT_DAMAGE: &str = "DAMAGE PER SHOT";
    pub const STAT_DISPERSION: &str = "DISPERSION";
    pub const STAT_DISPERSION_MOVING: &str = "DISPERSION MOVING";
    pub const STAT_AIM_TIME: &str = "AIM TIME";
    pub const STAT_RELOAD: &str = "RELOAD";
    /// G14: the hull is in a battle that still runs.
    pub const IN_BATTLE: &str = "IN BATTLE";
    /// The inspector's legend.
    pub const LEGEND_TITLE: &str = "ARMOUR";
    pub const LEGEND_UNIT: &str = "MM";
    /// The nameplate's repair tag (L2).
    pub const NAME_DAMAGED: &str = "DAMAGED";
    pub const NAME_REPAIRING: &str = "REPAIRING...";
    /// The key legend on the top bar (G6): the words beside the table's keys — the keys
    /// themselves come from the binding table, never from here.
    pub const HINT_SELECT: &str = "SELECT";
    pub const HINT_FOCUS: &str = "FOCUS";
    pub const HINT_CYCLE: &str = "CYCLE";
    pub const HINT_AMMO: &str = "AMMO";
    pub const HINT_MAP: &str = "MAP";
    pub const HINT_ARMOUR: &str = "ARMOUR";
    pub const HINT_LIGHT: &str = "LIGHT";
    pub const HINT_REPAIR: &str = "REPAIR";
    pub const HINT_TREE: &str = "TREE";
    /// The tooltips (G6).
    pub const TIP_LOCKED: &str = "LOCKED UNTIL THE BATTLE ENDS";
    pub const TIP_DEPLOY: &str = "DEPLOY";
    pub const TIP_MAP: &str = "NEXT MAP";
    pub const TIP_SHIFT_BACK: &str = "SHIFT-CLICK BACK";
    pub const TIP_CHOOSE: &str = "CLICK TO CHOOSE";
    pub const TIP_LOAD: &str = "LOAD THIS ROUND";
    pub const TIP_COUNT: &str = "ONE ROUND \u{b7} SHIFT FIVE";
    pub const TIP_COMPARE: &str = "SHIFT-CLICK TO COMPARE";
    pub const TIP_SCROLL: &str = "SCROLL THE ROSTER";
    pub const TIP_INSTALL: &str = "INSTALL";
    pub const TIP_FILTER: &str = "FILTER BY";
    /// The carousel's chips (G9) and the compared cell's tag (G3).
    pub const CHIP_CLASS: &str = "CLASS";
    pub const CHIP_NATION: &str = "NATION";
    pub const CHIP_TIER: &str = "TIER";
    pub const CHIP_ALL: &str = "ALL";
    pub const COMPARE_TAG: &str = "VS";

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
        UNIT_KILOWATTS_PER_TONNE,
        UNIT_HIT_POINTS,
        STAT_HIT_POINTS,
        STAT_ARMOUR,
        STAT_EFFECTIVE_FRONT,
        STAT_POWER,
        STAT_POWER_PER_TONNE,
        STAT_TOP_SPEED,
        STAT_TRAVERSE,
        STAT_PENETRATION,
        STAT_DAMAGE,
        STAT_DISPERSION,
        STAT_DISPERSION_MOVING,
        STAT_AIM_TIME,
        STAT_RELOAD,
        IN_BATTLE,
        LEGEND_TITLE,
        LEGEND_UNIT,
        NAME_DAMAGED,
        NAME_REPAIRING,
        HINT_SELECT,
        HINT_FOCUS,
        HINT_CYCLE,
        HINT_AMMO,
        HINT_MAP,
        HINT_ARMOUR,
        HINT_LIGHT,
        HINT_REPAIR,
        HINT_TREE,
        TIP_LOCKED,
        TIP_DEPLOY,
        TIP_MAP,
        TIP_SHIFT_BACK,
        TIP_CHOOSE,
        TIP_LOAD,
        TIP_COUNT,
        TIP_COMPARE,
        TIP_SCROLL,
        TIP_INSTALL,
        TIP_FILTER,
        CHIP_CLASS,
        CHIP_NATION,
        CHIP_TIER,
        CHIP_ALL,
        COMPARE_TAG,
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
    /// The escape menu's way into the HUD editor (H21).
    pub const PAUSE_HUD_EDITOR: &str = "HUD EDITOR";
    /// The editor's footer: what the hands do.
    pub const EDITOR_FOOTER: &str =
        "DRAG TO MOVE \u{b7} 1 2 3 PRESET \u{b7} CTRL+R RESET \u{b7} ESC DONE";
    /// The instruments' names on the editor's frames (H21), `Instrument::ALL` order.
    pub const INSTRUMENT_TOP_BAR: &str = "TOP BAR";
    pub const INSTRUMENT_TEAM_LISTS: &str = "TEAM LISTS";
    pub const INSTRUMENT_TOP_STACK: &str = "LAMP \u{b7} BUDGET \u{b7} WORD";
    pub const INSTRUMENT_MINIMAP: &str = "MINIMAP";
    pub const INSTRUMENT_DAMAGE_PANEL: &str = "DAMAGE PANEL";
    pub const INSTRUMENT_SPEED: &str = "SPEED";
    pub const INSTRUMENT_AMMO: &str = "AMMUNITION";
    pub const INSTRUMENT_HIT_LOG: &str = "HIT LOG";
    pub const INSTRUMENT_KILL_FEED: &str = "KILL FEED";
    pub const INSTRUMENT_NET: &str = "CONNECTION";
    /// Header of the ESC modal. Phrased as the question being asked, so neither button has to
    /// repeat the stakes.
    pub const MENU_TITLE: &str = "MENU";
    /// The cold garage's way out (P8).
    pub const MENU_QUIT: &str = "QUIT";
    pub const FOOTER_CHOOSE: &str = "CHOOSE";
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
    /// The escape menu's SETTINGS (P6).
    pub const PAUSE_SETTINGS: &str = "SETTINGS";
    /// The settings page (P6): its title, its rows, its words.
    pub const SETTINGS_TITLE: &str = "SETTINGS";
    pub const SET_PALETTE: &str = "PALETTE";
    pub const SET_VOLUME: &str = "MASTER VOLUME";
    pub const SET_SENS_THIRD: &str = "SENSITIVITY \u{b7} THIRD PERSON";
    pub const SET_SENS_SCOPE: &str = "SENSITIVITY \u{b7} SCOPE";
    pub const SET_UI_SCALE: &str = "INTERFACE SCALE";
    pub const SET_SNIPER_KEY: &str = "SNIPER KEY";
    pub const SET_FULLSCREEN: &str = "FULLSCREEN";
    pub const SET_DAYLIGHT: &str = "HALL DAYLIGHT";
    pub const SET_HUD_PRESET: &str = "HUD PRESET";
    pub const WORD_HOLD: &str = "HOLD";
    pub const WORD_TOGGLE: &str = "TOGGLE";
    pub const WORD_BORDERLESS: &str = "BORDERLESS";
    pub const WORD_WINDOWED: &str = "WINDOWED";
    pub const WORD_AUTO: &str = "AUTO";
    pub const WORD_MORNING: &str = "MORNING";
    pub const WORD_DAY: &str = "DAY";
    pub const WORD_EVENING: &str = "EVENING";
    pub const WORD_MINIMAL: &str = "MINIMAL";
    pub const WORD_STANDARD: &str = "STANDARD";
    pub const WORD_FULL: &str = "FULL";
    pub const WORD_DEUTERANOPIA: &str = "DEUTERANOPIA";
    pub const WORD_PROTANOPIA: &str = "PROTANOPIA";
    pub const WORD_TRITANOPIA: &str = "TRITANOPIA";
    /// The escape menu's KEY BINDINGS and the page (P8).
    pub const PAUSE_KEYBINDS: &str = "KEY BINDINGS";
    pub const KEYBINDS_TITLE: &str = "KEY BINDINGS";
    pub const SET_CONTEXT: &str = "CONTEXT";
    pub const CONTEXT_GLOBAL: &str = "WINDOW";
    pub const CONTEXT_BATTLE: &str = "BATTLE";
    pub const CONTEXT_GARAGE: &str = "GARAGE";
    pub const CONTEXT_HUD_EDITOR: &str = "HUD EDITOR";
    pub const CONTEXT_SHELL: &str = "MENUS";
    /// A row listening for its next key, and the note beside a row whose key another shares.
    pub const LISTENING: &str = "PRESS A KEY";
    pub const SHARED_WITH: &str = "SHARED WITH";
    pub const FOOTER_REBIND: &str = "REBIND";
    pub const FOOTER_RESET: &str = "RESET";
    pub const FOOTER_CONTEXT: &str = "CONTEXT";
    /// The results page (P1, P2, P10): its title, its tabs, its numbers, its timeline words.
    pub const RESULTS_TITLE: &str = "RESULTS";
    pub const TAB_WORD: &str = "TAB";
    pub const TAB_SUMMARY: &str = "SUMMARY";
    pub const TAB_TIMELINE: &str = "TIMELINE";
    pub const TAB_TEAM: &str = "TEAM";
    pub const STAT_SHOTS: &str = "SHOTS";
    pub const STAT_HITS: &str = "HITS";
    pub const STAT_PENETRATIONS: &str = "PENETRATIONS";
    pub const STAT_DAMAGE_DEALT: &str = "DAMAGE DEALT";
    pub const STAT_DAMAGE_TAKEN: &str = "DAMAGE TAKEN";
    pub const STAT_KILLS: &str = "KILLS";
    pub const STAT_SPOTTED: &str = "TIMES SPOTTED";
    pub const STAT_SEEN_BY: &str = "SEEN BY";
    pub const STAT_DURATION: &str = "DURATION";
    pub const REPLAY: &str = "REPLAY";
    /// P10: why the button is disabled — no viewer exists until L3.
    pub const REPLAY_REASON: &str = "NO VIEWER YET";
    pub const RECORDED_TO: &str = "RECORDED TO";
    pub const FOOTER_CONTINUE: &str = "CONTINUE";
    pub const FOOTER_SCROLL: &str = "SCROLL";
    pub const TL_SHOT: &str = "SHOT";
    pub const TL_NO_HIT: &str = "NO HIT SEEN";
    pub const TL_TAKEN: &str = "TAKEN";
    pub const TL_DESTROYED: &str = "DESTROYED";
    pub const TL_HULL_LOST: &str = "HULL LOST";
    pub const TL_SEEN_BY: &str = "SEEN BY";
    pub const TL_UNSEEN: &str = "UNSEEN";
    pub const TL_BY: &str = "BY";
    pub const WORD_ALIVE: &str = "ALIVE";
    pub const WORD_BOT: &str = "BOT";
    pub const WORD_HUMAN: &str = "HUMAN";
    pub const WORD_YOU: &str = "YOU";
    /// The battle history (P4, P5): the garage menu's entry, the page, its words.
    pub const MENU_BATTLES: &str = "BATTLES";
    pub const BATTLES_TITLE: &str = "BATTLES";
    pub const NO_BATTLES: &str = "NO BATTLES YET";
    pub const DMG_UNIT: &str = "DMG";
    pub const FOOTER_OPEN: &str = "OPEN";
    /// The page's arrows and the footer's words.
    pub const ARROW_DEC: &str = "<";
    pub const ARROW_INC: &str = ">";
    pub const FOOTER_SELECT: &str = "SELECT";
    pub const FOOTER_CHANGE: &str = "CHANGE";
    pub const FOOTER_BACK: &str = "BACK";
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
        PAUSE_HUD_EDITOR,
        EDITOR_FOOTER,
        INSTRUMENT_TOP_BAR,
        INSTRUMENT_TEAM_LISTS,
        INSTRUMENT_TOP_STACK,
        INSTRUMENT_MINIMAP,
        INSTRUMENT_DAMAGE_PANEL,
        INSTRUMENT_SPEED,
        INSTRUMENT_AMMO,
        INSTRUMENT_HIT_LOG,
        INSTRUMENT_KILL_FEED,
        INSTRUMENT_NET,
        MENU_TITLE,
        MENU_QUIT,
        FOOTER_CHOOSE,
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
        PAUSE_SETTINGS,
        SETTINGS_TITLE,
        SET_PALETTE,
        SET_VOLUME,
        SET_SENS_THIRD,
        SET_SENS_SCOPE,
        SET_UI_SCALE,
        SET_SNIPER_KEY,
        SET_FULLSCREEN,
        SET_DAYLIGHT,
        SET_HUD_PRESET,
        WORD_HOLD,
        WORD_TOGGLE,
        WORD_BORDERLESS,
        WORD_WINDOWED,
        WORD_AUTO,
        WORD_MORNING,
        WORD_DAY,
        WORD_EVENING,
        WORD_MINIMAL,
        WORD_STANDARD,
        WORD_FULL,
        WORD_DEUTERANOPIA,
        WORD_PROTANOPIA,
        WORD_TRITANOPIA,
        ARROW_DEC,
        ARROW_INC,
        FOOTER_SELECT,
        FOOTER_CHANGE,
        FOOTER_BACK,
        MENU_BATTLES,
        BATTLES_TITLE,
        NO_BATTLES,
        DMG_UNIT,
        FOOTER_OPEN,
        RESULTS_TITLE,
        TAB_WORD,
        TAB_SUMMARY,
        TAB_TIMELINE,
        TAB_TEAM,
        STAT_SHOTS,
        STAT_HITS,
        STAT_PENETRATIONS,
        STAT_DAMAGE_DEALT,
        STAT_DAMAGE_TAKEN,
        STAT_KILLS,
        STAT_SPOTTED,
        STAT_SEEN_BY,
        STAT_DURATION,
        REPLAY,
        REPLAY_REASON,
        RECORDED_TO,
        FOOTER_CONTINUE,
        FOOTER_SCROLL,
        TL_SHOT,
        TL_NO_HIT,
        TL_TAKEN,
        TL_DESTROYED,
        TL_HULL_LOST,
        TL_SEEN_BY,
        TL_UNSEEN,
        TL_BY,
        WORD_ALIVE,
        WORD_BOT,
        WORD_HUMAN,
        WORD_YOU,
        PAUSE_KEYBINDS,
        KEYBINDS_TITLE,
        SET_CONTEXT,
        CONTEXT_GLOBAL,
        CONTEXT_BATTLE,
        CONTEXT_GARAGE,
        CONTEXT_HUD_EDITOR,
        CONTEXT_SHELL,
        LISTENING,
        SHARED_WITH,
        FOOTER_REBIND,
        FOOTER_RESET,
        FOOTER_CONTEXT,
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
