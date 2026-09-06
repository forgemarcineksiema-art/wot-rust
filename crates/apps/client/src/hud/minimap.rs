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

/// Screen anchor (clip space) of the square's centre at the standard size. The x half-extent
/// is aspect-corrected at draw time so the map reads square on any viewport; a larger size
/// grows the square about its bottom-right corner so it never leaves the screen.
const CENTER: [f32; 2] = [0.80, -0.58];
const HALF_H: f32 = 0.185;

/// The three sizes M cycles (H15): the standard square, a compact one, and the large one for
/// reading the field. Append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MinimapSize {
    Small,
    #[default]
    Standard,
    Large,
}

impl MinimapSize {
    pub const ALL: [MinimapSize; 3] =
        [MinimapSize::Small, MinimapSize::Standard, MinimapSize::Large];

    /// The square's half-height in clip units.
    pub fn half_h(self) -> f32 {
        match self {
            MinimapSize::Small => 0.14,
            MinimapSize::Standard => HALF_H,
            MinimapSize::Large => 0.25,
        }
    }

    /// The square's centre at `aspect`: the bottom-right corner stays where the standard
    /// square's is, so the x growth is aspect-corrected like the x half-extent.
    pub fn center(self, aspect: f32) -> [f32; 2] {
        let grow = self.half_h() - HALF_H;
        [CENTER[0] - grow / aspect.max(0.01), CENTER[1] + grow]
    }

    /// The next size on the cycle, wrapping.
    pub fn next(self) -> MinimapSize {
        let at = Self::ALL.iter().position(|size| *size == self).unwrap_or(0);
        Self::ALL[(at + 1) % Self::ALL.len()]
    }

    /// Whether the grid's letters and the blips' seats are printed: only where they fit.
    pub fn labelled(self) -> bool {
        self != MinimapSize::Small
    }
}
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
const VIEW_WEDGE: [f32; 4] = [0.95, 0.93, 0.85, 0.10];
const BLIP_HALF: f32 = 0.012;
const VIEW_LENGTH: f32 = 0.5;
/// The ten-by-ten grid's hairlines, and the two circles.
const GRID: [f32; 4] = [0.85, 0.87, 0.80, 0.16];
const VIEW_RANGE_RING: [f32; 4] = [0.85, 0.87, 0.80, 0.28];
const SEEN_FROM_RING: [f32; 4] = [0.90, 0.36, 0.30, 0.34];
const GHOST: [f32; 4] = [0.87, 0.24, 0.20, 0.45];
const PING: [f32; 4] = [1.0, 0.86, 0.62, 0.9];
const TURRET_LINE: [f32; 4] = [0.95, 0.93, 0.85, 0.75];
pub(crate) const GRID_CELLS: usize = 10;
/// A ping rings for this long.
pub(crate) const PING_TTL_S: f32 = 6.0;

/// A static cover footprint on the map, in world XZ metres.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapBox {
    pub center_xz: [f32; 2],
    pub half_xz: [f32; 2],
}

/// One hull on the map (H15): where, what class, which seat — the roster's identity, so a blip
/// is never an anonymous dot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blip {
    pub xz: [f32; 2],
    pub class: game_core::VehicleClass,
    pub seat: char,
}

/// A hull seen before and not now: drawn hollow and dim, fading with `age_s`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ghost {
    pub xz: [f32; 2],
    pub class: game_core::VehicleClass,
    pub age_s: f32,
}

/// A teammate's ping on the map (W-5), ringing for `PING_TTL_S`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ping {
    pub xz: [f32; 2],
    pub age_s: f32,
}

/// Everything the minimap draws for one frame. World coordinates are metres in `0..extent_m` on
/// both axes; the caller supplies enemy blips already filtered to those the player team has spotted.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapModel {
    pub size: MinimapSize,
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
    /// The turret's world yaw: the short line off the arrow that says where the gun looks.
    pub player_turret_yaw_rad: f32,
    pub view_yaw_rad: f32,
    pub view_half_fov_rad: f32,
    /// The player's own view range, metres: the circle we spot at.
    pub view_range_m: f32,
    /// The range we are seen from (H14's budget), metres, when the roster has an enemy.
    pub seen_from_m: Option<f32>,
    pub allies: Vec<Blip>,
    pub enemies: Vec<Blip>,
    pub ghosts: Vec<Ghost>,
    pub pings: Vec<Ping>,
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
        let half_h = self.size.half_h();
        let center = self.size.center(aspect);
        let hx = half_h / aspect.max(0.01);
        [center[0] + (u - 0.5) * 2.0 * hx, center[1] + (v - 0.5) * 2.0 * half_h]
    }

    /// The same point in physical pixels on `ui`'s viewport.
    pub(crate) fn world_to_px(&self, xz: [f32; 2], ui: &Ui) -> [f32; 2] {
        let clip = self.world_to_clip(xz, ui.aspect());
        let viewport = ui.viewport();
        // The square's nudge (H21) moves its blips with it.
        let nudge = ui.nudge_px();
        [
            (clip[0] + 1.0) * 0.5 * viewport.w + nudge[0],
            (1.0 - clip[1]) * 0.5 * viewport.h + nudge[1],
        ]
    }

    pub(crate) fn world_to_clip(&self, xz: [f32; 2], aspect: f32) -> [f32; 2] {
        let (ex, ez) = (self.extent_m[0].max(1.0), self.extent_m[1].max(1.0));
        self.uv_to_clip((xz[0] / ex).clamp(0.0, 1.0), (xz[1] / ez).clamp(0.0, 1.0), aspect)
    }
}

/// The map square in physical pixels at `size`: where the baked relief is stretched and the
/// plate sits.
pub(crate) fn map_rect_px(ui: &Ui, size: MinimapSize) -> Rect {
    let viewport = ui.viewport();
    let half_h = size.half_h();
    let center = size.center(ui.aspect());
    let hx = half_h / ui.aspect().max(0.01);
    let nudge = ui.nudge_px();
    let left = (center[0] - hx + 1.0) * 0.5 * viewport.w + nudge[0];
    let top = (1.0 - (center[1] + half_h)) * 0.5 * viewport.h + nudge[1];
    Rect::new(left, top, hx * viewport.w, half_h * viewport.h)
}

/// Append the minimap's vector overlays for `model` to the HUD vertex buffer: roads, cover,
/// the view wedge, the blips and the player's arrow. The plate under them and the relief
/// behind them are the draw list's (`MinimapPlate`, `MinimapRelief`).
pub(crate) fn push_minimap(vertices: &mut Vec<HudVertex>, model: &MinimapModel, aspect: f32) {
    push_grid(vertices, model, aspect);
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

    push_circles(vertices, model, aspect);
    push_view_wedge(vertices, model, aspect);

    // The blips themselves are the draw list's class glyphs (`MinimapBlip`); here the memory
    // and the team's word: ghosts as hollow rings fading with age, pings as rings that grow.
    let blip_m = model.extent_m[1] * BLIP_HALF / (2.0 * model.size.half_h());
    for ghost in &model.ghosts {
        let fade = (1.0 - ghost.age_s / crate::app::ghosts::GHOST_TTL_S).clamp(0.0, 1.0);
        let color = [GHOST[0], GHOST[1], GHOST[2], GHOST[3] * fade];
        push_ring(vertices, model, ghost.xz, blip_m, 12, aspect, color, 0.0012);
    }
    for ping in &model.pings {
        let life = (ping.age_s / PING_TTL_S).clamp(0.0, 1.0);
        let color = [PING[0], PING[1], PING[2], PING[3] * (1.0 - life)];
        push_ring(vertices, model, ping.xz, blip_m * (1.0 + 2.5 * life), 24, aspect, color, 0.0012);
    }

    push_player_arrow(vertices, model, aspect);
    push_turret_line(vertices, model, aspect);
}

/// The ten-by-ten grid: hairlines across the square (the letters are the draw list's).
fn push_grid(vertices: &mut Vec<HudVertex>, model: &MinimapModel, aspect: f32) {
    for i in 1..GRID_CELLS {
        let t = i as f32 / GRID_CELLS as f32;
        push_polyline(vertices, model, [[t, 0.0], [t, 1.0]].into_iter(), aspect, GRID, 0.0008);
        push_polyline(vertices, model, [[0.0, t], [1.0, t]].into_iter(), aspect, GRID, 0.0008);
    }
}

/// The two circles about the player: what we spot at, and what we are seen from. No draw
/// circle — there is no such mechanic here.
fn push_circles(vertices: &mut Vec<HudVertex>, model: &MinimapModel, aspect: f32) {
    push_ring(
        vertices,
        model,
        model.player_xz,
        model.view_range_m,
        48,
        aspect,
        VIEW_RANGE_RING,
        0.0010,
    );
    if let Some(seen_from) = model.seen_from_m {
        push_ring(vertices, model, model.player_xz, seen_from, 48, aspect, SEEN_FROM_RING, 0.0010);
    }
}

/// A thin ring of `radius_m` about a world point, walked in map space.
#[allow(clippy::too_many_arguments)]
fn push_ring(
    vertices: &mut Vec<HudVertex>,
    model: &MinimapModel,
    center_xz: [f32; 2],
    radius_m: f32,
    segments: u32,
    aspect: f32,
    color: [f32; 4],
    half_thick: f32,
) {
    let (ex, ez) = (model.extent_m[0].max(1.0), model.extent_m[1].max(1.0));
    let points = (0..=segments).map(move |i| {
        let angle = i as f32 / segments as f32 * std::f32::consts::TAU;
        [(center_xz[0] + radius_m * angle.cos()) / ex, (center_xz[1] + radius_m * angle.sin()) / ez]
    });
    push_polyline(vertices, model, points, aspect, color, half_thick);
}

/// The turret's yaw off the arrow: where the gun looks, independent of where the hull points.
fn push_turret_line(vertices: &mut Vec<HudVertex>, model: &MinimapModel, aspect: f32) {
    let (ex, ez) = (model.extent_m[0].max(1.0), model.extent_m[1].max(1.0));
    let center = [model.player_xz[0] / ex, model.player_xz[1] / ez];
    // The line's length in map space: the square is square on screen, so one map unit is the
    // same on both axes.
    let length = BLIP_HALF * 3.2 / (2.0 * model.size.half_h());
    let dir = [model.player_turret_yaw_rad.sin(), model.player_turret_yaw_rad.cos()];
    let tip = [center[0] + dir[0] * length, center[1] + dir[1] * length];
    push_polyline(vertices, model, [center, tip].into_iter(), aspect, TURRET_LINE, 0.0018);
}

/// A polyline in map space (`u, v` in 0..1), CUT at the square's edge: a segment with an end
/// off the map is dropped, so nothing lines over the plate. The overlays are one legacy
/// element the draw list cannot clip (its wedge and arrow are triangles), so the map clips its
/// own lines — through this ONE call site of the old kit.
fn push_polyline(
    vertices: &mut Vec<HudVertex>,
    model: &MinimapModel,
    points: impl Iterator<Item = [f32; 2]>,
    aspect: f32,
    color: [f32; 4],
    half_thick: f32,
) {
    let inside = |uv: [f32; 2]| (0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1]);
    let mut previous: Option<[f32; 2]> = None;
    for point in points {
        if let Some(a) = previous
            && inside(a)
            && inside(point)
        {
            let a = model.uv_to_clip(a[0], a[1], aspect);
            let b = model.uv_to_clip(point[0], point[1], aspect);
            push_segment(vertices, a, b, half_thick, color);
        }
        previous = Some(point);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> MinimapModel {
        MinimapModel {
            size: MinimapSize::Standard,
            extent_m: [1000.0, 1000.0],
            relief: vec![0.5; RELIEF_RES * RELIEF_RES],
            water: vec![false; RELIEF_RES * RELIEF_RES],
            roads: vec![vec![[0.0, 500.0], [1000.0, 500.0]]],
            cover: vec![MinimapBox { center_xz: [500.0, 500.0], half_xz: [30.0, 12.0] }],
            player_xz: [400.0, 300.0],
            player_heading_rad: 0.3,
            player_turret_yaw_rad: 1.1,
            view_yaw_rad: 0.3,
            view_half_fov_rad: 0.5,
            view_range_m: 440.0,
            seen_from_m: Some(308.0),
            allies: vec![Blip {
                xz: [420.0, 320.0],
                class: game_core::VehicleClass::Medium,
                seat: 'B',
            }],
            enemies: vec![Blip {
                xz: [600.0, 640.0],
                class: game_core::VehicleClass::Heavy,
                seat: 'A',
            }],
            ghosts: vec![Ghost {
                xz: [700.0, 200.0],
                class: game_core::VehicleClass::TankDestroyer,
                age_s: 4.0,
            }],
            pings: vec![Ping { xz: [500.0, 500.0], age_s: 1.0 }],
        }
    }

    /// H15: the three sizes share the bottom-right corner, grow from it, and only the compact
    /// one goes without letters.
    #[test]
    fn the_sizes_share_a_corner_and_cycle() {
        let ui = Ui::reference();
        let small = map_rect_px(&ui, MinimapSize::Small);
        let standard = map_rect_px(&ui, MinimapSize::Standard);
        let large = map_rect_px(&ui, MinimapSize::Large);
        assert!(small.w < standard.w && standard.w < large.w);
        for rect in [small, standard, large] {
            assert!(
                (rect.right() - standard.right()).abs() < 0.5
                    && (rect.bottom() - standard.bottom()).abs() < 0.5
            );
            assert!(ui.viewport().encloses(&rect));
        }
        assert_eq!(MinimapSize::Small.next().next().next(), MinimapSize::Small);
        assert!(!MinimapSize::Small.labelled() && MinimapSize::Large.labelled());
    }

    /// H0: the overlays are a few hundred vertices — the relief that was 7 776 of them is one
    /// quad in the draw list now.
    #[test]
    fn the_overlays_carry_no_relief_and_stay_within_a_small_vertex_budget() {
        let mut v = Vec::new();
        push_minimap(&mut v, &model(), 16.0 / 9.0);
        assert!(!v.is_empty());
        // H15 added the grid (18 hairlines), two rings, the ghosts and the pings: ~1 100.
        assert!(v.len() < 1_600, "minimap overlay vertex count regressed: {}", v.len());
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
        let rect = map_rect_px(&ui, MinimapSize::Standard);
        let [left, top] = ui.to_clip([rect.x, rect.y]);
        let [right, bottom] = ui.to_clip([rect.right(), rect.bottom()]);
        let hx = HALF_H / (1920.0 / 1080.0);
        assert!((left - (CENTER[0] - hx)).abs() < 1e-4 && (right - (CENTER[0] + hx)).abs() < 1e-4);
        assert!(
            (top - (CENTER[1] + HALF_H)).abs() < 1e-4
                && (bottom - (CENTER[1] - HALF_H)).abs() < 1e-4
        );
    }

    /// H15: a blip carries its class and its seat off the roster — no anonymous dots.
    #[test]
    fn every_blip_carries_its_class_and_seat() {
        let m = model();
        for blip in m.allies.iter().chain(&m.enemies) {
            assert!(blip.seat.is_ascii_uppercase(), "a seat letter: {blip:?}");
            assert!(game_core::VehicleClass::ALL.contains(&blip.class));
        }
        let px = m.world_to_px(m.enemies[0].xz, &Ui::reference());
        let square = map_rect_px(&Ui::reference(), MinimapSize::Standard);
        assert!(square.contains(px), "{px:?} inside {square:?}");
    }

    /// The grid, the circles, the ghosts and the pings all land inside the square's margin.
    #[test]
    fn every_overlay_lands_inside_the_map_square() {
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
