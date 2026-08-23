//! The vehicle nameplate over the hero scene: the historical designation with its tier + nation
//! line, celebrating the subject the phase-1a relight put back in the frame. Non-interactive —
//! it never hit-tests; a click through it orbits the camera like any other scene click.

use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::layout::{CHAMFER_PANEL, PANEL, TEXT, TEXT_DIM};
use crate::hud::font::{push_text, text_width};
use crate::hud::push_panel;

pub(in crate::app::garage) const NAMEPLATE_CENTER: [f32; 2] = [0.0, 0.70];
pub(in crate::app::garage) const NAMEPLATE_HALF: [f32; 2] = [0.30, 0.047];

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    let kind = state.selected_vehicle();
    push_panel(v, NAMEPLATE_CENTER, NAMEPLATE_HALF, CHAMFER_PANEL, aspect, PANEL);

    let name = kind.display_name();
    let w = text_width(name, 0.032, aspect);
    push_text(
        v,
        name,
        NAMEPLATE_CENTER[0] - w / 2.0,
        NAMEPLATE_CENTER[1] + 0.038,
        0.032,
        aspect,
        TEXT,
    );

    let sub = format!("{} - {}", game_core::tier_roman(kind.tier()), kind.nation().label());
    let w = text_width(&sub, 0.022, aspect);
    push_text(
        v,
        &sub,
        NAMEPLATE_CENTER[0] - w / 2.0,
        NAMEPLATE_CENTER[1] - 0.004,
        0.022,
        aspect,
        TEXT_DIM,
    );

    // L2: the hero wears battle damage — the plate says so and offers the fix. During the
    // beat the line switches to the work in progress. Earned state only: a clean machine
    // never shows the tag.
    let tag = if state.repair_active() {
        Some(("REPAIRING...", [0.75, 0.72, 0.55, 0.95]))
    } else if state.hero_is_marked() {
        Some(("DAMAGED - [R] REPAIR", [0.86, 0.56, 0.34, 0.95]))
    } else {
        None
    };
    if let Some((line, color)) = tag {
        let w = text_width(line, 0.022, aspect);
        push_text(v, line, NAMEPLATE_CENTER[0] - w / 2.0, 0.625, 0.022, aspect, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::VehicleKind;

    #[test]
    fn the_nameplate_prints_the_designation_with_its_tier_and_nation() {
        let mut state = GarageState::default();
        state.select_vehicle(VehicleKind::IS3);
        let mut v = Vec::new();
        draw(&mut v, &state, 16.0 / 9.0);

        // Panel quad + "IS-3" glyphs + the tier/nation sub-line.
        assert_eq!(
            format!(
                "{} - {}",
                game_core::tier_roman(VehicleKind::IS3.tier()),
                VehicleKind::IS3.nation().label()
            ),
            "VIII - USSR"
        );
        assert!(v.len() >= 80, "the nameplate must print name + tier + nation, got {}", v.len());
        // Every vertex stays in the top-centre band, clear of the tabs above and the hero below.
        assert!(
            v.iter().all(|vert| vert.position[1] > 0.60 && vert.position[1] < 0.80),
            "the plate sits in its band"
        );
    }

    /// L2: the repair offer is EARNED — a clean hero's plate stays quiet, a marked one
    /// grows the damage tag, and the beat swaps it for the work line.
    #[test]
    fn the_plate_offers_repair_only_when_the_hero_is_marked() {
        let mut state = GarageState::default();
        let mut clean = Vec::new();
        draw(&mut clean, &state, 16.0 / 9.0);

        let mut worn = crate::app::garage_render::garage_preview_snapshot(VehicleKind::PLAYABLE[0]);
        worn.destroyed_modules_mask = 0b0000_0100;
        state.wear_from_the_field(crate::app::garage::wear::FieldWear::from_battle(&worn, None));
        let mut marked = Vec::new();
        draw(&mut marked, &state, 16.0 / 9.0);
        assert!(marked.len() > clean.len(), "the damage tag prints only when earned");

        assert!(state.start_repair());
        let mut repairing = Vec::new();
        draw(&mut repairing, &state, 16.0 / 9.0);
        assert_ne!(repairing.len(), marked.len(), "the beat swaps the tag for the work line");
    }

    #[test]
    fn the_nameplate_follows_the_selected_vehicle() {
        let mut state = GarageState::default();
        let mut t54 = Vec::new();
        draw(&mut t54, &state, 16.0 / 9.0);
        state.select_vehicle(VehicleKind::Jagdtiger);
        let mut jagdtiger = Vec::new();
        draw(&mut jagdtiger, &state, 16.0 / 9.0);
        assert_ne!(
            t54.len(),
            jagdtiger.len(),
            "a longer designation prints more glyphs — the plate is live, not static"
        );
    }
}
