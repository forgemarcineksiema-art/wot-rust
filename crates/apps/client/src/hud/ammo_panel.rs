//! The ammunition panel (interface program H6), bottom-centre: one slot per round the gun
//! carries — the round's glyph, its designation, its penetration at a hundred metres and its
//! damage, straight from the spec — the count, and the key that picks it. The selected slot
//! sits on painted steel with a lamp hairline; an empty slot is dimmed.
//!
//! Honesty: the panel shows the server's selection. A switch the crew asked for and the server
//! has not confirmed wears the SWITCHING band across the panel — the loader is swapping the
//! round and the reload restarts (`sim`: "any real switch restarts the full reload") — until the
//! snapshot says the new round is in the breech.

use game_core::{MAX_AMMO_SLOTS, ShellSpec, ShellType};
use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload, WidgetState};
use ui_kit::font::Style;
use ui_kit::icons::HudIcon;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::elements::{AmmoPart, HudElement};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmmoSlotHud {
    pub icon: HudIcon,
    pub shell_type: ShellType,
    pub designation: &'static str,
    pub penetration_mm: u32,
    pub damage_hp: u32,
    pub count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmmoHudModel {
    pub slots: [Option<AmmoSlotHud>; MAX_AMMO_SLOTS],
    /// The slot the SERVER has in the breech.
    pub selected: u8,
    /// The slot the crew asked for, when it differs from `selected`: the switch in flight.
    pub requested: Option<u8>,
}

fn type_name(shell_type: ShellType) -> &'static str {
    match shell_type {
        ShellType::ArmorPiercing => "AP",
        ShellType::Apcr => "APCR",
        ShellType::Heat => "HEAT",
        ShellType::HighExplosive => "HE",
    }
}

impl AmmoHudModel {
    /// From the gun's options, the counts and the two selections: the server's and the crew's.
    pub(crate) fn new(
        shells: &[ShellSpec],
        counts: [u16; MAX_AMMO_SLOTS],
        selected: u8,
        requested: u8,
    ) -> Self {
        Self {
            slots: std::array::from_fn(|i| {
                shells.get(i).map(|shell| AmmoSlotHud {
                    icon: HudIcon::for_shell(shell.shell_type),
                    shell_type: shell.shell_type,
                    designation: shell
                        .round
                        .map_or(type_name(shell.shell_type), |round| round.designation()),
                    penetration_mm: shell.penetration_mm_at_100m.round().max(0.0) as u32,
                    damage_hp: shell.damage_hp,
                    count: counts[i],
                })
            }),
            selected,
            requested: (requested != selected).then_some(requested),
        }
    }

    pub fn switching(&self) -> bool {
        self.requested.is_some()
    }
}

const SLOT_SIZE_U: [f32; 2] = [156.0, 58.0];
const SLOT_GAP_U: f32 = 8.0;
const PANEL_BOTTOM_U: f32 = 12.0;
const TEXT_U: f32 = 16.0;

pub(crate) fn push_ammo_panel(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    model: &AmmoHudModel,
    z: &mut i16,
) {
    let mut push = |element: Element<HudElement>| {
        list.push(element.z(*z));
        *z += 1;
    };
    let key = HudElement::Ammo;
    let slots: Vec<(usize, &AmmoSlotHud)> = model
        .slots
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| slot.as_ref().map(|s| (i, s)))
        .collect();
    let n = slots.len() as f32;
    let width = n * SLOT_SIZE_U[0] + (n - 1.0).max(0.0) * SLOT_GAP_U;
    let frame = ui.anchor(Anchor::Bottom, [width, SLOT_SIZE_U[1]], [0.0, PANEL_BOTTOM_U]);
    let steel = theme.plates.steel_painted;
    let enamel = theme.plates.enamel_black;
    for (index, slot) in slots {
        let i = index as u8;
        let rect = Rect::new(
            frame.x + index as f32 * ui.px(SLOT_SIZE_U[0] + SLOT_GAP_U),
            frame.y,
            ui.px(SLOT_SIZE_U[0]),
            frame.h,
        );
        let selected = i == model.selected;
        let requested = model.requested == Some(i);
        let empty = slot.count == 0;
        let material = if selected { steel } else { enamel };
        let mut plate = Element::new(
            key(AmmoPart::Slot(i)),
            rect,
            Payload::Plate {
                tile: material.tile,
                radius_u: 4.0,
                bevel_u: if selected { theme.bevel_u } else { 1.0 },
                color: material.color,
            },
        );
        if empty {
            plate.state = WidgetState::Disabled;
        }
        push(plate);
        if selected || requested {
            // The lamp hairline under the breech's round; the requested one wears it dimmer
            // until the server confirms.
            let lamp = if selected {
                theme.lamp
            } else {
                [theme.lamp[0], theme.lamp[1], theme.lamp[2], 0.5]
            };
            push(Element::new(
                key(AmmoPart::Lamp(i)),
                Rect::new(
                    rect.x + ui.px(6.0),
                    rect.bottom() - ui.px(4.0),
                    rect.w - ui.px(12.0),
                    ui.px(2.0),
                ),
                Payload::Plate { tile: enamel.tile, radius_u: 1.0, bevel_u: 0.0, color: lamp },
            ));
        }
        let dim =
            |c: [f32; 4]| if empty { [c[0], c[1], c[2], c[3] * theme.disabled_alpha] } else { c };
        let icon = ui.px(28.0);
        push(Element::new(
            key(AmmoPart::Icon(i)),
            Rect::new(rect.x + ui.px(8.0), rect.y + (rect.h - icon) * 0.5, icon, icon),
            Payload::Icon {
                icon: slot.icon,
                color: dim(theme.semantic.ammo[slot.shell_type as usize % 4]),
            },
        ));
        let text_x = rect.x + ui.px(44.0);
        let text_w = rect.w - ui.px(44.0) - ui.px(52.0);
        push(Element::new(
            key(AmmoPart::Designation(i)),
            Rect::new(text_x, rect.y + ui.px(8.0), text_w, ui.px(TEXT_U)),
            Payload::Text {
                text: slot.designation.to_string(),
                style: Style::LABEL,
                size_u: TEXT_U,
                align: Align::Left,
                color: dim(theme.text.label),
                digits: DigitMode::Proportional,
            },
        ));
        push(Element::new(
            key(AmmoPart::Numbers(i)),
            Rect::new(text_x, rect.y + ui.px(30.0), text_w + ui.px(20.0), ui.px(TEXT_U)),
            Payload::Text {
                text: format!(
                    "{} {} \u{b7} {}",
                    slot.penetration_mm,
                    crate::ui_strings::battle::MILLIMETRES,
                    slot.damage_hp
                ),
                style: Style::VALUE,
                size_u: TEXT_U,
                align: Align::Left,
                color: dim(theme.text.unit),
                digits: DigitMode::Tabular,
            },
        ));
        push(Element::new(
            key(AmmoPart::Count(i)),
            Rect::new(rect.right() - ui.px(56.0), rect.y + ui.px(6.0), ui.px(48.0), ui.px(24.0)),
            Payload::Text {
                text: slot.count.min(999).to_string(),
                style: Style::VALUE_BOLD,
                size_u: 22.0,
                align: Align::Right,
                color: dim(if empty { theme.semantic.module[2] } else { theme.text.value }),
                digits: DigitMode::Tabular,
            },
        ));
        push(Element::new(
            key(AmmoPart::Key(i)),
            Rect::new(
                rect.right() - ui.px(24.0),
                rect.bottom() - ui.px(22.0),
                ui.px(16.0),
                ui.px(TEXT_U),
            ),
            Payload::Text {
                text: (index + 1).to_string(),
                style: Style::VALUE,
                size_u: TEXT_U,
                align: Align::Right,
                color: theme.text.label_dim,
                digits: DigitMode::Tabular,
            },
        ));
    }
    if model.switching() {
        // The band over the panel: the loader is swapping the round, the reload has restarted.
        let band = Rect::new(frame.x, frame.y - ui.px(26.0), frame.w, ui.px(22.0));
        push(Element::new(
            key(AmmoPart::SwitchingBand),
            band,
            Payload::Plate {
                tile: enamel.tile,
                radius_u: 3.0,
                bevel_u: 0.0,
                color: [
                    theme.semantic.module[1][0],
                    theme.semantic.module[1][1],
                    theme.semantic.module[1][2],
                    0.9,
                ],
            },
        ));
        push(Element::new(
            key(AmmoPart::SwitchingText),
            band,
            Payload::Text {
                text: crate::ui_strings::battle::AMMO_SWITCHING.to_string(),
                style: Style::LABEL,
                size_u: TEXT_U,
                align: Align::Center,
                color: theme.text.value,
                digits: DigitMode::Proportional,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::VehicleKind;

    fn shells() -> Vec<ShellSpec> {
        VehicleKind::BENCHMARK.spec_ref().gun.ammo_options()
    }

    fn build(model: &AmmoHudModel) -> DrawList<HudElement> {
        let mut list = DrawList::new();
        let mut z = 0;
        push_ammo_panel(&mut list, &Ui::reference(), &Theme::standard(), model, &mut z);
        list
    }

    fn text_of(list: &DrawList<HudElement>, part: AmmoPart) -> String {
        match &list.find(HudElement::Ammo(part)).expect("element").payload {
            Payload::Text { text, .. } => text.clone(),
            other => panic!("{other:?}"),
        }
    }

    /// H6: every slot prints the round's designation, its penetration at a hundred metres and
    /// its damage — the spec's own numbers, never a rounding of the HUD's.
    #[test]
    fn every_ammo_slot_prints_designation_penetration_and_damage() {
        let shells = shells();
        let model = AmmoHudModel::new(&shells, [22, 9, 6], 0, 0);
        let list = build(&model);
        for (i, shell) in shells.iter().enumerate() {
            let slot = model.slots[i].expect("a fielded slot");
            assert_eq!(slot.designation, shell.round.expect("a fielded round").designation());
            assert_eq!(text_of(&list, AmmoPart::Designation(i as u8)), slot.designation);
            let numbers = text_of(&list, AmmoPart::Numbers(i as u8));
            assert!(
                numbers.starts_with(&format!("{} ", shell.penetration_mm_at_100m.round() as u32)),
                "{numbers}"
            );
            assert!(numbers.ends_with(&shell.damage_hp.to_string()), "{numbers}");
            assert_eq!(text_of(&list, AmmoPart::Count(i as u8)), [22, 9, 6][i].to_string());
            assert_eq!(text_of(&list, AmmoPart::Key(i as u8)), (i + 1).to_string());
        }
        assert!(
            list.find(HudElement::Ammo(AmmoPart::Lamp(0))).is_some(),
            "the breech's round wears the lamp"
        );
        assert!(list.find(HudElement::Ammo(AmmoPart::Lamp(1))).is_none());
        assert!(!model.switching());
    }

    /// H6: a switch the server has not confirmed wears the band; the confirmed one does not.
    #[test]
    fn a_switch_shows_its_cost_before_the_snapshot_confirms_it() {
        let shells = shells();
        let in_flight = AmmoHudModel::new(&shells, [22, 9, 6], 0, 1);
        assert!(in_flight.switching() && in_flight.requested == Some(1));
        let list = build(&in_flight);
        assert_eq!(
            text_of(&list, AmmoPart::SwitchingText),
            crate::ui_strings::battle::AMMO_SWITCHING
        );
        assert!(
            list.find(HudElement::Ammo(AmmoPart::Lamp(1))).is_some(),
            "the requested slot is marked"
        );
        assert!(
            list.find(HudElement::Ammo(AmmoPart::Lamp(0))).is_some(),
            "the breech still holds the old round"
        );
        let confirmed = AmmoHudModel::new(&shells, [22, 9, 6], 1, 1);
        assert!(!confirmed.switching());
        assert!(build(&confirmed).find(HudElement::Ammo(AmmoPart::SwitchingBand)).is_none());
    }

    #[test]
    fn a_heat_loading_gun_shows_the_shaped_charge_glyph_not_apcr() {
        let model = AmmoHudModel::new(&shells(), [20, 8, 6], 0, 0);
        assert_eq!(
            model.slots[1].expect("slot").icon,
            HudIcon::AmmoHeat,
            "slot 1 carries the BK-5 glyph"
        );
        assert_ne!(model.slots[1].expect("slot").icon, model.slots[0].expect("slot").icon);
    }

    /// An empty slot is dimmed and its count reads red; the panel hangs from the bottom edge.
    #[test]
    fn an_empty_slot_is_dimmed_and_the_panel_hangs_from_the_bottom() {
        let theme = Theme::standard();
        let model = AmmoHudModel::new(&shells(), [22, 0, 6], 0, 0);
        let list = build(&model);
        assert_eq!(
            list.find(HudElement::Ammo(AmmoPart::Slot(1))).expect("slot").state,
            WidgetState::Disabled
        );
        match &list.find(HudElement::Ammo(AmmoPart::Count(1))).expect("count").payload {
            Payload::Text { color, .. } => {
                assert!((color[0] - theme.semantic.module[2][0]).abs() < 1e-6)
            }
            other => panic!("{other:?}"),
        }
        let ui = Ui::reference();
        let first = list.find(HudElement::Ammo(AmmoPart::Slot(0))).expect("slot").rect;
        let last = list.find(HudElement::Ammo(AmmoPart::Slot(2))).expect("slot").rect;
        assert!(ui.viewport().encloses(&first) && ui.viewport().encloses(&last));
        assert!(((first.x + last.right()) * 0.5 - 960.0).abs() < 1.0, "centred");
        assert!(first.bottom() > 1040.0);
    }
}
