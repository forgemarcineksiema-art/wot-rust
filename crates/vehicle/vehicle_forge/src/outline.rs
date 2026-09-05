//! The reference-outline gate (Inny Poziom K0): the bake against the dossier's DRAWING.
//!
//! Seventeen dimension anchors pin lengths, heights and diameters, and every one of them can
//! pass while the shape between them is wrong — a dome 45 mm too long (K15) sits inside the
//! ±0.06 ratio lock that watches it. `t54_silhouette.rs` rasterises the turret into its own
//! bounds, so it can only judge proportion, never position or form. The Blender overlay
//! against the three-view (`docs/vehicles/refs/t-54.md`, R1) is the instrument that catches
//! form, and it was a manual loop nobody ran.
//!
//! This module is that overlay as a gate. A vehicle's reference pack may carry OUTLINES: closed
//! polylines in metres, in the vehicle's own frame, one set per canonical view, drawn from the
//! dossier's numbers and — the day the owner traces the drawing — from the drawing itself. The
//! authoritative bake (exterior submeshes plus the rest-pose running gear the client instances)
//! is rasterised orthographically into the same grid, and the two silhouettes are compared by
//! intersection-over-union. The bar is IoU ≥ `min_iou` (0.95 in the register) per view.
//!
//! Like the dimension anchors, an outline is `Locked` (gates) or `Target` (reports as debt).
//! An outline authored from the dossier's tables and the blueprint's own turret extents is
//! `Target` by construction — the instrument and the thing it measures must not be the same
//! mistake (Model Idealny's lesson) — and flips to `Locked` when it has been checked against
//! the drawing with the overlay PNG this module also renders.

use std::io;

use glam::{Mat4, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use vehicle_geometry::{
    BakedVehicle, GearPart, GeometryMesh, RunningGearKinematics, idler_unit_mesh,
    return_roller_unit_mesh, road_wheel_unit_mesh, running_gear_placements, sprocket_unit_mesh,
    swing_arm_unit_mesh, track_link_unit_mesh,
};

use crate::reference_measure::is_exterior;
use crate::{AnchorStatus, ReferenceSource};

/// The canonical orthographic views. Append-only: reports serialize this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutlineView {
    /// Looking at the vehicle's port side: horizontal = Z (bow to the right), vertical = Y.
    Side,
    /// Looking at the bow: horizontal = X, vertical = Y.
    Front,
    /// Looking down: horizontal = X, vertical = Z (bow up).
    Plan,
}

impl OutlineView {
    pub const ALL: [OutlineView; 3] = [OutlineView::Side, OutlineView::Front, OutlineView::Plan];

    pub fn label(self) -> &'static str {
        match self {
            OutlineView::Side => "side",
            OutlineView::Front => "front",
            OutlineView::Plan => "plan",
        }
    }

    /// The orthographic projection of a vehicle-frame point into this view's plane (metres).
    pub fn project(self, p: Vec3) -> Vec2 {
        match self {
            OutlineView::Side => Vec2::new(p.z, p.y),
            OutlineView::Front => Vec2::new(p.x, p.y),
            OutlineView::Plan => Vec2::new(p.x, p.z),
        }
    }
}

/// One view's reference silhouette: the UNION of closed loops (hull, track envelope, turret,
/// gun tube… each a polygon in view-plane metres), the bar it must clear, and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutlineSpec {
    view: OutlineView,
    /// Closed polygons; the last point joins the first. Winding does not matter (even-odd fill
    /// per loop, loops OR-ed together).
    loops: Vec<Vec<[f32; 2]>>,
    /// The IoU the bake must reach in this view.
    min_iou: f32,
    #[serde(default)]
    status: AnchorStatus,
    /// The IoU measured the day the outline was authored — a `Target` view may not fall below
    /// it. The FLOOR/TARGET mechanism: the bar waits for the traced drawing, the floor holds
    /// today's silhouette against regression from the first day.
    #[serde(default)]
    floor_iou: Option<f32>,
    source: ReferenceSource,
}

impl OutlineSpec {
    pub fn new(
        view: OutlineView,
        loops: Vec<Vec<[f32; 2]>>,
        min_iou: f32,
        status: AnchorStatus,
        source: ReferenceSource,
    ) -> Self {
        assert!(!loops.is_empty(), "an outline needs at least one loop");
        assert!(loops.iter().all(|l| l.len() >= 3), "a loop needs at least three points");
        assert!((0.0..=1.0).contains(&min_iou), "min_iou is a fraction");
        Self { view, loops, min_iou, status, floor_iou: None, source }
    }

    /// The same outline with a regression floor (the IoU measured when it was authored).
    pub fn with_floor(mut self, floor_iou: f32) -> Self {
        assert!((0.0..=1.0).contains(&floor_iou), "floor_iou is a fraction");
        self.floor_iou = Some(floor_iou);
        self
    }

    pub fn floor_iou(&self) -> Option<f32> {
        self.floor_iou
    }

    pub fn view(&self) -> OutlineView {
        self.view
    }

    pub fn loops(&self) -> &[Vec<[f32; 2]>] {
        &self.loops
    }

    pub fn min_iou(&self) -> f32 {
        self.min_iou
    }

    pub fn status(&self) -> AnchorStatus {
        self.status
    }

    pub fn source(&self) -> &ReferenceSource {
        &self.source
    }

    /// The same outline moved by `delta` (metres) — the instrument's own sanity check.
    pub fn translated(&self, delta: Vec2) -> Self {
        let loops = self
            .loops
            .iter()
            .map(|l| l.iter().map(|p| [p[0] + delta.x, p[1] + delta.y]).collect())
            .collect();
        Self { loops, ..self.clone() }
    }
}

/// A vehicle's outlines as they are authored: one RON file per vehicle beside the crate
/// (`outlines/<slug>.outline.ron`), embedded with `include_str!`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutlineSet {
    vehicle: String,
    views: Vec<OutlineSpec>,
}

impl OutlineSet {
    pub fn parse(ron: &str) -> Result<Self, ron::error::SpannedError> {
        let set: Self = ron::from_str(ron)?;
        for spec in &set.views {
            assert!(!spec.loops.is_empty(), "{}: {} has no loops", set.vehicle, spec.view.label());
            assert!(
                spec.loops.iter().all(|l| l.len() >= 3),
                "{}: {} has a loop with fewer than three points",
                set.vehicle,
                spec.view.label()
            );
        }
        Ok(set)
    }

    pub fn vehicle(&self) -> &str {
        &self.vehicle
    }

    pub fn views(&self) -> &[OutlineSpec] {
        &self.views
    }

    pub fn into_views(self) -> Vec<OutlineSpec> {
        self.views
    }
}

/// Grid resolution. One centimetre resolves a gun tube (12 cells across) and a fender lip
/// (5 cells) while a nine-metre side view stays under a million cells.
pub const OUTLINE_CELL_M: f32 = 0.01;

/// What one view measured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutlineMeasurement {
    view: OutlineView,
    iou: f32,
    /// Fraction of the outline the bake covers (1.0 = nothing missing from the model).
    outline_covered: f32,
    /// Fraction of the bake inside the outline (1.0 = nothing proud of the drawing).
    bake_inside: f32,
    min_iou: f32,
    status: AnchorStatus,
    floor_iou: Option<f32>,
    cell_m: f32,
}

impl OutlineMeasurement {
    pub fn view(&self) -> OutlineView {
        self.view
    }

    pub fn iou(&self) -> f32 {
        self.iou
    }

    pub fn outline_covered(&self) -> f32 {
        self.outline_covered
    }

    pub fn bake_inside(&self) -> f32 {
        self.bake_inside
    }

    pub fn min_iou(&self) -> f32 {
        self.min_iou
    }

    pub fn status(&self) -> AnchorStatus {
        self.status
    }

    pub fn passed(&self) -> bool {
        self.iou.is_finite() && self.iou >= self.min_iou
    }

    pub fn floor_iou(&self) -> Option<f32> {
        self.floor_iou
    }

    /// Above the regression floor (or there is none). A `Target` view that fails this has
    /// moved AWAY from the drawing since the day it was measured — that is a gate, not debt.
    pub fn holds_floor(&self) -> bool {
        self.floor_iou.is_none_or(|floor| self.iou.is_finite() && self.iou >= floor)
    }

    pub fn summary_line(&self, vehicle: &str) -> String {
        format!(
            "{vehicle} {}: IoU {:.3} (bar {:.2}{}, {:?}) — outline covered {:.1}%, bake inside {:.1}%",
            self.view.label(),
            self.iou,
            self.min_iou,
            self.floor_iou.map(|f| format!(", floor {f:.2}")).unwrap_or_default(),
            self.status,
            self.outline_covered * 100.0,
            self.bake_inside * 100.0,
        )
    }
}

/// A binary occupancy grid over a rectangle of the view plane.
#[derive(Debug, Clone)]
pub struct SilhouetteGrid {
    min: Vec2,
    cell: f32,
    width: usize,
    height: usize,
    cells: Vec<bool>,
}

impl SilhouetteGrid {
    fn empty(min: Vec2, max: Vec2, cell: f32) -> Self {
        let span = (max - min).max(Vec2::splat(cell));
        let width = (span.x / cell).ceil() as usize + 1;
        let height = (span.y / cell).ceil() as usize + 1;
        Self { min, cell, width, height, cells: vec![false; width * height] }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn cell_m(&self) -> f32 {
        self.cell
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        self.cells[y * self.width + x]
    }

    fn centre(&self, x: usize, y: usize) -> Vec2 {
        self.min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * self.cell
    }

    fn col_range(&self, lo: f32, hi: f32) -> (usize, usize) {
        let a = ((lo - self.min.x) / self.cell).floor().max(0.0) as usize;
        let b = (((hi - self.min.x) / self.cell).ceil().max(0.0) as usize).min(self.width - 1);
        (a.min(self.width - 1), b)
    }

    fn row_range(&self, lo: f32, hi: f32) -> (usize, usize) {
        let a = ((lo - self.min.y) / self.cell).floor().max(0.0) as usize;
        let b = (((hi - self.min.y) / self.cell).ceil().max(0.0) as usize).min(self.height - 1);
        (a.min(self.height - 1), b)
    }

    pub fn count(&self) -> usize {
        self.cells.iter().filter(|c| **c).count()
    }

    /// Fill every cell whose centre lies inside the triangle (a cell also fills when a vertex
    /// lands in it, so a sliver thinner than a cell still leaves its mark).
    fn fill_triangle(&mut self, a: Vec2, b: Vec2, c: Vec2) {
        let lo = a.min(b).min(c);
        let hi = a.max(b).max(c);
        let (x0, x1) = self.col_range(lo.x, hi.x);
        let (y0, y1) = self.row_range(lo.y, hi.y);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if point_in_triangle(self.centre(x, y), a, b, c) {
                    self.cells[y * self.width + x] = true;
                }
            }
        }
        for p in [a, b, c] {
            let (x, _) = self.col_range(p.x, p.x);
            let (y, _) = self.row_range(p.y, p.y);
            self.cells[y * self.width + x] = true;
        }
    }

    /// Even-odd scanline fill of one closed loop, OR-ed into the grid.
    fn fill_loop(&mut self, points: &[[f32; 2]]) {
        let n = points.len();
        if n < 3 {
            return;
        }
        let lo_y = points.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        let hi_y = points.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        let (y0, y1) = self.row_range(lo_y, hi_y);
        let mut crossings = Vec::new();
        for y in y0..=y1 {
            let sy = self.centre(0, y).y;
            crossings.clear();
            for i in 0..n {
                let (p, q) = (points[i], points[(i + 1) % n]);
                let (py, qy) = (p[1], q[1]);
                if (py <= sy) != (qy <= sy) {
                    let t = (sy - py) / (qy - py);
                    crossings.push(p[0] + t * (q[0] - p[0]));
                }
            }
            crossings.sort_by(f32::total_cmp);
            for pair in crossings.chunks_exact(2) {
                let (x0, x1) = self.col_range(pair[0], pair[1]);
                for x in x0..=x1 {
                    if (self.centre(x, y).x - pair[0]) * (self.centre(x, y).x - pair[1]) <= 0.0 {
                        self.cells[y * self.width + x] = true;
                    }
                }
            }
        }
    }
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    // Barycentric signs: inside (or on an edge) when all three sub-areas share the polygon's
    // orientation or vanish.
    let area = (b - a).perp_dot(c - a);
    let s1 = (b - a).perp_dot(p - a) * area;
    let s2 = (c - b).perp_dot(p - b) * area;
    let s3 = (a - c).perp_dot(p - c) * area;
    s1 >= 0.0 && s2 >= 0.0 && s3 >= 0.0
}

/// The composed silhouette of the vehicle as shipped: the bake plus its embedded running gear.
pub fn composed_triangles_for(vehicle: &BakedVehicle) -> Vec<[Vec3; 3]> {
    let kin = RunningGearKinematics::for_vehicle(vehicle.kind());
    composed_triangles(vehicle, kin.as_ref())
}

/// Every triangle a production review draws, in the vehicle frame: the exterior of each baked
/// submesh plus the rest-pose running gear the client instances (the same list the review
/// tiles compose, so the silhouette judged is the silhouette shipped).
pub fn composed_triangles(
    vehicle: &BakedVehicle,
    gear: Option<&RunningGearKinematics>,
) -> Vec<[Vec3; 3]> {
    let mut tris = Vec::new();
    for submesh in vehicle.submeshes() {
        push_mesh_triangles(&mut tris, &submesh.mesh, Mat4::IDENTITY, true);
    }
    if let Some(kin) = gear {
        let road_wheel = road_wheel_unit_mesh(kin);
        let idler = idler_unit_mesh(kin);
        let sprocket = sprocket_unit_mesh(kin);
        let link = track_link_unit_mesh(kin);
        let swing_arm = swing_arm_unit_mesh(kin);
        let swing_arm_left = vehicle_geometry::swing_arm_unit_mesh_left(kin);
        let damper = vehicle_geometry::damper_unit_mesh(kin);
        let damper_left = vehicle_geometry::damper_unit_mesh_left(kin);
        let return_roller = return_roller_unit_mesh(kin);
        for placement in running_gear_placements(kin, 0.0, 0.0) {
            let mesh = match placement.part {
                GearPart::RoadWheel => &road_wheel,
                GearPart::Idler => &idler,
                GearPart::Sprocket => &sprocket,
                GearPart::Link => &link,
                GearPart::SwingArm => &swing_arm,
                GearPart::SwingArmLeft => &swing_arm_left,
                GearPart::Damper => &damper,
                GearPart::DamperLeft => &damper_left,
                GearPart::ReturnRoller => &return_roller,
            };
            push_mesh_triangles(&mut tris, mesh, placement.transform, false);
        }
    }
    tris
}

fn push_mesh_triangles(
    out: &mut Vec<[Vec3; 3]>,
    mesh: &GeometryMesh,
    transform: Mat4,
    exterior_only: bool,
) {
    let vertices = mesh.vertices();
    for tri in mesh.indices().chunks_exact(3) {
        let a = &vertices[tri[0] as usize];
        if exterior_only && !is_exterior(a.material) {
            continue;
        }
        let b = &vertices[tri[1] as usize];
        let c = &vertices[tri[2] as usize];
        out.push([a, b, c].map(|v| transform.transform_point3(v.position)));
    }
}

fn outline_bounds(spec: &OutlineSpec) -> (Vec2, Vec2) {
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    for p in spec.loops.iter().flatten() {
        min = min.min(Vec2::from(*p));
        max = max.max(Vec2::from(*p));
    }
    (min, max)
}

/// Both silhouettes on one grid: the bake's and the outline's.
pub struct OutlineRaster {
    pub bake: SilhouetteGrid,
    pub outline: SilhouetteGrid,
}

pub fn rasterise(tris: &[[Vec3; 3]], spec: &OutlineSpec, cell: f32) -> OutlineRaster {
    let view = spec.view;
    let (mut min, mut max) = outline_bounds(spec);
    for tri in tris {
        for p in tri {
            let q = view.project(*p);
            min = min.min(q);
            max = max.max(q);
        }
    }
    let pad = Vec2::splat(cell * 2.0);
    let (min, max) = (min - pad, max + pad);
    let mut bake = SilhouetteGrid::empty(min, max, cell);
    for tri in tris {
        let [a, b, c] = tri.map(|p| view.project(p));
        bake.fill_triangle(a, b, c);
    }
    let mut outline = SilhouetteGrid::empty(min, max, cell);
    for l in &spec.loops {
        outline.fill_loop(l);
    }
    OutlineRaster { bake, outline }
}

pub fn measure(tris: &[[Vec3; 3]], spec: &OutlineSpec) -> OutlineMeasurement {
    let raster = rasterise(tris, spec, OUTLINE_CELL_M);
    let (mut inter, mut union, mut bake_n, mut outline_n) = (0usize, 0usize, 0usize, 0usize);
    for (b, o) in raster.bake.cells.iter().zip(&raster.outline.cells) {
        bake_n += usize::from(*b);
        outline_n += usize::from(*o);
        inter += usize::from(*b && *o);
        union += usize::from(*b || *o);
    }
    let frac = |num: usize, den: usize| if den == 0 { 0.0 } else { num as f32 / den as f32 };
    OutlineMeasurement {
        view: spec.view,
        iou: frac(inter, union),
        outline_covered: frac(inter, outline_n),
        bake_inside: frac(inter, bake_n),
        min_iou: spec.min_iou,
        status: spec.status,
        floor_iou: spec.floor_iou,
        cell_m: OUTLINE_CELL_M,
    }
}

/// The overlay a reviewer compares with the drawing: agreement in grey, the bake proud of the
/// outline in red, the outline the bake does not reach in blue. Y up on screen for the side
/// and front views; bow up for the plan.
pub fn overlay_png(tris: &[[Vec3; 3]], spec: &OutlineSpec) -> Result<Vec<u8>, io::Error> {
    let raster = rasterise(tris, spec, OUTLINE_CELL_M);
    let (w, h) = (raster.bake.width, raster.bake.height);
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let (b, o) = (raster.bake.get(x, y), raster.outline.get(x, y));
            let colour = match (b, o) {
                (true, true) => [96, 96, 96, 255],
                (true, false) => [220, 40, 40, 255],
                (false, true) => [40, 80, 220, 255],
                (false, false) => [244, 244, 240, 255],
            };
            // Row 0 of the grid is the lowest metre; the image's row 0 is its top.
            let row = h - 1 - y;
            rgba[(row * w + x) * 4..(row * w + x) * 4 + 4].copy_from_slice(&colour);
        }
    }
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&rgba)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> ReferenceSource {
        ReferenceSource::new("unit", "", "")
    }

    fn square(view: OutlineView, x0: f32, y0: f32, x1: f32, y1: f32) -> OutlineSpec {
        OutlineSpec::new(
            view,
            vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]],
            0.95,
            AnchorStatus::Locked,
            source(),
        )
    }

    /// Two triangles making a 1 × 1 m square in the side plane (z, y), at x = 0.
    fn square_tris(z0: f32, y0: f32, z1: f32, y1: f32) -> Vec<[Vec3; 3]> {
        let p = |z: f32, y: f32| Vec3::new(0.0, y, z);
        vec![[p(z0, y0), p(z1, y0), p(z1, y1)], [p(z0, y0), p(z1, y1), p(z0, y1)]]
    }

    #[test]
    fn a_square_against_the_same_square_is_one() {
        let m = measure(
            &square_tris(0.0, 0.0, 1.0, 1.0),
            &square(OutlineView::Side, 0.0, 0.0, 1.0, 1.0),
        );
        assert!(m.iou() > 0.98, "IoU {}", m.iou());
        assert!(m.passed());
    }

    #[test]
    fn a_square_shifted_by_a_tenth_loses_a_fifth() {
        // Overlap 0.9 × 1.0 over a union of 1.1 × 1.0: IoU 0.818.
        let m = measure(
            &square_tris(0.0, 0.0, 1.0, 1.0),
            &square(OutlineView::Side, 0.1, 0.0, 1.1, 1.0),
        );
        assert!((m.iou() - 0.818).abs() < 0.02, "IoU {}", m.iou());
        assert!(!m.passed());
        assert!((m.outline_covered() - 0.9).abs() < 0.02);
        assert!((m.bake_inside() - 0.9).abs() < 0.02);
    }

    #[test]
    fn loops_union_and_a_hole_is_a_hole() {
        // An outer square with a concentric inner loop: even-odd per loop would punch a hole
        // only if both were one loop; as two loops they OR, so the inner loop adds nothing.
        let spec = OutlineSpec::new(
            OutlineView::Front,
            vec![
                vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                vec![[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75]],
            ],
            0.95,
            AnchorStatus::Target,
            source(),
        );
        let p = |x: f32, y: f32| Vec3::new(x, y, 0.0);
        let tris =
            vec![[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)], [p(0.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)]];
        let m = measure(&tris, &spec);
        assert!(m.iou() > 0.98, "IoU {}", m.iou());
        // A single self-overlapping loop DOES punch the hole (even-odd), which is what lets an
        // author draw an idler's rim as one loop.
        let ring = OutlineSpec::new(
            OutlineView::Front,
            vec![vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [0.0, 0.0],
                [0.25, 0.25],
                [0.25, 0.75],
                [0.75, 0.75],
                [0.75, 0.25],
                [0.25, 0.25],
            ]],
            0.95,
            AnchorStatus::Target,
            source(),
        );
        let m = measure(&tris, &ring);
        assert!((m.outline_covered() - 1.0).abs() < 0.02);
        assert!((m.bake_inside() - 0.75).abs() < 0.02, "bake inside {}", m.bake_inside());
    }

    #[test]
    fn the_overlay_encodes_a_png_the_size_of_the_grid() {
        let tris = square_tris(0.0, 0.0, 1.0, 1.0);
        let spec = square(OutlineView::Side, 0.0, 0.0, 1.0, 1.0);
        let bytes = overlay_png(&tris, &spec).expect("png");
        assert_eq!(&bytes[1..4], b"PNG");
        let raster = rasterise(&tris, &spec, OUTLINE_CELL_M);
        assert!(raster.bake.width() >= 100 && raster.bake.height() >= 100);
    }

    #[test]
    fn a_translated_spec_moves_every_point() {
        let spec = square(OutlineView::Plan, 0.0, 0.0, 1.0, 1.0).translated(Vec2::new(0.5, -0.5));
        assert_eq!(spec.loops()[0][0], [0.5, -0.5]);
        assert_eq!(spec.loops()[0][2], [1.5, 0.5]);
    }
}
