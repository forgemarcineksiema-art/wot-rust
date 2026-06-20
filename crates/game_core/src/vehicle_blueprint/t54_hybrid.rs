//! The authoritative hybrid visual dimensions for the T-54 benchmark. These reproduce the previous
//! hand-tuned generator constants exactly, so migrating the generators onto this single source
//! leaves the baked mesh unchanged; refining the forms toward the hitbox is a later stage.

use glam::Vec3;

use super::{
    BoxVisual, FenderVisual, GunVisual, HullPlatesVisual, HullVisual, HybridVisual,
    RunningGearVisual, TrackBeltVisual, TurretVisual,
};

pub(super) fn t54_hybrid() -> HybridVisual {
    HybridVisual {
        hull: HullVisual {
            half_width: 1.45,
            belly_y: 0.10,
            roof_y: 1.20,
            half_len: 2.90,
            glacis_offset: 1.85,
            nose_normal: Vec3::new(0.0, -0.5, 1.0),
            nose_offset: 2.55,
        },
        hull_plates: HullPlatesVisual {
            glacis_base_z: 2.85,
            nose_base_z: 2.55,
            deck_bevel: 0.10,
            sponson_overhang: 0.23,
        },
        turret: TurretVisual {
            dome_radius: 0.95,
            dome_front: Vec3::new(0.0, 1.80, 0.28),
            dome_rear: Vec3::new(0.0, 1.80, -0.32),
            dome_blend: 0.60,
            ring_radius: 0.98,
            ring_half_height: 0.30,
            ring_center: Vec3::new(0.0, 1.60, 0.0),
            ring_blend: 0.45,
            roof_plane_y: 2.05,
            ring_plane_y: 1.30,
            cupola_radius: 0.23,
            cupola_half_height: 0.17,
            cupola_center: Vec3::new(-0.34, 2.11, -0.10),
            cupola_blend: 0.05,
            socket_radius: 0.34,
            socket_center: Vec3::new(0.0, 1.72, 1.05),
            socket_blend: 0.06,
            bbox_min: Vec3::new(-1.20, 1.25, -1.20),
            bbox_max: Vec3::new(1.20, 2.30, 1.45),
            budget: 9_000,
        },
        gun: GunVisual {
            barrel_radius: 0.09,
            muzzle_radius: 0.085,
            muzzle_taper: 0.20,
            barrel_segments: 20,
            mantlet_profile: [(-0.18, 0.22), (0.0, 0.34), (0.18, 0.24)],
            mantlet_segments: 20,
            mantlet_scale: Vec3::new(1.45, 0.72, 1.0),
            module_delta_scale: 0.65,
        },
        deck: BoxVisual { center: Vec3::new(0.0, 1.25, -1.75), half: Vec3::new(1.30, 0.06, 0.95) },
        fender: FenderVisual { side_x: 1.5, center_y: 0.90, half: Vec3::new(0.28, 0.03, 2.55) },
        running_gear: RunningGearVisual {
            wheel_radius: 0.42,
            wheel_half_width: 0.12,
            hub_radius: 0.24,
            hub_overhang: 0.02,
            wheel_segments: 20,
            hub_segments: 14,
            wheel_count: 5,
            first_z: -2.10,
            spacing: 1.05,
            first_gap: 1.30,
            side_x: 1.5,
        },
        track_belt: TrackBeltVisual {
            front_z: 2.3,
            rear_z: -2.3,
            axle_y: 0.42,
            radius: 0.47,
            side_x: 1.5,
            half_thickness: 0.06,
            half_width: 0.22,
            straight_segments: 6,
            arc_segments: 8,
        },
    }
}
