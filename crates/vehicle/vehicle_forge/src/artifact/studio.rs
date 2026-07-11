//! The Forge Studio bundle: everything an AUTHOR (human or AI) needs to judge one vehicle in a
//! single bake→look→edit iteration. One `report.md` carries the numbers (dimensions, reference
//! ratios, mesh quality, budgets, golden-hash status, blueprint lint); a `contact_sheet.png`
//! carries the eyes (every review camera as an annotated tile); `views/*.png` are the
//! per-camera tiles at golden-compare granularity. All rendered through the deterministic CPU
//! rasterizer (`review_images`) — the renderer-free rule holds; real-light beauty shots stay in
//! the client examples.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use game_core::{VehicleBlueprint, VehicleKind, lint};
use vehicle_geometry::{
    BakedVehicle, OPEN_OR_CLOSED_MESH, SubmeshKind, VEHICLE_BUDGETS, golden_bake_hash,
};

use crate::authoritative_baked_vehicle;

use super::{ArtifactError, review_images, review_text};

/// Tile resolution: 2x the artifact review images in each axis, sized to be READ, not archived.
const TILE_WIDTH: u32 = 512;
const TILE_HEIGHT: u32 = 320;
/// The annotation strip drawn under each tile in the contact sheet.
const LABEL_STRIP: u32 = 18;
const SHEET_COLUMNS: u32 = 3;
const INK: [u8; 4] = [30, 32, 30, 255];
const STRIP_BG: [u8; 4] = [226, 226, 220, 255];

/// One studio view tile: the camera's file name and its encoded PNG.
#[derive(Debug, Clone)]
pub struct StudioView {
    pub name: &'static str,
    pub png: Vec<u8>,
}

/// The whole review bundle for one vehicle.
#[derive(Debug, Clone)]
pub struct StudioBundle {
    vehicle: VehicleKind,
    views: Vec<StudioView>,
    contact_sheet_png: Vec<u8>,
    report_md: String,
}

impl StudioBundle {
    pub fn vehicle(&self) -> VehicleKind {
        self.vehicle
    }

    pub fn views(&self) -> &[StudioView] {
        &self.views
    }

    pub fn report_md(&self) -> &str {
        &self.report_md
    }

    pub fn contact_sheet_png(&self) -> &[u8] {
        &self.contact_sheet_png
    }

    /// Write `report.md`, `contact_sheet.png` and `views/<name>.png` under `dir`.
    pub fn write_to_dir(&self, dir: &Path) -> Result<(), ArtifactError> {
        fs::create_dir_all(dir.join("views"))?;
        fs::write(dir.join("report.md"), self.report_md.as_bytes())?;
        fs::write(dir.join("contact_sheet.png"), &self.contact_sheet_png)?;
        for view in &self.views {
            fs::write(dir.join("views").join(view.name), &view.png)?;
        }
        Ok(())
    }
}

/// Bake `kind` through its authoritative mesh path and assemble the review bundle.
pub fn bake_studio_bundle(kind: VehicleKind) -> Result<StudioBundle, ArtifactError> {
    let baked = authoritative_baked_vehicle(kind)?;
    bundle_from_baked(kind, baked)
}

/// The fast loop: assemble the bundle from a LIVE blueprint (an edited RON parsed from disk,
/// no rebuild). Bakes through the recipe path — for the T-54 that is its legacy recipe, not
/// the authoritative hybrid, which is fine for shape-tuning the blueprint fields.
pub fn bake_studio_bundle_from_blueprint(
    blueprint: &game_core::VehicleBlueprint,
) -> Result<StudioBundle, ArtifactError> {
    let baked = vehicle_geometry::bake_vehicle_from_blueprint(blueprint)?;
    bundle_from_baked(blueprint.kind, baked)
}

fn bundle_from_baked(
    kind: VehicleKind,
    baked: BakedVehicle,
) -> Result<StudioBundle, ArtifactError> {
    let spec =
        crate::registry::forge_spec(kind).ok_or(ArtifactError::MissingReferencePack(kind))?;
    let cameras = (spec.review_cameras)();
    let reference = (spec.reference_pack)();
    let ratio_report =
        reference.measure_baked_vehicle(&baked).ok_or(ArtifactError::RatioReportRejected(kind))?;

    // Per-camera tiles, each annotated with the camera name in its top-left corner.
    let mut views = Vec::new();
    for camera in cameras.cameras() {
        let mut pixels =
            review_images::render_camera_sized(&baked, camera, TILE_WIDTH, TILE_HEIGHT);
        review_text::draw_text(
            &mut pixels,
            TILE_WIDTH,
            TILE_HEIGHT,
            6,
            6,
            2,
            trim_view_name(camera.file_name()),
            INK,
        );
        views.push(StudioView {
            name: camera.file_name(),
            png: review_images::encode_png_sized(TILE_WIDTH, TILE_HEIGHT, &pixels)?,
        });
    }

    let contact_sheet_png = bake_contact_sheet(&baked, &cameras)?;
    let report_md = build_report(kind, &baked, &ratio_report);

    Ok(StudioBundle { vehicle: kind, views, contact_sheet_png, report_md })
}

fn trim_view_name(file_name: &str) -> &str {
    file_name.strip_suffix(".png").unwrap_or(file_name)
}

/// Compose every camera into one grid image with an annotation strip under each tile.
fn bake_contact_sheet(
    baked: &BakedVehicle,
    cameras: &super::ReviewCameraSet,
) -> Result<Vec<u8>, ArtifactError> {
    let count = cameras.cameras().len() as u32;
    let columns = SHEET_COLUMNS.min(count.max(1));
    let rows = count.div_ceil(columns);
    let cell_h = TILE_HEIGHT + LABEL_STRIP;
    let (sheet_w, sheet_h) = (columns * TILE_WIDTH, rows * cell_h);
    let mut sheet = vec![0u8; (sheet_w * sheet_h * 4) as usize];
    for pixel in sheet.chunks_exact_mut(4) {
        pixel.copy_from_slice(&STRIP_BG);
    }

    for (index, camera) in cameras.cameras().iter().enumerate() {
        let tile = review_images::render_camera_sized(baked, camera, TILE_WIDTH, TILE_HEIGHT);
        let (col, row) = (index as u32 % columns, index as u32 / columns);
        let (x0, y0) = (col * TILE_WIDTH, row * cell_h);
        for y in 0..TILE_HEIGHT {
            let src = ((y * TILE_WIDTH) * 4) as usize;
            let dst = (((y0 + y) * sheet_w + x0) * 4) as usize;
            sheet[dst..dst + (TILE_WIDTH * 4) as usize]
                .copy_from_slice(&tile[src..src + (TILE_WIDTH * 4) as usize]);
        }
        review_text::draw_text(
            &mut sheet,
            sheet_w,
            sheet_h,
            x0 as i32 + 6,
            (y0 + TILE_HEIGHT + 3) as i32,
            1,
            trim_view_name(camera.file_name()),
            INK,
        );
    }
    Ok(review_images::encode_png_sized(sheet_w, sheet_h, &sheet)?)
}

/// The one file the author reads first: every number the gates enforce, quoted from the same
/// sources the gates read.
fn build_report(
    kind: VehicleKind,
    baked: &BakedVehicle,
    ratio_report: &crate::RatioReport,
) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "# Forge Studio — {}\n", kind.display_name());

    // Dimensions per submesh.
    let _ = writeln!(md, "## Dimensions (baked mesh bounds, metres)\n");
    let _ = writeln!(md, "| Submesh | X (width) | Y (height) | Z (length) | Tris | Verts |");
    let _ = writeln!(md, "| --- | --- | --- | --- | ---: | ---: |");
    for submesh in baked.submeshes() {
        if let Some(bounds) = submesh.mesh.bounds() {
            let size = bounds.max - bounds.min;
            let _ = writeln!(
                md,
                "| {:?} | {:.2} ({:+.2}..{:+.2}) | {:.2} ({:+.2}..{:+.2}) | {:.2} ({:+.2}..{:+.2}) | {} | {} |",
                submesh.kind,
                size.x,
                bounds.min.x,
                bounds.max.x,
                size.y,
                bounds.min.y,
                bounds.max.y,
                size.z,
                bounds.min.z,
                bounds.max.z,
                submesh.mesh.triangle_count(),
                submesh.mesh.vertex_count(),
            );
        }
    }

    // Reference ratios.
    let _ = writeln!(md, "\n## Reference ratios\n");
    let _ = writeln!(md, "{}", ratio_report.markdown_summary());

    // Budgets vs the shared envelope (the same source `vehicle_budgets.rs` enforces).
    let budgets = VEHICLE_BUDGETS;
    let tris = |sub| baked.submesh(sub).map_or(0, |s| s.mesh.triangle_count());
    let verts = |sub| baked.submesh(sub).map_or(0, |s| s.mesh.vertex_count());
    let total_tris = tris(SubmeshKind::Hull) + tris(SubmeshKind::Turret) + tris(SubmeshKind::Gun);
    let total_verts =
        verts(SubmeshKind::Hull) + verts(SubmeshKind::Turret) + verts(SubmeshKind::Gun);
    let _ = writeln!(md, "## Budgets\n");
    let _ = writeln!(md, "| Meter | Value | Envelope | Status |");
    let _ = writeln!(md, "| --- | ---: | --- | --- |");
    for (label, value, (min, max)) in [
        ("hull tris", tris(SubmeshKind::Hull), budgets.hull_tri),
        ("turret tris", tris(SubmeshKind::Turret), budgets.turret_tri),
        ("gun tris", tris(SubmeshKind::Gun), budgets.gun_tri),
        ("vehicle tris", total_tris, budgets.vehicle_tri),
    ] {
        let ok = (min..=max).contains(&value);
        let _ = writeln!(
            md,
            "| {label} | {value} | {min}..={max} | {} |",
            if ok { "OK" } else { "OUT OF BUDGET" }
        );
    }
    let _ = writeln!(
        md,
        "| vehicle verts | {total_verts} | <= {} | {} |",
        budgets.vehicle_vert_max,
        if total_verts <= budgets.vehicle_vert_max { "OK" } else { "OUT OF BUDGET" }
    );

    // Mesh quality per submesh.
    let _ = writeln!(md, "\n## Mesh quality (spec: open-or-closed manifold)\n");
    for submesh in baked.submeshes() {
        let report = submesh.mesh.quality_report(OPEN_OR_CLOSED_MESH);
        let clean = report.non_manifold_edges == 0
            && report.degenerate_triangles == 0
            && report.invalid_indices == 0
            && report.non_finite_vertices == 0
            && report.non_unit_normals == 0
            && report.inconsistent_winding_edges == 0;
        let _ = writeln!(
            md,
            "- {:?}: {} (boundary edges {}, non-manifold {}, degenerate {}, bad normals {}, \
             inconsistent winding {})",
            submesh.kind,
            if clean { "CLEAN" } else { "DEFECTS" },
            report.boundary_edges,
            report.non_manifold_edges,
            report.degenerate_triangles,
            report.non_unit_normals,
            report.inconsistent_winding_edges,
        );
    }

    // Golden hash status.
    let hash = baked.deterministic_hash();
    let _ = writeln!(md, "\n## Determinism\n");
    match golden_bake_hash(kind) {
        Some(golden) if golden == hash => {
            let _ = writeln!(md, "- bake hash `{hash}` MATCHES the recorded golden.");
        }
        Some(golden) => {
            let _ = writeln!(
                md,
                "- bake hash `{hash}` DIFFERS from golden `{golden}` — the geometry changed. \
                 If intentional, re-record in `vehicle_geometry/src/budgets.rs`."
            );
        }
        None => {
            let _ = writeln!(md, "- bake hash `{hash}` (no golden recorded for this kind).");
        }
    }

    // Blueprint lint.
    let _ = writeln!(md, "\n## Blueprint lint\n");
    match VehicleBlueprint::for_vehicle(kind) {
        Some(blueprint) => {
            let issues = lint::validate_blueprint(&blueprint);
            if issues.is_empty() {
                let _ = writeln!(md, "- no issues.");
            }
            for item in issues {
                let _ = writeln!(md, "- [{:?}] {}: {}", item.severity, item.field, item.message);
            }
        }
        None => {
            let _ = writeln!(md, "- no blueprint (legacy path).");
        }
    }

    // The loop, spelled out where the author is already looking.
    let _ = writeln!(md, "\n## The loop\n");
    let _ = writeln!(
        md,
        "1. Edit `crates/foundation/game_core/blueprints/{}.blueprint.ron`\n2. `cargo run -p \
         tools -- studio --vehicle {}`\n3. Re-read this report, then `contact_sheet.png`.",
        kind.slug(),
        kind.slug(),
    );
    md
}
