//! The tech tree on the draw list (interface program G12): columns by TIER, ascending left to
//! right (the R lane's class bands will replace the key), a row per LINE — a nation's class —
//! with the nation's word and the class's glyph at its head, an EDGE from every hull to the
//! one that follows it in its line, and on every node what it is (glyph, name, tier) and what
//! follows it — and nothing it cannot know: no XP, no research, no locks. The selected hull's
//! node is the focused one; BACK is a plain slot (G7).

use game_core::{Nation, VehicleClass, VehicleKind, tier_roman};
use ui_kit::draw_list::{Align, DigitMode, DrawList, Payload, WidgetState};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::GarageState;
use super::elements::GarageElement as E;
use super::filter::tiers_present;
use super::screen::{plate, put, put_control, text};
use crate::hud::icons::HudIcon;
use crate::ui_strings::garage as words;

// The panel under the bar, above the carousel.
const PANEL_SIZE_U: [f32; 2] = [1560.0, 720.0];
const PANEL_TOP_U: f32 = 140.0;
const PAD_U: f32 = 20.0;
// The line labels down the left, then a column per tier.
const LINE_LABEL_W_U: f32 = 280.0;
const COLUMN_PITCH_U: f32 = 300.0;
const HEADER_TOP_U: f32 = 24.0;
const HEADER_H_U: f32 = 32.0;
const ROWS_TOP_U: f32 = 84.0;
const ROW_PITCH_U: f32 = 100.0;
const NODE_SIZE_U: [f32; 2] = [220.0, 76.0];
const NODE_ICON_U: f32 = 28.0;
const EDGE_H_U: f32 = 2.0;
// BACK: a plain slot in the panel's top-right corner.
const BACK_SIZE_U: [f32; 2] = [160.0, 40.0];

/// A line: a nation's class, the hulls of the roster on it ascending by tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Line {
    pub nation: Nation,
    pub class: VehicleClass,
    pub hulls: Vec<VehicleKind>,
}

/// The lines the roster has, nation-major then class-major, empty ones skipped.
pub(super) fn tree_lines() -> Vec<Line> {
    let mut lines = Vec::new();
    for nation in Nation::ALL {
        for class in VehicleClass::ALL {
            let mut hulls: Vec<VehicleKind> = VehicleKind::PLAYABLE
                .into_iter()
                .filter(|kind| kind.nation() == nation && kind.class() == class)
                .collect();
            if hulls.is_empty() {
                continue;
            }
            hulls.sort_by_key(|kind| kind.tier());
            lines.push(Line { nation, class, hulls });
        }
    }
    lines
}

/// The hull that follows `kind` in its line: the next tier up of the same nation and class,
/// gaps allowed; `None` at the end of a line — which the node then does not pretend to know.
pub(super) fn line_successor(kind: VehicleKind) -> Option<VehicleKind> {
    VehicleKind::PLAYABLE
        .into_iter()
        .filter(|other| {
            other.nation() == kind.nation()
                && other.class() == kind.class()
                && other.tier() > kind.tier()
        })
        .min_by_key(|other| other.tier())
}

fn tree_roster_index(kind: VehicleKind) -> u8 {
    VehicleKind::PLAYABLE.iter().position(|k| *k == kind).expect("the roster is playable") as u8
}

/// The tree's elements: the panel, the tier headers, the line heads, the nodes and their
/// edges, BACK. Laid out in `u` from the panel's top-left.
pub(super) fn push_tree(list: &mut DrawList<E>, ui: &Ui, theme: &Theme, state: &GarageState) {
    let panel = ui.anchor(Anchor::Top, PANEL_SIZE_U, [0.0, PANEL_TOP_U]);
    put(list, E::TreePanel, panel, plate(theme, theme.plates.steel_brushed, 4.0));
    let pad = ui.px(PAD_U);
    let tiers = tiers_present();
    let column_x =
        |column: usize| panel.x + pad + ui.px(LINE_LABEL_W_U + column as f32 * COLUMN_PITCH_U);
    let column_of =
        |tier: u8| tiers.iter().position(|t| *t == tier).expect("a tier the roster has");
    // The key: LINE over the heads, a tier over every column, in the lamp.
    let header_y = panel.y + ui.px(HEADER_TOP_U);
    put(
        list,
        E::TreeLinesLabel,
        Rect::new(panel.x + pad, header_y, ui.px(LINE_LABEL_W_U), ui.px(HEADER_H_U)),
        text(
            words::TREE_LINE,
            Style::VALUE_STRONG,
            18.0,
            Align::Left,
            theme.text.label_dim,
            DigitMode::Proportional,
        ),
    );
    for (column, tier) in tiers.iter().enumerate() {
        put(
            list,
            E::TreeTierLabel(column as u8),
            Rect::new(column_x(column), header_y, ui.px(NODE_SIZE_U[0]), ui.px(HEADER_H_U)),
            text(
                &format!("{} {}", words::TREE_TIER, tier_roman(*tier)),
                Style::BANNER,
                Style::STENCIL_FLOOR_U,
                Align::Center,
                theme.lamp,
                DigitMode::Proportional,
            ),
        );
    }
    let selected = state.selected_vehicle();
    for (row, line) in tree_lines().iter().enumerate() {
        let row_top = panel.y + ui.px(ROWS_TOP_U + row as f32 * ROW_PITCH_U);
        let node_h = ui.px(NODE_SIZE_U[1]);
        // The line's head: its nation in the nation's colour, its class glyph and word.
        let head = Rect::new(panel.x + pad, row_top, ui.px(LINE_LABEL_W_U), node_h);
        let icon = Rect::new(
            head.x,
            head.y + (node_h - ui.px(NODE_ICON_U)) * 0.5,
            ui.px(NODE_ICON_U),
            ui.px(NODE_ICON_U),
        );
        put(
            list,
            E::TreeLineIcon(row as u8),
            icon,
            Payload::Icon { icon: HudIcon::for_class(line.class), color: theme.text.label },
        );
        // The nation is a swatch beside the head, and the words wear the value's ink: the
        // nation colours were made for the carousel's cells and sank into the panel here.
        let c = line.nation.color();
        put(
            list,
            E::TreeLineSwatch(row as u8),
            Rect::new(head.x - ui.px(12.0), head.y + ui.px(8.0), ui.px(6.0), node_h - ui.px(16.0)),
            Payload::Bar { frac: 0.0, fill: theme.text.value, back: [c[0], c[1], c[2], 0.95] },
        );
        put(
            list,
            E::TreeLineLabel(row as u8),
            Rect::new(
                icon.right() + ui.px(12.0),
                head.y,
                head.w - ui.px(NODE_ICON_U + 12.0),
                node_h,
            ),
            text(
                &format!(
                    "{} \u{b7} {}",
                    line.nation.label().to_uppercase(),
                    line.class.label().to_uppercase()
                ),
                Style::VALUE_STRONG,
                18.0,
                Align::Left,
                theme.text.value,
                DigitMode::Proportional,
            ),
        );
        for kind in &line.hulls {
            let index = tree_roster_index(*kind);
            let node =
                Rect::new(column_x(column_of(kind.tier())), row_top, ui.px(NODE_SIZE_U[0]), node_h);
            let focused = *kind == selected;
            let painted = theme.plates.steel_painted;
            put_control(
                list,
                E::TreeNode(index),
                node,
                Payload::Plate {
                    tile: painted.tile,
                    radius_u: 2.0,
                    bevel_u: 1.0,
                    color: painted.color,
                },
                if focused { WidgetState::Focused } else { WidgetState::Idle },
            );
            let inner = node.inset(ui.px(8.0));
            put(
                list,
                E::TreeNodeIcon(index),
                Rect::new(inner.x, inner.y, ui.px(NODE_ICON_U), ui.px(NODE_ICON_U)),
                Payload::Icon { icon: HudIcon::for_class(kind.class()), color: theme.text.label },
            );
            let name_x = inner.x + ui.px(NODE_ICON_U + 8.0);
            put(
                list,
                E::TreeNodeName(index),
                Rect::new(name_x, inner.y, inner.right() - name_x, ui.px(NODE_ICON_U)),
                text(
                    kind.short_name(),
                    Style::VALUE_STRONG,
                    20.0,
                    Align::Left,
                    if focused { theme.lamp } else { theme.text.value },
                    DigitMode::Proportional,
                ),
            );
            put(
                list,
                E::TreeNodeTier(index),
                Rect::new(name_x, inner.y, inner.right() - name_x, ui.px(NODE_ICON_U)),
                text(
                    tier_roman(kind.tier()),
                    Style::VALUE,
                    16.0,
                    Align::Right,
                    theme.text.label,
                    DigitMode::Proportional,
                ),
            );
            // What follows it — and the edge to it; the end of a line says nothing.
            if let Some(next) = line_successor(*kind) {
                put(
                    list,
                    E::TreeNodeNext(index),
                    Rect::new(inner.x, inner.bottom() - ui.px(18.0), inner.w, ui.px(18.0)),
                    text(
                        &format!("{} \u{b7} {}", words::TREE_NEXT, next.short_name()),
                        Style::VALUE,
                        16.0,
                        Align::Left,
                        theme.lamp,
                        DigitMode::Proportional,
                    ),
                );
                let next_x = column_x(column_of(next.tier()));
                let edge = Rect::new(
                    node.right(),
                    node.y + (node_h - ui.px(EDGE_H_U)) * 0.5,
                    next_x - node.right(),
                    ui.px(EDGE_H_U),
                );
                put(
                    list,
                    E::TreeEdge(index),
                    edge,
                    Payload::Bar {
                        frac: 0.0,
                        fill: theme.text.label_dim,
                        back: theme.text.label_dim,
                    },
                );
            }
        }
    }
    // BACK: a way out, not a commit — a plain slot in the corner (G7).
    let back = Rect::new(
        panel.right() - pad - ui.px(BACK_SIZE_U[0]),
        panel.y + ui.px(HEADER_TOP_U - 4.0),
        ui.px(BACK_SIZE_U[0]),
        ui.px(BACK_SIZE_U[1]),
    );
    put_control(
        list,
        E::TreeBack,
        back,
        plate(theme, theme.plates.steel_painted, 2.0),
        WidgetState::Idle,
    );
    put(
        list,
        E::TreeBackLabel,
        back,
        text(
            words::BACK,
            Style::VALUE_STRONG,
            20.0,
            Align::Center,
            theme.text.value,
            DigitMode::Proportional,
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every line is a nation's class with its hulls ascending by tier; a successor is the
    /// next tier up of the same line and nothing else; the last of a line has none.
    #[test]
    fn a_line_is_a_nations_class_ascending_by_tier_and_a_successor_is_its_next_tier() {
        let lines = tree_lines();
        assert_eq!(lines.iter().map(|l| l.hulls.len()).sum::<usize>(), VehicleKind::PLAYABLE.len());
        for line in &lines {
            assert!(
                line.hulls.iter().all(|k| k.nation() == line.nation && k.class() == line.class)
            );
            assert!(line.hulls.windows(2).all(|pair| pair[0].tier() < pair[1].tier()));
            for (i, kind) in line.hulls.iter().enumerate() {
                assert_eq!(line_successor(*kind), line.hulls.get(i + 1).copied(), "{kind:?}");
            }
        }
        assert!(lines.iter().any(|l| l.hulls.len() >= 2), "the roster has a line with an edge");
    }
}
