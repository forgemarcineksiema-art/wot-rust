//! The in-battle escape menu: ESC raises a two-choice modal — leave for the garage, or stay.
//!
//! Two rules shape it. **The battle does not pause**: this is an authoritative server sim with
//! six other crews in it, so the world keeps running while the modal is up. The honest
//! consequence is that the hull coasts to a stop (the driving keys are released when the menu
//! opens, see `app::input`) instead of driving on blind — standing still is a cost the player
//! accepts by opening the menu, and it is a cost they can SEE. **Geometry lives here once**: the
//! same rects feed the drawing and the hit test, so what is lit under the cursor is exactly what
//! a click activates.

use renderer_api::HudVertex;

use super::primitives::in_rect;
use super::{font, push_panel, push_quad, theme};

/// Full-screen wash that pushes the battle back so the modal reads as modal.
const SCRIM_COLOR: [f32; 4] = [0.02, 0.025, 0.03, 0.55];
const PANEL_CENTER: [f32; 2] = [0.0, 0.0];
const PANEL_HALF: [f32; 2] = [0.30, 0.175];
const TITLE_HEIGHT: f32 = 0.045;
const TITLE_TOP_Y: f32 = 0.128;
/// Rule under the title, in the garage's panel idiom.
const RULE_Y: f32 = 0.058;
const RULE_HALF_X: f32 = 0.255;

pub(crate) const BUTTON_HALF: [f32; 2] = [0.235, 0.042];
pub(crate) const EXIT_CENTER: [f32; 2] = [0.0, -0.005];
pub(crate) const STAY_CENTER: [f32; 2] = [0.0, -0.108];
const BUTTON_LABEL_HEIGHT: f32 = 0.036;

/// Which button the cursor is over. The menu is only ever two choices, so an enum beats an index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseMenuButton {
    /// Leave the battle for the garage.
    ExitToGarage,
    /// Dismiss the menu and go back to driving.
    Stay,
}

/// What the battle HUD needs to draw the menu. `None` on [`super::BattleHudModel`] means the
/// menu is closed — the model carries no "open" flag, so a closed menu cannot draw by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PauseMenuModel {
    /// The button under the cursor, lit for the player.
    pub hovered: Option<PauseMenuButton>,
}

/// The button at a clip-space point, or `None` between them. Shares its rects with the drawing
/// below — a layout edit moves the picture and the click target together, never one of them.
pub(crate) fn button_at(point: [f32; 2]) -> Option<PauseMenuButton> {
    if in_rect(point, EXIT_CENTER, BUTTON_HALF) {
        return Some(PauseMenuButton::ExitToGarage);
    }
    if in_rect(point, STAY_CENTER, BUTTON_HALF) {
        return Some(PauseMenuButton::Stay);
    }
    None
}

pub(crate) fn push_pause_menu(vertices: &mut Vec<HudVertex>, model: &PauseMenuModel, aspect: f32) {
    // The scrim covers the whole viewport, so it is stated in clip space directly rather than
    // through the aspect-corrected panel helper.
    push_quad(vertices, [0.0, 0.0], [1.0, 1.0], SCRIM_COLOR);
    push_panel(
        vertices,
        PANEL_CENTER,
        PANEL_HALF,
        theme::CHAMFER_PANEL,
        aspect,
        theme::color::PANEL,
    );

    let title = crate::ui_strings::battle::PAUSE_TITLE;
    let title_width = font::text_width(title, TITLE_HEIGHT, aspect);
    font::push_text(
        vertices,
        title,
        -title_width * 0.5,
        TITLE_TOP_Y,
        TITLE_HEIGHT,
        aspect,
        theme::color::TEXT,
    );
    push_quad(
        vertices,
        [0.0, RULE_Y],
        [RULE_HALF_X, theme::HAIRLINE_THICKNESS * 0.5],
        theme::color::HAIRLINE,
    );

    // Leaving is the destructive choice, so it carries the commit/danger red the garage's BATTLE
    // button uses; staying is an ordinary slot. Neither is pre-selected: this is a decision the
    // player makes, not one the UI nudges.
    push_button(
        vertices,
        EXIT_CENTER,
        theme::color::SIGNAL,
        crate::ui_strings::battle::PAUSE_EXIT_TO_GARAGE,
        model.hovered == Some(PauseMenuButton::ExitToGarage),
        aspect,
    );
    push_button(
        vertices,
        STAY_CENTER,
        theme::color::SLOT,
        crate::ui_strings::battle::PAUSE_STAY,
        model.hovered == Some(PauseMenuButton::Stay),
        aspect,
    );
}

fn push_button(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    fill: [f32; 4],
    label: &str,
    hovered: bool,
    aspect: f32,
) {
    push_panel(vertices, center, BUTTON_HALF, theme::CHAMFER_SLOT, aspect, fill);
    if hovered {
        push_quad(vertices, center, BUTTON_HALF, theme::color::HOVER);
    }
    let width = font::text_width(label, BUTTON_LABEL_HEIGHT, aspect);
    font::push_text(
        vertices,
        label,
        center[0] - width * 0.5,
        center[1] + BUTTON_LABEL_HEIGHT * 0.5,
        BUTTON_LABEL_HEIGHT,
        aspect,
        theme::color::TEXT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The picture and the click target are the same rectangle. A cursor on a button's centre
    /// hits that button; the gap between them hits neither.
    #[test]
    fn the_hit_test_answers_the_rects_the_menu_draws() {
        assert_eq!(button_at(EXIT_CENTER), Some(PauseMenuButton::ExitToGarage));
        assert_eq!(button_at(STAY_CENTER), Some(PauseMenuButton::Stay));

        let gap_y = (EXIT_CENTER[1] + STAY_CENTER[1]) * 0.5;
        assert_eq!(button_at([0.0, gap_y]), None, "the gap between the buttons is not a button");
        assert_eq!(button_at([0.0, 0.9]), None, "the world outside the panel is not a button");
    }

    /// The two buttons must not overlap — an overlap would make one choice unreachable wherever
    /// it lost, and the player would blame the click, not the layout.
    #[test]
    fn the_buttons_do_not_overlap() {
        let gap = (EXIT_CENTER[1] - STAY_CENTER[1]).abs();
        assert!(
            gap > BUTTON_HALF[1] * 2.0,
            "buttons overlap: centres {gap} apart, each {} tall",
            BUTTON_HALF[1] * 2.0
        );
    }
}
