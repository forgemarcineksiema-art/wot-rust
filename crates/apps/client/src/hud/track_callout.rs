//! Track feedback: a center-top callout the moment the PLAYER's own track is damaged or thrown,
//! plus a re-seat progress bar while a side is down. This is the "what happened, where, and when
//! does it come back" the old HUD never gave — a clean track break deals 0 HP, so the damage log
//! swallowed it silently and the only tell was the hull suddenly handling wrong.

use game_core::{DamageEvent, TankId, TrackSide};
use renderer_api::HudVertex;

use super::primitives::push_bar;
use super::theme::color as theme;

/// Seconds the callout stays up before fading out.
pub(crate) const CALLOUT_TTL_S: f32 = 2.6;
/// The re-seat window a broken side counts through (mirrors `sim::TRACK_REPAIR_S`); the bar fills
/// over exactly this long.
const RESEAT_S: f32 = sim::TRACK_REPAIR_S;

const CALLOUT_Y: f32 = 0.42;
const CALLOUT_TEXT_H: f32 = 0.044;
const BAR_Y: f32 = 0.35;
const BAR_HALF: [f32; 2] = [0.11, 0.010];
const BAR_GAP: f32 = 0.015;

/// Thrown-track red and damaged-track amber — reuse the instrument palette's signal/accent so the
/// callout reads in the same language as the rest of the HUD.
const BROKE_COLOR: [f32; 4] = theme::SIGNAL;
const DAMAGED_COLOR: [f32; 4] = theme::ACCENT;

/// App-side store: the latest callout (fading) plus per-side "seconds this side has been thrown",
/// which drives the re-seat bar. Fed from the snapshot ingest and ticked on the render clock.
#[derive(Debug, Default)]
pub(crate) struct TrackFeedback {
    callout: Option<Callout>,
    left_down_s: Option<f32>,
    right_down_s: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
struct Callout {
    broke: bool,
    side: TrackSide,
    age_s: f32,
}

impl TrackFeedback {
    /// Raise a callout for any track the player TOOK this snapshot. A fresh throw always wins over
    /// a lingering mere-damage callout; a second damage does not stomp a still-fresh throw.
    pub(crate) fn ingest(&mut self, events: &[DamageEvent], player: TankId) {
        for event in events {
            if event.target != player {
                continue;
            }
            let Some(hit) = event.track_hit else { continue };
            let supersede = self.callout.is_none_or(|current| hit.broke || !current.broke);
            if supersede {
                self.callout = Some(Callout { broke: hit.broke, side: hit.side, age_s: 0.0 });
            }
        }
    }

    /// Watch the player's own broken sides (from the replicated broken mask) to run the re-seat
    /// bars: a side newly down starts its clock, a re-seated side clears it.
    pub(crate) fn sync_player_broken(&mut self, left_broken: bool, right_broken: bool) {
        set_down(&mut self.left_down_s, left_broken);
        set_down(&mut self.right_down_s, right_broken);
    }

    pub(crate) fn tick(&mut self, dt: f32) {
        if let Some(callout) = &mut self.callout {
            callout.age_s += dt;
            if callout.age_s >= CALLOUT_TTL_S {
                self.callout = None;
            }
        }
        for seconds in [&mut self.left_down_s, &mut self.right_down_s].into_iter().flatten() {
            *seconds += dt;
        }
    }

    /// Snapshot the drawable state for one frame.
    pub(crate) fn model(&self) -> TrackFeedbackModel {
        let progress = |down: Option<f32>| down.map(|s| (s / RESEAT_S).clamp(0.0, 1.0));
        TrackFeedbackModel {
            callout: self
                .callout
                .map(|c| CalloutView { broke: c.broke, side: c.side, age_s: c.age_s }),
            reseat: [progress(self.left_down_s), progress(self.right_down_s)],
        }
    }
}

fn set_down(clock: &mut Option<f32>, broken: bool) {
    match (broken, *clock) {
        (true, None) => *clock = Some(0.0),
        (false, _) => *clock = None,
        (true, Some(_)) => {}
    }
}

/// The drawable track feedback for one HUD frame (lives in `BattleHudModel`). Empty by default so
/// every non-battle HUD path draws nothing.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TrackFeedbackModel {
    pub callout: Option<CalloutView>,
    /// Re-seat progress `0..1` per side `[left, right]` while that side is thrown; `None` = up.
    pub reseat: [Option<f32>; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalloutView {
    pub broke: bool,
    pub side: TrackSide,
    pub age_s: f32,
}

/// Draw the callout line and any active re-seat bars.
pub(crate) fn push_track_callout(vertices: &mut Vec<HudVertex>, model: &TrackFeedbackModel, aspect: f32) {
    if let Some(callout) = model.callout {
        let base = if callout.broke { BROKE_COLOR } else { DAMAGED_COLOR };
        let fade = ((CALLOUT_TTL_S - callout.age_s) / 0.6).clamp(0.0, 1.0);
        let mut color = base;
        color[3] *= fade;
        if color[3] > 0.0 {
            let side = match callout.side {
                TrackSide::Left => "L",
                TrackSide::Right => "R",
            };
            let label = if callout.broke { "TRACK DESTROYED" } else { "TRACK DAMAGED" };
            let text = format!("{label} {side}");
            let width = super::font::text_width(&text, CALLOUT_TEXT_H, aspect);
            super::font::push_text(vertices, &text, -width * 0.5, CALLOUT_Y, CALLOUT_TEXT_H, aspect, color);
        }
    }

    // A re-seat bar per thrown side, stacked under the callout: left over right.
    let mut row = 0;
    for (index, progress) in model.reseat.iter().enumerate() {
        let Some(frac) = progress else { continue };
        let y = BAR_Y - row as f32 * (BAR_HALF[1] * 2.0 + BAR_GAP);
        let mut fill = theme::ACCENT;
        // The bar brightens as it nears re-seat; a just-broken bar reads dimmer.
        fill[3] *= 0.55 + 0.45 * frac;
        push_bar(vertices, [-BAR_HALF[0], y], BAR_HALF, *frac, fill);
        // A side tag on the left of its bar so two down tracks are told apart.
        let tag = if index == 0 { "L" } else { "R" };
        super::font::push_text(vertices, tag, -BAR_HALF[0] - 0.045, y + 0.014, 0.026, aspect, theme::TEXT_DIM);
        row += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{DamageEvent, TrackHit};

    fn track_event(target: u64, side: TrackSide, broke: bool) -> DamageEvent {
        DamageEvent {
            target: TankId(target),
            track_hit: Some(TrackHit { side, broke }),
            ..Default::default()
        }
    }

    #[test]
    fn a_taken_throw_raises_a_destroyed_callout_over_a_damage_one() {
        let mut fb = TrackFeedback::default();
        fb.ingest(&[track_event(1, TrackSide::Left, false)], TankId(1));
        assert!(fb.model().callout.is_some_and(|c| !c.broke), "damage callout first");
        fb.ingest(&[track_event(1, TrackSide::Left, true)], TankId(1));
        assert!(fb.model().callout.is_some_and(|c| c.broke), "a throw supersedes the damage callout");
    }

    #[test]
    fn other_tanks_track_hits_do_not_call_out_for_the_player() {
        let mut fb = TrackFeedback::default();
        fb.ingest(&[track_event(2, TrackSide::Left, true)], TankId(1));
        assert!(fb.model().callout.is_none(), "only the player's own tracks call out");
    }

    #[test]
    fn a_broken_side_runs_a_reseat_bar_that_clears_on_repair() {
        let mut fb = TrackFeedback::default();
        fb.sync_player_broken(true, false);
        fb.tick(RESEAT_S * 0.5);
        let mid = fb.model().reseat[0].expect("left bar runs while down");
        assert!((mid - 0.5).abs() < 0.05, "bar tracks the re-seat window: {mid}");
        assert!(fb.model().reseat[1].is_none(), "the whole side shows no bar");
        // Re-seated: the bar clears.
        fb.sync_player_broken(false, false);
        assert!(fb.model().reseat[0].is_none(), "a re-seated side drops its bar");
    }

    #[test]
    fn the_callout_fades_out_after_its_ttl() {
        let mut fb = TrackFeedback::default();
        fb.ingest(&[track_event(1, TrackSide::Right, true)], TankId(1));
        fb.tick(CALLOUT_TTL_S + 0.1);
        assert!(fb.model().callout.is_none(), "the callout ages out");
    }
}
