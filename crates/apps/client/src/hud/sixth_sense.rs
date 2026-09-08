//! The sixth sense (interface program H13): the lamp under the top bar that is lit exactly
//! while the crew's own hull is spotted — the own `spotted_by_teams_mask` survives the snapshot
//! filter untouched, so the lamp is the wire's own word — and the chime that plays once per
//! spotted span (the app's edge, `sixth_sense_edge`). World of Tanks' bulb, with the honesty
//! that it never lights from a guess and never stays lit from a memory.

use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::elements::HudElement;

/// Whether the own mask says an enemy team sees us.
pub fn lit(own_spotted_mask: u8, enemy_bit: u8) -> bool {
    own_spotted_mask & enemy_bit != 0
}

const LAMP_SIZE_U: [f32; 2] = [148.0, 26.0];
/// Under the top bar.
pub(crate) const LAMP_TOP_U: f32 = 64.0;

pub(crate) fn push_sixth_sense(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    is_lit: bool,
    z: &mut i16,
) {
    if !is_lit {
        return;
    }
    let plate = ui.anchor(Anchor::Top, LAMP_SIZE_U, [0.0, LAMP_TOP_U]);
    let enamel = theme.plates.enamel_black;
    list.push(
        Element::new(
            HudElement::SixthSenseLamp,
            plate,
            Payload::Plate { tile: enamel.tile, radius_u: 3.0, bevel_u: 1.0, color: enamel.color },
        )
        .z(*z),
    );
    *z += 1;
    list.push(
        Element::new(
            HudElement::SixthSenseText,
            plate,
            Payload::Text {
                text: crate::ui_strings::battle::SPOTTED_LAMP.to_string(),
                style: Style::VALUE_STRONG,
                size_u: 16.0,
                align: Align::Center,
                color: theme.lamp,
                digits: DigitMode::Proportional,
            },
        )
        .z(*z),
    );
    *z += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::TeamId;

    /// H13: the lamp is lit exactly while the own mask carries the enemy's bit — not the
    /// friendly team's bit, not a memory — and the element exists only then.
    #[test]
    fn the_lamp_is_lit_exactly_while_the_own_mask_says_spotted() {
        let enemy = TeamId(2).spotting_bit();
        let friend = TeamId(1).spotting_bit();
        assert!(lit(enemy, enemy));
        assert!(lit(enemy | friend, enemy));
        assert!(!lit(friend, enemy), "our own team seeing us is not the sixth sense");
        assert!(!lit(0, enemy));
        let build = |is_lit: bool| {
            let mut list = DrawList::new();
            let mut z = 0;
            push_sixth_sense(&mut list, &Ui::reference(), &Theme::standard(), is_lit, &mut z);
            list
        };
        assert!(build(false).find(HudElement::SixthSenseLamp).is_none());
        let on = build(true);
        let lamp = on.find(HudElement::SixthSenseLamp).expect("lamp").rect;
        assert!(
            (lamp.center()[0] - 960.0).abs() < 1.0 && lamp.y > 56.0 && lamp.y < 100.0,
            "{lamp:?}"
        );
        match &on.find(HudElement::SixthSenseText).expect("word").payload {
            Payload::Text { text, color, .. } => {
                assert_eq!(text, "SPOTTED");
                assert_eq!(*color, Theme::standard().lamp);
            }
            other => panic!("{other:?}"),
        }
    }
}
