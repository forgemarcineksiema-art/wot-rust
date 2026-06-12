use game_core::VehicleKind;
use vehicle_geometry::{SmoothingGroup, SubmeshKind, bake_vehicle};

const SG_CUPOLA: SmoothingGroup = SmoothingGroup(3);
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

/// Cupolas are authored with absolute heights, so this pins their seating: the cupola must crown
/// the turret (be its highest point) while its base stays sunk into the roof — a mount-frame or
/// turret-shape change that leaves a cupola floating or swallowed turns this red. The Jagdtiger
/// (bare casemate) and the prototype medium (plain box turret) carry no cupola by design.
#[test]
fn cupolas_crown_their_turret_roofs() {
    for kind in VehicleKind::ALL {
        if matches!(kind, VehicleKind::Jagdtiger | VehicleKind::PrototypeMedium) {
            continue;
        }
        let vehicle = bake_vehicle(kind).unwrap_or_else(|e| panic!("{kind:?} should bake: {e}"));
        let turret = vehicle.submesh(SubmeshKind::Turret).expect("turret submesh");

        let mut cupola_min_y = f32::INFINITY;
        let mut cupola_max_y = f32::NEG_INFINITY;
        let mut rest_max_y = f32::NEG_INFINITY;
        for vertex in turret.mesh.vertices() {
            if vertex.smoothing == SG_CUPOLA {
                cupola_min_y = cupola_min_y.min(vertex.position.y);
                cupola_max_y = cupola_max_y.max(vertex.position.y);
            } else {
                rest_max_y = rest_max_y.max(vertex.position.y);
            }
        }

        assert!(cupola_max_y.is_finite(), "{kind:?} turret should carry a cupola");
        assert!(
            cupola_max_y >= rest_max_y - 1.0e-3,
            "{kind:?} cupola top {cupola_max_y:.2} swallowed below turret roof {rest_max_y:.2}"
        );
        assert!(
            cupola_min_y <= rest_max_y + 1.0e-3,
            "{kind:?} cupola base {cupola_min_y:.2} floats above turret roof {rest_max_y:.2}"
        );
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
