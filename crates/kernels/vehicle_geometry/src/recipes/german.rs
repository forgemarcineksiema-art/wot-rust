//! German turreted heavies still on the legacy path (Tiger II; the Tiger I moved to its
//! blueprint-born recipe in [`super::tiger_i`]).
//!
//! - **Tiger II**: a long raked glacis and a sloped, faceted Henschel turret swept from a side
//!   silhouette, carrying the longest gun in the lineup.
//!
//! Hull width and length derive from the hitbox as explicit fractions; deck height and gun axis
//! come from the authoritative mount frames in [`game_core`].

use game_core::{HitboxProfile, MountFrames, VehicleKind};
use glam::{Vec2, Vec3};

use super::{
    GunPlan, HullPlan, SG_HARD, add_cupola, add_mantlet_socket, add_turret_ring, assemble,
    build_gun, hull_body, legacy_track_band, shade_hull,
};
use crate::{Axis, BakedVehicle, ExtrudeSpec, MaterialRole, MeshBuilder};

pub(crate) fn tiger_ii(hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let hull = shade_hull(
        hull_body(
            &HullPlan {
                half_len: hitbox.half_length_m * 0.962,
                belly_y: 0.35,
                deck_y: mounts.turret_ring.translation.y,
                glacis_run: 1.05,
                nose_rise: 0.06,
                half_width: hitbox.half_width_m * 0.749,
            },
            MaterialRole::RolledArmor,
        )
        .append(&legacy_track_band(VehicleKind::TigerII))
        .build(),
    );

    let mantlet = Some((0.30, 1.18, 1.55));
    let turret = add_mantlet_socket(
        add_turret_ring(
            add_cupola(
                sloped_turret(
                    &[
                        Vec2::new(-1.30, 1.50),
                        Vec2::new(1.20, 1.50),
                        Vec2::new(1.48, 2.18),
                        Vec2::new(1.05, 2.86),
                        Vec2::new(-1.12, 2.86),
                        Vec2::new(-1.35, 2.20),
                    ],
                    0.95,
                ),
                -0.55,
                -0.70,
                2.86,
                0.22,
                0.18,
                false,
            ),
            mounts.turret_ring.translation.z,
            mounts.turret_ring.translation.y,
            0.92,
            0.13,
            14,
        ),
        mounts.gun_trunnion.translation.y,
        mantlet,
        12,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: mounts.gun_trunnion.translation.y,
        breech_z: mounts.gun_trunnion.translation.z - 0.25,
        muzzle_z: mounts.muzzle.translation.z,
        radius: 0.105,
        segments: 12,
        mantlet,
        evacuator: None,
        muzzle_brake: Some(0.18),
    });

    assemble(VehicleKind::TigerII, hull, turret, gun, *mounts)
}

fn sloped_turret(section: &[Vec2], half_width: f32) -> MeshBuilder {
    MeshBuilder::new().extrude(
        Vec3::ZERO,
        ExtrudeSpec {
            section: section.to_vec(),
            axis: Axis::X,
            half_depth: half_width,
            material: MaterialRole::RolledArmor,
            smoothing: SG_HARD,
        },
    )
}
