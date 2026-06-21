//! Hitbox-honesty gate for the hybrid T-54, mirroring the procedural lineup gate in
//! `vehicle_geometry/tests/all_vehicles.rs`. Before the hybrid can be the in-game mesh it must
//! obey the same "what you see is what you shoot" contract: the visible body sits inside the
//! collision hitbox and fills it, the visible turret sits inside its gameplay turret plan and
//! fills it (with the traverse pivot inside the plan), and the gun barrel protrudes past the
//! hitbox like a real gun. Without this, the dense mesh could drift from the collision volume the
//! penetration model uses.

use game_core::{HitboxProfile, VehicleKind};
use vehicle_build::t54_description;
use vehicle_geometry::SubmeshKind;

#[test]
fn hybrid_t54_body_fits_and_fills_its_hitbox() {
    // Side/roof/end seating slack, and the deliberate underside sink that hides the belly — the
    // same tolerances the procedural lineup gate uses.
    const EPS: f32 = 0.05;
    const SINK: f32 = 0.15;

    let baked = t54_description().build();
    let hitbox = HitboxProfile::for_vehicle(VehicleKind::T54_1951);
    let body = baked.body_bounds().expect("hybrid T-54 has body bounds");
    let gun = baked
        .submesh(SubmeshKind::Gun)
        .expect("hybrid T-54 has a gun submesh")
        .mesh
        .bounds()
        .expect("gun bounds");

    let top = hitbox.center_y_m + hitbox.half_height_m;
    let floor = hitbox.center_y_m - hitbox.half_height_m;

    // Containment.
    assert!(
        body.min.x >= -hitbox.half_width_m - EPS,
        "body pokes left of hitbox: {:.2}",
        body.min.x
    );
    assert!(
        body.max.x <= hitbox.half_width_m + EPS,
        "body pokes right of hitbox: {:.2}",
        body.max.x
    );
    assert!(
        body.min.z >= -hitbox.half_length_m - EPS,
        "body pokes behind hitbox: {:.2}",
        body.min.z
    );
    assert!(
        body.max.z <= hitbox.half_length_m + EPS,
        "body pokes ahead of hitbox: {:.2}",
        body.max.z
    );
    assert!(body.max.y <= top + EPS, "roof {:.2} pokes above {top:.2}", body.max.y);
    assert!(body.min.y >= floor - SINK, "belly {:.2} sinks past floor {floor:.2}", body.min.y);

    // Fill — guards against narrow / stubby / flat regressions.
    assert!(
        body.max.x >= 0.88 * hitbox.half_width_m,
        "too narrow: {:.2} of {:.2}",
        body.max.x,
        hitbox.half_width_m
    );
    assert!(
        body.max.z >= 0.88 * hitbox.half_length_m,
        "too short: {:.2} of {:.2}",
        body.max.z,
        hitbox.half_length_m
    );
    // The hitbox is sized to the real lowered casting, so the roof must still fill it tightly: no
    // fat air gap a shot could "hit" above the tank.
    assert!(body.max.y >= top - 0.30, "too flat: roof {:.2} vs {top:.2}", body.max.y);

    // The barrel must extend beyond the hitbox.
    assert!(
        gun.max.z > hitbox.half_length_m,
        "barrel {:.2} should protrude past hitbox length {:.2}",
        gun.max.z,
        hitbox.half_length_m
    );
}

#[test]
fn hybrid_t54_turret_fits_and_fills_its_turret_plan() {
    const EPS: f32 = 0.05;

    let baked = t54_description().build();
    let hitbox = HitboxProfile::for_vehicle(VehicleKind::T54_1951);
    let turret = baked
        .submesh(SubmeshKind::Turret)
        .expect("hybrid T-54 has a turret submesh")
        .mesh
        .bounds()
        .expect("turret bounds");
    let z_lo = hitbox.turret_center_z_m - hitbox.turret_half_length_m;
    let z_hi = hitbox.turret_center_z_m + hitbox.turret_half_length_m;

    // Containment: the visual turret may not poke out of its gameplay volume.
    assert!(
        turret.min.x >= -hitbox.turret_half_width_m - EPS,
        "turret pokes left of its plan: {:.2} vs {:.2}",
        turret.min.x,
        -hitbox.turret_half_width_m
    );
    assert!(
        turret.max.x <= hitbox.turret_half_width_m + EPS,
        "turret pokes right of its plan: {:.2} vs {:.2}",
        turret.max.x,
        hitbox.turret_half_width_m
    );
    assert!(
        turret.min.z >= z_lo - EPS,
        "turret pokes behind its plan: {:.2} vs {z_lo:.2}",
        turret.min.z
    );
    assert!(
        turret.max.z <= z_hi + EPS,
        "turret pokes ahead of its plan: {:.2} vs {z_hi:.2}",
        turret.max.z
    );

    // Fill: the volume may not be padded much wider/longer than the visible turret.
    assert!(
        turret.max.x >= 0.85 * hitbox.turret_half_width_m,
        "turret plan too wide: turret reaches {:.2} of {:.2}",
        turret.max.x,
        hitbox.turret_half_width_m
    );
    assert!(
        (turret.max.z - turret.min.z) >= 0.85 * 2.0 * hitbox.turret_half_length_m,
        "turret plan too long: turret spans {:.2} of {:.2}",
        turret.max.z - turret.min.z,
        2.0 * hitbox.turret_half_length_m
    );

    // The traverse pivot sits inside the plan.
    let ring_z = baked.mounts().turret_ring.translation.z;
    assert!(
        ring_z > z_lo && ring_z < z_hi,
        "ring pivot z {ring_z:.2} outside turret plan [{z_lo:.2}, {z_hi:.2}]"
    );
}
