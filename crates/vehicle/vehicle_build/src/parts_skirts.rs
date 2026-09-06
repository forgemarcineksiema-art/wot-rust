//! Side skirts as library plates (Forge 2.0 K3, the Tiger II's Schürzen first): the plate the
//! blueprint hangs on the spaced-armour plane (`SkirtShape` — honest both ways, the armour
//! bakes the same plane as a HEAT screen), one part a side, the chamfered prism the recipe drew
//! (`blueprint_skirts`), so a welded-hull vehicle with skirts can leave its recipe behind.

use game_core::{SkirtShape, TrackShape, VehicleBlueprint};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, MeshBuilder, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::smoothing::SG_HARD;

/// The skirt plates for `bp`, or `None` when the library does not build its hull or the
/// blueprint hangs no skirts.
pub fn skirt_parts_for_blueprint(bp: &VehicleBlueprint) -> Option<Vec<VehiclePart>> {
    bp.visual_detail()?.construction?;
    let skirt = bp.hull.skirt?;
    Some(skirt_parts(&bp.track, &skirt))
}

/// Both plates, port and starboard, on the spaced-armour plane outside the belt.
pub fn skirt_parts(track: &TrackShape, skirt: &SkirtShape) -> Vec<VehiclePart> {
    let cx = track.outer_x + skirt.standoff_m + skirt.thickness_m * 0.5;
    let cy = (skirt.top_y + skirt.bottom_y) * 0.5;
    let cz = (skirt.front_z + skirt.rear_z) * 0.5;
    let half = Vec3::new(
        skirt.thickness_m * 0.5,
        (skirt.top_y - skirt.bottom_y).abs() * 0.5,
        (skirt.front_z - skirt.rear_z).abs() * 0.5,
    );
    [1.0_f32, -1.0]
        .into_iter()
        .enumerate()
        .map(|(i, sign)| VehiclePart {
            key: PartKey::indexed("skirt_plate", i as u16),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::RolledArmor,
            smoothing: SG_HARD,
            shape: PartShape::Mesh(
                MeshBuilder::new()
                    .chamfered_prism(
                        Vec3::new(sign * cx, cy, cz),
                        half,
                        0.02,
                        MaterialRole::RolledArmor,
                        SG_HARD,
                    )
                    .build(),
            ),
            lod: PartLod::Silhouette,
            generator: GeneratorKind::Solid,
        })
        .collect()
}
