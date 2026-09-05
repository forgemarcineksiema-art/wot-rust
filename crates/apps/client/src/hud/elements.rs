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
    /// The dealt/taken damage log.
    DamageLog,
    /// The thrown-track callout and re-seat bar.
    TrackCallout,
    /// The ammo-rack fuze callout.
    RackCallout,
    /// Incoming-hit direction arcs.
    HitDirection,
    AmmoPanel,
    ModulePanel,
    CrewPanel,
    Minimap,
    /// The battle outcome banner.
    Outcome,
    /// The kill confirmation beat.
    KillConfirm,
    /// The escape modal.
    PauseMenu,
    /// World-anchored enemy health bars.
    EnemyBars,
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
    pub const ALL: [HudElement; 31] = [
        HudElement::ScopeSurround,
        HudElement::Reticle,
        HudElement::ReadyRing,
        HudElement::DeniedFlash,
        HudElement::ReloadNumber,
        HudElement::Readouts,
        HudElement::DamageLog,
        HudElement::TrackCallout,
        HudElement::RackCallout,
        HudElement::HitDirection,
        HudElement::AmmoPanel,
        HudElement::ModulePanel,
        HudElement::CrewPanel,
        HudElement::Minimap,
        HudElement::Outcome,
        HudElement::KillConfirm,
        HudElement::PauseMenu,
        HudElement::EnemyBars,
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
