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
    push_gun_marker, push_impact_leader, push_impact_marker, push_reload_arc,
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
/// Continuous dispersion ring. Bright enough to live on the pale straw steppe (the old 0.38
/// alpha vanished against it); the dark underlay ring supplies contrast on shade.
pub(crate) const RETICLE_RING: [f32; 4] = [0.88, 0.92, 0.78, 0.72];
/// The ring's dark underlay: a slightly larger twin that reads as an outline on any ground.
pub(crate) const RETICLE_RING_OUTLINE: [f32; 4] = [0.05, 0.06, 0.05, 0.55];
/// The ring once the gun has SETTLED to its minimum dispersion: a touch whiter and fully
/// opaque — the convergence signal the sight never had. Fire now and the circle is the truth.
pub(crate) const RETICLE_RING_CONVERGED: [f32; 4] = [0.96, 0.98, 0.92, 0.95];
/// Amber "X" at the shell's real landing point (gravity + collision).
pub(crate) const RETICLE_IMPACT: [f32; 4] = [0.98, 0.66, 0.18, 0.92];
/// The reload arc draining clockwise around the marker while the gun is loading. RED because the
/// arc's colour IS the gun's state — red means "this trigger does nothing yet". Deeper and duller
/// than the pen verdict's [`RETICLE_NO_PEN`], but deliberately the same family: both say "no
/// damage from here, now".
pub(crate) const RETICLE_RELOAD: [f32; 4] = [0.86, 0.24, 0.18, 0.86];
/// The completed reload: the drained arc closes into one full circle at the same radius, holds,
/// and dissolves.
///
/// It used to be GREEN, on the reasoning that green is already the sight's "yes" so a loaded gun
/// may speak the same word. Measured against a player instead of against a palette, that was the
/// defect the 2026-08-07 report opened with: `[0.40, 0.90, 0.42]` against the pen verdict's
/// `[0.35, 0.85, 0.40]` is five per cent apart, and this is the LOUDER of the two — a full circle
/// at ~46 px radius against a marker whose arms span 24 px. The sight's most visible green meant
/// the thing that has nothing to do with the target.
///
/// Red's two meanings can share a family because they agree by construction (`RETICLE_RELOAD` and
/// `RETICLE_NO_PEN` both say "no damage from here, now"). Loaded and penetrates are independent —
/// all four combinations occur — so sharing a colour there is a homonym, and a colour that
/// answers two independent questions answers neither.
///
/// Steel cyan is the only cool hue at this sight, which is the argument for it: this is the one
/// signal that reports the MACHINE rather than the target or the ground, and that difference
/// should arrive before the shape is read.
pub(crate) const RETICLE_LOADED: [f32; 4] = [0.45, 0.82, 0.92, 0.88];

/// Seconds of easing on the central marker's colour. Long enough that a verdict flipping across
/// a plate edge reads as one settling colour instead of a strobe, short enough that a deliberate
/// re-aim answers immediately.
pub(crate) const MARKER_FADE_TAU_S: f32 = 0.12;

/// The honesty matrix as a colour (`docs/aiming-model-policy.md`): neutral in third person,
/// the pen verdict in sniper — scaled by how much of the optics is actually there, so the
/// verdict arrives WITH the scope housing instead of snapping mid-blend.
pub(crate) fn marker_color(
    mode: ReticleMode,
    hint: Option<PenetrationHint>,
    scope_fade: f32,
) -> [f32; 4] {
    let Some(hint) = hint.filter(|_| mode == ReticleMode::Sniper) else {
        return RETICLE_NEUTRAL;
    };
    let verdict = if hint.penetrates { RETICLE_PEN } else { RETICLE_NO_PEN };
    let t = scope_fade.clamp(0.0, 1.0);
    // Settled optics must land on the verdict EXACTLY (a lerp by 1.0 lands a float's breadth
    // away, and the locks read these colours as tags).
    if t >= 1.0 {
        return verdict;
    }
    std::array::from_fn(|i| RETICLE_NEUTRAL[i] + (verdict[i] - RETICLE_NEUTRAL[i]) * t)
}

/// Ease the drawn marker colour toward the matrix's answer. Exponential, frame-rate independent.
///
/// Without it the marker strobed: sweeping a plate edge flips the verdict every frame the mouse
/// twitches, and a mode switch swapped the colour in one frame while the camera was still
/// travelling into the optics. The verdict itself is unchanged — only how fast the eye is asked
/// to accept it.
pub(crate) fn ease_marker_color(current: [f32; 4], target: [f32; 4], dt_s: f32) -> [f32; 4] {
    let t = 1.0 - (-dt_s.max(0.0) / MARKER_FADE_TAU_S).exp();
    std::array::from_fn(|i| current[i] + (target[i] - current[i]) * t)
}

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
    /// Metres to whatever eats the round, when the shot dies short of the crosshair. Drawn under
    /// the range in the BLOCKED grey, so a refusal says WHERE instead of only "no".
    pub block_distance_m: Option<f32>,
    /// The end of the gun arc that refused the shot, when the ARC did (Inny Poziom A3). Drawn
    /// in both modes as the arc's own form — a stop bar under (or over) the crosshair and its
    /// label — because it reports the player's own gun, not the target: "you cannot depress
    /// onto that" is a fact about this hull on this slope.
    pub arc_limit: Option<crate::aim::ArcLimit>,
    pub status: ReticleStatus,
    pub penetration_hint: Option<PenetrationHint>,
    /// Reload progress in `[0, 1]` (1 = ready). Below 1 the reticle arc shows what remains.
    pub reload_fraction: f32,
    /// The player's most recent landed hit, if fresh — drawn as confirm ticks at the reticle.
    pub hit_confirm: Option<HitConfirm>,
    /// The honesty regime: third person draws fully neutral, sniper may speak penetration.
    pub mode: ReticleMode,
    /// The central marker's colour, already eased toward the matrix's answer by the frame clock
    /// ([`marker_color`] + [`ease_marker_color`]). Carried rather than derived here because a
    /// fade needs the previous frame, and only the app owns that.
    pub marker_color: [f32; 4],
    /// Whether the gun has settled to its minimum dispersion (aim fully taken): the ring
    /// brightens as the ready-to-fire signal. Server truth via the replicated dispersion.
    pub converged: bool,
}

/// The arc's stop bar (Inny Poziom A3): a short horizontal bar just outside the dispersion
/// ring on the side the gun cannot travel to — under the crosshair for a depression limit,
/// over it for an elevation limit — joined to the ring by a stub. It reads as "the barrel
/// stopped here", which is exactly what happened; a wall's broken form says "something out
/// there stops the shell", and the two are no longer the same picture.
pub(crate) fn push_arc_limit_stop(
    vertices: &mut Vec<HudVertex>,
    aim_clip: [f32; 2],
    ring_radius: f32,
    limit: crate::aim::ArcLimit,
    aspect: f32,
) {
    let sign = match limit {
        crate::aim::ArcLimit::Depression => -1.0,
        crate::aim::ArcLimit::Elevation => 1.0,
    };
    let bar_y = aim_clip[1] + sign * (ring_radius + 0.022);
    // The stub from the ring to the bar, then the bar itself: both in the BLOCKED grey, both
    // backed like the crosshair arms so they survive bright ground.
    let stub_y = aim_clip[1] + sign * (ring_radius + 0.011);
    push_quad(vertices, [aim_clip[0], stub_y], [0.0034 / aspect, 0.011], RETICLE_RING_OUTLINE);
    push_quad(vertices, [aim_clip[0], stub_y], [0.0018 / aspect, 0.010], RETICLE_BLOCKED);
    push_quad(vertices, [aim_clip[0], bar_y], [0.030 / aspect, 0.0044], RETICLE_RING_OUTLINE);
    push_quad(vertices, [aim_clip[0], bar_y], [0.028 / aspect, 0.0026], RETICLE_BLOCKED);
}

/// Draw the aiming overlay: dispersion ring, reload arc, gun marker, the single center marker,
/// and — in sniper mode only — the pen verdict, the real-impact marker and the mm readout.
pub(crate) fn push_reticle(vertices: &mut Vec<HudVertex>, reticle: &HudReticle, aspect: f32) {
    let sniper = reticle.mode == ReticleMode::Sniper;
    push_dispersion_ring(
        vertices,
        reticle.aim_clip,
        reticle.aim_radius_clip,
        aspect,
        reticle.converged,
    );
    push_reload_arc(
        vertices,
        reticle.aim_clip,
        reticle.reload_fraction,
        reticle.aim_radius_clip,
        aspect,
    );

    match reticle.status {
        ReticleStatus::Blocked => {
            push_blocked_marker(vertices, reticle.aim_clip, aspect);
            // The arc's refusal is not a wall's: the stop bar says which end of the arc bit,
            // and the label under the range says it in words, in both modes.
            if let Some(limit) = reticle.arc_limit {
                push_arc_limit_stop(
                    vertices,
                    reticle.aim_clip,
                    reticle.aim_radius_clip,
                    limit,
                    aspect,
                );
                super::reticle_readouts::push_arc_limit_label(
                    vertices,
                    reticle.aim_clip,
                    reticle.aim_radius_clip,
                    limit,
                    aspect,
                );
            }
        }
        ReticleStatus::Clear => {
            // Third person never speaks armor: the eased colour is computed from the matrix,
            // which answers neutral there even with a pen hint in hand (the hint keeps flowing
            // so a mode switch answers immediately).
            let color = reticle.marker_color;
            push_crosshair(vertices, reticle.aim_clip, 0.012, 0.024, 0.0036, aspect, color);
            // The aim dot, backed like the arms so it survives bright ground.
            push_quad(vertices, reticle.aim_clip, [0.0034 / aspect, 0.0034], RETICLE_RING_OUTLINE);
            push_quad(vertices, reticle.aim_clip, [0.0022 / aspect, 0.0022], color);
        }
    }

    // The gun marker FADES out as the barrel converges on the sight — once merged, drawing a
    // second glyph under the crosshair would only be noise. Drawn in both modes, and also while
    // BLOCKED: it reports the player's own barrel, not the target's armor.
    if let Some(gun_clip) = reticle.gun_clip {
        let alpha =
            impact_separation_alpha(reticle.aim_clip, gun_clip, reticle.aim_radius_clip, aspect);
        if alpha > 0.0 {
            push_gun_marker(vertices, gun_clip, aspect, alpha);
        }
    }

    // Sniper only: the real-impact X, fading in as it separates from the aim point instead of
    // popping at a threshold.
    if sniper && let Some(impact_clip) = reticle.impact_clip {
        let alpha =
            impact_separation_alpha(reticle.aim_clip, impact_clip, reticle.aim_radius_clip, aspect);
        if alpha > 0.0 {
            // A refused shot dies far from the crosshair — a fold 60 m out projects most of a
            // screen below the aim at maximum zoom — and an unconnected mark down there reads as
            // clutter rather than as an answer. The leader ties the two into one sentence:
            // "not there; HERE". Only while blocked: on an arriving shot the X is the ordinary
            // drop marker and a line to it would be noise on every long shot in the game.
            if reticle.status == ReticleStatus::Blocked {
                push_impact_leader(vertices, reticle.aim_clip, impact_clip, alpha);
            }
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
            reticle.aim_radius_clip,
            distance_m,
            aspect,
        );
        if sniper && let Some(hint) = reticle.penetration_hint {
            super::reticle_readouts::push_pen_numbers(
                vertices,
                reticle.aim_clip,
                reticle.aim_radius_clip,
                hint,
                aspect,
            );
        }
    }
    // The second row, in the broken marker's own grey. It never collides with the pen millimetres
    // that share the row: a blocked shot reaches no armour, so there are no millimetres to print.
    // Both modes — like the BLOCKED form itself, it reports the player's own gun and leaks
    // nothing about the target.
    if let Some(block_m) = reticle.block_distance_m {
        super::reticle_readouts::push_block_distance(
            vertices,
            reticle.aim_clip,
            reticle.aim_radius_clip,
            block_m,
            aspect,
        );
    }
}
