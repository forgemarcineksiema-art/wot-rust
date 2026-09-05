//! The Tiger I shape cage: locks the slab anatomy the blueprint migration bought — vertical
//! plates that ARE the armor planes, the horseshoe turret prism, the interleaved wheel rows,
//! the braked KwK 36, and the drum cupola topping the documented 3.00 m silhouette. Each lock
//! names a defect that would silently un-Tiger the tank.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_geometry::SubmeshKind;
use vehicle_recipes::bake_vehicle;

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::TigerI).expect("Tiger I has a blueprint")
}

/// The bow shelf (K3-2d): the STT 1944 side view runs the near-horizontal glacis from the nose
/// line 0.80 m back and up to 1.35, where the 9° driver's plate begins. The armour carries the
/// shelf as a plane facing mostly up through the nose line and the shelf's top edge, the
/// driver's plate through that top edge (not the nose), and the visible metal lies on both —
/// the bow a player sees is the two planes a shell meets.
#[test]
fn the_bow_shelf_sets_the_drivers_plate_back_from_the_nose() {
    let bp = blueprint();
    let (top, setback) = bp.armor.hull_bow_shelf.expect("the Tiger authors its bow shelf");
    assert!(
        (top - 1.35).abs() < 1.0e-6 && (setback - 0.80).abs() < 1.0e-6,
        "STT side view: the driver's plate begins 0.80 m behind the nose, 1.35 m up"
    );
    assert!(top > bp.hull.sponson_y && top < bp.hull.deck_y, "the shelf climbs within the box");
    let volumes = vehicle_armor_volumes(VehicleKind::TigerI).expect("armor volumes");
    let cy = bp.hull.hitbox_center_y;
    let nose = Vec3::new(0.0, bp.hull.sponson_y - cy, bp.hull.half_len);
    let fold = Vec3::new(0.0, top - cy, bp.hull.half_len - setback);
    let on_plane =
        |normal: Vec3, offset: f32, point: Vec3| (normal.dot(point) - offset).abs() < 1.0e-3;
    // The shelf: a glacis-zoned hull plane facing mostly UP, through the nose line and the fold.
    let shelf = volumes
        .hull
        .iter()
        .flat_map(|volume| volume.planes.iter())
        .find(|plane| plane.zone == ArmorZone::UpperGlacis && plane.normal.y > 0.9)
        .expect("the shelf plane");
    assert!(on_plane(shelf.normal, shelf.offset, nose), "the shelf starts at the nose line");
    assert!(on_plane(shelf.normal, shelf.offset, fold), "the shelf ends at the plate's foot");
    // The driver's plate: the upper hull's glacis plane runs through the fold, and the nose
    // line lies well AHEAD of it.
    let plate = volumes.hull[0]
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::UpperGlacis)
        .expect("glacis plane");
    assert!(
        on_plane(plate.normal, plate.offset, fold),
        "the driver's plate folds at the shelf top"
    );
    assert!(plate.normal.dot(nose) - plate.offset > 0.5, "the nose stands ahead of the plate");
    // The visible metal lies on both planes.
    let baked = bake_vehicle(VehicleKind::TigerI).expect("Tiger I bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let vertices_on = |normal: Vec3, offset: f32| {
        hull_mesh
            .vertices()
            .iter()
            .map(|vertex| vertex.position - Vec3::Y * cy)
            .filter(|point| point.x.abs() <= bp.hull.half_width + 1.0e-3)
            .filter(|point| on_plane(normal, offset, *point))
            .count()
    };
    assert!(vertices_on(shelf.normal, shelf.offset) >= 4, "the glacis lies on its plane");
    assert!(vertices_on(plate.normal, plate.offset) >= 4, "the driver's plate lies on its plane");
    // Above the shelf's top edge nothing of the box stands ahead of the set-back plate — only
    // the visor, the MG ball and the headlight, which ride the plate by less than a quarter
    // metre (the old plate stood 0.80 m ahead, so the box cannot hide inside that). Judged on the
    // SHIPPED composition (the recipe's pieces plus the library's fittings): the recipe's own
    // bow furniture is a German-family default the Tiger's authored fittings replace.
    let shipped = vehicle_recipes::describe(VehicleKind::TigerI).expect("Tiger describes").build();
    let proud = shipped
        .submesh(SubmeshKind::Hull)
        .expect("hull submesh")
        .mesh
        .vertices()
        .iter()
        .map(|vertex| vertex.position)
        .filter(|point| point.y > top + 0.01 && point.y <= bp.hull.deck_y + 0.01)
        // The central body only: over the belts the guards' hinged flaps hang ahead of the
        // sprocket wraps, and they are the fender line's, not the box's.
        .filter(|point| point.x.abs() <= bp.hull.lower_half_width)
        .filter(|point| point.z > bp.hull.half_len - setback + 0.25)
        .count();
    assert_eq!(proud, 0, "above the shelf the bow is the set-back driver's plate: {proud}");
}
