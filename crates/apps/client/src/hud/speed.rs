//! The speed instrument (interface program H5): the hull's speed and the cruise latch, on a
//! small plate beside the damage panel. Five notches — three forward, two in reverse — light in
//! the lamp colour as far as the latch is set; a held W/S overrides the latch and the notches
//! stay where the latch is, so the eye reads what the hull will do when the key lifts.

use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::elements::{HudElement, SpeedPart};

pub const CRUISE_FORWARD_STEPS: i8 = 3;
pub const CRUISE_REVERSE_STEPS: i8 = 2;

const PLATE_SIZE_U: [f32; 2] = [132.0, 56.0];
/// Above the damage panel at the left edge — never beside it, where the ammunition panel's
/// centred width would meet it at the large size class.
const PLATE_OFFSET_U: [f32; 2] = [12.0, 152.0];

pub(crate) fn push_speed(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    speed_kmh: f32,
    cruise_level: i8,
    z: &mut i16,
) {
    let mut push = |element: Element<HudElement>| {
        list.push(element.z(*z));
        *z += 1;
    };
    let steel = theme.plates.steel_painted;
    let plate = ui.anchor(Anchor::BottomLeft, PLATE_SIZE_U, PLATE_OFFSET_U);
    push(Element::new(
        HudElement::Speed(SpeedPart::Plate),
        plate,
        Payload::Plate {
            tile: steel.tile,
            radius_u: 4.0,
            bevel_u: theme.bevel_u,
            color: steel.color,
        },
    ));
    let number = Rect::new(plate.x + ui.px(8.0), plate.y + ui.px(4.0), ui.px(66.0), ui.px(30.0));
    push(Element::new(
        HudElement::Speed(SpeedPart::Number),
        number,
        Payload::Text {
            text: (speed_kmh.round().clamp(0.0, 999.0) as u32).to_string(),
            style: Style::VALUE_BOLD,
            size_u: 28.0,
            align: Align::Right,
            color: theme.text.value,
            digits: DigitMode::Tabular,
        },
    ));
    push(Element::new(
        HudElement::Speed(SpeedPart::Unit),
        Rect::new(number.right() + ui.px(6.0), plate.y + ui.px(12.0), ui.px(44.0), ui.px(16.0)),
        Payload::Text {
            text: crate::ui_strings::battle::SPEED_UNIT.to_string(),
            style: Style::VALUE_STRONG,
            size_u: 16.0,
            align: Align::Left,
            color: theme.text.unit,
            digits: DigitMode::Proportional,
        },
    ));
    // The notches: reverse on the left of a gap, forward on the right, lit up to the latch.
    let notch_w = ui.px(16.0);
    let gap = ui.px(4.0);
    let row_y = plate.bottom() - ui.px(14.0);
    let total = CRUISE_FORWARD_STEPS + CRUISE_REVERSE_STEPS;
    let row_w = f32::from(total) * notch_w + f32::from(total) * gap;
    let mut x = plate.center()[0] - row_w * 0.5;
    let enamel = theme.plates.enamel_black;
    for level in (-CRUISE_REVERSE_STEPS..=CRUISE_FORWARD_STEPS).filter(|level| *level != 0) {
        let lit = (level > 0 && cruise_level >= level) || (level < 0 && cruise_level <= level);
        let color =
            if lit { theme.lamp } else { [enamel.color[0], enamel.color[1], enamel.color[2], 0.8] };
        push(Element::new(
            HudElement::Speed(SpeedPart::Notch(level)),
            Rect::new(x, row_y, notch_w, ui.px(5.0)),
            Payload::Plate { tile: enamel.tile, radius_u: 1.0, bevel_u: 0.0, color },
        ));
        x += notch_w + gap;
        if level == -1 {
            x += gap * 2.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(speed_kmh: f32, cruise_level: i8) -> DrawList<HudElement> {
        let mut list = DrawList::new();
        let mut z = 0;
        push_speed(
            &mut list,
            &Ui::reference(),
            &Theme::standard(),
            speed_kmh,
            cruise_level,
            &mut z,
        );
        list
    }

    /// H5: the notches light as far as the latch, forward or reverse, and the number reads the
    /// hull's speed whether it moves or not.
    #[test]
    fn the_speed_instrument_shows_the_cruise_level() {
        let theme = Theme::standard();
        let lit = |list: &DrawList<HudElement>| -> Vec<i8> {
            (-CRUISE_REVERSE_STEPS..=CRUISE_FORWARD_STEPS)
                .filter(|level| *level != 0)
                .filter(|level| {
                    matches!(
                        list.find(HudElement::Speed(SpeedPart::Notch(*level))).map(|e| &e.payload),
                        Some(Payload::Plate { color, .. }) if *color == theme.lamp
                    )
                })
                .collect()
        };
        assert_eq!(lit(&build(0.0, 0)), Vec::<i8>::new());
        assert_eq!(lit(&build(20.0, 2)), vec![1, 2]);
        assert_eq!(lit(&build(6.0, -2)), vec![-2, -1]);
        match &build(42.4, 0).find(HudElement::Speed(SpeedPart::Number)).expect("number").payload {
            Payload::Text { text, .. } => assert_eq!(text, "42"),
            other => panic!("{other:?}"),
        }
        let plate = build(0.0, 0).find(HudElement::Speed(SpeedPart::Plate)).expect("plate").rect;
        assert!(
            Ui::reference().viewport().encloses(&plate)
                && plate.x < 100.0
                && plate.bottom() < 940.0
        );
    }
}
