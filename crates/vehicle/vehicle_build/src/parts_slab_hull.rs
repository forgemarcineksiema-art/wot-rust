//! The welded slab hull as library plates (Forge 2.0 K3, step 4a): three convex solids built
//! straight from the blueprint — the tub between the belts, the upper box behind the driver's
//! plate (its sides leaning the armour table's `hull_side` degrees from the sponson fold: the
//! Tiger I's vertical slab, the Tiger II's 25°), and the bow shelf's wedge ahead of it when the
//! armour authors one — every face ON the plane the armour volumes bake, so a slab vehicle
//! needs no recipe for its hull. The sections
//! are the recipe's own (`tiger_slab_hull`), lifted into the library so the German line shares
//! them; the fleet gate proves the plates lie on their planes.

use game_core::VehicleBlueprint;
use glam::{Vec2, Vec3};
use solid::{ConvexSolid, Plane};
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};

/// The hull's construction, as the visual file declares it.
pub use game_core::HullConstruction;

/// The slab hull's plates for `bp`, or `None` when its visual file declares no slab construction.
pub fn slab_hull_parts_for_blueprint(bp: &VehicleBlueprint) -> Option<Vec<VehiclePart>> {
    match bp.visual_detail()?.construction? {
        HullConstruction::WeldedSlab => Some(slab_hull_parts(bp)),
    }
}

/// The tub, the upper box and (when authored) the bow shelf's wedge, as plate solids.
pub fn slab_hull_parts(bp: &VehicleBlueprint) -> Vec<VehiclePart> {
    let hull = &bp.hull;
    let glacis = hull.glacis_slope_deg.to_radians().tan();
    // The lower-plate slope derives from the glacis exactly like the armour model's zone table,
    // unless the armour authors the nose plate's own angle.
    let lower_deg =
        bp.armor.hull_lower_front.map(|(deg, _)| deg).unwrap_or(hull.glacis_slope_deg * 0.45);
    let lower = lower_deg.to_radians().tan();
    let rear = hull.rear_slope_deg.to_radians().tan();
    let step = hull.sponson_y;
    let nose_y = hull.belly_y + hull.nose_rise;
    let shelf = bp.armor.hull_bow_shelf;

    // Sections in (z, y): the same polygons the recipe extrudes, as plane sets.
    let tub = vec![
        Vec2::new(-hull.half_len + (step - hull.belly_y) * rear, hull.belly_y),
        Vec2::new(hull.half_len - (step - nose_y) * lower, nose_y),
        Vec2::new(hull.half_len, step),
        Vec2::new(-hull.half_len, step),
    ];
    // Where the driver's plate stands at height `y`: leaning back from its fold — the nose
    // line, or the shelf's top edge.
    let plate_z = |y: f32| match shelf {
        Some((top, setback)) => hull.half_len - setback - (y - top) * glacis,
        None => hull.half_len - (y - step) * glacis,
    };
    let upper = vec![
        Vec2::new(-hull.half_len, step),
        Vec2::new(plate_z(step), step),
        Vec2::new(plate_z(hull.deck_y), hull.deck_y),
        Vec2::new(-hull.half_len + (hull.deck_y - step) * rear, hull.deck_y),
    ];
    let upright = side_planes(hull.lower_half_width, step, 0.0);
    let leaned = side_planes(hull.half_width, step, bp.armor.hull_side.0);
    let mut parts = vec![
        plate_part("slab_tub", prism_solid(&tub, upright)),
        plate_part("slab_upper_box", prism_solid(&upper, leaned)),
    ];
    if let Some((top, setback)) = shelf {
        let wedge = vec![
            Vec2::new(hull.half_len, step),
            Vec2::new(plate_z(step), step),
            Vec2::new(hull.half_len - setback, top),
        ];
        parts.push(plate_part("slab_bow_shelf", prism_solid(&wedge, leaned)));
    }
    parts
}

/// The two side planes of a plate box: through `half_x` at the fold height `fold_y`, leaning
/// inward above it by `lean_deg` — the armour table's `hull_side` degrees, so the upper walls
/// stand on the plane the side armour bakes. 0° is the vertical pair at `±half_x`.
fn side_planes(half_x: f32, fold_y: f32, lean_deg: f32) -> [Plane; 2] {
    let (sin, cos) = lean_deg.to_radians().sin_cos();
    let offset = half_x * cos + fold_y * sin;
    [Plane::new(Vec3::new(cos, sin, 0.0), offset), Plane::new(Vec3::new(-cos, sin, 0.0), offset)]
}

fn plate_part(key: &'static str, solid: ConvexSolid) -> VehiclePart {
    VehiclePart {
        key: PartKey::new(key),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Plates(solid),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::Solid,
    }
}

/// A convex prism from a convex section in the (z, y) plane between two side planes: one plane
/// per section edge (outward normal) plus the pair. The section may be listed in either
/// winding; the centroid decides which way each edge normal faces.
fn prism_solid(section: &[Vec2], sides: [Plane; 2]) -> ConvexSolid {
    let centroid = section.iter().fold(Vec2::ZERO, |sum, p| sum + *p) / section.len() as f32;
    let mut planes = sides.to_vec();
    for i in 0..section.len() {
        let a = section[i];
        let b = section[(i + 1) % section.len()];
        let edge = b - a;
        // A normal to the edge in the (z, y) plane, pointed away from the centroid.
        let mut normal = Vec2::new(edge.y, -edge.x);
        if normal.dot(centroid - a) > 0.0 {
            normal = -normal;
        }
        let normal = normal.normalize_or_zero();
        let n3 = Vec3::new(0.0, normal.y, normal.x);
        let point = Vec3::new(0.0, a.y, a.x);
        planes.push(Plane::new(n3, n3.dot(point)));
    }
    ConvexSolid::new(planes)
}
