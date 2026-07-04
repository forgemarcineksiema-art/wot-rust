//! Locks the reticle overlay: the single center marker, dispersion ring, reload arc, impact
//! marker, distance readout and hit confirm. Split from `hud/tests.rs` for the file budget.

use super::reticle::ReticleStatus;
use super::reticle_overlay::{
    RETICLE_BLOCKED, RETICLE_IMPACT, RETICLE_NEUTRAL, RETICLE_NO_PEN, RETICLE_PEN, RETICLE_RELOAD,
    RETICLE_RING,
};
use super::tests::{hint, reticle_at, vitals};
use super::*;
use crate::hud::number::TARGET_DISTANCE_COLOR;

#[test]
fn a_blocked_shot_draws_the_broken_red_marker_and_nothing_neutral() {
    let hud = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        // Even with a pen hint present, BLOCKED must override: the shot will not arrive.
        Some(reticle_at(ReticleStatus::Blocked, Some(hint(true)))),
        0.0,
        0.0,
        None,
    );

    assert!(hud.iter().any(|vertex| vertex.color == RETICLE_BLOCKED), "blocked form draws");
    assert!(!hud.iter().any(|vertex| vertex.color == RETICLE_NEUTRAL));
    assert!(!hud.iter().any(|vertex| vertex.color == RETICLE_PEN), "no pen color can lie over it");
}

#[test]
fn one_marker_owns_the_center_and_speaks_penetration_by_color() {
    let neutral = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(reticle_at(ReticleStatus::Clear, None)),
        0.0,
        0.0,
        None,
    );
    assert!(neutral.iter().any(|vertex| vertex.color == RETICLE_NEUTRAL));

    let pen = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(reticle_at(ReticleStatus::Clear, Some(hint(true)))),
        0.0,
        0.0,
        None,
    );
    assert!(pen.iter().any(|vertex| vertex.color == RETICLE_PEN));
    assert!(!pen.iter().any(|vertex| vertex.color == RETICLE_NEUTRAL), "one marker, one color");

    let bounce = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(reticle_at(ReticleStatus::Clear, Some(hint(false)))),
        0.0,
        0.0,
        None,
    );
    assert!(bounce.iter().any(|vertex| vertex.color == RETICLE_NO_PEN));
}

#[test]
fn the_dispersion_ring_is_continuous_geometry_around_the_aim() {
    let hud = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(HudReticle { aim_radius_clip: 0.10, ..reticle_at(ReticleStatus::Clear, None) }),
        0.0,
        0.0,
        None,
    );

    let ring: Vec<_> = hud.iter().filter(|vertex| vertex.color == RETICLE_RING).collect();
    // 40 segments x 6 vertices: a drawn circle, not a scatter of dots.
    assert_eq!(ring.len(), 240, "continuous ring geometry");
    assert!(ring.iter().any(|v| v.position[0] > 0.04), "ring reaches right of center");
    assert!(ring.iter().any(|v| v.position[1] < -0.09), "ring reaches below center");
}

#[test]
fn the_reload_arc_drains_at_the_reticle_and_vanishes_when_ready() {
    let reloading = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(HudReticle { reload_fraction: 0.25, ..reticle_at(ReticleStatus::Clear, None) }),
        0.0,
        0.0,
        None,
    );
    let early: Vec<_> = reloading.iter().filter(|v| v.color == RETICLE_RELOAD).collect();
    assert!(!early.is_empty(), "the arc draws while loading");

    let nearly = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(HudReticle { reload_fraction: 0.9, ..reticle_at(ReticleStatus::Clear, None) }),
        0.0,
        0.0,
        None,
    );
    let late = nearly.iter().filter(|v| v.color == RETICLE_RELOAD).count();
    assert!(late < early.len(), "the arc drains as the reload progresses");

    let ready = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(reticle_at(ReticleStatus::Clear, None)),
        0.0,
        0.0,
        None,
    );
    assert!(
        !ready.iter().any(|v| v.color == RETICLE_RELOAD),
        "a ready gun draws nothing — silence is the signal"
    );
}

pub(super) fn reticle_with_impact(aim_clip: [f32; 2], impact_clip: Option<[f32; 2]>) -> HudReticle {
    HudReticle {
        aim_clip,
        impact_clip,
        aim_radius_clip: 0.0,
        target_distance_m: None,
        status: ReticleStatus::Clear,
        penetration_hint: None,
        reload_fraction: 1.0,
        hit_confirm: None,
        show_penetration_numbers: false,
    }
}

#[test]
fn reticle_marks_the_real_impact_point_when_the_shell_drops_off_the_aim() {
    // A howitzer arc: the shell lands well below and right of where the mouse points.
    let impact = [0.35, -0.4];
    let hud = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(reticle_with_impact([0.0, 0.0], Some(impact))),
        0.0,
        0.0,
        None,
    );

    let impact_vertices: Vec<_> = hud.iter().filter(|v| v.color == RETICLE_IMPACT).collect();
    assert!(!impact_vertices.is_empty(), "a dropped impact should draw its own marker");
    assert!(
        impact_vertices.iter().all(|v| v.position[0] > 0.25 && v.position[1] < -0.30),
        "the impact marker should sit at the real landing point, not the aim point"
    );
}

#[test]
fn reticle_omits_the_impact_marker_when_it_sits_on_the_crosshair() {
    // High muzzle velocity: impact is essentially the aim point, so no redundant marker.
    let hud = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(reticle_with_impact([0.1, 0.1], Some([0.1, 0.105]))),
        0.0,
        0.0,
        None,
    );

    assert!(
        !hud.iter().any(|v| v.color == RETICLE_IMPACT),
        "an on-crosshair impact must not clutter the center with a second marker"
    );
}

#[test]
fn reticle_draws_target_distance_meters_near_aim_marker() {
    let hud = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(HudReticle {
            aim_clip: [0.20, 0.10],
            target_distance_m: Some(347.4),
            ..reticle_with_impact([0.20, 0.10], None)
        }),
        0.0,
        0.0,
        None,
    );
    let distance_vertices: Vec<_> =
        hud.iter().filter(|vertex| vertex.color == TARGET_DISTANCE_COLOR).collect();

    assert!(!distance_vertices.is_empty(), "target distance should be drawn as meters");
    assert!(
        distance_vertices
            .iter()
            .all(|v| v.position[0] > -0.05 && v.position[1] < 0.05 && v.position[1] > -0.20),
        "distance digits should sit just below and right of the aim reticle"
    );
}

#[test]
fn a_fresh_hit_flares_confirm_ticks_that_a_stale_one_no_longer_draws() {
    use super::reticle_readouts::HitConfirm;

    let confirm_color = [0.45, 1.0, 0.50, 0.95]; // pen ticks at full life
    let fresh = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(HudReticle {
            hit_confirm: Some(HitConfirm { age_s: 0.0, penetrated: true, ricocheted: false }),
            ..reticle_at(ReticleStatus::Clear, None)
        }),
        0.0,
        0.0,
        None,
    );
    assert!(fresh.iter().any(|v| v.color == confirm_color), "fresh pen hit flares green ticks");

    let stale = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(HudReticle {
            hit_confirm: Some(HitConfirm { age_s: 10.0, penetrated: true, ricocheted: false }),
            ..reticle_at(ReticleStatus::Clear, None)
        }),
        0.0,
        0.0,
        None,
    );
    assert!(
        !stale.iter().any(|v| v.color[1] == confirm_color[1] && v.color[0] == confirm_color[0]),
        "an aged confirm draws nothing"
    );
}

#[test]
fn pen_numbers_print_only_when_the_sniper_flag_is_set() {
    let base = HudReticle {
        target_distance_m: Some(300.0),
        ..reticle_at(ReticleStatus::Clear, Some(hint(true)))
    };
    let arcade = build_hud_with_reticle(vitals(), 16.0 / 9.0, Some(base), 0.0, 0.0, None);
    let sniper = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(HudReticle { show_penetration_numbers: true, ..base }),
        0.0,
        0.0,
        None,
    );
    // The sniper view draws strictly more glyph geometry: pen + armor mm join the readout.
    assert!(sniper.len() > arcade.len() + 30, "sniper adds the pen/armor mm readout");
}

#[test]
fn the_impact_marker_fades_in_near_the_merge_threshold_instead_of_popping() {
    // Just past the threshold: the X draws, but dimmer than its full-alpha color.
    let near = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(reticle_with_impact([0.0, 0.0], Some([0.0, 0.030]))),
        0.0,
        0.0,
        None,
    );
    let dimmed: Vec<_> = near
        .iter()
        .filter(|v| {
            v.color[0] == RETICLE_IMPACT[0]
                && v.color[3] > 0.0
                && v.color[3] < RETICLE_IMPACT[3] - 0.05
        })
        .collect();
    assert!(!dimmed.is_empty(), "near the threshold the X draws at partial alpha");

    let zoom = build_hud_with_reticle(vitals(), 16.0 / 9.0, None, 0.0, 0.0, Some(6.89));
    let zoom_glyphs = zoom.iter().filter(|v| v.color == number::ZOOM_COLOR).count();
    assert!(zoom_glyphs > 0, "sniper zoom factor prints under the reticle");
}
