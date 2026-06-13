use game_core::{HitboxProfile, VehicleKind};
use vehicle_geometry::{SmoothingGroup, SubmeshKind, bake_vehicle};

/// The migrated T-54's visible body (hull + turret, tracks included) must sit inside the collision
/// box the blueprint derives — what you see is what you hit. The gun barrel is excluded by design
/// (it reaches past the hull), matching `body_bounds`.
#[test]
fn t54_body_fits_within_its_blueprint_hitbox() {
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let bounds = vehicle.body_bounds().expect("body has bounds");
    let hitbox = HitboxProfile::for_vehicle(VehicleKind::T54_1951);
    let eps = 0.02;

    assert!(bounds.min.x >= -hitbox.half_width_m - eps, "left {}", bounds.min.x);
    assert!(bounds.max.x <= hitbox.half_width_m + eps, "right {}", bounds.max.x);
    assert!(bounds.min.z >= -hitbox.half_length_m - eps, "rear {}", bounds.min.z);
    assert!(bounds.max.z <= hitbox.half_length_m + eps, "front {}", bounds.max.z);
    let top = hitbox.center_y_m + hitbox.half_height_m;
    assert!(bounds.max.y <= top + eps, "roof {} above hitbox top {top}", bounds.max.y);
}

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
