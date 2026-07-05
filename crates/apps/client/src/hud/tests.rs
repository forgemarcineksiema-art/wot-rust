//! Locks the frame-level HUD: bars, readouts, panels and the `BattleHudModel` contract. The
//! reticle overlay's own locks live in `hud/reticle_overlay_tests.rs` (shared helpers are here).

use super::*;
use crate::hud::number::{FPS_COLOR, HP_COLOR, RELOAD_TIME_COLOR, SPEED_COLOR};
use crate::hud::reticle::ReticleStatus;

pub(super) fn vitals() -> HudVitals {
    HudVitals {
        hit_points: 1000,
        max_hit_points: 1000,
        reload_remaining_s: 0.0,
        reload_seconds: 5.0,
    }
}

pub(super) fn reticle_at(
    status: ReticleStatus,
    hint: Option<super::reticle::PenetrationHint>,
) -> HudReticle {
    HudReticle {
        aim_clip: [0.0, 0.0],
        impact_clip: None,
        gun_clip: None,
        aim_radius_clip: 0.08,
        target_distance_m: None,
        status,
        penetration_hint: hint,
        reload_fraction: 1.0,
        hit_confirm: None,
        mode: super::reticle::ReticleMode::ThirdPerson,
    }
}

pub(super) fn sniper(reticle: HudReticle) -> HudReticle {
    HudReticle { mode: super::reticle::ReticleMode::Sniper, ..reticle }
}

pub(super) fn hint(penetrates: bool) -> super::reticle::PenetrationHint {
    super::reticle::PenetrationHint {
        penetrates,
        shell_pen_mm: 200.0,
        armor_mm: 150.0,
        facing: game_core::ArmorFacing::HullFront,
    }
}

#[test]
fn fps_readout_draws_digits_in_the_top_right_only_when_positive() {
    let without = build_hud(vitals(), 16.0 / 9.0);
    assert!(!without.iter().any(|vertex| vertex.color == FPS_COLOR), "0 fps draws nothing");

    // More digits means strictly more segment geometry (same digit isolates digit count
    // from which-segments-are-lit). "8" lights 7 segments; "888" lights 21.
    let one_digit = build_hud_with_reticle(vitals(), 16.0 / 9.0, None, 8.0, 0.0, None);
    let three_digit = build_hud_with_reticle(vitals(), 16.0 / 9.0, None, 888.0, 0.0, None);
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
    let stopped = build_hud_with_reticle(vitals(), 16.0 / 9.0, None, 0.0, 0.0, None);
    assert!(!stopped.iter().any(|vertex| vertex.color == SPEED_COLOR), "0 km/h draws nothing");

    let moving = build_hud_with_reticle(vitals(), 16.0 / 9.0, None, 0.0, 42.0, None);
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

#[test]
fn a_chamfered_panel_cuts_its_corners_and_stays_inside_its_rect() {
    let mut v = Vec::new();
    let (center, half, chamfer, aspect) =
        ([0.1_f32, -0.2_f32], [0.4_f32, 0.3_f32], 0.05_f32, 2.0_f32);
    push_panel(&mut v, center, half, chamfer, aspect, theme::color::PANEL);

    assert_eq!(v.len(), 18, "a chamfered panel is a 6-triangle fan");
    let inside = |p: [f32; 2]| {
        (p[0] - center[0]).abs() <= half[0] + 1e-6 && (p[1] - center[1]).abs() <= half[1] + 1e-6
    };
    assert!(v.iter().all(|vert| inside([vert.position[0], vert.position[1]])));

    // No vertex sits on an exact rect corner: the 45-degree cut removed all four.
    let corner = |p: [f32; 2]| {
        ((p[0] - center[0]).abs() - half[0]).abs() < 1e-6
            && ((p[1] - center[1]).abs() - half[1]).abs() < 1e-6
    };
    assert!(
        v.iter().all(|vert| !corner([vert.position[0], vert.position[1]])),
        "chamfer must cut every corner"
    );

    // The x cut is aspect-corrected: a vertex at the top edge starts chamfer/aspect in from the left.
    let top = center[1] + half[1];
    let leftmost_top = v
        .iter()
        .filter(|vert| (vert.position[1] - top).abs() < 1e-6)
        .map(|vert| vert.position[0])
        .fold(f32::INFINITY, f32::min);
    let expected = center[0] - half[0] + chamfer / aspect;
    assert!((leftmost_top - expected).abs() < 1e-5, "45-degree cut must be aspect-corrected");
}

#[test]
fn the_positional_wrapper_and_the_model_build_identical_huds() {
    let reticle = reticle_at(ReticleStatus::Clear, Some(hint(true)));
    let model = BattleHudModel {
        vitals: vitals(),
        reticle: Some(reticle),
        fps: 61.0,
        speed_kmh: 33.0,
        zoom_factor: Some(4.2),
        damage_log: Vec::new(),
        incoming_hits: Vec::new(),
        ammo: None,
        minimap: None,
        battle_outcome: None,
        battle_clock_remaining_s: None,
        kill_confirm_age_s: None,
        reload_ready_age_s: None,
        scope_fade: 0.0,
    };
    let from_model = build_battle_hud(&model, 16.0 / 9.0);
    let from_wrapper =
        build_hud_with_reticle(vitals(), 16.0 / 9.0, Some(reticle), 61.0, 33.0, Some(4.2));
    assert_eq!(from_model, from_wrapper, "wrapper must stay a pure forwarding shim");
    assert!(!from_model.is_empty());
}

/// The scope surround follows `scope_fade` (the camera's mode-blend clock), NOT the reticle
/// mode: mid-transition the reticle has already flipped while the housing is still irising, and
/// on exit the housing lingers a beat after the reticle returned to third person.
#[test]
fn the_scope_surround_is_fade_driven_not_mode_driven() {
    let base = BattleHudModel {
        vitals: vitals(),
        reticle: Some(reticle_at(ReticleStatus::Clear, None)),
        fps: 0.0,
        speed_kmh: 0.0,
        zoom_factor: None,
        damage_log: Vec::new(),
        incoming_hits: Vec::new(),
        ammo: None,
        minimap: None,
        battle_outcome: None,
        battle_clock_remaining_s: None,
        kill_confirm_age_s: None,
        reload_ready_age_s: None,
        scope_fade: 0.0,
    };
    let housing = |hud: &[HudVertex]| {
        let [r, g, b, _] = super::scope_overlay::VIGNETTE_COLOR;
        hud.iter().any(|v| v.color[0] == r && v.color[1] == g && v.color[2] == b)
    };

    // A third-person reticle mid-entry: the housing already shows at partial fade.
    let entering = BattleHudModel { scope_fade: 0.5, ..base.clone() };
    assert!(housing(&build_battle_hud(&entering, 16.0 / 9.0)), "fade in draws the surround");

    // A sniper reticle with the fade at zero (first logical frame of an exit): nothing draws.
    let exited = BattleHudModel {
        reticle: Some(sniper(reticle_at(ReticleStatus::Clear, None))),
        scope_fade: 0.0,
        ..base.clone()
    };
    assert!(!housing(&build_battle_hud(&exited, 16.0 / 9.0)), "zero fade draws no surround");
}

#[test]
fn battle_outcome_banner_draws_only_when_the_battle_has_ended() {
    let running = BattleHudModel {
        vitals: vitals(),
        reticle: None,
        fps: 0.0,
        speed_kmh: 0.0,
        zoom_factor: None,
        damage_log: Vec::new(),
        incoming_hits: Vec::new(),
        ammo: None,
        minimap: None,
        battle_outcome: None,
        battle_clock_remaining_s: None,
        kill_confirm_age_s: None,
        reload_ready_age_s: None,
        scope_fade: 0.0,
    };
    let victory =
        BattleHudModel { battle_outcome: Some(BattleHudOutcome::Victory), ..running.clone() };

    let running_hud = build_battle_hud(&running, 16.0 / 9.0);
    let victory_hud = build_battle_hud(&victory, 16.0 / 9.0);

    assert!(!running_hud.iter().any(|vertex| vertex.color == OUTCOME_VICTORY_COLOR));
    assert!(victory_hud.iter().any(|vertex| vertex.color == OUTCOME_VICTORY_COLOR));

    // A draw (mutual wipe or clock expiry) banners in its own neutral color.
    let draw = BattleHudModel { battle_outcome: Some(BattleHudOutcome::Draw), ..running.clone() };
    let draw_hud = build_battle_hud(&draw, 16.0 / 9.0);
    assert!(draw_hud.iter().any(|vertex| vertex.color == super::outcome::OUTCOME_DRAW_COLOR));
    assert!(!running_hud.iter().any(|vertex| vertex.color == super::outcome::OUTCOME_DRAW_COLOR));

    // Input is dead once the battle ends; every banner carries the way-out hint.
    let hint = |hud: &[HudVertex]| {
        hud.iter().any(|vertex| vertex.color == super::outcome::OUTCOME_HINT_COLOR)
    };
    assert!(hint(&victory_hud) && hint(&draw_hud), "the ended battle points back to the garage");
    assert!(!hint(&running_hud), "no garage hint while the battle runs");
}

/// The battle clock draws top-center when the server reports a timed battle and disappears for
/// untimed ones; the last minute switches to the alert color so the squeeze reads at a glance.
#[test]
fn battle_clock_draws_only_when_timed_and_warms_in_the_last_minute() {
    let untimed = BattleHudModel {
        vitals: vitals(),
        reticle: None,
        fps: 0.0,
        speed_kmh: 0.0,
        zoom_factor: None,
        damage_log: Vec::new(),
        incoming_hits: Vec::new(),
        ammo: None,
        minimap: None,
        battle_outcome: None,
        battle_clock_remaining_s: None,
        kill_confirm_age_s: None,
        reload_ready_age_s: None,
        scope_fade: 0.0,
    };
    let timed = BattleHudModel { battle_clock_remaining_s: Some(474.0), ..untimed.clone() };
    let closing = BattleHudModel { battle_clock_remaining_s: Some(42.0), ..untimed.clone() };

    let untimed_hud = build_battle_hud(&untimed, 16.0 / 9.0);
    let timed_hud = build_battle_hud(&timed, 16.0 / 9.0);
    let closing_hud = build_battle_hud(&closing, 16.0 / 9.0);

    assert!(
        timed_hud.len() > untimed_hud.len(),
        "a timed battle adds clock glyph quads to the HUD"
    );
    let alert = |hud: &[HudVertex]| {
        hud.iter().any(|vertex| vertex.color == super::readouts::CLOCK_CLOSING_COLOR)
    };
    assert!(!alert(&timed_hud), "mid-battle clock stays in the calm readout color");
    assert!(alert(&closing_hud), "the last minute warms to the alert color");
}
