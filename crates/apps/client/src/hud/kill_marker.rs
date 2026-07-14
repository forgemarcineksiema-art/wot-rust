//! Kill confirmation — the payoff moment the whole combat loop builds toward. When a vehicle the
//! player damaged is destroyed, a short expanding diamond flare plays around the reticle and the
//! stencil line "TARGET DESTROYED" holds beneath it, then everything is gone. One beat, no residue:
//! the wreck itself (smoke, tint) carries the long-term record.

use game_core::TankId;
use net::Snapshot;
use renderer_api::HudVertex;

use super::primitives::push_segment;
use super::theme;

/// How long the whole confirmation lives on screen.
pub(crate) const KILL_CONFIRM_TTL_S: f32 = 1.8;
/// The expanding-flare beat at the front of the confirmation.
const FLARE_S: f32 = 0.45;
/// Fade-out tail of the text at the end of the TTL.
const TEXT_FADE_S: f32 = 0.4;

/// Whether this snapshot contains the player finishing a vehicle off: a damage event from the
/// player whose target is a wreck NOW but was alive in the previous snapshot (a target first seen
/// already dead — absent from `previous` — still counts: the event and the fresh wreck arrived
/// together, so the kill is the player's).
pub(crate) fn player_scored_kill(
    previous: Option<&Snapshot>,
    snapshot: &Snapshot,
    player: TankId,
) -> bool {
    snapshot.damage_events.iter().any(|event| {
        event.source == player
            && event.target != player
            && snapshot
                .tanks
                .iter()
                .any(|tank| tank.tank_id == event.target && tank.hit_points == 0)
            && previous.is_none_or(|previous| {
                previous
                    .tanks
                    .iter()
                    .find(|tank| tank.tank_id == event.target)
                    .is_none_or(|tank| tank.hit_points > 0)
            })
    })
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
    use game_core::{DamageCause, DamageEvent, TeamId};
    use net::TankSnapshot;

    use super::*;

    fn snapshot(tanks: Vec<(u64, u32)>, events: Vec<(u64, u64)>) -> Snapshot {
        let spec = game_core::VehicleKind::T54_1951.spec();
        Snapshot {
            server_tick: 1,
            tanks: tanks
                .into_iter()
                .map(|(id, hit_points)| TankSnapshot {
                    tank_id: TankId(id),
                    team: TeamId(if id == 1 { 1 } else { 2 }),
                    vehicle: spec.kind,
                    position: [id as f32, 0.0, 0.0],
                    yaw_rad: 0.0,
                    hull_pitch_rad: 0.0,
                    hull_roll_rad: 0.0,
                    turret_yaw_rad: 0.0,
                    turret_yaw_velocity_rad_s: 0.0,
                    gun_pitch_rad: 0.0,
                    hit_points,
                    reload_remaining_s: 0.0,
                    aim_dispersion_mrad: spec.gun.dispersion_mrad,
                    module_hit_points: spec.module_health.hit_points_by_slot(),
                    destroyed_modules_mask: 0,
                    track_damage_mask: 0,
                    track_hp: [game_core::TRACK_HP_MAX; 2],
                    ammo_counts: spec.ammo.counts,
                    selected_ammo: spec.ammo.initial_selected,
                    spotted_by_teams_mask: u8::MAX,
                    armor_breaches: Default::default(),
                    track_break_t: [None, None],
                    engine_fire: false,
                    fuel_fire: false,
                })
                .collect(),
            damage_events: events
                .into_iter()
                .map(|(source, target)| DamageEvent {
                    source: TankId(source),
                    target: TankId(target),
                    damage_hp: 120,
                    penetrated: true,
                    cause: DamageCause::Shell,
                    ..DamageEvent::default()
                })
                .collect(),
            ..Snapshot::default()
        }
    }

    #[test]
    fn a_target_the_player_just_wrecked_scores_a_kill() {
        let previous = snapshot(vec![(1, 1000), (2, 120)], vec![]);
        let current = snapshot(vec![(1, 1000), (2, 0)], vec![(1, 2)]);
        assert!(player_scored_kill(Some(&previous), &current, TankId(1)));
    }

    #[test]
    fn hitting_an_already_dead_wreck_is_not_a_kill() {
        let previous = snapshot(vec![(1, 1000), (2, 0)], vec![]);
        let current = snapshot(vec![(1, 1000), (2, 0)], vec![(1, 2)]);
        assert!(!player_scored_kill(Some(&previous), &current, TankId(1)));
    }

    #[test]
    fn someone_elses_kill_or_no_event_is_not_the_players() {
        let previous = snapshot(vec![(1, 1000), (2, 120), (3, 500)], vec![]);
        let killed_by_other = snapshot(vec![(1, 1000), (2, 0), (3, 500)], vec![(3, 2)]);
        assert!(!player_scored_kill(Some(&previous), &killed_by_other, TankId(1)));

        let no_events = snapshot(vec![(1, 1000), (2, 0), (3, 500)], vec![]);
        assert!(!player_scored_kill(Some(&previous), &no_events, TankId(1)));
    }

    #[test]
    fn a_first_sighted_wreck_with_the_players_event_still_counts() {
        // The target was never in a previous snapshot (unspotted until the killing blow).
        let current = snapshot(vec![(1, 1000), (2, 0)], vec![(1, 2)]);
        assert!(player_scored_kill(None, &current, TankId(1)));
        let previous_without_target = snapshot(vec![(1, 1000)], vec![]);
        assert!(player_scored_kill(Some(&previous_without_target), &current, TankId(1)));
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
