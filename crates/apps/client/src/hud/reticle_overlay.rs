//! The aiming overlay: ONE marker owns the center, and the mode decides how honest it is.
//! Third person draws the policy's three neutral layers — central marker, hollow gun marker
//! (fading out as the barrel converges), dispersion ring — and never speaks armor: no pen
//! colors, no impact X, no millimeters (`docs/aiming-model-policy.md`). Sniper mode is
//! deliberate aimed fire: the marker carries the pen verdict by color, the amber X appears when
//! the real ballistic landing point separates from the aim, and the pen/armor mm print by the
//! distance. The broken BLOCKED form (terrain/ally/arc) and the reload arc draw in both modes —
//! they report the player's own gun, not the target's armor. The glyphs themselves live in
//! `hud/reticle_marks.rs`; this module owns the honesty matrix.

use renderer_api::HudVertex;

use super::push_quad;
use super::reticle_marks::{
    impact_separation_alpha, push_blocked_marker, push_crosshair, push_dispersion_ring,
    push_gun_marker, push_impact_marker, push_reload_arc,
};
use crate::hud::reticle::{PenetrationHint, ReticleMode, ReticleStatus};
use crate::hud::reticle_readouts::HitConfirm;

pub(crate) const RETICLE_NEUTRAL: [f32; 4] = [0.88, 0.90, 0.84, 0.85];
pub(crate) const RETICLE_PEN: [f32; 4] = [0.35, 0.85, 0.40, 0.92];
pub(crate) const RETICLE_NO_PEN: [f32; 4] = [0.90, 0.30, 0.25, 0.92];
/// The hollow gun marker: where the barrel actually points at target range. Distinct bytes from
/// `RETICLE_NEUTRAL` so tests can tag it; visually the same quiet off-white family.
pub(crate) const RETICLE_GUN: [f32; 4] = [0.86, 0.90, 0.86, 0.80];
/// The BLOCKED form: the shot will not reach the aim point (terrain, ally, gun arc).
/// Desaturated gray on purpose — RED is reserved for "reaches but bounces", so the two states
/// can never be confused: gray broken = no shot here, red = shot arrives and fails.
pub(crate) const RETICLE_BLOCKED: [f32; 4] = [0.62, 0.62, 0.58, 0.95];
/// Continuous dispersion ring — quiet on purpose; it frames, the marker speaks.
pub(crate) const RETICLE_RING: [f32; 4] = [0.86, 0.90, 0.72, 0.38];
/// Amber "X" at the shell's real landing point (gravity + collision).
pub(crate) const RETICLE_IMPACT: [f32; 4] = [0.98, 0.66, 0.18, 0.92];
/// The reload arc draining clockwise around the marker while the gun is loading.
pub(crate) const RETICLE_RELOAD: [f32; 4] = [0.92, 0.62, 0.20, 0.88];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HudReticle {
    pub aim_clip: [f32; 2],
    /// Where the shell actually lands (full ballistic + collision trace), if on screen.
    pub impact_clip: Option<[f32; 2]>,
    /// Where the barrel currently points at target range, if on screen — the policy's gun
    /// marker, showing turret and elevation catch-up.
    pub gun_clip: Option<[f32; 2]>,
    pub aim_radius_clip: f32,
    pub target_distance_m: Option<f32>,
    pub status: ReticleStatus,
    pub penetration_hint: Option<PenetrationHint>,
    /// Reload progress in `[0, 1]` (1 = ready). Below 1 the reticle arc shows what remains.
    pub reload_fraction: f32,
    /// The player's most recent landed hit, if fresh — drawn as confirm ticks at the reticle.
    pub hit_confirm: Option<HitConfirm>,
    /// The honesty regime: third person draws fully neutral, sniper may speak penetration.
    pub mode: ReticleMode,
}

/// Draw the aiming overlay: dispersion ring, reload arc, gun marker, the single center marker,
/// and — in sniper mode only — the pen verdict, the real-impact marker and the mm readout.
pub(crate) fn push_reticle(vertices: &mut Vec<HudVertex>, reticle: &HudReticle, aspect: f32) {
    let sniper = reticle.mode == ReticleMode::Sniper;
    push_dispersion_ring(vertices, reticle.aim_clip, reticle.aim_radius_clip, aspect);
    push_reload_arc(vertices, reticle.aim_clip, reticle.reload_fraction, aspect);

    match reticle.status {
        ReticleStatus::Blocked => push_blocked_marker(vertices, reticle.aim_clip, aspect),
        ReticleStatus::Clear => {
            // Third person never speaks armor: the marker stays neutral even with a pen hint
            // computed (the hint keeps flowing so a mode switch answers instantly).
            let color = if sniper {
                reticle.penetration_hint.map_or(RETICLE_NEUTRAL, |hint| {
                    if hint.penetrates { RETICLE_PEN } else { RETICLE_NO_PEN }
                })
            } else {
                RETICLE_NEUTRAL
            };
            push_crosshair(vertices, reticle.aim_clip, 0.020, 0.0026, aspect, color);
            push_quad(vertices, reticle.aim_clip, [0.0016 / aspect, 0.0016], color);
        }
    }

    // The gun marker FADES out as the barrel converges on the sight — once merged, drawing a
    // second glyph under the crosshair would only be noise. Drawn in both modes, and also while
    // BLOCKED: it reports the player's own barrel, not the target's armor.
    if let Some(gun_clip) = reticle.gun_clip {
        let alpha = impact_separation_alpha(reticle.aim_clip, gun_clip, aspect);
        if alpha > 0.0 {
            push_gun_marker(vertices, gun_clip, aspect, alpha);
        }
    }

    // Sniper only: the real-impact X, fading in as it separates from the aim point instead of
    // popping at a threshold.
    if sniper && let Some(impact_clip) = reticle.impact_clip {
        let alpha = impact_separation_alpha(reticle.aim_clip, impact_clip, aspect);
        if alpha > 0.0 {
            push_impact_marker(vertices, impact_clip, aspect, alpha);
        }
    }
    if let Some(confirm) = reticle.hit_confirm {
        super::reticle_readouts::push_hit_confirm(vertices, reticle.aim_clip, confirm, aspect);
    }
    if let Some(distance_m) = reticle.target_distance_m {
        super::reticle_readouts::push_target_distance(
            vertices,
            reticle.aim_clip,
            distance_m,
            aspect,
        );
        if sniper && let Some(hint) = reticle.penetration_hint {
            super::reticle_readouts::push_pen_numbers(vertices, reticle.aim_clip, hint, aspect);
        }
    }
}
