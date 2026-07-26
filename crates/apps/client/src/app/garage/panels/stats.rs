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
    let (panel_center, panel_half) = stat_panel();
    push_panel(v, panel_center, panel_half, CHAMFER_PANEL, aspect, PANEL);
    let left = STAT_X - STAT_HALF_X;
    push_text(v, strings::VEHICLE, left, PANEL_TOP, 0.04, aspect, TEXT_DIM);
    // The matchmaking bracket, right-aligned on the header row: a battle is one era, and the
    // player should know which one this hull deploys into before pressing Battle.
    let era = spec.kind.era().label();
    let era_width = crate::hud::font::text_width(era, 0.04, aspect);
    push_text(v, era, STAT_X + STAT_HALF_X - era_width, PANEL_TOP, 0.04, aspect, VALUE);
    push_hairline(v, left, STAT_X + STAT_HALF_X, 0.755, HAIRLINE);

    for (i, (icon, value)) in rows(spec).iter().enumerate() {
        let y = STAT_TOP - i as f32 * STAT_PITCH;
        push_icon(v, *icon, left, y + 0.03, 0.055, aspect, ICON);
        push_text(v, value, left + 0.07, y + 0.02, 0.04, aspect, VALUE);
    }
}

/// The column, top to bottom, as a sentence about the hull: what keeps it alive, what moves it,
/// what it kills with. Separated from the drawing so the *content* of the column can be locked
/// directly — a vertex buffer can only ever prove that something was drawn, not what it said.
///
/// The four firepower rows are what the six-row column used to compress into "penetration +
/// reload". A gun in this game is chosen on its GROUPING and on how long it takes to get there —
/// `dispersion_mrad` and `aim_time_seconds` are what "no +-25% roll" means in numbers — and
/// neither was on the screen where the player picks a gun and presses Battle.
fn rows(spec: &TankSpec) -> [(HudIcon, String); STAT_ROWS] {
    [
        (HudIcon::StatHp, format!("{}", spec.hit_points)),
        (
            HudIcon::StatArmor,
            format!(
                "{} / {} {}",
                spec.hull.hull_front_mm.round() as i32,
                spec.hull.turret_front_mm.round() as i32,
                strings::UNIT_MILLIMETERS
            ),
        ),
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
        (
            HudIcon::StatDispersion,
            format!("{:.2} {}", spec.gun.dispersion_mrad, strings::UNIT_MRAD),
        ),
        (
            HudIcon::StatAimTime,
            format!("{:.1} {}", spec.gun.aim_time_seconds, strings::UNIT_SECONDS),
        ),
        (HudIcon::StatReload, format!("{:.1} {}", spec.gun.reload_seconds, strings::UNIT_SECONDS)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The header row carries the era bracket: the only VALUE-colored glyphs above the hairline
    /// (every stat value sits at `STAT_TOP` and below). The player must see which era this hull
    /// deploys into before pressing Battle.
    #[test]
    fn the_stats_header_carries_the_vehicles_era_bracket() {
        let mut vertices = Vec::new();
        draw(&mut vertices, &game_core::VehicleKind::TigerI.spec(), 16.0 / 9.0);
        let header_value_glyphs =
            vertices.iter().filter(|v| v.color == VALUE && v.position[1] > 0.78).count();
        assert!(header_value_glyphs > 0, "the era label must sit on the header row");
    }

    /// The column has to carry the promise the game is sold on. `dispersion_mrad` and
    /// `aim_time_seconds` are what "no +-25% roll" means in numbers, and the armour the shell has
    /// to beat is the other half of every fight — none of the three reached the screen where the
    /// player picks a gun and presses Battle. Asserting the rendered STRING, units included: a
    /// bare number in an unlabelled column is exactly the failure this row set exists to answer.
    #[test]
    fn the_column_prints_the_aiming_promise_and_the_armour() {
        let spec = game_core::VehicleKind::T54_1951.spec();
        let printed: Vec<String> = rows(&spec).into_iter().map(|(_, value)| value).collect();

        let dispersion = format!("{:.2} mrad", spec.gun.dispersion_mrad);
        let aim_time = format!("{:.1} s", spec.gun.aim_time_seconds);
        let armor = format!(
            "{} / {} mm",
            spec.hull.hull_front_mm.round() as i32,
            spec.hull.turret_front_mm.round() as i32
        );
        for expected in [&dispersion, &aim_time, &armor] {
            assert!(printed.contains(expected), "the column must print {expected:?}: {printed:?}");
        }
    }

    /// The rows track the assembled spec, so swapping to a gun that groups differently changes the
    /// column. Locked against a regression where a row prints a constant and looks alive.
    #[test]
    fn the_dispersion_row_follows_the_fitted_gun() {
        let mut spec = game_core::VehicleKind::T54_1951.spec();
        let stock = rows(&spec)[6].1.clone();
        spec.gun.dispersion_mrad += 0.10;
        assert_ne!(stock, rows(&spec)[6].1, "the dispersion row must read the fitted gun");
    }

    /// Every row carries its own glyph. The column has no text labels — nine unlabelled rows mean
    /// nine icons that each have to say what they are (the masks are locked distinct in
    /// `hud::icons::no_two_icons_raster_to_the_same_mask`).
    #[test]
    fn no_two_rows_share_an_icon() {
        let icons: Vec<HudIcon> =
            rows(&game_core::VehicleKind::TigerI.spec()).into_iter().map(|(i, _)| i).collect();
        assert_eq!(icons.len(), STAT_ROWS);
        for (i, icon) in icons.iter().enumerate() {
            assert!(!icons[i + 1..].contains(icon), "{icon:?} appears twice in the column");
        }
    }

    /// Every row prints inside its plate. The six-row column was hand-sized beside its rows, so
    /// the seventh would have printed onto the hangar floor. `stat_panel()` derives the plate from
    /// `STAT_ROWS` instead; this proves the derivation actually covers the glyphs.
    #[test]
    fn every_stat_row_prints_inside_the_vehicle_plate() {
        let (center, half) = stat_panel();
        let (bottom, top) = (center[1] - half[1], center[1] + half[1]);

        for kind in game_core::VehicleKind::PLAYABLE {
            let mut vertices = Vec::new();
            draw(&mut vertices, &kind.spec(), 16.0 / 9.0);
            for vertex in &vertices {
                let y = vertex.position[1];
                assert!(
                    y >= bottom - 1.0e-4 && y <= top + 1.0e-4,
                    "{kind:?}: a stat vertex at y={y} sits outside the plate {bottom}..{top}"
                );
            }
        }
    }

    /// Regression: the first data row used to render its icon above the 0.755 header hairline,
    /// colliding with the "VEHICLE"/"CREW" header (visible as a clipped top stat). The highest
    /// row-0 pixel is the icon top at `TOP + icon_offset`; it must clear the hairline. If a future
    /// edit raises `STAT_TOP`/`CREW_TOP` back toward the header, this catches it.
    #[test]
    fn first_row_clears_the_header_hairline() {
        const HAIRLINE_Y: f32 = 0.755;
        // Compile-time locks (the layout values are all consts): the highest row-0 pixel is the icon
        // top at `TOP + icon_offset` — 0.03 here and 0.035 in crew.rs, mirroring the `push_icon`
        // calls. It must clear the header hairline; raising STAT_TOP/CREW_TOP back toward the header
        // fails the build.
        const { assert!(STAT_TOP + 0.03 < HAIRLINE_Y, "stats row 0 must clear the header hairline") };
        const { assert!(CREW_TOP + 0.035 < HAIRLINE_Y, "crew row 0 must clear the header hairline") };
    }
}
