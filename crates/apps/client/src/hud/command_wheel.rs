//! The command wheel (interface program H16): Z held opens a ring of eight words about the
//! reticle; the mouse's travel picks a sector, the release says it; the hub says nothing.
//! World of Tanks' wheel, with two honesties of its own: the counter under it is the server's
//! allowance (W-5, five a minute) mirrored on the client, and a refusal is KNOCKED — the word
//! REFUSED, the seconds to wait, `UiReject` — never swallowed.

use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::Ui;

use super::elements::{HudElement, WheelPart};
use crate::ui_strings::battle as words;

/// The commands on the wheel, clockwise from the top: the wire's order.
pub const COMMAND_COUNT: usize = net::TeamCommand::ALL.len();
/// The mouse must travel this far from the hub before a sector is chosen.
pub const DEAD_ZONE_PX: f32 = 24.0;
/// The ring's radius, in `u`.
const RING_RADIUS_U: f32 = 200.0;
const SECTOR_SIZE_U: [f32; 2] = [150.0, 32.0];
const COUNTER_SIZE_U: [f32; 2] = [280.0, 22.0];
/// How long the knock shows under the wheel.
pub const KNOCK_TTL_S: f32 = 1.5;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandWheelModel {
    /// Whether Z is held: the ring shows. With it down, only a fresh knock shows its line.
    pub open: bool,
    /// The sector under the mouse, an index into `net::TeamCommand::ALL`.
    pub selected: Option<usize>,
    /// Commands the allowance still admits this minute.
    pub remaining: usize,
    /// Seconds until a slot frees, while none is free.
    pub wait_s: Option<f32>,
    /// Seconds since the last refusal; `None` once the knock has faded.
    pub knock_age_s: Option<f32>,
}

/// The word of a command.
pub fn command_word(command: net::TeamCommand) -> &'static str {
    match command {
        net::TeamCommand::Attack => words::CMD_ATTACK,
        net::TeamCommand::Help => words::CMD_HELP,
        net::TeamCommand::Reloading => words::CMD_RELOADING,
        net::TeamCommand::Affirmative => words::CMD_AFFIRMATIVE,
        net::TeamCommand::Negative => words::CMD_NEGATIVE,
        net::TeamCommand::BackToBase => words::CMD_BACK_TO_BASE,
        net::TeamCommand::FollowMe => words::CMD_FOLLOW_ME,
        net::TeamCommand::Ping => words::CMD_PING,
    }
}

/// The sector under a mouse travel of `delta_px` from the hub (screen y grows downward): the
/// top sector is the first command, then clockwise. Inside the dead zone nothing is chosen.
pub fn sector_of(delta_px: [f32; 2]) -> Option<usize> {
    if delta_px[0].hypot(delta_px[1]) < DEAD_ZONE_PX {
        return None;
    }
    let n = COMMAND_COUNT as f32;
    let turn = delta_px[0].atan2(-delta_px[1]) / std::f32::consts::TAU;
    let sector = (turn * n + 0.5).rem_euclid(n).floor() as usize;
    Some(sector.min(COMMAND_COUNT - 1))
}

pub(crate) fn push_command_wheel(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    model: &CommandWheelModel,
    z: &mut i16,
) {
    // The wheel's centre is the screen's, where the reticle rests.
    let hub = ui.viewport().center();
    if model.open {
        for (index, command) in net::TeamCommand::ALL.iter().enumerate() {
            let angle = index as f32 / COMMAND_COUNT as f32 * std::f32::consts::TAU;
            let center = [
                hub[0] + angle.sin() * ui.px(RING_RADIUS_U),
                hub[1] - angle.cos() * ui.px(RING_RADIUS_U),
            ];
            let size = [ui.px(SECTOR_SIZE_U[0]), ui.px(SECTOR_SIZE_U[1])];
            let rect =
                Rect::new(center[0] - size[0] * 0.5, center[1] - size[1] * 0.5, size[0], size[1]);
            let chosen = model.selected == Some(index);
            let plate = if chosen { theme.plates.enamel_black } else { theme.plates.steel_painted };
            list.push(
                Element::new(
                    HudElement::CommandWheel(WheelPart::Sector(index as u8)),
                    rect,
                    Payload::Plate {
                        tile: plate.tile,
                        radius_u: 3.0,
                        bevel_u: theme.bevel_u,
                        color: plate.color,
                    },
                )
                .z(*z),
            );
            *z += 1;
            list.push(
                Element::new(
                    HudElement::CommandWheel(WheelPart::Label(index as u8)),
                    rect,
                    Payload::Text {
                        text: command_word(*command).to_string(),
                        style: Style::LABEL,
                        size_u: 16.0,
                        align: Align::Center,
                        color: if chosen { theme.lamp } else { theme.text.label },
                        digits: DigitMode::Proportional,
                    },
                )
                .z(*z),
            );
            *z += 1;
        }
    }
    let knocking = model.knock_age_s.is_some_and(|age| age < KNOCK_TTL_S);
    if !model.open && !knocking {
        return;
    }
    // Under the ring (a Center anchor takes no offset, so the counter is placed by hand).
    let counter_size = [ui.px(COUNTER_SIZE_U[0]), ui.px(COUNTER_SIZE_U[1])];
    let counter = Rect::new(
        hub[0] - counter_size[0] * 0.5,
        hub[1] + ui.px(RING_RADIUS_U + 44.0) - counter_size[1] * 0.5,
        counter_size[0],
        counter_size[1],
    );
    let (text, color) = match (knocking, model.wait_s) {
        (true, Some(wait_s)) => (
            format!(
                "{} \u{b7} {} {} {}",
                words::WHEEL_REFUSED,
                words::WHEEL_WAIT,
                wait_s.ceil().max(1.0) as u32,
                words::SECONDS_UNIT
            ),
            theme.semantic.verdict.no_pen,
        ),
        (true, None) => (words::WHEEL_REFUSED.to_string(), theme.semantic.verdict.no_pen),
        (false, _) => (
            format!(
                "{} {}/{}",
                words::WHEEL_COMMANDS,
                model.remaining,
                net::TEAM_COMMANDS_PER_WINDOW
            ),
            theme.text.label_dim,
        ),
    };
    list.push(
        Element::new(
            HudElement::CommandWheel(WheelPart::Counter),
            counter,
            Payload::Text {
                text,
                style: Style::VALUE,
                size_u: 16.0,
                align: Align::Center,
                color,
                digits: DigitMode::Tabular,
            },
        )
        .z(*z),
    );
    *z += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// H16: up is the first word, the ring runs clockwise, and the hub chooses nothing.
    #[test]
    fn the_wheel_picks_the_sector_under_the_mouse_and_none_at_the_hub() {
        assert_eq!(sector_of([0.0, -100.0]), Some(0), "up: ATTACK");
        assert_eq!(sector_of([100.0, 0.0]), Some(2), "right");
        assert_eq!(sector_of([0.0, 100.0]), Some(4), "down");
        assert_eq!(sector_of([-100.0, 0.0]), Some(6), "left");
        assert_eq!(sector_of([70.0, -70.0]), Some(1), "up-right");
        assert_eq!(sector_of([5.0, 5.0]), None, "the dead zone");
        assert_eq!(net::TeamCommand::ALL[0], net::TeamCommand::Attack);
        assert_eq!(command_word(net::TeamCommand::BackToBase), "BACK TO BASE");
    }

    /// The open wheel draws every word once, the chosen one in the lamp; closed, only a fresh
    /// knock shows — the word REFUSED and the seconds — and a faded one draws nothing.
    #[test]
    fn the_ring_shows_every_word_and_a_refusal_is_a_word_not_a_silence() {
        let build = |model: &CommandWheelModel| {
            let mut list = DrawList::new();
            let mut z = 0;
            push_command_wheel(&mut list, &Ui::reference(), &Theme::standard(), model, &mut z);
            list
        };
        let open = build(&CommandWheelModel {
            open: true,
            selected: Some(3),
            remaining: 4,
            wait_s: None,
            knock_age_s: None,
        });
        for index in 0..COMMAND_COUNT as u8 {
            assert!(open.find(HudElement::CommandWheel(WheelPart::Label(index))).is_some());
        }
        let text_of =
            |list: &DrawList<HudElement>, id| match &list.find(id).expect("element").payload {
                Payload::Text { text, color, .. } => (text.clone(), *color),
                other => panic!("{other:?}"),
            };
        let (chosen, color) = text_of(&open, HudElement::CommandWheel(WheelPart::Label(3)));
        assert_eq!(chosen, "AFFIRMATIVE");
        assert_eq!(color, Theme::standard().lamp);
        assert_eq!(text_of(&open, HudElement::CommandWheel(WheelPart::Counter)).0, "COMMANDS 4/5");
        let counter =
            open.find(HudElement::CommandWheel(WheelPart::Counter)).expect("counter").rect;
        assert!(
            (counter.center()[0] - 960.0).abs() < 1.0 && counter.y > 540.0 + 200.0,
            "under the ring: {counter:?}"
        );
        let knocked = build(&CommandWheelModel {
            open: false,
            selected: None,
            remaining: 0,
            wait_s: Some(11.2),
            knock_age_s: Some(0.2),
        });
        assert!(knocked.find(HudElement::CommandWheel(WheelPart::Label(0))).is_none());
        assert_eq!(
            text_of(&knocked, HudElement::CommandWheel(WheelPart::Counter)).0,
            "REFUSED \u{b7} WAIT 12 S"
        );
        let faded = build(&CommandWheelModel {
            open: false,
            selected: None,
            remaining: 0,
            wait_s: Some(9.0),
            knock_age_s: Some(KNOCK_TTL_S + 0.1),
        });
        assert!(faded.find(HudElement::CommandWheel(WheelPart::Counter)).is_none());
    }
}
