//! Kill confirmation — the payoff moment the whole combat loop builds toward. When a vehicle the
//! player damaged is destroyed, a short expanding diamond flare plays around the reticle and the
//! stencil line "TARGET DESTROYED" holds beneath it, then everything is gone. One beat, no residue:
//! the wreck itself (smoke, tint) carries the long-term record.

use game_core::TankId;
use renderer_api::HudVertex;

use super::primitives::push_segment;
use super::theme;

/// How long the whole confirmation lives on screen.
pub(crate) const KILL_CONFIRM_TTL_S: f32 = 1.8;
/// The expanding-flare beat at the front of the confirmation.
const FLARE_S: f32 = 0.45;
/// Fade-out tail of the text at the end of the TTL.
const TEXT_FADE_S: f32 = 0.4;

/// Whether authoritative combat truth says the player dealt the lethal hit.
///
/// The server stamps `target_destroyed` at resolution time. This avoids crediting a player who
/// merely wounded the target shortly before somebody else's lethal hit.
pub(crate) fn player_scored_kill(events: &[game_core::DamageEvent], player: TankId) -> bool {
    events
        .iter()
        .any(|event| event.source == player && event.target != player && event.target_destroyed)
}

/// Draw the confirmation for a kill `age_s` old. Nothing draws outside the TTL.
pub(crate) fn push_kill_confirm(vertices: &mut Vec<HudVertex>, age_s: f32, aspect: f32) {
    if !(0.0..KILL_CONFIRM_TTL_S).contains(&age_s) {
        return;
    }
    // The flare: a diamond outline expanding out of the reticle and fading over one beat —
    // motion at the exact point of attention, without covering the sight picture.
    let t = (age_s / FLARE_S).clamp(0.0, 1.0);
    let flare_alpha = 1.0 - t;
    if flare_alpha > 0.0 {
        let radius = 0.030 + 0.075 * t;
        let color = theme::tagged(theme::color::ACCENT, 0.9 * flare_alpha);
        let points = [
            [0.0, radius],
            [radius / aspect, 0.0],
            [0.0, -radius],
            [-radius / aspect, 0.0],
            [0.0, radius],
        ];
        for pair in points.windows(2) {
            push_segment(vertices, pair[0], pair[1], 0.0016, color);
        }
    }
    // The stencil line: centred under the reticle (clear of the zoom readout), holding for the
    // whole TTL and fading out at the tail.
    let text_alpha = ((KILL_CONFIRM_TTL_S - age_s) / TEXT_FADE_S).clamp(0.0, 1.0);
    let text = crate::ui_strings::battle::TARGET_DESTROYED;
    let height = 0.045;
    let width = super::font::text_width(text, height, aspect);
    super::font::push_text(
        vertices,
        text,
        -width * 0.5,
        -0.21,
        height,
        aspect,
        theme::tagged(theme::color::ACCENT, 0.95 * text_alpha),
    );
}

#[cfg(test)]
mod tests {
    use game_core::{DamageCause, DamageEvent};

    use super::*;

    fn events(items: &[(u64, u64, bool)]) -> Vec<DamageEvent> {
        items
            .iter()
            .map(|&(source, target, target_destroyed)| DamageEvent {
                source: TankId(source),
                target: TankId(target),
                damage_hp: 120,
                penetrated: true,
                cause: DamageCause::Shell,
                target_destroyed,
                ..DamageEvent::default()
            })
            .collect()
    }

    #[test]
    fn the_players_authoritative_lethal_hit_scores_a_kill() {
        assert!(player_scored_kill(&events(&[(1, 2, true)]), TankId(1)));
    }

    #[test]
    fn a_nonlethal_hit_is_not_a_kill() {
        assert!(!player_scored_kill(&events(&[(1, 2, false)]), TankId(1)));
    }

    #[test]
    fn someone_elses_kill_or_no_event_is_not_the_players() {
        assert!(!player_scored_kill(&events(&[(3, 2, true)]), TankId(1)));
        assert!(!player_scored_kill(&[], TankId(1)));
    }

    #[test]
    fn a_prior_wound_does_not_steal_someone_elses_lethal_hit() {
        let same_delivery_window = events(&[(1, 2, false), (3, 2, true)]);
        assert!(!player_scored_kill(&same_delivery_window, TankId(1)));
        assert!(player_scored_kill(&same_delivery_window, TankId(3)));
    }

    #[test]
    fn the_confirmation_draws_inside_its_ttl_and_never_after() {
        let aspect = 16.0 / 9.0;
        let mut fresh = Vec::new();
        push_kill_confirm(&mut fresh, 0.1, aspect);
        assert!(!fresh.is_empty(), "a fresh kill must draw the flare and the text");

        let mut tail = Vec::new();
        push_kill_confirm(&mut tail, KILL_CONFIRM_TTL_S - 0.05, aspect);
        assert!(!tail.is_empty(), "the text still holds near the end of the TTL");
        assert!(tail.len() < fresh.len(), "the flare is over by the tail — only the text remains");

        let mut expired = Vec::new();
        push_kill_confirm(&mut expired, KILL_CONFIRM_TTL_S + 0.01, aspect);
        assert!(expired.is_empty(), "an expired confirmation draws nothing");
    }
}
