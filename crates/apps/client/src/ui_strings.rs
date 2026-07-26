//! Every user-facing UI string lives here, grouped by screen, so the copy is auditable in one
//! place and future localization is a table swap instead of a literal hunt. UI copy only: entity
//! identity stays data-side (vehicle short names in `garage::layout::short_name`, nation labels in
//! `game_core::Nation::label`, crew role labels in `game_core::CrewRole::label`) — those name
//! things, they don't phrase them. The font atlas bakes ASCII, so every string here must be ASCII.

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
    /// Label over the shared crew proficiency control.
    pub const PROFICIENCY: &str = "Proficiency";
    /// Right stats panel header.
    pub const VEHICLE: &str = "VEHICLE";
    /// Tech-tree close button.
    pub const BACK: &str = "BACK";
    /// Placeholder in an era band with nothing fielded yet (Era I) — the reserved bracket is
    /// shown, not hidden, because the era axis IS the tree's message.
    pub const ERA_RESERVED: &str = "RESERVED - NO VEHICLES FIELDED YET";

    /// Stat-row units (value formatting stays at the call site; the unit is copy).
    pub const UNIT_KILOWATTS: &str = "kW";
    pub const UNIT_KMH: &str = "km/h";
    pub const UNIT_DEGREES_PER_S: &str = "d/s";
    pub const UNIT_MILLIMETERS: &str = "mm";
    pub const UNIT_SECONDS: &str = "s";
    /// Milliradians — the unit the whole aiming promise is written in (no +-25% roll, a gun that
    /// groups where it is pointed). It belongs on the screen where the gun is chosen.
    pub const UNIT_MRAD: &str = "mrad";
}

/// Battle HUD copy.
pub(crate) mod battle {
    /// Unit tag beside the bottom-left speed readout.
    pub const SPEED_UNIT: &str = "KM/H";
    /// Unit tag beside the target-distance readout at the reticle.
    pub const DISTANCE_UNIT: &str = "M";
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
}

/// OS window title; the selected vehicle's display name is appended after a dash.
pub(crate) const WINDOW_TITLE: &str = "WOT Rust Prototype";

#[cfg(test)]
mod tests {
    /// The glyph atlas bakes printable ASCII only — a non-ASCII string would render as gaps.
    #[test]
    fn no_string_is_empty_and_all_are_ascii() {
        let all = [
            super::garage::BATTLE,
            super::garage::TAB_GARAGE,
            super::garage::TAB_TECH_TREE,
            super::garage::CREW,
            super::garage::PROFICIENCY,
            super::garage::VEHICLE,
            super::garage::BACK,
            super::garage::ERA_RESERVED,
            super::garage::UNIT_KILOWATTS,
            super::garage::UNIT_KMH,
            super::garage::UNIT_DEGREES_PER_S,
            super::garage::UNIT_MILLIMETERS,
            super::garage::UNIT_SECONDS,
            super::garage::UNIT_MRAD,
            super::battle::SPEED_UNIT,
            super::battle::DISTANCE_UNIT,
            super::battle::ZOOM_PREFIX,
            super::battle::VICTORY,
            super::battle::DEFEAT,
            super::battle::DRAW,
            super::battle::CONNECTION_LOST,
            super::battle::BATTLE_OVER,
            super::battle::TARGET_DESTROYED,
            super::battle::RETURN_TO_GARAGE_HINT,
            super::battle::PAUSE_TITLE,
            super::battle::PAUSE_EXIT_TO_GARAGE,
            super::battle::PAUSE_STAY,
            super::WINDOW_TITLE,
        ];
        for s in all {
            assert!(!s.is_empty(), "UI strings must not be empty");
            assert!(s.is_ascii(), "the font atlas covers ASCII only: {s:?}");
        }
    }
}
