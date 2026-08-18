use game_core::ModuleSlot;
use renderer_api::HudVertex;

use crate::hud::push_quad;

pub(super) const GRN: [f32; 4] = [0.30, 0.82, 0.34, 1.0];
pub(super) const RED: [f32; 4] = [0.90, 0.26, 0.22, 1.0];
const YLW: [f32; 4] = [0.92, 0.78, 0.20, 1.0];
/// The near-penetration's own heat: between the bounce's yellow and the ricochet's red — the
/// "same spot again" cue, still a glyph and nothing more (no mm duel).
const ORN: [f32; 4] = [0.95, 0.55, 0.18, 1.0];
const WHT: [f32; 4] = [0.92, 0.90, 0.86, 0.95];

pub(super) fn fade(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], (c[3] * a).clamp(0.0, 1.0)]
}

pub(super) fn color_for(pen: bool, ric: bool, near_pen: bool) -> [f32; 4] {
    if pen {
        GRN
    } else if ric {
        RED
    } else if near_pen {
        ORN
    } else {
        YLW
    }
}

pub(super) fn push_marker(
    verts: &mut Vec<HudVertex>,
    c: [f32; 2],
    pen: bool,
    ric: bool,
    near_pen: bool,
    a: f32,
    asp: f32,
) {
    let h: [f32; 2] = [0.005 / asp, 0.005];
    if pen {
        push_quad(verts, c, h, fade(GRN, a));
    } else if ric {
        push_quad(verts, c, [0.004 / asp, h[1]], fade(RED, a));
    } else {
        // The non-pen cross; a near-penetration heats it orange — that was CLOSE, same spot
        // again — from numbers the shooter already owns, never from concealed intel.
        let tint = if near_pen { ORN } else { YLW };
        let o = 0.001;
        push_quad(verts, c, [h[0], o], fade(tint, a));
        push_quad(verts, c, [o / asp, h[1]], fade(tint, a));
    }
}

pub(super) fn push_module_icon(
    verts: &mut Vec<HudVertex>,
    c: [f32; 2],
    module: ModuleSlot,
    a: f32,
    asp: f32,
) {
    let s = 0.005;
    let hw = s / asp;
    let hh = s;
    match module {
        ModuleSlot::Engine => {
            push_quad(verts, [c[0], c[1] + hh * 0.7], [hw, hh * 0.35], fade(WHT, a));
            push_quad(
                verts,
                [c[0] + hw * 0.3, c[1] - hh * 0.15],
                [hw * 0.25, hh * 0.5],
                fade(WHT, a),
            );
            push_quad(
                verts,
                [c[0] - hw * 0.3, c[1] - hh * 0.15],
                [hw * 0.25, hh * 0.5],
                fade(WHT, a),
            );
        }
        ModuleSlot::Suspension => {
            push_quad(verts, c, [hw * 1.8, hh * 0.25], fade(WHT, a));
            push_quad(verts, [c[0] - hw * 0.5, c[1]], [hw * 0.2, hh], fade(WHT, a));
            push_quad(verts, [c[0] + hw * 0.5, c[1]], [hw * 0.2, hh], fade(WHT, a));
        }
        ModuleSlot::Turret => {
            push_quad(verts, c, [hw, hh * 0.15], fade(WHT, a));
            push_quad(verts, c, [hw * 0.15, hh], fade(WHT, a));
            push_quad(verts, [c[0] - hw * 0.45, c[1]], [hw * 0.3, hh * 0.15], fade(WHT, a));
        }
        ModuleSlot::Gun => {
            push_quad(verts, c, [hw * 0.25, hh], fade(WHT, a));
            push_quad(verts, [c[0], c[1] + hh * 0.6], [hw * 0.35, hh * 0.15], fade(WHT, a));
        }
        ModuleSlot::AmmoRack => {
            push_quad(verts, c, [hw * 0.2, hh], fade(WHT, a));
            push_quad(verts, c, [hw, hh * 0.15], fade(WHT, a));
        }
        ModuleSlot::Radio => {
            push_quad(verts, c, [hw * 0.35, hh * 0.35], fade(WHT, a));
        }
    }
}
