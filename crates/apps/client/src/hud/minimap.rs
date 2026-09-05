//! Battle minimap: a bottom-right instrument square showing the play area's relief, static cover
//! footprints, the camera's view wedge, and blips for the player, allies, and spotted enemies.
//! The relief and the water are BAKED into the material sheet once per battlefield (H0,
//! `app::minimap_build::bake_minimap_relief`) and drawn as one quad by the draw list; this
//! module draws the vector overlays on top and owns the map's square.
//!
//! Honesty: enemy blips are already gated to LOS-spotted enemies by the caller (`render_now` masks
//! them by the player team's `spotting_bit`), the same bit that gates floating HP bars. See
//! `docs/spotting-policy.md`.

use renderer_api::HudVertex;
use ui_kit::rect::Rect;
use ui_kit::ui::Ui;

use super::primitives::{push_quad, push_segment};
use super::theme;

/// Screen anchor (clip space) and half-height of the square. The x half-extent is aspect-corrected
/// at draw time so the map reads square on any viewport.
const CENTER: [f32; 2] = [0.80, -0.58];
const HALF_H: f32 = 0.185;
/// Height-grid resolution; `RES * RES` shaded cells back the map.
pub const RELIEF_RES: usize = 36;

// Terrain-toned relief ramp: valley olive up to pale high-ground khaki, contrasty enough
// that ridgelines and lowlands read at a glance instead of dissolving into one gray.
const RELIEF_LO: [f32; 3] = [0.085, 0.125, 0.075];
const RELIEF_HI: [f32; 3] = [0.42, 0.44, 0.30];
pub(crate) const WATER: [f32; 4] = [0.13, 0.22, 0.30, 0.92];
const ROAD: [f32; 4] = [0.45, 0.39, 0.28, 0.85];
const ROAD_THICKNESS: f32 = 0.0035;
const COVER: [f32; 4] = [0.24, 0.26, 0.17, 0.85];
const ALLY: [f32; 4] = [0.30, 0.72, 0.34, 0.95];
const ENEMY: [f32; 4] = [0.87, 0.24, 0.20, 0.96];
const VIEW_WEDGE: [f32; 4] = [0.95, 0.93, 0.85, 0.10];
const BLIP_HALF: f32 = 0.012;
const VIEW_LENGTH: f32 = 0.5;

/// A static cover footprint on the map, in world XZ metres.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapBox {
    pub center_xz: [f32; 2],
    pub half_xz: [f32; 2],
}

/// Everything the minimap draws for one frame. World coordinates are metres in `0..extent_m` on
/// both axes; the caller supplies enemy blips already filtered to those the player team has spotted.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapModel {
    /// Play-area size in metres on each axis; world 0..extent maps across the map square.
    pub extent_m: [f32; 2],
    /// `RELIEF_RES * RELIEF_RES` normalised heights (0..1), row-major with z increasing per row.
    pub relief: Vec<f32>,
    /// Same grid as `relief`: `true` where standing water covers the cell.
    pub water: Vec<bool>,
    /// Road polylines in world XZ metres, drawn as thin worn-earth lines.
    pub roads: Vec<Vec<[f32; 2]>>,
    pub cover: Vec<MinimapBox>,
    pub player_xz: [f32; 2],
    pub player_heading_rad: f32,
    pub view_yaw_rad: f32,
    pub view_half_fov_rad: f32,
    pub allies: Vec<[f32; 2]>,
    pub enemies: Vec<[f32; 2]>,
}

/// Height (0..1) to a relief tint on the low->high ramp (the bake paints with it).
pub(crate) fn relief_tint(h: f32) -> [f32; 4] {
    let (a, b) = (RELIEF_LO, RELIEF_HI);
    [a[0] + (b[0] - a[0]) * h, a[1] + (b[1] - a[1]) * h, a[2] + (b[2] - a[2]) * h, 0.92]
}

/// Filled triangle from three clip-space points.
fn push_tri(vertices: &mut Vec<HudVertex>, a: [f32; 2], b: [f32; 2], c: [f32; 2], color: [f32; 4]) {
    for position in [a, b, c] {
        vertices.push(HudVertex::new(position, color));
    }
}

impl MinimapModel {
    /// Map normalised map coordinates (`u, v` in 0..1, +v up = +world z) to clip space.
    fn uv_to_clip(&self, u: f32, v: f32, aspect: f32) -> [f32; 2] {
        let hx = HALF_H / aspect.max(0.01);
        [CENTER[0] + (u - 0.5) * 2.0 * hx, CENTER[1] + (v - 0.5) * 2.0 * HALF_H]
    }

    fn world_to_clip(&self, xz: [f32; 2], aspect: f32) -> [f32; 2] {
        let (ex, ez) = (self.extent_m[0].max(1.0), self.extent_m[1].max(1.0));
        self.uv_to_clip((xz[0] / ex).clamp(0.0, 1.0), (xz[1] / ez).clamp(0.0, 1.0), aspect)
    }
}

/// The map square in physical pixels: where the baked relief is stretched and the plate sits.
pub(crate) fn map_rect_px(ui: &Ui) -> Rect {
    let viewport = ui.viewport();
    let hx = HALF_H / ui.aspect().max(0.01);
    let left = (CENTER[0] - hx + 1.0) * 0.5 * viewport.w;
    let top = (1.0 - (CENTER[1] + HALF_H)) * 0.5 * viewport.h;
    Rect::new(left, top, hx * viewport.w, HALF_H * viewport.h)
}

/// Append the minimap's vector overlays for `model` to the HUD vertex buffer: roads, cover,
/// the view wedge, the blips and the player's arrow. The plate under them and the relief
/// behind them are the draw list's (`MinimapPlate`, `MinimapRelief`).
pub(crate) fn push_minimap(vertices: &mut Vec<HudVertex>, model: &MinimapModel, aspect: f32) {
    push_roads(vertices, model, aspect);

    for cover in &model.cover {
        let lo = model.world_to_clip(
            [cover.center_xz[0] - cover.half_xz[0], cover.center_xz[1] - cover.half_xz[1]],
            aspect,
        );
        let hi = model.world_to_clip(
            [cover.center_xz[0] + cover.half_xz[0], cover.center_xz[1] + cover.half_xz[1]],
            aspect,
        );
        let center = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5];
        let half = [(hi[0] - lo[0]).abs() * 0.5, (hi[1] - lo[1]).abs() * 0.5];
        push_quad(vertices, center, half, COVER);
    }

    push_view_wedge(vertices, model, aspect);

    for ally in &model.allies {
        blip(vertices, model.world_to_clip(*ally, aspect), aspect, ALLY);
    }
    for enemy in &model.enemies {
        blip(vertices, model.world_to_clip(*enemy, aspect), aspect, ENEMY);
    }

    push_player_arrow(vertices, model, aspect);
}

/// The map's roads as thin worn-earth polylines — the orientation grid a steppe map
/// otherwise lacks.
fn push_roads(vertices: &mut Vec<HudVertex>, model: &MinimapModel, aspect: f32) {
    for road in &model.roads {
        for pair in road.windows(2) {
            let a = model.world_to_clip(pair[0], aspect);
            let b = model.world_to_clip(pair[1], aspect);
            push_segment(vertices, a, b, ROAD_THICKNESS, ROAD);
        }
    }
}

fn push_view_wedge(vertices: &mut Vec<HudVertex>, model: &MinimapModel, aspect: f32) {
    let apex = model.world_to_clip(model.player_xz, aspect);
    let hx = HALF_H / aspect.max(0.01);
    let arm = |angle: f32| {
        // +world-z is up on the map, so a world yaw of 0 points up (+v).
        let dir = [angle.sin(), angle.cos()];
        [apex[0] + dir[0] * VIEW_LENGTH * hx, apex[1] + dir[1] * VIEW_LENGTH * HALF_H]
    };
    let left = arm(model.view_yaw_rad - model.view_half_fov_rad);
    let right = arm(model.view_yaw_rad + model.view_half_fov_rad);
    push_tri(vertices, apex, left, right, VIEW_WEDGE);
}

fn push_player_arrow(vertices: &mut Vec<HudVertex>, model: &MinimapModel, aspect: f32) {
    let center = model.world_to_clip(model.player_xz, aspect);
    // +world-z is up on the map; heading 0 points up. x arms are aspect-corrected so the arrowhead
    // reads symmetric.
    let fwd = [model.player_heading_rad.sin() / aspect.max(0.01), model.player_heading_rad.cos()];
    let side = [fwd[1], -fwd[0]];
    let nose = [center[0] + fwd[0] * BLIP_HALF * 2.0, center[1] + fwd[1] * BLIP_HALF * 2.0];
    let back = [center[0] - fwd[0] * BLIP_HALF, center[1] - fwd[1] * BLIP_HALF];
    let l = [back[0] + side[0] * BLIP_HALF, back[1] + side[1] * BLIP_HALF];
    let r = [back[0] - side[0] * BLIP_HALF, back[1] - side[1] * BLIP_HALF];
    push_tri(vertices, nose, l, r, theme::color::ACCENT);
}

/// A small square blip centred on a clip point (x aspect-corrected so it reads square).
fn blip(vertices: &mut Vec<HudVertex>, center: [f32; 2], aspect: f32, color: [f32; 4]) {
    push_quad(vertices, center, [BLIP_HALF / aspect.max(0.01), BLIP_HALF], color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> MinimapModel {
        MinimapModel {
            extent_m: [1000.0, 1000.0],
            relief: vec![0.5; RELIEF_RES * RELIEF_RES],
            water: vec![false; RELIEF_RES * RELIEF_RES],
            roads: vec![vec![[0.0, 500.0], [1000.0, 500.0]]],
            cover: vec![MinimapBox { center_xz: [500.0, 500.0], half_xz: [30.0, 12.0] }],
            player_xz: [400.0, 300.0],
            player_heading_rad: 0.3,
            view_yaw_rad: 0.3,
            view_half_fov_rad: 0.5,
            allies: vec![[420.0, 320.0]],
            enemies: vec![[600.0, 640.0]],
        }
    }

    /// H0: the overlays are a few hundred vertices — the relief that was 7 776 of them is one
    /// quad in the draw list now.
    #[test]
    fn the_overlays_carry_no_relief_and_stay_within_a_small_vertex_budget() {
        let mut v = Vec::new();
        push_minimap(&mut v, &model(), 16.0 / 9.0);
        assert!(!v.is_empty());
        assert!(v.len() < 400, "minimap overlay vertex count regressed: {}", v.len());
        assert!(!v.iter().any(|vert| vert.color == WATER), "water is the bake's, not a quad");
    }

    /// The vector layer still reads: a road polyline draws as worn-earth segments.
    #[test]
    fn roads_read_on_the_map() {
        let mut v = Vec::new();
        push_minimap(&mut v, &model(), 16.0 / 9.0);
        assert!(v.iter().any(|vert| vert.color == ROAD), "roads draw as worn-earth lines");
    }

    /// The pixel square and the clip square are the same square, so the baked relief lands
    /// exactly under the overlays.
    #[test]
    fn the_map_rect_matches_the_clip_square() {
        let ui = Ui::new(1920, 1080, 1.0);
        let rect = map_rect_px(&ui);
        let [left, top] = ui.to_clip([rect.x, rect.y]);
        let [right, bottom] = ui.to_clip([rect.right(), rect.bottom()]);
        let hx = HALF_H / (1920.0 / 1080.0);
        assert!((left - (CENTER[0] - hx)).abs() < 1e-4 && (right - (CENTER[0] + hx)).abs() < 1e-4);
        assert!(
            (top - (CENTER[1] + HALF_H)).abs() < 1e-4
                && (bottom - (CENTER[1] - HALF_H)).abs() < 1e-4
        );
    }

    #[test]
    fn every_blip_lands_inside_the_map_square() {
        let mut v = Vec::new();
        push_minimap(&mut v, &model(), 16.0 / 9.0);
        let hx = HALF_H / (16.0 / 9.0);
        for vert in &v {
            assert!(vert.position[0] >= CENTER[0] - hx * 1.1 - 0.05);
            assert!(vert.position[0] <= CENTER[0] + hx * 1.1 + 0.05);
            assert!(vert.position[1] >= CENTER[1] - HALF_H * 1.1 - 0.05);
            assert!(vert.position[1] <= CENTER[1] + HALF_H * 1.1 + 0.05);
        }
    }
}
