use game_core::{HitboxProfile, VehicleBlueprint, VehicleKind};
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind, bake_vehicle};

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

#[test]
fn t54_blueprint_hull_is_a_narrow_box_beside_exposed_tracks() {
    // The T-54 hull is a NARROW box with vertical sides riding between fully exposed tracks —
    // there are no Panther-style sponsons overhanging the running gear. The armour body must stay
    // inside the track inner faces at every height below the fender line, so the tracks (not the
    // hull) carry the side silhouette.
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let hull = vehicle.submesh(SubmeshKind::Hull).expect("hull submesh");

    // The armour body below the fender line (the fender shelf itself rides at ~1.12).
    let body_side_x = hull
        .mesh
        .vertices()
        .iter()
        .filter(|vertex| vertex.material == MaterialRole::RolledArmor)
        .filter(|vertex| vertex.position.y <= 1.05)
        .map(|vertex| vertex.position.x.abs())
        .fold(0.0_f32, f32::max);
    // The armour body at roof height: the same vertical plane — no widening near the top.
    let roof_side_x = hull
        .mesh
        .vertices()
        .iter()
        .filter(|vertex| vertex.material == MaterialRole::RolledArmor)
        .filter(|vertex| {
            vertex.position.y >= blueprint.hull.deck_y - 0.06
                && vertex.position.y <= blueprint.hull.deck_y + 0.02
        })
        .map(|vertex| vertex.position.x.abs())
        .fold(0.0_f32, f32::max);
    let gear_side_x = hull
        .mesh
        .vertices()
        .iter()
        .filter(|vertex| matches!(vertex.material, MaterialRole::TrackMetal | MaterialRole::Rubber))
        .map(|vertex| vertex.position.x.abs())
        .fold(0.0_f32, f32::max);

    assert!(
        body_side_x <= blueprint.hull.half_width + 0.03,
        "hull body {body_side_x:.2} must stay inside the narrow box half-width {:.2}",
        blueprint.hull.half_width
    );
    assert!(
        roof_side_x <= blueprint.hull.half_width + 0.03,
        "hull roof edge {roof_side_x:.2} must stay on the same vertical side plane {:.2}",
        blueprint.hull.half_width
    );
    // The exposed tracks stand well proud of the hull body — they do the visual work.
    assert!(
        gear_side_x >= blueprint.hull.half_width + 0.45,
        "running gear {gear_side_x:.2} should stand far proud of the narrow hull {:.2}",
        blueprint.hull.half_width
    );
}

/// The T-54 hull carries welded engine-deck detail (raised beveled plates) on the flat deck behind
/// the turret — the only hull geometry that rises above the deck there, so finding it proves the
/// plate_box deck panels baked in.
#[test]
fn t54_hull_has_raised_engine_deck_detail_behind_the_turret() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let hull = vehicle.submesh(SubmeshKind::Hull).expect("hull submesh");

    let raised_behind_turret = hull.mesh.vertices().iter().any(|vertex| {
        vertex.position.z < bp.turret.ring_z - 1.0
            && vertex.position.y > bp.hull.deck_y + 0.05
            && vertex.position.x.abs() < bp.hull.half_width
    });
    assert!(
        raised_behind_turret,
        "engine-deck plates should rise above the rear deck behind the turret ring"
    );
}

const SG_CAST: SmoothingGroup = SmoothingGroup(2);
const SG_CUPOLA: SmoothingGroup = SmoothingGroup(3);
const SG_MANTLET: SmoothingGroup = SmoothingGroup(6);
const SG_RING: SmoothingGroup = SmoothingGroup(7);

/// The Soviet cast turret is a lofted, egg-shaped shell — fuller ahead of the ring than behind —
/// not a circular revolve dome. Measured on the shell's own smoothing group so the cupola, ring,
/// and mantlet fittings don't muddy the read; a symmetric dome would tie front and rear.
#[test]
fn t54_cast_turret_shell_carries_its_mass_forward() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let turret = vehicle.submesh(SubmeshKind::Turret).expect("turret submesh");

    let shell_z: Vec<f32> = turret
        .mesh
        .vertices()
        .iter()
        .filter(|vertex| vertex.smoothing == SG_CAST)
        .map(|vertex| vertex.position.z)
        .collect();
    assert!(!shell_z.is_empty(), "turret must contain a lofted cast shell");

    let front = shell_z.iter().copied().fold(f32::MIN, f32::max) - bp.turret.ring_z;
    let rear = bp.turret.ring_z - shell_z.iter().copied().fold(f32::MAX, f32::min);
    assert!(
        front > rear + 0.10,
        "cast shell should carry more mass ahead of the ring than behind ({front:.2} vs {rear:.2})"
    );
}

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

#[test]
fn t54_mantlet_socket_visibly_seats_the_moving_mask() {
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let trunnion = vehicle.mounts().gun_trunnion.translation;
    let radial_from_gun_axis = |vertex: &vehicle_geometry::GeometryVertex| {
        let dx = vertex.position.x - trunnion.x;
        let dy = vertex.position.y - trunnion.y;
        (dx * dx + dy * dy).sqrt()
    };

    let turret = vehicle.submesh(SubmeshKind::Turret).expect("turret submesh");
    let socket_radius = turret
        .mesh
        .vertices()
        .iter()
        .filter(|v| v.smoothing == SG_MANTLET)
        .map(radial_from_gun_axis)
        .fold(0.0_f32, f32::max);

    let gun = vehicle.submesh(SubmeshKind::Gun).expect("gun submesh");
    let moving_mantlet_radius = gun
        .mesh
        .vertices()
        .iter()
        .filter(|v| v.smoothing == SG_MANTLET)
        .map(radial_from_gun_axis)
        .fold(0.0_f32, f32::max);

    assert!(
        socket_radius >= moving_mantlet_radius * 1.35,
        "T-54 socket radius {socket_radius:.2} should visibly frame moving mantlet radius {moving_mantlet_radius:.2}"
    );
}
