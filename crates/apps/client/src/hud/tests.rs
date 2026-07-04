use super::reticle_overlay::{
    RETICLE_BLOCKED, RETICLE_IMPACT, RETICLE_NEUTRAL, RETICLE_NO_PEN, RETICLE_PEN, RETICLE_RELOAD,
    RETICLE_RING,
};
use super::*;
use crate::hud::number::{
    FPS_COLOR, HP_COLOR, RELOAD_TIME_COLOR, SPEED_COLOR, TARGET_DISTANCE_COLOR,
};
use crate::hud::reticle::ReticleStatus;

fn vitals() -> HudVitals {
    HudVitals {
        hit_points: 1000,
        max_hit_points: 1000,
        reload_remaining_s: 0.0,
        reload_seconds: 5.0,
    }
}

fn reticle_at(status: ReticleStatus, hint: Option<super::reticle::PenetrationHint>) -> HudReticle {
    HudReticle {
        aim_clip: [0.0, 0.0],
        impact_clip: None,
        aim_radius_clip: 0.08,
        target_distance_m: None,
        status,
        penetration_hint: hint,
        reload_fraction: 1.0,
    }
}

fn hint(penetrates: bool) -> super::reticle::PenetrationHint {
    super::reticle::PenetrationHint {
        penetrates,
        shell_pen_mm: 200.0,
        armor_mm: 150.0,
        facing: game_core::ArmorFacing::HullFront,
    }
}

#[test]
fn a_blocked_shot_draws_the_broken_red_marker_and_nothing_neutral() {
    let hud = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        // Even with a pen hint present, BLOCKED must override: the shot will not arrive.
        Some(reticle_at(ReticleStatus::Blocked, Some(hint(true)))),
        0.0,
        0.0,
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
    );
    assert!(neutral.iter().any(|vertex| vertex.color == RETICLE_NEUTRAL));

    let pen = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(reticle_at(ReticleStatus::Clear, Some(hint(true)))),
        0.0,
        0.0,
    );
    assert!(pen.iter().any(|vertex| vertex.color == RETICLE_PEN));
    assert!(!pen.iter().any(|vertex| vertex.color == RETICLE_NEUTRAL), "one marker, one color");

    let bounce = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(reticle_at(ReticleStatus::Clear, Some(hint(false)))),
        0.0,
        0.0,
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
    );
    let early: Vec<_> = reloading.iter().filter(|v| v.color == RETICLE_RELOAD).collect();
    assert!(!early.is_empty(), "the arc draws while loading");

    let nearly = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(HudReticle { reload_fraction: 0.9, ..reticle_at(ReticleStatus::Clear, None) }),
        0.0,
        0.0,
    );
    let late = nearly.iter().filter(|v| v.color == RETICLE_RELOAD).count();
    assert!(late < early.len(), "the arc drains as the reload progresses");

    let ready = build_hud_with_reticle(
        vitals(),
        16.0 / 9.0,
        Some(reticle_at(ReticleStatus::Clear, None)),
        0.0,
        0.0,
    );
    assert!(
        !ready.iter().any(|v| v.color == RETICLE_RELOAD),
        "a ready gun draws nothing — silence is the signal"
    );
}

#[test]
fn fps_readout_draws_digits_in_the_top_right_only_when_positive() {
    let without = build_hud(vitals(), 16.0 / 9.0);
    assert!(!without.iter().any(|vertex| vertex.color == FPS_COLOR), "0 fps draws nothing");

    // More digits means strictly more segment geometry (same digit isolates digit count
    // from which-segments-are-lit). "8" lights 7 segments; "888" lights 21.
    let one_digit = build_hud_with_reticle(vitals(), 16.0 / 9.0, None, 8.0, 0.0);
    let three_digit = build_hud_with_reticle(vitals(), 16.0 / 9.0, None, 888.0, 0.0);
    let count = |hud: &[HudVertex]| hud.iter().filter(|v| v.color == FPS_COLOR).count();
    assert!(count(&one_digit) > 0, "fps digits should be drawn");
    assert!(count(&three_digit) > count(&one_digit), "888 needs more segments than 8");

    assert!(
        three_digit
            .iter()
            .filter(|v| v.color == FPS_COLOR)
            .all(|v| v.position[0] > 0.0 && v.position[1] > 0.0)
    );
}

#[test]
fn speed_readout_draws_vehicle_speed_in_bottom_left_only_when_moving() {
    let stopped = build_hud_with_reticle(vitals(), 16.0 / 9.0, None, 0.0, 0.0);
    assert!(!stopped.iter().any(|vertex| vertex.color == SPEED_COLOR), "0 km/h draws nothing");

    let moving = build_hud_with_reticle(vitals(), 16.0 / 9.0, None, 0.0, 42.0);
    let speed_vertices: Vec<_> =
        moving.iter().filter(|vertex| vertex.color == SPEED_COLOR).collect();

    assert!(!speed_vertices.is_empty(), "moving tank should draw speed digits");
    assert!(
        speed_vertices.iter().all(|v| v.position[0] < 0.0 && v.position[1] < 0.0),
        "speed readout should sit in the bottom-left quadrant"
    );
}

#[test]
fn hp_bar_draws_current_hit_points_on_top_left_bar() {
    let hud = build_hud(
        HudVitals {
            hit_points: 750,
            max_hit_points: 1000,
            reload_remaining_s: 0.0,
            reload_seconds: 5.0,
        },
        16.0 / 9.0,
    );
    let hp_vertices: Vec<_> = hud.iter().filter(|vertex| vertex.color == HP_COLOR).collect();

    assert!(!hp_vertices.is_empty(), "current HP should be drawn as digits");
    assert!(
        hp_vertices.iter().all(|v| v.position[0] < -0.45 && v.position[1] > 0.78),
        "HP digits should sit on the top-left HP bar"
    );
}

#[test]
fn reload_bar_draws_remaining_seconds_above_bottom_reload_bar() {
    let hud = build_hud(
        HudVitals {
            hit_points: 1000,
            max_hit_points: 1000,
            reload_remaining_s: 4.2,
            reload_seconds: 5.0,
        },
        16.0 / 9.0,
    );
    let reload_vertices: Vec<_> =
        hud.iter().filter(|vertex| vertex.color == RELOAD_TIME_COLOR).collect();

    assert!(!reload_vertices.is_empty(), "reload seconds should be drawn while reloading");
    assert!(
        reload_vertices.iter().all(|v| v.position[1] < -0.72),
        "reload digits should stay near the bottom reload bar"
    );
}

fn reticle_with_impact(aim_clip: [f32; 2], impact_clip: Option<[f32; 2]>) -> HudReticle {
    HudReticle {
        aim_clip,
        impact_clip,
        aim_radius_clip: 0.0,
        target_distance_m: None,
        status: ReticleStatus::Clear,
        penetration_hint: None,
        reload_fraction: 1.0,
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
            impact_clip: None,
            aim_radius_clip: 0.0,
            target_distance_m: Some(347.4),
            status: ReticleStatus::Clear,
            penetration_hint: None,
            reload_fraction: 1.0,
        }),
        0.0,
        0.0,
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
