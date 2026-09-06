//! The connection readout (interface program H18): the round trip and the age of the newest
//! state, under the frame counter — the number the session measures, never a guess; the local
//! host, which has no wire, says LOCAL.

use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::elements::HudElement;
use crate::ui_strings::battle as words;

/// Past these the readout turns to the held colour: the wire is the reason the picture lags.
pub const RTT_ALERT_MS: u32 = 150;
pub const SNAPSHOT_ALERT_MS: u32 = 250;
const READOUT_TOP_U: f32 = 76.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NetReadoutModel {
    /// The local host: no wire, no numbers.
    pub local: bool,
    /// The measured round trip, once the session has one.
    pub rtt_ms: Option<u32>,
    /// Milliseconds since the newest snapshot arrived.
    pub snapshot_age_ms: u32,
}

impl NetReadoutModel {
    pub fn line(&self) -> String {
        if self.local {
            return words::NET_LOCAL.to_string();
        }
        let rtt = match self.rtt_ms {
            Some(ms) => format!("{} {} {}", words::NET_RTT, ms, words::MILLISECONDS),
            None => format!("{} \u{2014}", words::NET_RTT),
        };
        format!(
            "{rtt} \u{b7} {} {} {}",
            words::NET_SNAPSHOT,
            self.snapshot_age_ms,
            words::MILLISECONDS
        )
    }

    pub fn alert(&self) -> bool {
        !self.local
            && (self.rtt_ms.is_some_and(|ms| ms > RTT_ALERT_MS)
                || self.snapshot_age_ms > SNAPSHOT_ALERT_MS)
    }
}

pub(crate) fn push_net_readout(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    model: &NetReadoutModel,
    z: &mut i16,
) {
    list.push(
        Element::new(
            HudElement::NetReadout,
            ui.anchor(Anchor::TopRight, [300.0, 18.0], [12.0, READOUT_TOP_U]),
            Payload::Text {
                text: model.line(),
                style: Style::VALUE,
                size_u: 16.0,
                align: Align::Right,
                color: if model.alert() {
                    theme.semantic.verdict.no_pen
                } else {
                    theme.text.label_dim
                },
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

    /// H18: the readout prints the session's own numbers, and the local host says LOCAL.
    #[test]
    fn the_rtt_readout_prints_the_sessions_number_and_local_says_local() {
        let remote = NetReadoutModel { local: false, rtt_ms: Some(48), snapshot_age_ms: 32 };
        assert_eq!(remote.line(), "RTT 48 MS \u{b7} SNAP 32 MS");
        assert!(!remote.alert());
        let slow = NetReadoutModel { local: false, rtt_ms: Some(210), snapshot_age_ms: 32 };
        assert!(slow.alert());
        let unmeasured = NetReadoutModel { local: false, rtt_ms: None, snapshot_age_ms: 5 };
        assert_eq!(unmeasured.line(), "RTT \u{2014} \u{b7} SNAP 5 MS");
        let local = NetReadoutModel { local: true, rtt_ms: None, snapshot_age_ms: 0 };
        assert_eq!(local.line(), "LOCAL");
        assert!(!local.alert());
        let mut list = DrawList::new();
        let mut z = 0;
        push_net_readout(&mut list, &Ui::reference(), &Theme::standard(), &remote, &mut z);
        let rect = list.find(HudElement::NetReadout).expect("readout").rect;
        assert!(rect.right() > 1900.0 && rect.y > 40.0 && rect.y < 96.0, "{rect:?}");
    }
}
