use renderer_api::HudVertex;

use crate::hud::reticle::ReticleStatus;

pub(crate) mod ammo_panel;
pub(crate) mod damage_log;
pub(crate) mod demo;
pub(crate) mod font;
pub(crate) mod health;
pub(crate) mod health_bar;
pub(crate) mod hit_direction;
pub(crate) mod icons;
pub(crate) mod kill_marker;
pub(crate) mod minimap;
pub(crate) mod number;
pub(crate) mod outcome;
pub(crate) mod primitives;
pub(crate) mod readouts;
pub(crate) mod reticle;
pub(crate) mod reticle_marks;
pub(crate) mod reticle_overlay;
pub(crate) mod reticle_readouts;
pub(crate) mod reticle_sweep;
pub(crate) mod scope_overlay;
pub(crate) mod theme;

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
    /// Draws the bottom-left speed readout when at least 0.5 km/h.
    pub speed_kmh: f32,
    /// Sniper magnification; `None` in third person (no readout).
    pub zoom_factor: Option<f32>,
    /// Recent dealt/taken damage rows, newest first (`hud/damage_log.rs`).
    pub damage_log: Vec<damage_log::DamageLogEntry>,
    /// Incoming hits resolved to screen bearings (`hud/hit_direction.rs`).
    pub incoming_hits: Vec<hit_direction::IncomingHit>,
    pub ammo: Option<ammo_panel::AmmoHudModel>,
    pub minimap: Option<minimap::MinimapModel>,
    pub battle_outcome: Option<BattleHudOutcome>,
    /// Seconds since the player's most recent kill; `None` once the confirmation has played out.
    pub kill_confirm_age_s: Option<f32>,
    /// Seconds since the reload finished, driving the gun-ready flash at the reticle.
    pub reload_ready_age_s: Option<f32>,
}

/// Build the 2D HUD overlay (center crosshair, top-left health bar, bottom-center reload
/// bar) in clip space. `aspect` keeps the crosshair square on non-square viewports.
pub fn build_hud(vitals: HudVitals, aspect: f32) -> Vec<HudVertex> {
    build_battle_hud(
        &BattleHudModel {
            vitals,
            reticle: None,
            fps: 0.0,
            speed_kmh: 0.0,
            zoom_factor: None,
            damage_log: Vec::new(),
            incoming_hits: Vec::new(),
            ammo: None,
            minimap: None,
            battle_outcome: None,
            kill_confirm_age_s: None,
            reload_ready_age_s: None,
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
            speed_kmh,
            zoom_factor,
            damage_log: Vec::new(),
            incoming_hits: Vec::new(),
            ammo: None,
            minimap: None,
            battle_outcome: None,
            kill_confirm_age_s: None,
            reload_ready_age_s: None,
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
        mode: crate::hud::reticle::ReticleMode::ThirdPerson,
    });
    // The scope surround paints first so every live marker (reticle, readouts) stays on top.
    if reticle.mode == crate::hud::reticle::ReticleMode::Sniper {
        scope_overlay::push_scope_overlay(&mut vertices, aspect);
    }
    reticle_overlay::push_reticle(&mut vertices, &reticle, aspect);
    if let Some(age_s) = model.reload_ready_age_s {
        reticle_marks::push_ready_flash(&mut vertices, reticle.aim_clip, age_s, aspect);
    }

    readouts::push_battle_readouts(&mut vertices, model, aspect);

    damage_log::push_damage_log(&mut vertices, &model.damage_log, aspect);
    hit_direction::push_hit_direction(&mut vertices, &model.incoming_hits, aspect);
    if let Some(ammo) = &model.ammo {
        ammo_panel::push_ammo_panel(&mut vertices, ammo, aspect);
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

    vertices
}

#[cfg(test)]
mod reticle_overlay_tests;
#[cfg(test)]
mod tests;
