//! The dead crew's HUD (interface program H19). A wreck's crew keeps the INTEL — the top bar,
//! the ears, the minimap, the feed, the connection — sat back at 0.7, and nothing of its own
//! gun: no reticle, no ammunition, no speed, no hit log, no markers, no wheel. With the arrows
//! it rides its living allies; the ally's panel is the wire's own snapshot of that hull (the
//! team's repair clocks ride the wire, W-4), and nothing about the ally's aim ever reaches the
//! screen — the aim is not on the wire, so the HUD cannot show it.

use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::damage_panel::DamagePanelModel;
use super::elements::{HudElement, SpectatePart};
use crate::ui_strings::battle as words;

/// The intel sits back this far for a dead crew.
pub const INTEL_ALPHA: f32 = 0.7;
const STRIP_SIZE_U: [f32; 2] = [440.0, 30.0];
/// Where the sixth-sense lamp sits for the living: the dead have no sixth sense.
const STRIP_TOP_U: f32 = super::sixth_sense::LAMP_TOP_U;

/// Whose hull the dead crew rides, and that hull's panel off the wire.
#[derive(Debug, Clone, PartialEq)]
pub struct SpectateStrip {
    /// The roster's name: vehicle · seat.
    pub name: String,
    /// Which of the living allies, and how many there are.
    pub index: usize,
    pub count: usize,
    pub panel: DamagePanelModel,
}

/// The dead crew's HUD: the wreck's own view, or an ally's.
#[derive(Debug, Clone, PartialEq)]
pub struct DeadModel {
    pub spectating: Option<SpectateStrip>,
}

/// What a dead crew still reads: the field's intel, and the modal and the banner over it.
pub fn survives_death(id: &HudElement) -> bool {
    matches!(
        id,
        HudElement::TopBar
            | HudElement::TopBarClock
            | HudElement::TopBarClockGlass
            | HudElement::TopBarAllyFrags
            | HudElement::TopBarEnemyFrags
            | HudElement::TopBarAllyPool
            | HudElement::TopBarEnemyPool
            | HudElement::TeamRow { .. }
            | HudElement::Minimap
            | HudElement::MinimapPlate
            | HudElement::MinimapRelief
            | HudElement::MinimapGlass
            | HudElement::MinimapBlip(_)
            | HudElement::MinimapSeat(_)
            | HudElement::MinimapGridLabel(_)
            | HudElement::KillFeed(_)
            | HudElement::NetReadout
            | HudElement::Outcome
            | HudElement::PauseMenu
    )
}

/// The intel sits back; the banner and the modal keep their light.
pub fn dimmed_when_dead(id: &HudElement) -> bool {
    survives_death(id) && !matches!(id, HudElement::Outcome | HudElement::PauseMenu)
}

pub(crate) fn push_spectate(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    strip: &SpectateStrip,
    z: &mut i16,
) {
    let plate = ui.anchor(Anchor::Top, STRIP_SIZE_U, [0.0, STRIP_TOP_U]);
    let enamel = theme.plates.enamel_black;
    list.push(
        Element::new(
            HudElement::Spectate(SpectatePart::Strip),
            plate,
            Payload::Plate { tile: enamel.tile, radius_u: 3.0, bevel_u: 1.0, color: enamel.color },
        )
        .z(*z),
    );
    *z += 1;
    list.push(
        Element::new(
            HudElement::Spectate(SpectatePart::Name),
            plate,
            Payload::Text {
                text: format!(
                    "{} \u{b7} {} \u{b7} {}/{}",
                    words::SPECTATING,
                    strip.name,
                    strip.index + 1,
                    strip.count
                ),
                style: Style::LABEL,
                size_u: 16.0,
                align: Align::Center,
                color: theme.lamp,
                digits: DigitMode::Tabular,
            },
        )
        .z(*z),
    );
    *z += 1;
    list.push(
        Element::new(
            HudElement::Spectate(SpectatePart::Keys),
            plate.inset(ui.px(10.0)),
            Payload::Text {
                text: words::SPECTATE_KEYS.to_string(),
                style: Style::VALUE,
                size_u: 14.0,
                align: Align::Right,
                color: theme.text.label_dim,
                digits: DigitMode::Proportional,
            },
        )
        .z(*z),
    );
    *z += 1;
    // The ally's panel, full-lit, where the crew's own panel sat: the wire's word on that hull.
    super::damage_panel::push_damage_panel(list, ui, theme, &strip.panel, z);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::elements::DamagePart;
    use crate::hud::{build_battle_hud_list, demo};

    /// H19: a dead crew's HUD keeps the intel — sat back — and never a reticle, ammunition,
    /// speed, hit log, markers or the wheel; with an ally under it, that ally's panel and the
    /// strip that names the seat come full-lit.
    #[test]
    fn a_dead_player_keeps_the_intel_hud_and_never_a_reticle() {
        let ui = Ui::reference();
        let mut model = demo::demo_model(false);
        model.command_wheel = Some(demo::demo_wheel());
        let alive = build_battle_hud_list(&model, &ui);
        let top_bar_alpha = |list: &DrawList<HudElement>| match &list
            .find(HudElement::TopBar)
            .expect("top bar")
            .payload
        {
            Payload::Plate { color, .. } => color[3],
            other => panic!("{other:?}"),
        };
        assert!(alive.find(HudElement::Reticle).is_some(), "precondition: the living aim");
        model.dead = Some(DeadModel { spectating: None });
        let dead = build_battle_hud_list(&model, &ui);
        for gone in [
            HudElement::Reticle,
            HudElement::Readouts,
            HudElement::HitDirection,
            HudElement::SixthSenseLamp,
            HudElement::BudgetLine,
            HudElement::DamagePanel(DamagePart::Plate),
        ] {
            assert!(dead.find(gone).is_none(), "{gone:?} is the living crew's");
        }
        assert!(!dead.iter().any(|e| matches!(
            e.id,
            HudElement::Ammo(_)
                | HudElement::Speed(_)
                | HudElement::HitLog(_)
                | HudElement::Marker(_)
                | HudElement::CommandWheel(_)
        )));
        assert!(
            dead.find(HudElement::TopBar).is_some()
                && dead.find(HudElement::MinimapPlate).is_some()
        );
        assert!(dead.iter().any(|e| matches!(e.id, HudElement::TeamRow { .. })));
        assert!(
            (top_bar_alpha(&dead) - top_bar_alpha(&alive) * INTEL_ALPHA).abs() < 1e-6,
            "the intel sits back at {INTEL_ALPHA}"
        );
        // Riding an ally: the strip and the ally's panel, full-lit.
        model.dead = Some(DeadModel {
            spectating: Some(SpectateStrip {
                name: "T-54 \u{b7} B".to_string(),
                index: 1,
                count: 3,
                panel: demo::quiet_damage_panel(),
            }),
        });
        let riding = build_battle_hud_list(&model, &ui);
        match &riding.find(HudElement::Spectate(SpectatePart::Name)).expect("strip").payload {
            Payload::Text { text, color, .. } => {
                assert_eq!(text, "SPECTATING \u{b7} T-54 \u{b7} B \u{b7} 2/3");
                assert_eq!(color[3], Theme::standard().lamp[3], "full-lit");
            }
            other => panic!("{other:?}"),
        }
        assert!(riding.find(HudElement::DamagePanel(DamagePart::HpBar)).is_some());
        assert!(riding.find(HudElement::Reticle).is_none(), "nothing about the ally's aim");
    }
}
