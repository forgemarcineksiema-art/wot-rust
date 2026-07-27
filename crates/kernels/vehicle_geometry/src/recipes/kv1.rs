//! The KV-1 mod. 1942: Era II's anvil, blueprint-born. The character is MASS WITHOUT SLOPE — a
//! shallow stepped bow over dead-vertical 75 mm sides, a flat deck, and a cast turret that is a
//! long slab-sided LOAF: near-vertical walls, a broad flat roof, rounded caps front and rear.
//!
//! Deliberately NOT built on `soviet_cast_turret`. That shell's silhouette is hard-coded in a
//! station table ending at a 0.66 roof scale — it never receives the slope fields and it discards
//! `roof_radius` outright — so no blueprint value can flatten it. A domed KV would be a fifth
//! member of the T-54/T-34-85/IS-3/Centurion dome family, and the loaf is the one thing this tank
//! is recognised by at 300 m. Dossier: docs/vehicles/kv-1.md.

use game_core::{HitboxProfile, HullShape, MountFrames, TrackShape, TurretShape, VehicleKind};
use glam::{Vec2, Vec3};

use super::{
    GunPlan, SG_CAST, SG_HARD, add_broad_mantlet_socket, add_commander_periscope,
    add_flush_ring_hatch, add_turret_ring, assemble, blueprint_prism_hull, build_gun, shade_hull,
};
use crate::{
    Axis, BakedVehicle, LoftSection, LoftSpec, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
};

pub(crate) fn kv1_1942(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp = super::active_blueprint(VehicleKind::KV1_1942).expect("KV-1 has a blueprint");
    let hull = shade_hull(
        blueprint_prism_hull(&bp.hull, bp.armor.hull_side.0)
            .append(&super::deck_details::kv1_deck(&bp))
            .append(&kv1_fenders(&bp.hull, &bp.track).build())
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    // Roof furniture: twin flush hatches at the rear of the long casting and the commander's
    // periscope between them. NO drum cupola — that arrived with the KV-1S.
    let turret = add_broad_mantlet_socket(
        add_turret_ring(
            add_commander_periscope(
                add_flush_ring_hatch(
                    add_flush_ring_hatch(
                        kv1_loaf_turret(t),
                        t.cupola_x,
                        t.cupola_z,
                        t.roof_y,
                        t.cupola_radius,
                        -1.0,
                    ),
                    -t.cupola_x,
                    t.cupola_z,
                    t.roof_y,
                    t.cupola_radius,
                    1.0,
                ),
                0.0,
                t.cupola_z + 0.40,
                t.roof_y,
            ),
            t.ring_z,
            t.ring_y,
            t.ring_radius,
            0.12,
            16,
        ),
        bp.gun.trunnion_y,
        mantlet,
        12,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: bp.gun.trunnion_y,
        breech_z: bp.gun.trunnion_z - 0.22,
        muzzle_z: bp.gun.muzzle_z,
        radius: bp.gun.barrel_radius,
        segments: bp.gun.segments,
        mantlet,
        evacuator: bp.gun.evacuator,
        muzzle_brake: bp.gun.muzzle_brake,
    });

    assemble(VehicleKind::KV1_1942, hull, turret, gun, *mounts)
}

/// The stadium plan the loaf is lofted from: straight parallel walls at `half_w` between an
/// elliptical front cap and a shallower rear cap. Authored front-first and swept through +x to
/// the rear, the same winding `cast_ring` uses, so the sections loft cleanly.
fn stadium_plan(
    half_w: f32,
    front_z: f32,
    rear_z: f32,
    cap_front: f32,
    cap_rear: f32,
) -> Vec<Vec2> {
    // Quarter-ellipse steps from the nose out to the shoulder (and mirrored at the tail).
    const STEPS: usize = 4;
    let mut plan = Vec::with_capacity(4 * STEPS + 2);
    let cap = |t: f32, depth: f32, z: f32, sign: f32| {
        let angle = t * std::f32::consts::FRAC_PI_2;
        Vec2::new(half_w * angle.sin(), z - sign * depth * (1.0 - angle.cos()))
    };
    // Front cap, centre-line out to the +x shoulder.
    for step in 0..=STEPS {
        plan.push(cap(step as f32 / STEPS as f32, cap_front, front_z, 1.0));
    }
    // Rear cap, +x shoulder back to the centre-line.
    for step in (0..=STEPS).rev() {
        plan.push(cap(step as f32 / STEPS as f32, cap_rear, rear_z, -1.0));
    }
    // Mirror the whole -x side, skipping the two centre-line points already placed.
    let mirrored: Vec<Vec2> =
        plan.iter().rev().skip(1).take(plan.len() - 2).map(|p| Vec2::new(-p.x, p.y)).collect();
    plan.extend(mirrored);
    plan
}

/// The loaf: a stadium plan lofted from the ring seat to the roof with the blueprint's REAL
/// slopes applied. 6° of side slope over the rise pulls the roof in by about a tenth of a metre,
/// so the crown keeps ~90% of the shoulder beam — that retained width is the difference between
/// a slab and a dome, and `kv1_benchmark` locks it.
fn kv1_loaf_turret(t: &TurretShape) -> MeshBuilder {
    let front_z = t.ring_z + t.plan_half_length;
    let rear_z = t.ring_z - t.plan_half_length;
    let cap_front = 0.52;
    let cap_rear = 0.48;
    let rise = t.roof_y - t.ring_y;

    let section_at = |fraction: f32| {
        let dx = rise * fraction * t.side_slope_deg.to_radians().tan();
        let d_front = rise * fraction * t.front_slope_deg.to_radians().tan();
        let d_rear = rise * fraction * t.rear_slope_deg.to_radians().tan();
        stadium_plan(
            t.plan_half_width - dx,
            front_z - d_front,
            rear_z + d_rear,
            cap_front,
            cap_rear,
        )
    };

    let shell = MeshBuilder::new().loft(
        Vec3::ZERO,
        LoftSpec {
            sections: vec![
                LoftSection::new(t.ring_y, section_at(0.0)),
                LoftSection::new(t.ring_y + rise * 0.55, section_at(0.55)),
                LoftSection::new(t.roof_y, section_at(1.0)),
            ],
            axis: Axis::Y,
            material: MaterialRole::CastArmor,
            smoothing: SG_CAST,
            cap_ends: true,
        },
    );

    // The rear DT ball in its armoured collar — the mod-1942 casting's signature, and the one
    // fitting no other vehicle in the fleet wears. A ball seated IN the rear wall, not a stub
    // stuck on it: the collar starts inside the casting and the ball emerges from it.
    let ball_fraction = 0.45;
    let ball_y = t.ring_y + rise * ball_fraction;
    let wall_z = rear_z + rise * ball_fraction * t.rear_slope_deg.to_radians().tan();
    shell.capped_revolve_at(
        Vec3::new(0.0, ball_y, 0.0),
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(0.16, wall_z + 0.06),
                ProfilePoint::new(0.115, wall_z - 0.07),
            ],
            axis: Axis::Z,
            segments: 12,
            material: MaterialRole::CastArmor,
            smoothing: SG_CAST,
        },
    )
}

/// The full-length squared track guards. Flat wide shelves running the whole hull just clear of
/// the top run — square-ended, not the T-34's stepped fender or the Centurion's hung skirt. They
/// stay inside the track band's outer face so nothing visible hangs over un-hittable air.
fn kv1_fenders(hull: &HullShape, track: &TrackShape) -> MeshBuilder {
    let mut builder = MeshBuilder::new();
    let inner = track.inner_x;
    let outer = track.outer_x - 0.01;
    let half_x = (outer - inner) * 0.5;
    let center_x = inner + half_x;
    for side in [-1.0_f32, 1.0] {
        builder = builder.plate_box(
            Vec3::new(side * center_x, track.top_y + 0.04, 0.0),
            Vec3::new(half_x, 0.018, hull.half_len),
            0.012,
            MaterialRole::RolledArmor,
            SG_HARD,
        );
    }
    builder
}
