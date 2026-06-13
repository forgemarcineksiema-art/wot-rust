//! The right-hand fitting panels (Modules / Ammo / Crew), the live stats panel, and the Battle
//! button. Geometry and palette come from [`super::layout`] so the hit test in [`super::overlay`]
//! stays aligned with what is drawn here.

use game_core::TankSpec;
use renderer_api::HudVertex;

use super::draft::FitSlot;
use super::layout::*;
use super::GarageState;
use crate::hud::push_quad;
use crate::hud_font::{push_text, text_width};

pub(super) fn push_modules_panel(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, [0.58, 0.46], [0.40, 0.33], PANEL);
    for (i, slot) in FitSlot::ALL.into_iter().enumerate() {
        let y = PANEL_ROW_TOP - i as f32 * PANEL_ROW_PITCH;
        push_text(v, slot.label(), 0.22, y + 0.03, 0.045, aspect, TEXT_DIM);
        let name = truncate(&state.draft().module_name(slot), 22);
        push_text(v, &name, 0.40, y + 0.03, 0.045, aspect, TEXT);
        if state.draft().has_choice(slot) {
            push_arrow(v, [ARROW_LEFT_X, y], false, aspect);
            push_arrow(v, [ARROW_RIGHT_X, y], true, aspect);
        }
    }
}

pub(super) fn push_ammo_panel(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, [0.58, 0.46], [0.40, 0.33], PANEL);
    let options = state.draft().ammo_options();
    let selected = state.draft().ammo_index();
    for (i, shell) in options.iter().enumerate() {
        let y = AMMO_TOP - i as f32 * AMMO_PITCH;
        push_quad(v, [AMMO_CENTER_X, y], AMMO_HALF, if i == selected { ROW_SELECTED } else { ROW });
        let color = if i == selected { TEXT_DARK } else { TEXT };
        push_text(v, shell_label(shell.shell_type), AMMO_CENTER_X - AMMO_HALF[0] + 0.02, y + 0.02, 0.045, aspect, color);
        let stats = format!("PEN {}  DMG {}", shell.penetration_mm_at_100m.round() as u32, shell.damage_hp);
        push_text(v, &stats, AMMO_CENTER_X - 0.05, y + 0.02, 0.04, aspect, color);
    }
}

pub(super) fn push_crew_panel(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, [0.58, 0.46], [0.40, 0.33], PANEL);
    for (i, role) in game_core::Crew::roles().into_iter().enumerate() {
        let y = 0.70 - i as f32 * 0.10;
        push_text(v, role.label(), 0.22, y + 0.025, 0.045, aspect, TEXT);
    }
    push_text(v, "Proficiency", 0.22, PROF_Y + 0.03, 0.045, aspect, TEXT_DIM);
    push_arrow(v, [PROF_LEFT_X, PROF_Y], false, aspect);
    push_arrow(v, [PROF_RIGHT_X, PROF_Y], true, aspect);
    let pct = (state.draft().crew().proficiency() * 100.0).round() as u32;
    let label = format!("{pct}%");
    let w = text_width(&label, 0.05, aspect);
    push_text(v, &label, (PROF_LEFT_X + PROF_RIGHT_X) / 2.0 - w / 2.0, PROF_Y + 0.028, 0.05, aspect, TEXT);
}

pub(super) fn push_stats_panel(v: &mut Vec<HudVertex>, spec: &TankSpec, aspect: f32) {
    push_quad(v, [0.58, -0.30], [0.40, 0.22], PANEL);
    push_labeled_stat(v, "HP", [0.36, -0.14], spec.hit_points as f32 / fleet_max_hp(), STAT_HP, aspect);
    push_labeled_stat(v, "SPD", [0.36, -0.24], spec.max_forward_speed_mps / fleet_max_speed(), STAT_SPEED, aspect);
    push_labeled_stat(v, "RLD", [0.36, -0.34], fleet_min_reload() / spec.gun.reload_seconds, STAT_RELOAD, aspect);
    let line = format!(
        "PEN {}mm   PWR {}kW",
        spec.gun.shell.penetration_mm_at_100m.round() as u32,
        spec.engine_power_kw.round() as u32,
    );
    push_text(v, &line, 0.22, -0.44, 0.04, aspect, TEXT_DIM);
}

pub(super) fn push_battle_button(v: &mut Vec<HudVertex>, aspect: f32) {
    push_quad(v, BATTLE_CENTER, BATTLE_HALF, BATTLE);
    let w = text_width("BITWA", 0.08, aspect);
    push_text(v, "BITWA", BATTLE_CENTER[0] - w / 2.0, BATTLE_CENTER[1] + 0.04, 0.08, aspect, TEXT);
}

fn push_arrow(v: &mut Vec<HudVertex>, center: [f32; 2], right: bool, aspect: f32) {
    push_quad(v, center, ARROW_HALF, BUTTON);
    let glyph = if right { ">" } else { "<" };
    let w = text_width(glyph, 0.05, aspect);
    push_text(v, glyph, center[0] - w / 2.0, center[1] + 0.025, 0.05, aspect, TEXT);
}

fn push_labeled_stat(
    v: &mut Vec<HudVertex>,
    label: &str,
    center: [f32; 2],
    fraction: f32,
    color: [f32; 4],
    aspect: f32,
) {
    push_text(v, label, center[0] - 0.14, center[1] + 0.022, 0.04, aspect, TEXT_DIM);
    let half = [0.26, 0.014];
    let bar_center = [center[0] + 0.30, center[1]];
    push_quad(v, bar_center, half, [0.0, 0.0, 0.0, 0.55]);
    let fill = half[0] * fraction.clamp(0.0, 1.0);
    push_quad(v, [bar_center[0] - half[0] + fill, bar_center[1]], [fill, half[1]], color);
}
