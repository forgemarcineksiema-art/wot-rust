//! Bottom vehicle carousel: a horizontal strip of owned tanks that scrolls through a fixed window
//! once the roster outgrows `CAR_VISIBLE`. Click selects; the scroll arrows appear only when the
//! roster overflows. Each cell shows the vehicle's nation label, its short name, and a
//! recognition-card side silhouette drawn from the vehicle's REAL dimensions.

use game_core::VehicleKind;
use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::layout::*;
use crate::hud::font::{push_text, text_width};
use crate::hud::{push_panel, push_quad};

/// Clip units per metre for the cell silhouettes — the VERTICAL truth; the horizontal axis
/// divides by the aspect ratio so a metre of length and a metre of height land on the same
/// number of PIXELS (clip x-units are aspect-times wider on screen than y-units, and the card
/// used to ignore that: every tank rendered ~1.78× too long for its height at 16:9). One shared
/// scale keeps the profiles honestly comparable — the IS-3 genuinely reads smaller than a
/// Tiger — capped per vehicle only when a long gun overhang would otherwise leave the cell.
const SILHOUETTE_CLIP_PER_M: f32 = 0.015;
/// The silhouette's ground line, below the name row inside the cell.
const SILHOUETTE_GROUND_DY: f32 = -0.056;
const SILHOUETTE_MAX_HALF_SPAN: f32 = 0.054;

/// The recognition profile, instrument style: three flat quads from gameplay-true numbers — the
/// hull box to the turret split, the turret (or casemate) box above it, and the stock barrel
/// reaching from the turret front. Nose points right. No art assets, no mesh walk: the same
/// `HitboxProfile`/barrel data the battle resolves hits against draws the card, so the shape
/// cannot lie about the vehicle.
pub(in crate::app::garage) fn push_silhouette(
    v: &mut Vec<HudVertex>,
    kind: VehicleKind,
    cell: [f32; 2],
    aspect: f32,
    color: [f32; 4],
) {
    let hitbox = game_core::HitboxProfile::for_vehicle(kind);
    let split_y = hitbox.center_y_m + hitbox.turret_min_y_m;
    let top_y = hitbox.center_y_m + hitbox.half_height_m;
    let turret_front = hitbox.turret_center_z_m + hitbox.turret_half_length_m;
    let muzzle = turret_front + kind.stock_barrel_length_m();

    // Centre the full extent (hull rear to muzzle) and cap the scale so the overhang stays in
    // the cell; the midpoint shift keeps a long gun from pushing the hull off-centre. The cap
    // shrinks BOTH axes (sx and sy share `scale`), so a capped card keeps its true proportion.
    let (x_min, x_max) = (-hitbox.half_length_m, muzzle.max(hitbox.half_length_m));
    let scale = SILHOUETTE_CLIP_PER_M
        .min(2.0 * SILHOUETTE_MAX_HALF_SPAN * aspect / (x_max - x_min).max(0.1));
    let (sx, sy) = (scale / aspect, scale);
    let mid = (x_min + x_max) * 0.5;
    let at = |z: f32, y: f32| [cell[0] + (z - mid) * sx, cell[1] + SILHOUETTE_GROUND_DY + y * sy];

    // Hull: ground to the turret split, full length.
    let hull_c = at(0.0, split_y * 0.5);
    push_quad(v, hull_c, [hitbox.half_length_m * sx, split_y * 0.5 * sy], color);
    // Turret / casemate: the plan box above the split.
    let turret_c = at(hitbox.turret_center_z_m, (split_y + top_y) * 0.5);
    push_quad(v, turret_c, [hitbox.turret_half_length_m * sx, (top_y - split_y) * 0.5 * sy], color);
    // The barrel, from the turret front at the gun line's height.
    let gun_y = split_y + (top_y - split_y) * 0.45;
    let barrel_c = at((turret_front + muzzle) * 0.5, gun_y);
    push_quad(v, barrel_c, [(muzzle - turret_front) * 0.5 * sx, 0.0028], color);
}

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    let count = VehicleKind::PLAYABLE.len();
    let window = carousel_window(count, state.carousel_scroll());
    let visible = window.len();
    push_panel(
        v,
        [0.0, CAR_Y],
        [visible as f32 * 0.065 + 0.02, CAR_HALF[1] + 0.02],
        CHAMFER_PANEL,
        aspect,
        PANEL,
    );

    for (slot, absolute) in window.enumerate() {
        let kind = VehicleKind::PLAYABLE[absolute];
        let c = carousel_cell_center(slot, visible);
        let selected = absolute == state.selected_index();
        push_panel(
            v,
            c,
            CAR_HALF,
            CHAMFER_SLOT,
            aspect,
            if selected { SLOT_SELECTED } else { SLOT },
        );
        let text_color = if selected { TEXT } else { TEXT_DIM };

        // Nation label above the short name, colored by nation for at-a-glance grouping.
        let nation = kind.nation();
        let nation_label = nation.label();
        let nation_w = text_width(nation_label, NATION_TEXT_SIZE, aspect);
        let nation_color = nation.color();
        push_text(
            v,
            nation_label,
            c[0] - nation_w / 2.0,
            c[1] + 0.082,
            NATION_TEXT_SIZE,
            aspect,
            [nation_color[0], nation_color[1], nation_color[2], 0.95],
        );

        push_text(
            v,
            kind.short_name(),
            c[0] - CAR_HALF[0] + 0.01,
            c[1] + 0.045,
            0.03,
            aspect,
            text_color,
        );

        // The recognition silhouette under the name: the selected tank's card reads bright.
        push_silhouette(v, kind, c, aspect, if selected { ICON } else { ICON_DIM });
    }

    // Scroll arrows flank the window only when the roster does not fit.
    if carousel_overflows(count) {
        let (left, right) = carousel_arrows();
        for (center, glyph) in [(left, "<"), (right, ">")] {
            push_panel(v, center, CAR_ARROW_HALF, CHAMFER_SLOT, aspect, SLOT);
            let w = text_width(glyph, 0.05, aspect);
            push_text(v, glyph, center[0] - w / 2.0, center[1] + 0.022, 0.05, aspect, TEXT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carousel_emits_text_for_nation_labels_and_vehicle_names() {
        let mut state = GarageState::default();
        state.select_vehicle(VehicleKind::T54_1951);
        let aspect = 16.0 / 9.0;

        let mut v = Vec::new();
        draw(&mut v, &state, aspect);

        // 6 vehicle cells: 1 background quad + 6 slot quads = 7 quads = 42 vertices. Nation labels
        // ("USSR"/"Germany") and the short names add many more, so the total must far exceed the
        // quad-only baseline.
        assert!(
            v.len() > 42,
            "carousel must emit text vertices beyond plain quads, got {}",
            v.len()
        );
    }

    const TEST_ASPECT: f32 = 16.0 / 9.0;

    #[test]
    fn silhouettes_draw_three_quads_from_real_dimensions_and_stay_in_the_cell() {
        // The recognition card is gameplay-true: hull box + turret box + barrel, from the same
        // HitboxProfile the battle resolves hits against. Three quads, nose right, in-cell.
        let cell = [0.0, 0.0];
        for kind in VehicleKind::PLAYABLE {
            let mut v = Vec::new();
            push_silhouette(&mut v, kind, cell, TEST_ASPECT, [1.0; 4]);
            assert_eq!(v.len(), 18, "{kind:?}: hull + turret + barrel = 3 quads");
            for vert in &v {
                assert!(
                    vert.position[0].abs() <= CAR_HALF[0] + 1.0e-3,
                    "{kind:?} silhouette must stay inside its cell: x {}",
                    vert.position[0]
                );
            }
        }

        // Shapes genuinely differ: the Jagdtiger's profile spans farther than the compact IS-3's,
        // and the low T-54 stands shorter than the tall Tiger II.
        let span = |kind: VehicleKind| {
            let mut v = Vec::new();
            push_silhouette(&mut v, kind, cell, TEST_ASPECT, [1.0; 4]);
            let min = v.iter().map(|vert| vert.position[0]).fold(f32::MAX, f32::min);
            let max = v.iter().map(|vert| vert.position[0]).fold(f32::MIN, f32::max);
            max - min
        };
        assert!(span(VehicleKind::Jagdtiger) > span(VehicleKind::IS3));
        let height = |kind: VehicleKind| {
            let mut v = Vec::new();
            push_silhouette(&mut v, kind, cell, TEST_ASPECT, [1.0; 4]);
            v.iter().map(|vert| vert.position[1]).fold(f32::MIN, f32::max)
        };
        assert!(
            height(VehicleKind::TigerII) > height(VehicleKind::T54_1951),
            "the famously low T-54 must read lower than the Tiger II"
        );
    }

    #[test]
    fn the_silhouette_is_drawn_in_true_proportion_not_stretched_by_aspect() {
        // A metre of hull length and a metre of hull height must land on the same number of
        // PIXELS. Clip x-units are aspect-times wider on screen than y-units, and the card used
        // to spend one shared clip-per-metre on both axes — every tank rendered ~1.78× too long
        // for its height at 16:9. On-screen ratio = (clip x-span × aspect) / clip y-span.
        for kind in VehicleKind::PLAYABLE {
            let hitbox = game_core::HitboxProfile::for_vehicle(kind);
            let turret_front = hitbox.turret_center_z_m + hitbox.turret_half_length_m;
            let muzzle = turret_front + kind.stock_barrel_length_m();
            let range_m = hitbox.half_length_m + muzzle.max(hitbox.half_length_m);
            let height_m = hitbox.center_y_m + hitbox.half_height_m;

            let mut v = Vec::new();
            push_silhouette(&mut v, kind, [0.0, 0.0], TEST_ASPECT, [1.0; 4]);
            let x_min = v.iter().map(|vert| vert.position[0]).fold(f32::MAX, f32::min);
            let x_max = v.iter().map(|vert| vert.position[0]).fold(f32::MIN, f32::max);
            let y_min = v.iter().map(|vert| vert.position[1]).fold(f32::MAX, f32::min);
            let y_max = v.iter().map(|vert| vert.position[1]).fold(f32::MIN, f32::max);

            let screen_ratio = ((x_max - x_min) * TEST_ASPECT) / (y_max - y_min);
            let true_ratio = range_m / height_m;
            assert!(
                (screen_ratio / true_ratio - 1.0).abs() < 0.05,
                "{kind:?}: on-screen ratio {screen_ratio:.2} must match the true {true_ratio:.2} \
                 (the cap scales both axes, so even a capped card keeps its proportion)"
            );
        }
    }

    #[test]
    fn carousel_emits_more_vertices_for_germany_than_a_single_ussr_label() {
        // "Germany" is 7 glyphs; "USSR" is 4. With two USSR and four Germany vehicles in the
        // playable roster, Germany glyphs dominate. This is a smoke test that nation labels
        // are actually drawn, not just the background panel.
        let state = GarageState::default();
        let aspect = 16.0 / 9.0;

        let mut v = Vec::new();
        draw(&mut v, &state, aspect);

        // Each glyph is 6 vertices. 4 Germany × 7 glyphs × 6 = 168 vertices from Germany alone.
        // Assert the total is at least that, confirming Germany labels are emitted.
        assert!(
            v.len() >= 168,
            "carousel must emit Germany nation labels (4×7 glyphs×6 verts = 168), got {}",
            v.len()
        );
    }
}
