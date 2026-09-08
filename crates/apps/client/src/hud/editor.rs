//! The HUD editor's overlay (interface program H21): a frame and a name over every movable
//! instrument, the hovered one lit, the dragged one in the lamp, and a footer that says what
//! the hands do. The instruments underneath draw as they will look — the editor is a layer
//! over the living HUD, not a mock of it.

use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::elements::{EditorPart, HudElement};
use super::layout::{Instrument, instrument_of};
use crate::ui_strings::battle as words;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditorModel {
    pub hovered: Option<Instrument>,
    pub dragging: Option<Instrument>,
}

/// The frame of every instrument on `list`: the union of its elements' rectangles (legacy
/// payloads have none; the minimap's plate stands in for its overlay).
pub fn instrument_frames(list: &DrawList<HudElement>) -> Vec<(Instrument, Rect)> {
    let mut frames: Vec<(Instrument, Rect)> = Vec::new();
    for element in list.iter() {
        let Some(instrument) = instrument_of(element.id) else { continue };
        if element.rect.is_empty() {
            continue;
        }
        match frames.iter_mut().find(|(i, _)| *i == instrument) {
            Some((_, rect)) => *rect = rect.union(&element.rect),
            None => frames.push((instrument, element.rect)),
        }
    }
    frames
}

pub(crate) fn push_editor(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    frames: &[(Instrument, Rect)],
    model: &EditorModel,
    z: &mut i16,
) {
    let enamel = theme.plates.enamel_black;
    for (instrument, frame) in frames {
        let lit = model.dragging == Some(*instrument) || model.hovered == Some(*instrument);
        let frame = frame.inset(-ui.px(4.0));
        list.push(
            Element::new(
                HudElement::Editor(EditorPart::Frame(*instrument)),
                frame,
                Payload::Glass {
                    radius_u: 2.0,
                    phase: 0.35,
                    color: if lit {
                        [theme.lamp[0], theme.lamp[1], theme.lamp[2], 0.22]
                    } else {
                        [enamel.color[0], enamel.color[1], enamel.color[2], 0.18]
                    },
                },
            )
            .z(*z),
        );
        *z += 1;
        list.push(
            Element::new(
                HudElement::Editor(EditorPart::Label(*instrument)),
                Rect::new(
                    frame.x + ui.px(6.0),
                    frame.y - ui.px(20.0),
                    frame.w.max(ui.px(160.0)),
                    ui.px(18.0),
                ),
                Payload::Text {
                    text: instrument.name().to_string(),
                    style: Style::VALUE_STRONG,
                    size_u: 16.0,
                    align: Align::Left,
                    color: if lit { theme.lamp } else { theme.text.label },
                    digits: DigitMode::Proportional,
                },
            )
            .z(*z),
        );
        *z += 1;
    }
    list.push(
        Element::new(
            HudElement::Editor(EditorPart::Footer),
            ui.anchor(Anchor::Bottom, [820.0, 22.0], [0.0, 132.0]),
            Payload::Text {
                text: words::EDITOR_FOOTER.to_string(),
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
    use crate::hud::{build_battle_hud_list, demo};

    /// The overlay frames every instrument on the staged HUD, names it, lights the hovered
    /// one, and never frames the reticle.
    #[test]
    fn the_editor_frames_every_instrument_and_never_the_reticle() {
        let ui = Ui::reference();
        let mut model = demo::demo_model(false);
        model.kill_feed = Some(demo::demo_kill_feed());
        model.editor = Some(EditorModel { hovered: Some(Instrument::Minimap), dragging: None });
        let list = build_battle_hud_list(&model, &ui);
        let framed: Vec<Instrument> = list
            .iter()
            .filter_map(|e| match e.id {
                HudElement::Editor(EditorPart::Frame(i)) => Some(i),
                _ => None,
            })
            .collect();
        for instrument in [
            Instrument::TopBar,
            Instrument::Minimap,
            Instrument::DamagePanel,
            Instrument::Ammo,
            Instrument::HitLog,
            Instrument::KillFeed,
        ] {
            assert!(framed.contains(&instrument), "{instrument:?} framed: {framed:?}");
        }
        let minimap =
            list.find(HudElement::Editor(EditorPart::Frame(Instrument::Minimap))).expect("frame");
        let plate = list.find(HudElement::MinimapPlate).expect("plate").rect;
        assert!(minimap.rect.encloses(&plate));
        match &list
            .find(HudElement::Editor(EditorPart::Label(Instrument::Minimap)))
            .expect("label")
            .payload
        {
            Payload::Text { text, color, .. } => {
                assert_eq!(text, "MINIMAP");
                assert_eq!(*color, Theme::standard().lamp, "the hovered one is lit");
            }
            other => panic!("{other:?}"),
        }
        assert!(list.find(HudElement::Editor(EditorPart::Footer)).is_some());
        assert!(list.find(HudElement::Reticle).is_some(), "the reticle draws under the editor");
    }
}
