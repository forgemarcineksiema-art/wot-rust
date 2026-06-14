//! Right stats list: the assembled vehicle's key numbers with icons. Re-reads `assembled_spec`
//! every frame, so it tracks module/ammo/crew edits live.

use game_core::TankSpec;
use renderer_api::HudVertex;

use crate::app::garage::layout::*;
use crate::hud::push_quad;
use crate::hud_font::{push_icon, push_text};
use crate::hud_icons::HudIcon;

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, spec: &TankSpec, aspect: f32) {
    push_quad(v, [STAT_X, 0.46], [STAT_HALF_X + 0.02, 0.34], PANEL);
    let left = STAT_X - STAT_HALF_X;
    push_text(v, "VEHICLE", left, 0.80, 0.04, aspect, TEXT_DIM);

    let rows = [
        (HudIcon::StatHp, format!("{}", spec.hit_points)),
        (HudIcon::StatPower, format!("{} kW", spec.engine_power_kw.round() as i32)),
        (HudIcon::StatSpeed, format!("{} km/h", (spec.max_forward_speed_mps * 3.6).round() as i32)),
        (
            HudIcon::StatTraverse,
            format!("{} d/s", spec.turret_rotation_rad_s.to_degrees().round() as i32),
        ),
        (
            HudIcon::StatPenetration,
            format!("{} mm", spec.gun.shell.penetration_mm_at_100m.round() as i32),
        ),
        (HudIcon::StatReload, format!("{:.1} s", spec.gun.reload_seconds)),
    ];
    for (i, (icon, value)) in rows.iter().enumerate() {
        let y = STAT_TOP - i as f32 * STAT_PITCH;
        push_icon(v, *icon, left, y + 0.03, 0.055, aspect, ICON);
        push_text(v, value, left + 0.07, y + 0.02, 0.04, aspect, VALUE);
    }
}
