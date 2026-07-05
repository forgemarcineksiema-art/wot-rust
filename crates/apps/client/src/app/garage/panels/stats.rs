//! Right stats list: the assembled vehicle's key numbers with icons. Re-reads `assembled_spec`
//! every frame, so it tracks module/ammo/crew edits live.

use game_core::TankSpec;
use renderer_api::HudVertex;

use crate::app::garage::layout::*;
use crate::hud::font::{push_icon, push_text};
use crate::hud::icons::HudIcon;
use crate::hud::{push_hairline, push_panel};
use crate::ui_strings::garage as strings;

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, spec: &TankSpec, aspect: f32) {
    push_panel(v, [STAT_X, 0.46], [STAT_HALF_X + 0.02, 0.34], CHAMFER_PANEL, aspect, PANEL);
    let left = STAT_X - STAT_HALF_X;
    push_text(v, strings::VEHICLE, left, 0.80, 0.04, aspect, TEXT_DIM);
    push_hairline(v, left, STAT_X + STAT_HALF_X, 0.755, HAIRLINE);

    let rows = [
        (HudIcon::StatHp, format!("{}", spec.hit_points)),
        (
            HudIcon::StatPower,
            format!("{} {}", spec.engine_power_kw.round() as i32, strings::UNIT_KILOWATTS),
        ),
        (
            HudIcon::StatSpeed,
            format!("{} {}", (spec.max_forward_speed_mps * 3.6).round() as i32, strings::UNIT_KMH),
        ),
        (
            HudIcon::StatTraverse,
            format!(
                "{} {}",
                spec.turret_rotation_rad_s.to_degrees().round() as i32,
                strings::UNIT_DEGREES_PER_S
            ),
        ),
        (
            HudIcon::StatPenetration,
            format!(
                "{} {}",
                spec.gun.shell.penetration_mm_at_100m.round() as i32,
                strings::UNIT_MILLIMETERS
            ),
        ),
        (HudIcon::StatReload, format!("{:.1} {}", spec.gun.reload_seconds, strings::UNIT_SECONDS)),
    ];
    for (i, (icon, value)) in rows.iter().enumerate() {
        let y = STAT_TOP - i as f32 * STAT_PITCH;
        push_icon(v, *icon, left, y + 0.03, 0.055, aspect, ICON);
        push_text(v, value, left + 0.07, y + 0.02, 0.04, aspect, VALUE);
    }
}
