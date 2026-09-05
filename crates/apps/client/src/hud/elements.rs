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
}

impl HudElement {
    /// Walked by the tests and by the census probe (F8/F9); the identity rule wants it whole.
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ALL: [HudElement; 20] = [
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
