use game_core::ModuleSlot;
use renderer_api::HudVertex;

use crate::hud::push_quad;

/// The outcome families' tones (interface program H9): the theme's quiet floating tokens —
/// penetration, held (a bounce, near or not), ricochet, shatter. One muted tone each, never
/// the shell's colour, never neon (Inny Poziom S7; the owner's „nie natrętnie").
fn family_tone(index: usize) -> [f32; 4] {
    ui_kit::theme::Theme::standard().semantic.floating[index]
}
/// The near-penetration's own heat: between the bounce's yellow and the ricochet's red — the
/// "same spot again" cue, still a glyph and nothing more (no mm duel).
const WHT: [f32; 4] = [0.92, 0.90, 0.86, 0.95];

pub(super) fn fade(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], (c[3] * a).clamp(0.0, 1.0)]
}

/// The one-glyph outcome taxonomy the marker draws: penetration / bounce / shatter /
/// near-penetration rattle.
#[derive(Clone, Copy)]
pub(super) struct MarkerOutcome {
    pub pen: bool,
    pub ric: bool,
    pub shattered: bool,
}

pub(super) fn color_for(outcome: MarkerOutcome) -> [f32; 4] {
    if outcome.pen {
        family_tone(0)
    } else if outcome.shattered {
        family_tone(3)
    } else if outcome.ric {
        family_tone(2)
    } else {
        // A held shell, near penetration or not: the same family, the same tone — the near
        // miss is the reticle's story (its verdict), not a colour of its own on the number.
        family_tone(1)
    }
}

pub(super) fn push_marker(
    verts: &mut Vec<HudVertex>,
    c: [f32; 2],
    outcome: MarkerOutcome,
    a: f32,
    asp: f32,
) {
    let h: [f32; 2] = [0.005 / asp, 0.005];
    // One quiet tone per family (H9): the number and its glyph agree, and the near miss is the
    // reticle's story (its verdict), not a colour of its own here.
    let tint = color_for(outcome);
    if outcome.pen {
        push_quad(verts, c, h, fade(tint, a));
    } else if outcome.shattered {
        // The core DIED on the plate (v47): a cross, not the skip's bar — "that angle eats
        // tungsten" is a different lesson than "it skipped somewhere".
        let o = 0.0012;
        push_quad(verts, c, [h[0], o], fade(tint, a));
        push_quad(verts, c, [o / asp, h[1]], fade(tint, a));
    } else if outcome.ric {
        push_quad(verts, c, [0.004 / asp, h[1]], fade(tint, a));
    } else {
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
