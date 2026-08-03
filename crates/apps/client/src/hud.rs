use renderer_api::HudVertex;

use crate::hud::reticle::ReticleStatus;

pub(crate) mod ammo_panel;
pub(crate) mod damage_log;
pub(crate) mod demo;
pub use ui_kit::font;
pub(crate) mod health;
pub(crate) mod health_bar;
pub(crate) mod hit_direction;
pub(crate) use ui_kit::icons;
pub(crate) mod kill_marker;
pub(crate) mod minimap;
pub(crate) mod module_panel;
pub(crate) mod number;
pub(crate) mod outcome;
pub(crate) mod pause_menu;
pub(crate) mod rack_callout;
pub use ui_kit::primitives;
pub(crate) mod readouts;
pub(crate) mod reticle;
pub(crate) mod reticle_marks;
pub(crate) mod reticle_overlay;
pub(crate) mod reticle_readouts;
pub(crate) mod reticle_sweep;
pub(crate) mod scope_overlay;
pub use ui_kit::theme;
pub(crate) mod track_callout;

pub(crate) use health::health_color;
pub(crate) use outcome::BattleHudOutcome;
#[cfg(test)]
pub(crate) use outcome::OUTCOME_VICTORY_COLOR;
pub(crate) use primitives::{push_hairline, push_panel, push_quad};
pub(crate) use reticle_overlay::HudReticle;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudVitals {
    pub hit_points: u32,
    pub max_hit_points: u32,
    pub reload_remaining_s: f32,
    pub reload_seconds: f32,
}

/// Everything the battle HUD draws in one frame, gathered by `render_now` and consumed by
/// `build_battle_hud`. One struct instead of a growing positional parameter list â€” upcoming
/// elements (damage log, ammo panel, minimap) land here as fields.
#[derive(Debug, Clone, PartialEq)]
pub struct BattleHudModel {
    pub vitals: HudVitals,
    pub reticle: Option<HudReticle>,
    /// Draws the top-right frame-rate readout when positive; `0.0` omits it (offline examples).
    pub fps: f32,
    /// 95th-percentile frame interval (ms) over the last ~1.5 s: the drops, as a number.
    pub frame_p95_ms: f32,
    /// Draws the bottom-left speed readout when at least 0.5 km/h.
    pub speed_kmh: f32,
    /// Sniper magnification; `None` in third person (no readout).
    pub zoom_factor: Option<f32>,
    /// Recent dealt/taken damage rows, newest first (`hud/damage_log.rs`).
    pub damage_log: Vec<damage_log::DamageLogEntry>,
    /// Track-damage callout + re-seat bars for the player's own hull (`hud/track_callout.rs`).
    pub track_feedback: track_callout::TrackFeedbackModel,
    /// Seconds left on the player's OWN lit ammunition rack (`hud/rack_callout.rs`); `None`
    /// when the rack is quiet. Protocol v43 — the ten seconds the crew can win, made visible.
    pub rack_fire_remaining_s: Option<f32>,
    /// Incoming hits resolved to screen bearings (`hud/hit_direction.rs`).
    pub incoming_hits: Vec<hit_direction::IncomingHit>,
    pub ammo: Option<ammo_panel::AmmoHudModel>,
    /// The player's own module-condition row under the health bar (`hud/module_panel.rs`); `None`
    /// before the first snapshot, then always present so a knocked-out gun is never a mystery.
    pub modules: Option<module_panel::ModulePanelModel>,
    pub minimap: Option<minimap::MinimapModel>,
    pub battle_outcome: Option<BattleHudOutcome>,
    /// Seconds left on the battle clock, drawn top-center as M:SS; `None` hides it (untimed).
    pub battle_clock_remaining_s: Option<f32>,
    /// Seconds since the player's most recent kill; `None` once the confirmation has played out.
    pub kill_confirm_age_s: Option<f32>,
    /// Seconds since the reload finished, driving the gun-ready flash at the reticle.
    pub reload_ready_age_s: Option<f32>,
    /// Seconds since a fire click was REFUSED (still reloading / empty slot / dead gun),
    /// driving the red denial pulse at the reticle — a swallowed shot is seen, never wondered
    /// about. `None` when no refusal is fresh.
    pub fire_denied_age_s: Option<f32>,
    /// How much of the scope surround shows (0 = none, 1 = settled sniper). Rides the presented
    /// camera's mode-blend clock, so the optics iris in/out WITH the view instead of hard-cutting
    /// (see `camera::present::scope_dressing`).
    pub scope_fade: f32,
    /// The ESC modal when it is up; `None` is a closed menu (`hud/pause_menu.rs`).
    pub pause_menu: Option<pause_menu::PauseMenuModel>,
}

/// Build the 2D HUD overlay (center crosshair, top-left health bar, bottom-center reload
/// bar) in clip space. `aspect` keeps the crosshair square on non-square viewports.
pub fn build_hud(vitals: HudVitals, aspect: f32) -> Vec<HudVertex> {
    build_battle_hud(
        &BattleHudModel {
            vitals,
            reticle: None,
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
            reload_ready_age_s: None,
            fire_denied_age_s: None,
            scope_fade: 0.0,
            pause_menu: None,
        },
        aspect,
    )
}

/// Compatibility wrapper over `build_battle_hud` for the HUD test suite's positional call sites;
/// production code passes a `BattleHudModel`.
#[cfg(test)]
pub(crate) fn build_hud_with_reticle(
    vitals: HudVitals,
    aspect: f32,
    reticle: Option<HudReticle>,
    fps: f32,
    speed_kmh: f32,
    zoom_factor: Option<f32>,
) -> Vec<HudVertex> {
    build_battle_hud(
        &BattleHudModel {
            vitals,
            reticle,
            fps,
            frame_p95_ms: 0.0,
            speed_kmh,
            zoom_factor,
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
            reload_ready_age_s: None,
            fire_denied_age_s: None,
            // The positional test path has no camera; sniper mode implies a settled scope.
            scope_fade: if reticle.is_some_and(|r| r.mode == reticle::ReticleMode::Sniper) {
                1.0
            } else {
                0.0
            },
            pause_menu: None,
        },
        aspect,
    )
}

pub(crate) fn build_battle_hud(model: &BattleHudModel, aspect: f32) -> Vec<HudVertex> {
    let mut vertices = Vec::new();

    let reticle = model.reticle.unwrap_or(HudReticle {
        aim_clip: [0.0, 0.0],
        impact_clip: None,
        gun_clip: None,
        aim_radius_clip: 0.0,
        target_distance_m: None,
        status: ReticleStatus::Clear,
        penetration_hint: None,
        reload_fraction: 1.0,
        hit_confirm: None,
        converged: false,
        mode: crate::hud::reticle::ReticleMode::ThirdPerson,
    });
    // The scope surround paints first so every live marker (reticle, readouts) stays on top.
    // Fade-driven, not mode-driven: during the TPP <-> sniper camera blend the housing is
    // already irising in (or lifting away) while the logical mode has long since flipped.
    if model.scope_fade > 0.001 {
        scope_overlay::push_scope_overlay(&mut vertices, aspect, model.scope_fade);
    }
    reticle_overlay::push_reticle(&mut vertices, &reticle, aspect);
    if let Some(age_s) = model.reload_ready_age_s {
        reticle_marks::push_ready_flash(&mut vertices, reticle.aim_clip, age_s, aspect);
    }
    if let Some(age_s) = model.fire_denied_age_s {
        reticle_marks::push_denied_flash(&mut vertices, reticle.aim_clip, age_s, aspect);
    }
    // The reload countdown lives AT the reticle with its arc — one loading display, where the
    // eye already is (the old bottom-center bar was a second, competing one).
    if model.vitals.reload_remaining_s > 0.05 {
        crate::hud::number::push_number(
            &mut vertices,
            model.vitals.reload_remaining_s.ceil().clamp(0.0, 99.0) as u32,
            reticle.aim_clip[0] + 0.075,
            reticle.aim_clip[1] - 0.115,
            0.042,
            aspect,
            crate::hud::number::RELOAD_TIME_COLOR,
        );
    }

    readouts::push_battle_readouts(&mut vertices, model, aspect);

    damage_log::push_damage_log(&mut vertices, &model.damage_log, aspect);
    track_callout::push_track_callout(&mut vertices, &model.track_feedback, aspect);
    rack_callout::push_rack_callout(&mut vertices, model.rack_fire_remaining_s, aspect);
    hit_direction::push_hit_direction(&mut vertices, &model.incoming_hits, aspect);
    if let Some(ammo) = &model.ammo {
        ammo_panel::push_ammo_panel(&mut vertices, ammo, aspect);
    }
    if let Some(modules) = &model.modules {
        module_panel::push_module_panel(&mut vertices, modules, aspect);
    }
    if let Some(map) = &model.minimap {
        minimap::push_minimap(&mut vertices, map, aspect);
    }
    if let Some(outcome) = model.battle_outcome {
        outcome::push_battle_outcome(&mut vertices, outcome, aspect);
    }
    if let Some(age_s) = model.kill_confirm_age_s {
        kill_marker::push_kill_confirm(&mut vertices, age_s, aspect);
    }
    // Last, so the modal sits over every battle marker — including the outcome banner, which a
    // player can be reading when they reach for ESC.
    if let Some(menu) = &model.pause_menu {
        pause_menu::push_pause_menu(&mut vertices, menu, aspect);
    }

    vertices
}

#[cfg(test)]
mod reticle_overlay_tests;
#[cfg(test)]
mod tests;
