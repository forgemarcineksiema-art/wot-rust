//! A teammate's ping in the world and the team's newest word (interface program H16). The
//! ping is a lamp disc with the sender's seat over the pinged ground and its range, fading
//! with the minimap's own ping clock; the word is one line under the budget: „C · ATTACK ·
//! Tiger II · A". Both come off the server's relay (W-5) from a hull on our team — the client
//! checks the roster itself, so a relay that was never ours can never be drawn.

use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::elements::{HudElement, PingPart};
use crate::ui_strings::battle as words;

/// A ping rings for as long as it does on the map.
pub const PING_TTL_S: f32 = super::minimap::PING_TTL_S;
/// A word stays on the strip this long.
pub const WORD_TTL_S: f32 = 4.0;
/// The disc's size in `u`.
const DISC_U: f32 = 26.0;
/// The word strip sits under the budget line.
const WORD_TOP_U: f32 = super::budget::LINE_TOP_U + 24.0;

#[derive(Debug, Clone, PartialEq)]
pub struct PingMark {
    /// The pinged ground on the screen, physical pixels.
    pub screen_px: [f32; 2],
    pub seat: char,
    pub distance_m: u32,
    pub age_s: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PingModel {
    pub marks: Vec<PingMark>,
}

impl PingModel {
    /// A staged model's marks are authored in reference pixels (1920 × 1080); the golden
    /// instrument scales them to the viewport it renders. The live model is never scaled.
    pub fn scaled_from_reference(mut self, viewport_px: [f32; 2]) -> Self {
        for mark in &mut self.marks {
            mark.screen_px = [
                mark.screen_px[0] * viewport_px[0] / 1920.0,
                mark.screen_px[1] * viewport_px[1] / 1080.0,
            ];
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TeamWord {
    pub seat: char,
    pub command: net::TeamCommand,
    /// The hull the word is about, „Tiger II · A", when it names one.
    pub target: Option<String>,
    pub age_s: f32,
}

impl TeamWord {
    pub fn line(&self) -> String {
        let word = super::command_wheel::command_word(self.command);
        match &self.target {
            Some(target) => format!("{} \u{b7} {word} \u{b7} {target}", self.seat),
            None => format!("{} \u{b7} {word}", self.seat),
        }
    }
}

fn faded(color: [f32; 4], age_s: f32, ttl_s: f32) -> [f32; 4] {
    let fade = (1.0 - age_s / ttl_s).clamp(0.0, 1.0);
    [color[0], color[1], color[2], color[3] * fade]
}

pub(crate) fn push_pings(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    model: &PingModel,
    z: &mut i16,
) {
    let enamel = theme.plates.enamel_black;
    for (index, mark) in model.marks.iter().enumerate() {
        let i = index as u8;
        let disc = ui.px(DISC_U);
        let rect = Rect::new(mark.screen_px[0] - disc * 0.5, mark.screen_px[1] - disc, disc, disc);
        let lamp = faded(theme.lamp, mark.age_s, PING_TTL_S);
        list.push(
            Element::new(
                HudElement::Ping(PingPart::Disc(i)),
                rect,
                Payload::Plate {
                    tile: enamel.tile,
                    radius_u: DISC_U * 0.5,
                    bevel_u: 1.0,
                    color: faded(enamel.color, mark.age_s, PING_TTL_S),
                },
            )
            .z(*z),
        );
        *z += 1;
        list.push(
            Element::new(
                HudElement::Ping(PingPart::Seat(i)),
                rect,
                Payload::Text {
                    text: mark.seat.to_string(),
                    style: Style::VALUE_STRONG,
                    size_u: 16.0,
                    align: Align::Center,
                    color: lamp,
                    digits: DigitMode::Proportional,
                },
            )
            .z(*z),
        );
        *z += 1;
        list.push(
            Element::new(
                HudElement::Ping(PingPart::Range(i)),
                Rect::new(
                    rect.x - ui.px(30.0),
                    rect.bottom() + ui.px(2.0),
                    disc + ui.px(60.0),
                    ui.px(16.0),
                ),
                Payload::Text {
                    text: format!("{} {}", mark.distance_m, words::DISTANCE_UNIT),
                    style: Style::VALUE,
                    size_u: 16.0,
                    align: Align::Center,
                    color: faded(theme.text.label_dim, mark.age_s, PING_TTL_S),
                    digits: DigitMode::Tabular,
                },
            )
            .z(*z),
        );
        *z += 1;
    }
}

pub(crate) fn push_team_word(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    word: &TeamWord,
    z: &mut i16,
) {
    if word.age_s >= WORD_TTL_S {
        return;
    }
    list.push(
        Element::new(
            HudElement::TeamWord,
            ui.anchor(Anchor::Top, [420.0, 22.0], [0.0, WORD_TOP_U]),
            Payload::Text {
                text: word.line(),
                style: Style::VALUE_STRONG,
                size_u: 16.0,
                align: Align::Center,
                color: faded(theme.lamp, word.age_s, WORD_TTL_S),
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

    /// A ping mark sits over its ground with the seat and the range; a word names who, what
    /// and about whom, and dies at its TTL.
    #[test]
    fn a_ping_wears_its_seat_and_range_and_a_word_names_who_said_what() {
        let mut list = DrawList::new();
        let mut z = 0;
        let model = PingModel {
            marks: vec![PingMark {
                screen_px: [1180.0, 470.0],
                seat: 'B',
                distance_m: 340,
                age_s: 1.0,
            }],
        };
        push_pings(&mut list, &Ui::reference(), &Theme::standard(), &model, &mut z);
        let disc = list.find(HudElement::Ping(PingPart::Disc(0))).expect("disc").rect;
        assert!((disc.center()[0] - 1180.0).abs() < 1.0 && disc.bottom() <= 470.5, "{disc:?}");
        match &list.find(HudElement::Ping(PingPart::Range(0))).expect("range").payload {
            Payload::Text { text, .. } => assert_eq!(text, "340 M"),
            other => panic!("{other:?}"),
        }
        let word = TeamWord {
            seat: 'C',
            command: net::TeamCommand::Attack,
            target: Some("Tiger II \u{b7} A".to_string()),
            age_s: 0.5,
        };
        assert_eq!(word.line(), "C \u{b7} ATTACK \u{b7} Tiger II \u{b7} A");
        let mut strip = DrawList::new();
        push_team_word(&mut strip, &Ui::reference(), &Theme::standard(), &word, &mut z);
        assert!(strip.find(HudElement::TeamWord).is_some());
        let mut gone = DrawList::new();
        let old = TeamWord { age_s: WORD_TTL_S, ..word };
        push_team_word(&mut gone, &Ui::reference(), &Theme::standard(), &old, &mut z);
        assert!(gone.find(HudElement::TeamWord).is_none());
    }
}
