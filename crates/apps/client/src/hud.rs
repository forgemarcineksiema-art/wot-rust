use renderer_api::HudVertex;

use crate::hud::reticle::ReticleStatus;

pub(crate) mod damage_log;
pub(crate) mod font;
pub(crate) mod health_bar;
pub(crate) mod hit_direction;
pub(crate) mod icons;
pub(crate) mod number;
pub(crate) mod primitives;
pub(crate) mod reticle;
pub(crate) mod reticle_marks;
pub(crate) mod reticle_overlay;
pub(crate) mod reticle_readouts;
pub(crate) mod reticle_sweep;
pub(crate) mod theme;

use primitives::push_bar;
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
/// `build_battle_hud`. One struct instead of a growing positional parameter list — upcoming
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
        },
        aspect,
    )
}

pub(crate) fn build_battle_hud(model: &BattleHudModel, aspect: f32) -> Vec<HudVertex> {
    let mut vertices = Vec::new();
    let vitals = model.vitals;

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
    reticle_overlay::push_reticle(&mut vertices, &reticle, aspect);

    let hp_frac = (vitals.hit_points as f32 / vitals.max_hit_points.max(1) as f32).clamp(0.0, 1.0);
    push_bar(&mut vertices, [-0.95, 0.9], [0.17, 0.018], hp_frac, health_color(hp_frac));
    crate::hud::number::push_number(
        &mut vertices,
        vitals.hit_points.min(9_999),
        -0.61,
        0.95,
        0.055,
        aspect,
        crate::hud::number::HP_COLOR,
    );

    let ready =
        (1.0 - vitals.reload_remaining_s / vitals.reload_seconds.max(0.001)).clamp(0.0, 1.0);
    let reload_color =
        if ready >= 1.0 { [0.55, 0.85, 0.96, 0.95] } else { [0.86, 0.55, 0.20, 0.92] };
    push_bar(&mut vertices, [-0.16, -0.9], [0.16, 0.016], ready, reload_color);
    crate::hud::number::push_number(
        &mut vertices,
        vitals.reload_remaining_s.ceil().clamp(0.0, 99.0) as u32,
        0.06,
        -0.76,
        0.065,
        aspect,
        crate::hud::number::RELOAD_TIME_COLOR,
    );

    // Sniper magnification readout, WT-style "X6.9", just under the reticle center so the
    // eye reads it without leaving the sight. Third person draws nothing.
    if let Some(zoom) = model.zoom_factor {
        let label =
            format!("{}{:.1}", crate::ui_strings::battle::ZOOM_PREFIX, zoom.clamp(0.0, 99.9));
        crate::hud::font::push_text(
            &mut vertices,
            &label,
            -0.03,
            -0.16,
            0.05,
            aspect,
            crate::hud::number::ZOOM_COLOR,
        );
    }

    if model.fps > 0.0 {
        crate::hud::number::push_number(
            &mut vertices,
            model.fps.round() as u32,
            0.97,
            0.97,
            0.05,
            aspect,
            crate::hud::number::FPS_COLOR,
        );
    }

    damage_log::push_damage_log(&mut vertices, &model.damage_log, aspect);
    hit_direction::push_hit_direction(&mut vertices, &model.incoming_hits, aspect);

    if model.speed_kmh >= 0.5 {
        crate::hud::number::push_number(
            &mut vertices,
            model.speed_kmh.round().clamp(0.0, 999.0) as u32,
            -0.78,
            -0.76,
            0.065,
            aspect,
            crate::hud::number::SPEED_COLOR,
        );
        // Unit sits just right of the value's anchor, dimmer and a touch smaller for hierarchy.
        crate::hud::font::push_text(
            &mut vertices,
            crate::ui_strings::battle::SPEED_UNIT,
            -0.765,
            -0.764,
            0.045,
            aspect,
            crate::hud::number::UNIT_COLOR,
        );
    }

    vertices
}

pub(crate) fn health_color(frac: f32) -> [f32; 4] {
    if frac > 0.5 {
        [0.30, 0.82, 0.34, 0.92]
    } else if frac > 0.25 {
        [0.92, 0.78, 0.20, 0.92]
    } else {
        [0.90, 0.26, 0.22, 0.92]
    }
}

#[cfg(test)]
mod reticle_overlay_tests;
#[cfg(test)]
mod tests;
