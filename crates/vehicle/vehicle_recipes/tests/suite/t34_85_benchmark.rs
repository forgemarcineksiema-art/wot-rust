//! The T-34-85's shape cage: each test names a defect that would un-T-34 the tank. The first
//! vehicle authored through the Forge Studio loop — its shape lives in
//! `blueprints/t34_85.blueprint.ron`, and these are the cage bars around that file.

use game_core::VehicleKind;
use vehicle_geometry::SubmeshKind;
use vehicle_recipes::bake_vehicle;

/// The baked mesh honours the blueprint: hull inside the 3.00 m beam, the dome forward of the
/// hull centre, the whole tank under the documented height.
#[test]
fn the_baked_mesh_reads_the_blueprint() {
    let baked = bake_vehicle(VehicleKind::T34_85).expect("bakes");
    let hull = baked.submesh(SubmeshKind::Hull).unwrap().mesh.bounds().unwrap();
    let turret = baked.submesh(SubmeshKind::Turret).unwrap().mesh.bounds().unwrap();

    assert!(hull.max.x - hull.min.x <= 3.02, "hull spans the documented 3.00 m beam");
    let turret_centre_z = (turret.min.z + turret.max.z) * 0.5;
    assert!(turret_centre_z > 0.1, "the dome sits forward, got centre z {turret_centre_z:.2}");
    assert!(turret.max.y <= 2.75, "the silhouette stays under 2.75 m, got {}", turret.max.y);
}
