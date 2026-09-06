//! Locks "what you see is what you shoot" for the IS-3 pike bow: the visible bow is built on
//! the EXACT planes the armor volumes shoot against. Every hull vertex in the bow zone stays
//! inside the baked armor half-spaces, and both upper pike faces are genuinely present in the
//! mesh (vertices lying ON each plane) — a bow that regressed to a flat glacis fails here.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_geometry::SubmeshKind;
use vehicle_recipes::bake_vehicle;

#[test]
fn the_visible_pike_bow_is_the_armor_pike() {
    let baked = bake_vehicle(VehicleKind::IS3).expect("IS-3 bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::IS3).expect("blueprint");
    let volumes = vehicle_armor_volumes(VehicleKind::IS3).expect("armor volumes");
    let cy = blueprint.hull.hitbox_center_y;

    // The bow zone: forward of where the upper pike face reaches the full hull width at deck
    // height. (The running gear and fenders live further back; the bow is pure hull plate.)
    let sweep = blueprint.hull.pike_sweep_deg.to_radians();
    let glacis = blueprint.armor.hull_front.0.to_radians();
    let normal = Vec3::new(sweep.sin() * glacis.cos(), glacis.sin(), sweep.cos() * glacis.cos());
    let fold = Vec3::new(0.0, blueprint.hull.sponson_y - cy, blueprint.hull.half_len);
    let offset = normal.dot(fold);
    let bow_z =
        (offset - normal.x * blueprint.hull.half_width - normal.y * (blueprint.hull.deck_y - cy))
            / normal.z;

    let upper_planes: Vec<_> = volumes.hull[0]
        .planes
        .iter()
        .filter(|plane| plane.zone == ArmorZone::UpperGlacis)
        .collect();
    assert_eq!(upper_planes.len(), 2, "the pike is two upper planes");

    let mut on_plane = [0_usize; 2];
    let mut bow_vertices = 0_usize;
    for vertex in hull_mesh.vertices() {
        let point = vertex.position - Vec3::Y * cy;
        if point.z <= bow_z + 1.0e-3 {
            continue;
        }
        // The track belt shares the hull submesh but answers to its own armor volume (the
        // spaced track box), not to the pike planes — skip belt geometry.
        let track = blueprint.track;
        if point.x.abs() >= track.inner_x - 1.0e-3
            && point.y <= track.top_y + track.belt_half_thickness - cy + 1.0e-3
        {
            continue;
        }
        bow_vertices += 1;
        for (index, plane) in upper_planes.iter().enumerate() {
            let signed = plane.normal.dot(point) - plane.offset;
            assert!(
                signed <= 1.0e-3,
                "a visible bow vertex pokes {signed} m out of pike plane {index}: {point:?}"
            );
            if signed.abs() < 1.0e-3 {
                on_plane[index] += 1;
            }
        }
    }

    assert!(bow_vertices >= 8, "the bow zone holds real geometry, got {bow_vertices} vertices");
    assert!(
        on_plane[0] >= 3 && on_plane[1] >= 3,
        "both pike faces must be present in the visible mesh, got {on_plane:?}"
    );
}
