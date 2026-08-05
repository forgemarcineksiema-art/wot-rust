//! Locks the hybrid reticle matrix (`docs/aiming-model-policy.md`): third person = the three
//! neutral layers (central marker, gun marker, dispersion ring) with no armor talk; sniper =
//! pen verdict by color, real-impact X, and mm readouts. Blocked, reload arc, distance and hit
//! confirm draw in both modes. Split from `hud/tests.rs` for the file budget.

use super::reticle::ReticleStatus;
use super::reticle_overlay::{
    RETICLE_BLOCKED, RETICLE_GUN, RETICLE_IMPACT, RETICLE_NEUTRAL, RETICLE_NO_PEN, RETICLE_PEN,
    RETICLE_RELOAD, RETICLE_RING,
};
use super::tests::{hint, reticle_at, sniper, vitals};
use super::*;
use crate::hud::number::TARGET_DISTANCE_COLOR;

fn hud_with(reticle: HudReticle) -> Vec<HudVertex> {
    build_hud_with_reticle(vitals(), 16.0 / 9.0, Some(reticle), 0.0, 0.0, None)
}

#[test]
fn a_blocked_shot_draws_the_broken_marker_in_both_modes() {
    for reticle in [
        reticle_at(ReticleStatus::Blocked, Some(hint(true))),
        sniper(reticle_at(ReticleStatus::Blocked, Some(hint(true)))),
    ] {
        let hud = hud_with(reticle);
        assert!(hud.iter().any(|vertex| vertex.color == RETICLE_BLOCKED), "blocked form draws");
        assert!(!hud.iter().any(|vertex| vertex.color == RETICLE_NEUTRAL));
        assert!(
            !hud.iter().any(|vertex| vertex.color == RETICLE_PEN),
            "no pen color can lie over it"
        );
    }
}

#[test]
fn the_third_person_marker_stays_neutral_even_with_a_pen_hint() {
    // The hint is computed (mode switches must answer instantly) but must not color the marker:
    // third person is situational awareness, not an armor oracle.
    for pen_hint in [Some(hint(true)), Some(hint(false)), None] {
        let hud = hud_with(reticle_at(ReticleStatus::Clear, pen_hint));
        assert!(hud.iter().any(|vertex| vertex.color == RETICLE_NEUTRAL), "neutral marker draws");
        assert!(!hud.iter().any(|vertex| vertex.color == RETICLE_PEN));
        assert!(!hud.iter().any(|vertex| vertex.color == RETICLE_NO_PEN));
    }
}

#[test]
fn the_sniper_marker_speaks_penetration_by_color() {
    let neutral = hud_with(sniper(reticle_at(ReticleStatus::Clear, None)));
    assert!(neutral.iter().any(|vertex| vertex.color == RETICLE_NEUTRAL));

    let pen = hud_with(sniper(reticle_at(ReticleStatus::Clear, Some(hint(true)))));
    assert!(pen.iter().any(|vertex| vertex.color == RETICLE_PEN));
    assert!(!pen.iter().any(|vertex| vertex.color == RETICLE_NEUTRAL), "one marker, one color");

    let bounce = hud_with(sniper(reticle_at(ReticleStatus::Clear, Some(hint(false)))));
    assert!(bounce.iter().any(|vertex| vertex.color == RETICLE_NO_PEN));
}

/// The verdict SETTLES, it does not strobe. Sweeping a plate edge flips the pen answer every
/// frame the mouse twitches, and a mode switch used to swap the colour in one frame while the
/// camera was still travelling into the optics.
#[test]
fn the_marker_colour_arrives_with_the_optics_and_eases_instead_of_snapping() {
    use super::reticle::ReticleMode;
    use super::reticle_overlay::{MARKER_FADE_TAU_S, ease_marker_color, marker_color};

    // The matrix still refuses to speak armor in third person, at any stage of the blend.
    assert_eq!(marker_color(ReticleMode::ThirdPerson, Some(hint(true)), 1.0), RETICLE_NEUTRAL);
    assert_eq!(marker_color(ReticleMode::Sniper, Some(hint(true)), 1.0), RETICLE_PEN);
    assert_eq!(marker_color(ReticleMode::Sniper, None, 1.0), RETICLE_NEUTRAL);
    // Half-way into the optics, half-way into the verdict.
    let entering = marker_color(ReticleMode::Sniper, Some(hint(true)), 0.5);
    assert!(
        entering != RETICLE_NEUTRAL && entering != RETICLE_PEN,
        "the verdict arrives WITH the housing, not in one frame mid-blend"
    );

    // And the ease itself: a 60 Hz frame is a step toward the answer, a beat later it is there.
    let span = (RETICLE_PEN[0] - RETICLE_NEUTRAL[0]).abs();
    let one_frame = ease_marker_color(RETICLE_NEUTRAL, RETICLE_PEN, 1.0 / 60.0);
    let travelled = (one_frame[0] - RETICLE_NEUTRAL[0]).abs() / span;
    assert!((0.05..0.30).contains(&travelled), "one frame is a step, not a jump: {travelled}");
    let settled = ease_marker_color(RETICLE_NEUTRAL, RETICLE_PEN, MARKER_FADE_TAU_S * 4.0);
    assert!((settled[0] - RETICLE_PEN[0]).abs() < 0.02, "four time constants have arrived");
}

/// The drawn marker uses the colour it was HANDED — the eased one — instead of re-deriving the
/// verdict at draw time, which is what would quietly undo the fade.
#[test]
fn the_central_marker_draws_the_eased_colour_it_was_given() {
    let mid_fade = [0.61, 0.88, 0.62, 0.92];
    // The override goes on LAST: `sniper` resolves the matrix colour, and the frame clock's
    // eased value is what actually reaches the draw call.
    let hud = hud_with(HudReticle {
        marker_color: mid_fade,
        ..sniper(reticle_at(ReticleStatus::Clear, Some(hint(true))))
    });

    assert!(
        hud.iter().any(|v| v.color == mid_fade),
        "the crosshair must draw the colour the frame clock eased for it"
    );
    assert!(
        !hud.iter().any(|v| v.color == RETICLE_PEN),
        "and must not re-derive the raw verdict underneath it"
    );
}

#[test]
fn the_gun_marker_draws_when_separated_and_fades_out_once_merged() {
    let is_gun = |v: &HudVertex| {
        v.color[0] == RETICLE_GUN[0] && v.color[1] == RETICLE_GUN[1] && v.color[2] == RETICLE_GUN[2]
    };

    // Barrel far from the sight: the hollow gun marker draws at its own position, full alpha.
    let separated = hud_with(HudReticle {
        gun_clip: Some([0.3, -0.2]),
        ..reticle_at(ReticleStatus::Clear, None)
    });
    let gun: Vec<_> = separated.iter().filter(|v| is_gun(v)).collect();
    assert!(!gun.is_empty(), "a lagging barrel draws the gun marker");
    assert!(
        gun.iter()
            .all(|v| (v.position[0] - 0.3).abs() < 0.02 && (v.position[1] + 0.2).abs() < 0.02),
        "the gun marker sits at the barrel's converged point"
    );
    assert!(gun.iter().all(|v| v.color[3] == RETICLE_GUN[3]), "fully separated = full alpha");

    // Barrel converged on the sight: the marker dissolves instead of stacking a second glyph.
    let merged = hud_with(HudReticle {
        gun_clip: Some([0.001, 0.0]),
        ..reticle_at(ReticleStatus::Clear, None)
    });
    assert!(!merged.iter().any(&is_gun), "a converged barrel draws no gun marker");

    // While BLOCKED the gun marker still reports the player's own barrel.
    let blocked = hud_with(HudReticle {
        gun_clip: Some([0.3, -0.2]),
        ..reticle_at(ReticleStatus::Blocked, None)
    });
    assert!(blocked.iter().any(is_gun), "blocked keeps the gun marker");
}

/// The gun marker is a DIAMOND, not a second circle. Every circle at this sight already means
/// the dispersion of this gun; a small circle carrying "the barrel is over there" was a homonym
/// that read as a knot in the ring whenever it sat on one.
#[test]
fn the_gun_marker_is_a_diamond_so_it_cannot_be_read_as_a_ring() {
    let hud = hud_with(HudReticle {
        gun_clip: Some([0.3, -0.2]),
        ..reticle_at(ReticleStatus::Clear, None)
    });
    let gun: Vec<_> = hud
        .iter()
        .filter(|v| v.color[..3] == RETICLE_GUN[..3])
        .map(|v| [v.position[0] - 0.3, v.position[1] + 0.2])
        .collect();

    // Four segments (6 vertices each), not a 16-segment circle's 96.
    assert_eq!(gun.len(), 24, "the gun marker is a four-sided outline");
    // A diamond reaches furthest on the axes: its topmost point sits on the vertical centre
    // line, where a circle would carry just as much geometry out at 45 degrees.
    let aspect = 16.0f32 / 9.0;
    let top =
        gun.iter().copied().fold([0.0f32, 0.0f32], |best, p| if p[1] > best[1] { p } else { best });
    assert!(
        (top[0] * aspect).abs() < 0.004,
        "the diamond's far point must sit on the axis, got {top:?}"
    );
}

/// The fade band is ANGULAR — a fraction of the live dispersion ring, not a fixed screen
/// distance. The same barrel error must read the same way at any zoom: fixed clip constants
/// meant 7..15 mrad in third person but 1..2 mrad under 6.9x sniper, which pinned the marker on
/// screen through the whole exponential tail of the turret's fine lay.
#[test]
fn the_gun_marker_band_scales_with_the_ring_so_the_same_angle_reads_the_same_at_any_zoom() {
    let aspect = 16.0f32 / 9.0;
    let is_gun = |v: &&HudVertex| v.color[..3] == RETICLE_GUN[..3];
    let drawn = |ring: f32, separation: f32| {
        let hud = hud_with(HudReticle {
            aim_radius_clip: ring,
            gun_clip: Some([separation / aspect, 0.0]),
            ..reticle_at(ReticleStatus::Clear, None)
        });
        hud.iter().filter(is_gun).any(|v| v.color[3] > 0.0)
    };

    // A settled third-person ring (~2.9 mrad through an 18-degree view) with the barrel nearly
    // three ring-radii off: clearly outside the cone, so the marker draws.
    let tpp_ring = 0.018;
    assert!(drawn(tpp_ring, tpp_ring * 2.8), "a barrel outside its own cone draws its marker");

    // The SAME screen distance under 6.9x zoom is a far smaller angle — the ring magnified with
    // the world, and inside the cone the gun cannot tell the two points apart.
    let sniper_ring = tpp_ring * 6.9;
    assert!(
        !drawn(sniper_ring, tpp_ring * 2.8),
        "under zoom the same clip distance is inside the cone and must stay silent"
    );

    // And the same ANGLE under that zoom (screen distance magnified with the ring) draws again.
    assert!(
        drawn(sniper_ring, sniper_ring * 2.8),
        "the same angular error must read the same way at any magnification"
    );
}

#[test]
fn the_dispersion_ring_is_continuous_geometry_around_the_aim() {
    let hud =
        hud_with(HudReticle { aim_radius_clip: 0.10, ..reticle_at(ReticleStatus::Clear, None) });

    let ring: Vec<_> = hud.iter().filter(|vertex| vertex.color == RETICLE_RING).collect();
    // 40 segments x 6 vertices: a drawn circle, not a scatter of dots.
    assert_eq!(ring.len(), 240, "continuous ring geometry");
    assert!(ring.iter().any(|v| v.position[0] > 0.04), "ring reaches right of center");
    assert!(ring.iter().any(|v| v.position[1] < -0.09), "ring reaches below center");
}

#[test]
fn the_reload_arc_drains_at_the_reticle_and_vanishes_when_ready() {
    let reloading =
        hud_with(HudReticle { reload_fraction: 0.25, ..reticle_at(ReticleStatus::Clear, None) });
    let early: Vec<_> = reloading.iter().filter(|v| v.color == RETICLE_RELOAD).collect();
    assert!(!early.is_empty(), "the arc draws while loading");

    let nearly =
        hud_with(HudReticle { reload_fraction: 0.9, ..reticle_at(ReticleStatus::Clear, None) });
    let late = nearly.iter().filter(|v| v.color == RETICLE_RELOAD).count();
    assert!(late < early.len(), "the arc drains as the reload progresses");

    let ready = hud_with(reticle_at(ReticleStatus::Clear, None));
    assert!(
        !ready.iter().any(|v| v.color == RETICLE_RELOAD),
        "a ready gun draws nothing — silence is the signal"
    );
}

pub(super) fn reticle_with_impact(aim_clip: [f32; 2], impact_clip: Option<[f32; 2]>) -> HudReticle {
    HudReticle { aim_clip, impact_clip, ..reticle_at(ReticleStatus::Clear, None) }
}

#[test]
fn the_sniper_impact_x_marks_the_real_landing_point() {
    // A howitzer arc: the shell lands well below and right of where the mouse points.
    let impact = [0.35, -0.4];
    let hud = hud_with(sniper(reticle_with_impact([0.0, 0.0], Some(impact))));

    let impact_vertices: Vec<_> = hud.iter().filter(|v| v.color == RETICLE_IMPACT).collect();
    assert!(!impact_vertices.is_empty(), "a dropped impact should draw its own marker");
    assert!(
        impact_vertices.iter().all(|v| v.position[0] > 0.25 && v.position[1] < -0.30),
        "the impact marker should sit at the real landing point, not the aim point"
    );
}

#[test]
fn third_person_never_draws_the_impact_x() {
    // The same dropped shot that earns an X in sniper draws nothing in third person: the real
    // landing point is a sniper-mode aid (docs/aiming-model-policy.md).
    let hud = hud_with(reticle_with_impact([0.0, 0.0], Some([0.35, -0.4])));
    assert!(!hud.iter().any(|v| v.color[..3] == RETICLE_IMPACT[..3]));
}

#[test]
fn the_sniper_impact_x_merges_into_the_crosshair_instead_of_stacking() {
    // High muzzle velocity: impact is essentially the aim point, so no redundant marker.
    let hud = hud_with(sniper(reticle_with_impact([0.1, 0.1], Some([0.1, 0.105]))));
    assert!(
        !hud.iter().any(|v| v.color == RETICLE_IMPACT),
        "an on-crosshair impact must not clutter the center with a second marker"
    );

    // And inside the fade band it draws at partial alpha instead of popping (the band runs
    // 0.75..1.6 of the live ring — a zero-width threshold used to flicker as the barrel settled
    // across it). A settled 0.02 ring puts a 0.022 separation squarely inside that band.
    let near = hud_with(sniper(HudReticle {
        aim_radius_clip: 0.02,
        ..reticle_with_impact([0.0, 0.0], Some([0.0, 0.022]))
    }));
    let dimmed: Vec<_> = near
        .iter()
        .filter(|v| {
            v.color[0] == RETICLE_IMPACT[0]
                && v.color[3] > 0.0
                && v.color[3] < RETICLE_IMPACT[3] - 0.05
        })
        .collect();
    assert!(!dimmed.is_empty(), "near the threshold the X draws at partial alpha");
}

#[test]
fn reticle_draws_target_distance_meters_in_both_modes() {
    for mode_wrap in [false, true] {
        let base = HudReticle {
            aim_clip: [0.20, 0.10],
            target_distance_m: Some(347.4),
            ..reticle_with_impact([0.20, 0.10], None)
        };
        let hud = hud_with(if mode_wrap { sniper(base) } else { base });
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
}

#[test]
fn a_fresh_hit_flares_confirm_ticks_that_a_stale_one_no_longer_draws() {
    use super::reticle_readouts::HitConfirm;

    let confirm_color = [0.45, 1.0, 0.50, 0.95]; // pen ticks at full life
    let fresh = hud_with(HudReticle {
        hit_confirm: Some(HitConfirm { age_s: 0.0, penetrated: true, ricocheted: false }),
        ..reticle_at(ReticleStatus::Clear, None)
    });
    assert!(fresh.iter().any(|v| v.color == confirm_color), "fresh pen hit flares green ticks");

    let stale = hud_with(HudReticle {
        hit_confirm: Some(HitConfirm { age_s: 10.0, penetrated: true, ricocheted: false }),
        ..reticle_at(ReticleStatus::Clear, None)
    });
    assert!(
        !stale.iter().any(|v| v.color[1] == confirm_color[1] && v.color[0] == confirm_color[0]),
        "an aged confirm draws nothing"
    );
}

#[test]
fn pen_numbers_print_only_in_sniper_mode() {
    let base = HudReticle {
        target_distance_m: Some(300.0),
        ..reticle_at(ReticleStatus::Clear, Some(hint(true)))
    };
    let third_person = hud_with(base);
    let sniper_hud = hud_with(sniper(base));
    // The sniper view draws strictly more glyph geometry: pen + armor mm join the readout.
    assert!(
        sniper_hud.len() > third_person.len() + 30,
        "sniper adds the pen/armor mm readout that third person must not print"
    );
}

/// One HUD frame with the reload beat driven directly — `build_hud_with_reticle` predates the
/// loaded ring and cannot carry its clock.
fn hud_with_ready_age(reticle: HudReticle, reload_ready_age_s: Option<f32>) -> Vec<HudVertex> {
    build_battle_hud(
        &BattleHudModel {
            vitals: vitals(),
            reticle: Some(reticle),
            fps: 0.0,
            frame_p95_ms: 0.0,
            speed_kmh: 0.0,
            zoom_factor: None,
            damage_log: Vec::new(),
            track_feedback: Default::default(),
            rack_fire_remaining_s: None,
            incoming_hits: Vec::new(),
            ammo: None,
            modules: None,
            minimap: None,
            battle_outcome: None,
            battle_clock_remaining_s: None,
            kill_confirm_age_s: None,
            reload_ready_age_s,
            fire_denied_age_s: None,
            scope_fade: 0.0,
            pause_menu: None,
        },
        16.0 / 9.0,
    )
}

/// The gun's state is a COLOUR the player already knows, not an invented event glyph: the arc
/// drains RED while loading and closes into ONE full GREEN circle on the same line when it is
/// done. The expanding blue flash is gone — no third colour may claim "ready" at the reticle.
#[test]
fn the_reload_arc_runs_red_and_the_loaded_gun_closes_a_green_ring_on_the_same_line() {
    use super::reticle_marks::{READY_RING_HOLD_S, READY_RING_TTL_S};
    use super::reticle_overlay::RETICLE_LOADED;

    let aspect = 16.0f32 / 9.0;
    let radius = |v: &HudVertex| {
        let dx = v.position[0] * aspect;
        (dx * dx + v.position[1] * v.position[1]).sqrt()
    };
    let ring_clip = 0.08;
    let loading = HudReticle {
        aim_radius_clip: ring_clip,
        reload_fraction: 0.4,
        ..reticle_at(ReticleStatus::Clear, None)
    };
    let loaded = HudReticle { reload_fraction: 1.0, ..loading };

    // Loading: a red arc, and nothing green claiming the gun is ready.
    let mid_reload = hud_with_ready_age(loading, None);
    let red: Vec<_> = mid_reload.iter().filter(|v| v.color == RETICLE_RELOAD).collect();
    assert!(!red.is_empty(), "a loading gun draws its red arc");
    assert!(
        !mid_reload.iter().any(|v| v.color[..3] == RETICLE_LOADED[..3]),
        "nothing may read as loaded while the arc is still draining"
    );

    // The moment it closes: one full green circle, at full strength, on the arc's own line.
    let ready = hud_with_ready_age(loaded, Some(0.0));
    let green: Vec<_> = ready.iter().filter(|v| v.color == RETICLE_LOADED).collect();
    assert_eq!(green.len(), 240, "the loaded ring is one closed 40-segment circle");
    let red_line = red.iter().map(|v| radius(v)).fold(0.0f32, f32::max);
    let green_line = green.iter().map(|v| radius(v)).fold(0.0f32, f32::max);
    assert!(
        (red_line - green_line).abs() < 2.0e-3,
        "the green ring must close on the line the red arc drained: {red_line} vs {green_line}"
    );
    assert!(
        !ready.iter().any(|v| v.color[2] > v.color[0] + 0.2 && v.color[3] > 0.0),
        "no blue flash: the colour change IS the ready event"
    );

    // It holds, then dissolves, then is silence again.
    let holding = hud_with_ready_age(loaded, Some(READY_RING_HOLD_S * 0.5));
    assert!(
        holding.iter().any(|v| v.color == RETICLE_LOADED),
        "the ring holds at full strength before dissolving"
    );
    let dissolving = hud_with_ready_age(loaded, Some((READY_RING_HOLD_S + READY_RING_TTL_S) * 0.5));
    let fading: Vec<_> = dissolving
        .iter()
        .filter(|v| v.color[..3] == RETICLE_LOADED[..3] && v.color[3] > 0.0)
        .collect();
    assert!(!fading.is_empty(), "mid-dissolve the ring is still on screen");
    assert!(
        fading.iter().all(|v| v.color[3] < RETICLE_LOADED[3] - 0.05),
        "mid-dissolve it draws dimmer than the hold"
    );
    let expired = hud_with_ready_age(loaded, Some(READY_RING_TTL_S + 0.01));
    assert!(
        !expired.iter().any(|v| v.color[..3] == RETICLE_LOADED[..3]),
        "a loaded gun settles into silence: the ring is a beat, not a permanent glyph"
    );
}

#[test]
fn the_dispersion_ring_is_honest_brightens_when_converged_and_carries_its_arc() {
    use super::reticle_overlay::{RETICLE_RELOAD, RETICLE_RING, RETICLE_RING_CONVERGED};
    let dist = |v: &renderer_api::HudVertex, c: [f32; 2], aspect: f32| {
        let dx = (v.position[0] - c[0]) * aspect;
        let dy = v.position[1] - c[1];
        (dx * dx + dy * dy).sqrt()
    };
    let aspect = 16.0 / 9.0;

    // HONEST: a settled 0.012-clip dispersion draws at ~0.012 — the old 0.025 floor forced a
    // permanently oversized, aim-dead circle in third person.
    let small =
        hud_with(HudReticle { aim_radius_clip: 0.012, ..reticle_with_impact([0.0, 0.0], None) });
    let ring: Vec<_> = small.iter().filter(|v| v.color == RETICLE_RING).collect();
    assert!(!ring.is_empty(), "the ring draws");
    assert!(
        ring.iter().all(|v| dist(v, [0.0, 0.0], aspect) < 0.021),
        "a settled ring draws at its true (small) radius, not the old 0.025 clamped floor"
    );

    // CONVERGED: the settled gun brightens the ring — the ready-to-fire signal.
    let converged = hud_with(HudReticle {
        aim_radius_clip: 0.012,
        converged: true,
        ..reticle_with_impact([0.0, 0.0], None)
    });
    assert!(
        converged.iter().any(|v| v.color == RETICLE_RING_CONVERGED),
        "a converged gun draws the bright ring"
    );

    // ONE CENTRE: the reload arc rides just outside the live ring instead of a fixed radius.
    let loading = hud_with(HudReticle {
        aim_radius_clip: 0.20,
        reload_fraction: 0.5,
        ..reticle_with_impact([0.0, 0.0], None)
    });
    let arc: Vec<_> = loading.iter().filter(|v| v.color == RETICLE_RELOAD).collect();
    assert!(!arc.is_empty(), "the reload arc draws while loading");
    assert!(
        arc.iter().all(|v| dist(v, [0.0, 0.0], aspect) > 0.19),
        "the reload arc rides the bloomed ring, not a fixed small circle"
    );
}
