use renderer_api::HudVertex;

use super::{push_panel, theme};

pub(crate) const OUTCOME_VICTORY_COLOR: [f32; 4] = [0.38, 0.82, 0.36, 0.96];
pub(crate) const OUTCOME_DEFEAT_COLOR: [f32; 4] = [0.90, 0.24, 0.18, 0.96];
/// Neutral steel — a draw is neither triumph nor loss.
pub(crate) const OUTCOME_DRAW_COLOR: [f32; 4] = [0.78, 0.76, 0.66, 0.96];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleHudOutcome {
    Victory,
    Defeat,
    /// Mutual elimination or the battle clock running out.
    Draw,
}

pub(crate) fn push_battle_outcome(
    vertices: &mut Vec<HudVertex>,
    outcome: BattleHudOutcome,
    aspect: f32,
) {
    let (label, color) = match outcome {
        BattleHudOutcome::Victory => (crate::ui_strings::battle::VICTORY, OUTCOME_VICTORY_COLOR),
        BattleHudOutcome::Defeat => (crate::ui_strings::battle::DEFEAT, OUTCOME_DEFEAT_COLOR),
        BattleHudOutcome::Draw => (crate::ui_strings::battle::DRAW, OUTCOME_DRAW_COLOR),
    };
    push_panel(
        vertices,
        [0.0, 0.45],
        [0.22, 0.055],
        theme::CHAMFER_SLOT,
        aspect,
        theme::color::PANEL,
    );
    crate::hud::font::push_text(vertices, label, -0.105, 0.463, 0.052, aspect, color);
}
