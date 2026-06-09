use game_core::VehicleKind;
use vehicle_geometry::{SmoothingGroup, SubmeshKind, bake_vehicle};

const SG_MANTLET: SmoothingGroup = SmoothingGroup(6);
const SG_RING: SmoothingGroup = SmoothingGroup(7);

#[test]
fn turreted_vehicles_have_dedicated_turret_ring_geometry() {
    for kind in VehicleKind::ALL {
        if kind == VehicleKind::Jagdtiger {
            continue;
        }
        let vehicle = bake_vehicle(kind).unwrap_or_else(|e| panic!("{kind:?} should bake: {e}"));
        let ring = vehicle.mounts().turret_ring.translation;
        let turret = vehicle.submesh(SubmeshKind::Turret).expect("turret submesh");
        let ring_vertices = turret
            .mesh
            .vertices()
            .iter()
            .filter(|v| v.smoothing == SG_RING)
            .filter(|v| (v.position.y - ring.y).abs() <= 0.10)
            .filter(|v| {
                let dx = v.position.x;
                let dz = v.position.z - ring.z;
                (dx * dx + dz * dz).sqrt() >= 0.35
            })
            .count();

        assert!(ring_vertices >= 12, "{kind:?} missing visible turret-ring collar");
    }
}

#[test]
fn mantlet_socket_is_fixed_to_turret_at_the_trunnion() {
    for kind in VehicleKind::ALL {
        let vehicle = bake_vehicle(kind).unwrap_or_else(|e| panic!("{kind:?} should bake: {e}"));
        let trunnion = vehicle.mounts().gun_trunnion.translation;
        let turret = vehicle.submesh(SubmeshKind::Turret).expect("turret submesh");
        let socket_vertices = turret
            .mesh
            .vertices()
            .iter()
            .filter(|v| v.smoothing == SG_MANTLET)
            .filter(|v| v.position.z >= trunnion.z - 0.45 && v.position.z <= trunnion.z + 0.10)
            .filter(|v| (v.position.y - trunnion.y).abs() <= 0.55)
            .filter(|v| v.position.x.abs() <= 0.70)
            .count();

        assert!(socket_vertices >= 12, "{kind:?} missing fixed mantlet socket");
    }
}
