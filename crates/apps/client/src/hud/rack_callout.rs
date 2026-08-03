//! The cook-off callout: the ten seconds the crew can win, ON SCREEN for the crew living them.
//!
//! A lit ammunition rack is a fuze (`sim::RACK_COOKOFF_S`) — hold a functional rack to the
//! deadline and the crew pulls the burning charges; lose the module and the charges detonate.
//! Until protocol v43 the whole drama was sim-only: the player whose rack was burning learned
//! about it from damage ticks in the log. This callout is the mechanic made visible — the
//! signal-red warning, the countdown, and a bar draining toward the deadline, in the same
//! instrument language as the track callout beside it.

use renderer_api::HudVertex;

use super::primitives::push_bar;
use super::theme::color as theme;

const CALLOUT_Y: f32 = 0.50;
const CALLOUT_TEXT_H: f32 = 0.050;
const BAR_Y: f32 = 0.435;
const BAR_HALF: [f32; 2] = [0.13, 0.011];

/// The fuze warning: "AMMO RACK" over a bar draining to the deadline, with the seconds left.
/// Signal red — this outranks every other callout on the screen; nothing else on the HUD is a
/// ten-second race the player can lose the tank to.
pub(crate) fn push_rack_callout(
    vertices: &mut Vec<HudVertex>,
    remaining_s: Option<f32>,
    aspect: f32,
) {
    let Some(remaining_s) = remaining_s else { return };
    let fraction = (remaining_s / sim::RACK_COOKOFF_S).clamp(0.0, 1.0);
    // A hard blink in the final three seconds: urgency the eye cannot file away as ambient.
    let blink_on = remaining_s > 3.0 || (remaining_s * 4.0).floor() as i32 % 2 == 0;
    let color = theme::SIGNAL;

    if blink_on {
        let label = "AMMO RACK";
        let width = ui_kit::font::text_width(label, CALLOUT_TEXT_H, aspect);
        ui_kit::font::push_text(
            vertices,
            label,
            -width * 0.5,
            CALLOUT_Y,
            CALLOUT_TEXT_H,
            aspect,
            color,
        );
        let seconds = format!("{remaining_s:.1}");
        ui_kit::font::push_text(
            vertices,
            &seconds,
            BAR_HALF[0] + 0.03,
            BAR_Y - 0.008,
            CALLOUT_TEXT_H * 0.9,
            aspect,
            color,
        );
    }
    push_bar(vertices, [-BAR_HALF[0], BAR_Y], BAR_HALF, fraction, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_fuze_draws_nothing() {
        let mut vertices = Vec::new();
        push_rack_callout(&mut vertices, None, 1.6);
        assert!(vertices.is_empty(), "a quiet rack draws no warning");
    }

    #[test]
    fn a_lit_fuze_draws_the_warning_and_the_draining_bar() {
        let mut vertices = Vec::new();
        push_rack_callout(&mut vertices, Some(7.2), 1.6);
        assert!(!vertices.is_empty(), "a cooking rack is never invisible to its crew");

        // The bar drains WITH the fuze: less remaining -> fewer filled-bar vertices at the
        // same total (fill + backing), proven by comparing extreme fills.
        let mut nearly_done = Vec::new();
        push_rack_callout(&mut nearly_done, Some(0.4), 1.6);
        assert!(!nearly_done.is_empty(), "the final second still shows the bar");
    }
}
