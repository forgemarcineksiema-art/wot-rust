use renderer_api::HudVertex;

use crate::hud::reticle::ReticleStatus;

pub(crate) mod font;
pub(crate) mod health_bar;
pub(crate) mod icons;
pub(crate) mod number;
pub(crate) mod reticle;
mod reticle_overlay;
pub(crate) mod reticle_sweep;

pub(crate) use reticle_overlay::HudReticle;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudVitals {
    pub hit_points: u32,
    pub max_hit_points: u32,
    pub reload_remaining_s: f32,
    pub reload_seconds: f32,
}

/// Build the 2D HUD overlay (center crosshair, top-left health bar, bottom-center reload
/// bar) in clip space. `aspect` keeps the crosshair square on non-square viewports.
pub fn build_hud(vitals: HudVitals, aspect: f32) -> Vec<HudVertex> {
    build_hud_with_reticle(vitals, aspect, None, 0.0, 0.0)
}

/// `fps` draws a 7-segment frame-rate readout in the top-right corner when positive; pass
/// `0.0` to omit it (e.g. the offline screenshot example).
pub(crate) fn build_hud_with_reticle(
    vitals: HudVitals,
    aspect: f32,
    reticle: Option<HudReticle>,
    fps: f32,
    speed_kmh: f32,
) -> Vec<HudVertex> {
    let mut vertices = Vec::new();

    let reticle = reticle.unwrap_or(HudReticle {
        aim_clip: [0.0, 0.0],
        gun_clip: None,
        impact_clip: None,
        aim_radius_clip: 0.0,
        target_distance_m: None,
        status: ReticleStatus::Clear,
        penetration_hint: None,
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

    if fps > 0.0 {
        crate::hud::number::push_number(
            &mut vertices,
            fps.round() as u32,
            0.97,
            0.97,
            0.05,
            aspect,
            crate::hud::number::FPS_COLOR,
        );
    }

    if speed_kmh >= 0.5 {
        crate::hud::number::push_number(
            &mut vertices,
            speed_kmh.round().clamp(0.0, 999.0) as u32,
            -0.78,
            -0.76,
            0.065,
            aspect,
            crate::hud::number::SPEED_COLOR,
        );
        // Unit sits just right of the value's anchor, dimmer and a touch smaller for hierarchy.
        crate::hud::font::push_text(
            &mut vertices,
            "KM/H",
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

/// A left-aligned bar: a dark background plus a colored fill of `frac` width.
/// `left` is the bar's left edge; `half` is the full-bar half-extent.
fn push_bar(
    vertices: &mut Vec<HudVertex>,
    left: [f32; 2],
    half: [f32; 2],
    frac: f32,
    color: [f32; 4],
) {
    push_quad(vertices, [left[0] + half[0], left[1]], half, [0.0, 0.0, 0.0, 0.55]);
    let fill = half[0] * frac.clamp(0.0, 1.0);
    push_quad(vertices, [left[0] + fill, left[1]], [fill, half[1]], color);
}

pub(crate) fn push_quad(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    half: [f32; 2],
    color: [f32; 4],
) {
    let (left, right) = (center[0] - half[0], center[0] + half[0]);
    let (bottom, top) = (center[1] - half[1], center[1] + half[1]);
    for position in
        [[left, bottom], [right, bottom], [right, top], [left, bottom], [right, top], [left, top]]
    {
        vertices.push(HudVertex::new(position, color));
    }
}

#[cfg(test)]
mod tests;
