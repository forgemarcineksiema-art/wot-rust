//! The battle HUD's element names (interface program F5): what the draw list is keyed by, so a
//! test asks `find(HudElement::AmmoPanel)` instead of counting vertices of one colour, and the
//! hit test answers with a name instead of a rectangle.
//!
//! Every legacy builder becomes one element carrying its vertices verbatim; the H wave replaces
//! payloads element by element, and the ratchet in `quality` counts what is left. The reticle
//! stack stays a legacy payload for as long as the program says (H25).

/// Append-only: the names are what the goldens' census and the H rows refer to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HudElement {
    /// The sniper scope's surround (vignette, sight window, stadia).
    ScopeSurround,
    /// The reticle stack: marker, ring, gun marker, verdict, readouts (A1–A12).
    Reticle,
    /// The gun-ready ring around the reticle.
    ReadyRing,
    /// The refused-fire flash.
    DeniedFlash,
    /// The reload countdown beside the reticle.
    ReloadNumber,
    /// Health, reload bars, zoom, FPS, speed.
    Readouts,
    /// Incoming-hit direction arcs.
    HitDirection,
    Minimap,
    /// The battle outcome banner.
    Outcome,
    /// The kill confirmation beat.
    KillConfirm,
    /// The escape modal.
    PauseMenu,
    /// The spotted-enemy corner brackets (A9).
    SpotBrackets,
    /// Floating damage numbers and outcome words at the hit point.
    HitIndicator,
    /// The top bar's plate (H1), and its parts: the clock under glass, the two frag counters
    /// and the two team pools.
    TopBar,
    TopBarClock,
    TopBarClockGlass,
    TopBarAllyFrags,
    TopBarEnemyFrags,
    TopBarAllyPool,
    TopBarEnemyPool,
    /// The minimap's plate, its baked relief and the glass over it (H0); `Minimap` is the
    /// vector overlay between them.
    MinimapPlate,
    MinimapRelief,
    MinimapGlass,
    /// One part of one team-list row (H2): `enemy` picks the ear, `index` the seat.
    TeamRow {
        enemy: bool,
        index: u8,
        part: TeamRowPart,
    },
    /// One part of the damage panel (H4, H17).
    DamagePanel(DamagePart),
    /// One part of the speed instrument (H5).
    Speed(SpeedPart),
    /// One part of the ammunition panel (H6).
    Ammo(AmmoPart),
    /// One part of one hit-log row (H8), newest first.
    HitLog(HitLogPart),
    /// One part of one world-anchored marker (H10), by marker index.
    Marker(MarkerPart),
    /// The sixth-sense lamp and its word (H13).
    SixthSenseLamp,
    SixthSenseText,
    /// The visibility budget line (H14).
    BudgetLine,
    /// One hull's blip on the minimap (H15): the class glyph, by blip index (allies first).
    MinimapBlip(u8),
    /// The blip's seat letter, on the sizes that fit it.
    MinimapSeat(u8),
    /// One grid letter or number along the map's edge (H15).
    MinimapGridLabel(u8),
    /// The enamel backing under a blip's seat letter (H23): ink over a plate, not over relief.
    MinimapSeatPlate(u8),
    /// The command wheel (H16): a plate and a word per command, and the counter under it.
    CommandWheel(WheelPart),
    /// A teammate's ping in the world (H16), by ping index.
    Ping(PingPart),
    /// The team's newest word, under the budget line (H16).
    TeamWord,
    /// One part of one kill-feed row (H3), newest first.
    KillFeed(KillFeedPart),
    /// The connection readout under the frame counter (H18).
    NetReadout,
    /// The dead crew's strip (H19): whose hull it rides, and the keys.
    Spectate(SpectatePart),
    /// The HUD editor's overlay (H21): a frame and a name per instrument, the footer.
    Editor(EditorPart),
}

/// The parts of the editor's overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorPart {
    Frame(super::layout::Instrument),
    Label(super::layout::Instrument),
    Footer,
}

/// The parts of the spectate strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpectatePart {
    Strip,
    Name,
    Keys,
}

/// The parts of a kill-feed row, by row index: the killer's name, the word, the wreck's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KillFeedPart {
    Killer(u8),
    Word(u8),
    Victim(u8),
}

/// The parts of the command wheel, by command index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WheelPart {
    Sector(u8),
    Label(u8),
    Counter,
}

/// The parts of a ping mark, by ping index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PingPart {
    Disc(u8),
    Seat(u8),
    Range(u8),
}

/// The parts of a marker: the target wears them all, a known hull only the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkerPart {
    Plate(u8),
    Class(u8),
    Name(u8),
    Bar(u8),
    Number(u8),
    Distance(u8),
}

/// The parts of a hit-log row, by row index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HitLogPart {
    Row(u8),
    /// The outcome family's square.
    Verdict(u8),
    Text(u8),
}

/// The parts of the speed instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpeedPart {
    Plate,
    Number,
    Unit,
    /// One cruise notch, by its level (negative in reverse).
    Notch(i8),
}

/// The parts of the ammunition panel, by slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmmoPart {
    Slot(u8),
    Lamp(u8),
    Icon(u8),
    Designation(u8),
    Numbers(u8),
    Count(u8),
    Key(u8),
    SwitchingBand,
    SwitchingText,
}

/// The parts of the damage panel, each its own element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamagePart {
    Plate,
    Hull,
    Turret,
    Track(game_core::TrackSide),
    /// The re-seat clock over a thrown track.
    TrackClock(game_core::TrackSide),
    Module(game_core::ModuleSlot),
    /// The field-patch clock under a destroyed module.
    ModuleClock(game_core::ModuleSlot),
    HpBar,
    HpNumber,
    Crew(game_core::CrewRole),
    /// The first-aid clock under a downed station.
    CrewClock(game_core::CrewRole),
    FireLamp,
    FireText,
    RackFuze,
    RadioOut,
}

/// The parts of a team-list row, each its own element so the hit test and the census can name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TeamRowPart {
    /// The enamel strip the row sits on.
    Strip,
    /// The lamp hairline marking the player's own row.
    Lamp,
    /// The class glyph.
    Class,
    /// The vehicle's short name.
    Name,
    /// The seat letter.
    Seat,
    /// The hit-point bar.
    Health,
}

impl HudElement {
    /// Walked by the tests and by the census probe (F8/F9); the identity rule wants it whole.
    /// The row elements (`TeamRow`) are keyed by seat and part; one representative is listed.
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ALL: [HudElement; 43] = [
        HudElement::ScopeSurround,
        HudElement::Reticle,
        HudElement::ReadyRing,
        HudElement::DeniedFlash,
        HudElement::ReloadNumber,
        HudElement::Readouts,
        HudElement::HitDirection,
        HudElement::Minimap,
        HudElement::Outcome,
        HudElement::KillConfirm,
        HudElement::PauseMenu,
        HudElement::SpotBrackets,
        HudElement::HitIndicator,
        HudElement::TopBar,
        HudElement::TopBarClock,
        HudElement::TopBarClockGlass,
        HudElement::TopBarAllyFrags,
        HudElement::TopBarEnemyFrags,
        HudElement::TopBarAllyPool,
        HudElement::TopBarEnemyPool,
        HudElement::MinimapPlate,
        HudElement::MinimapRelief,
        HudElement::MinimapGlass,
        // The row variant, named once by its first seat: the rows are keyed by seat and part,
        // and the identity rule wants every variant walked at least by name.
        HudElement::TeamRow { enemy: false, index: 0, part: TeamRowPart::Strip },
        HudElement::DamagePanel(DamagePart::Plate),
        HudElement::Speed(SpeedPart::Plate),
        HudElement::Ammo(AmmoPart::SwitchingBand),
        HudElement::HitLog(HitLogPart::Row(0)),
        HudElement::Marker(MarkerPart::Bar(0)),
        HudElement::SixthSenseLamp,
        HudElement::SixthSenseText,
        HudElement::BudgetLine,
        HudElement::MinimapBlip(0),
        HudElement::MinimapSeat(0),
        HudElement::MinimapGridLabel(0),
        HudElement::MinimapSeatPlate(0),
        HudElement::CommandWheel(WheelPart::Counter),
        HudElement::Ping(PingPart::Disc(0)),
        HudElement::TeamWord,
        HudElement::KillFeed(KillFeedPart::Word(0)),
        HudElement::NetReadout,
        HudElement::Spectate(SpectatePart::Strip),
        HudElement::Editor(EditorPart::Footer),
    ];
}

#[cfg(test)]
mod tests {
    use super::HudElement;

    #[test]
    fn every_element_is_named_once() {
        let names: std::collections::HashSet<String> =
            HudElement::ALL.iter().map(|e| format!("{e:?}")).collect();
        assert_eq!(names.len(), HudElement::ALL.len());
    }
}
