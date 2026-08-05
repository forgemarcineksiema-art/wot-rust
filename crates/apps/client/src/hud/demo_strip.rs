//! The reticle contact sheet: every state the sight can be in, drawn side by side on the two
//! grounds it has to survive — pale straw and deep shade.
//!
//! The vertex locks say WHAT draws; they cannot say whether a player reads it. This strip is the
//! other half: one `cargo run -p client --example probe -- reticle_strip` puts the whole glyph
//! language on one page, so a change to any mark is reviewed against every other mark instead of
//! against one staged frame that happened to catch a single state.
//!
//! Text readouts (distance, pen/armor mm) deliberately stay out: they belong to the populated
//! `battle_hud` frame, and their fixed offsets would spill across neighbouring cells here.

use renderer_api::HudVertex;

use super::reticle::{PenetrationHint, ReticleMode, ReticleStatus};
use super::reticle_overlay::HudReticle;
use super::reticle_readouts::HitConfirm;
use super::{primitives, reticle_marks, reticle_overlay};

/// Grid of the contact sheet: five states across, the whole set drawn twice (straw, then shade).
const COLUMNS: usize = 5;
const ROWS: usize = 4;
/// The pale straw steppe — the ground the old 0.38-alpha ring used to vanish against.
const STRAW: [f32; 4] = [0.78, 0.75, 0.55, 1.0];
/// Deep tree shade: the other end of the contrast range every mark has to hold.
const SHADE: [f32; 4] = [0.10, 0.13, 0.11, 1.0];

/// One cell: a named sight state plus the two reticle beats that live outside `push_reticle`.
struct StripCell {
    label: &'static str,
    reticle: HudReticle,
    ready_age_s: Option<f32>,
    denied_age_s: Option<f32>,
}

fn base(mode: ReticleMode) -> HudReticle {
    HudReticle {
        aim_clip: [0.0, 0.0],
        impact_clip: None,
        gun_clip: None,
        aim_radius_clip: 0.055,
        // Out on purpose (see the module note): the readouts' fixed offsets would cross cells.
        target_distance_m: None,
        status: ReticleStatus::Clear,
        penetration_hint: None,
        reload_fraction: 1.0,
        hit_confirm: None,
        mode,
        converged: false,
    }
}

fn cell(label: &'static str, reticle: HudReticle) -> StripCell {
    StripCell { label, reticle, ready_age_s: None, denied_age_s: None }
}

fn hint(penetrates: bool) -> PenetrationHint {
    PenetrationHint {
        penetrates,
        shell_pen_mm: 201.0,
        armor_mm: 120.0,
        facing: game_core::ArmorFacing::HullFront,
    }
}

/// The ten states worth comparing against each other, in the order a shot lives through them.
fn cells() -> Vec<StripCell> {
    let tpp = base(ReticleMode::ThirdPerson);
    let sniper = base(ReticleMode::Sniper);
    vec![
        cell("LOADING", HudReticle { reload_fraction: 0.35, ..tpp }),
        StripCell { ready_age_s: Some(0.0), ..cell("LOADED", tpp) },
        cell("CONVERGED", HudReticle { aim_radius_clip: 0.022, converged: true, ..tpp }),
        cell("BLOOMED", HudReticle { aim_radius_clip: 0.135, reload_fraction: 0.6, ..tpp }),
        StripCell {
            denied_age_s: Some(0.04),
            ..cell("DENIED", HudReticle { reload_fraction: 0.5, ..tpp })
        },
        cell("BLOCKED", HudReticle { status: ReticleStatus::Blocked, ..tpp }),
        cell("GUN LAG", HudReticle { gun_clip: Some([0.055, 0.030]), ..tpp }),
        cell(
            "HIT PEN",
            HudReticle {
                hit_confirm: Some(HitConfirm { age_s: 0.08, penetrated: true, ricocheted: false }),
                ..tpp
            },
        ),
        cell(
            "SNIPER PEN",
            HudReticle {
                penetration_hint: Some(hint(true)),
                impact_clip: Some([0.045, -0.055]),
                ..sniper
            },
        ),
        cell("SNIPER BOUNCE", HudReticle { penetration_hint: Some(hint(false)), ..sniper }),
    ]
}

/// Build the contact sheet. Each state draws twice: once over straw, once over shade.
pub fn demo_reticle_strip(aspect: f32) -> Vec<HudVertex> {
    let mut vertices = Vec::new();
    let states = cells();
    let (cell_w, cell_h) = (2.0 / COLUMNS as f32, 2.0 / ROWS as f32);
    for row in 0..ROWS {
        // Rows 0-1 hold the whole set on straw, rows 2-3 repeat it on shade.
        let dark = row >= ROWS / 2;
        let first = if row % (ROWS / 2) == 0 { 0 } else { COLUMNS };
        for column in 0..COLUMNS {
            let Some(state) = states.get(first + column) else { continue };
            let center = [-1.0 + (column as f32 + 0.5) * cell_w, 1.0 - (row as f32 + 0.5) * cell_h];
            primitives::push_quad(
                &mut vertices,
                center,
                [cell_w * 0.5, cell_h * 0.5],
                if dark { SHADE } else { STRAW },
            );
            let reticle = HudReticle {
                aim_clip: center,
                gun_clip: state.reticle.gun_clip.map(|g| [center[0] + g[0], center[1] + g[1]]),
                impact_clip: state
                    .reticle
                    .impact_clip
                    .map(|i| [center[0] + i[0], center[1] + i[1]]),
                ..state.reticle
            };
            reticle_overlay::push_reticle(&mut vertices, &reticle, aspect);
            if let Some(age_s) = state.ready_age_s {
                reticle_marks::push_ready_ring(
                    &mut vertices,
                    center,
                    age_s,
                    reticle.aim_radius_clip,
                    aspect,
                );
            }
            if let Some(age_s) = state.denied_age_s {
                reticle_marks::push_denied_flash(&mut vertices, center, age_s, aspect);
            }
            // The label reads against its own ground, not the other one.
            let ink = if dark { [0.92, 0.92, 0.86, 0.95] } else { [0.06, 0.06, 0.05, 0.95] };
            super::font::push_text(
                &mut vertices,
                state.label,
                center[0] - cell_w * 0.45,
                center[1] - cell_h * 0.40,
                0.030,
                aspect,
                ink,
            );
        }
    }
    vertices
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sheet must actually carry every state on both grounds — a silently short strip would
    /// review a sight that is only half drawn.
    #[test]
    fn the_strip_draws_every_state_over_both_grounds() {
        let vertices = demo_reticle_strip(16.0 / 9.0);

        let grounds = |color: [f32; 4]| vertices.iter().filter(|v| v.color == color).count();
        // Six vertices per background quad, one quad per cell, ten cells per ground.
        assert_eq!(grounds(STRAW), cells().len() * 6, "every state draws over straw");
        assert_eq!(grounds(SHADE), cells().len() * 6, "and every state draws over shade");

        // And the states are genuinely different pictures: the loaded ring, the red arc, the
        // blocked form and the two pen verdicts all appear somewhere on the sheet.
        for (name, color) in [
            ("loaded", reticle_overlay::RETICLE_LOADED),
            ("loading", reticle_overlay::RETICLE_RELOAD),
            ("blocked", reticle_overlay::RETICLE_BLOCKED),
            ("pen", reticle_overlay::RETICLE_PEN),
            ("no pen", reticle_overlay::RETICLE_NO_PEN),
        ] {
            assert!(
                vertices.iter().any(|v| v.color[..3] == color[..3]),
                "the {name} state must appear on the contact sheet"
            );
        }
    }
}
