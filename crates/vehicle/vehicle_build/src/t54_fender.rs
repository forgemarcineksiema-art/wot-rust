//! The fender shelves as what they are: folded sheet-steel pressings.
//!
//! They used to be four cuboids a side plus ONE more cuboid glued under the outer edge to stand in
//! for the fold — and `t54_fender_lip`'s own comment said what it was reaching for, "so the mudguard
//! reads as a folded-edge pressing". A box cannot fold, so the fold was a second box.
//!
//! That cost more than elegance. The shelf is deliberately SEGMENTED — four sections a side with a
//! 3 cm seam, because a real fender is bolted up in sections rather than rolled in one slab — but
//! the lip was a single strip spanning the whole run. The seams stopped at the edge and the lip
//! carried straight through them, which no sheet-metal assembly does.
//!
//! Each section is now one `panel` with a `Hem`: the plate, the chamfer and the return lip are one
//! pressing, so a seam is a seam all the way to the fold. The blueprint numbers are unchanged —
//! same footprint, same plate thickness, same lip drop, so the invariant that keeps the lip clear
//! of the track's top run holds exactly as before.

use game_core::{DetailVisual, FenderVisual};
use glam::{Vec2, Vec3};
use panel::{PanelEdge, PanelSpec, try_panel};
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};

/// Sections per fender run — the bolted-up sections, not an arbitrary tessellation.
const SECTIONS: usize = 4;

/// The gap between two bolted sections.
const SEAM_M: f32 = 0.03;

/// Both fenders, as folded pressings.
pub(crate) fn t54_fender_parts(fender: &FenderVisual, detail: &DetailVisual) -> Vec<VehiclePart> {
    let mut parts = Vec::with_capacity(SECTIONS * 2);
    let mut index = 0u16;
    for side_x in [fender.side_x, -fender.side_x] {
        for section in 0..SECTIONS {
            let Some(mesh) = section_panel(side_x, section, fender, detail) else { continue };
            parts.push(VehiclePart {
                key: PartKey::indexed("fender_segment", index),
                submesh: SubmeshKind::Hull,
                material: MaterialRole::RolledArmor,
                smoothing: SmoothingGroup::hard_edges(),
                shape: PartShape::Mesh(mesh),
                lod: PartLod::Detail,
                generator: GeneratorKind::Panel,
            });
            index += 1;
        }
    }
    parts
}

/// One bolted section: a rectangle in the horizontal plane at the shelf's TOP face, pressed down
/// into a plate and folded into its return lip.
///
/// `panel` puts the outline on the origin plane and extrudes along `-normal`, so the origin is the
/// visible top surface — `center_y + half.y`, exactly where the box's top face used to be.
///
/// The hem runs the whole perimeter, which is what a separately pressed section does on its free
/// edges. The inboard fold ends up buried in the hull side (the shelf's inner edge already overlaps
/// it by 25 mm, as the box did), so it costs a few triangles and shows nothing.
fn section_panel(
    side_x: f32,
    section: usize,
    fender: &FenderVisual,
    detail: &DetailVisual,
) -> Option<GeometryMesh> {
    let half_z = fender.half.z / SECTIONS as f32 - SEAM_M;
    if half_z <= 0.0 {
        return None;
    }
    let t = (section as f32 + 0.5) / SECTIONS as f32;
    let center_z = -fender.half.z + t * 2.0 * fender.half.z;
    let outline = vec![
        Vec2::new(-fender.half.x, -half_z),
        Vec2::new(fender.half.x, -half_z),
        Vec2::new(fender.half.x, half_z),
        Vec2::new(-fender.half.x, half_z),
    ];
    try_panel(&PanelSpec {
        outline,
        origin: Vec3::new(side_x, fender.center_y + fender.half.y, center_z),
        normal: Vec3::Y,
        thickness: fender.half.y * 2.0,
        edge: PanelEdge::Hem {
            width: detail.fender_lip_thickness,
            return_depth: detail.fender_lip_drop,
        },
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
    })
    .ok()
}
