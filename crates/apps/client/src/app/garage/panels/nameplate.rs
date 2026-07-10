//! The vehicle nameplate over the hero scene: the historical designation with its era + nation
//! line, celebrating the subject the phase-1a relight put back in the frame. Non-interactive —
//! it never hit-tests; a click through it orbits the camera like any other scene click.

use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::layout::{CHAMFER_PANEL, PANEL, TEXT, TEXT_DIM};
use crate::hud::font::{push_text, text_width};
use crate::hud::push_panel;

pub(in crate::app::garage) const NAMEPLATE_CENTER: [f32; 2] = [0.0, 0.70];
const NAMEPLATE_HALF: [f32; 2] = [0.30, 0.047];

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

    let sub = format!("{} - {}", kind.era().label(), kind.nation().label());
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::VehicleKind;

    #[test]
    fn the_nameplate_prints_the_designation_with_its_era_and_nation() {
        let mut state = GarageState::default();
        state.select_vehicle(VehicleKind::IS3);
        let mut v = Vec::new();
        draw(&mut v, &state, 16.0 / 9.0);

        // Panel quad (18 chamfered verts) + "IS-3" glyphs + the era/nation sub-line glyphs.
        // "IS-3" (4) + "Era III - USSR" (12 visible) = 16 glyphs × 6 = 96 verts over the panel.
        assert!(v.len() >= 96, "the nameplate must print name + era + nation, got {}", v.len());
        // Every vertex stays in the top-centre band, clear of the tabs above and the hero below.
        assert!(
            v.iter().all(|vert| vert.position[1] > 0.60 && vert.position[1] < 0.80),
            "the plate sits in its band"
        );
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
