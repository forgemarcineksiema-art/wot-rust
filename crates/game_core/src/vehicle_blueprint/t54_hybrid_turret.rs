//! The T-54 lofted cast-turret stations and shaping — split out of `t54_hybrid` to keep each file
//! within the reviewability budget. Stations run from the ring seat (1.30) up to the flat roof
//! (1.86), widest LOW (station 2) for the ring overhang, front-heavy (front > rear half-length) with
//! a rear-pulled bustle (negative z_center climbing with height), necking into the flat roof. All
//! within the ±1.00 / ±1.04 turret plan. Cheeks and the front gun embrasure ride as localized radial
//! modulations of the one surface.

use glam::Vec3;

use super::{LoftStation, TurretLoftVisual};

pub(super) fn turret_loft() -> TurretLoftVisual {
    TurretLoftVisual {
        stations: [
            LoftStation {
                y: 1.30,
                half_width: 0.84,
                half_len_front: 0.90,
                half_len_rear: 0.90,
                z_center: 0.00,
            },
            LoftStation {
                y: 1.40,
                half_width: 0.91,
                half_len_front: 0.98,
                half_len_rear: 0.94,
                z_center: -0.02,
            },
            LoftStation {
                y: 1.50,
                half_width: 0.90,
                half_len_front: 0.98,
                half_len_rear: 0.88,
                z_center: -0.03,
            },
            LoftStation {
                y: 1.60,
                half_width: 0.84,
                half_len_front: 0.92,
                half_len_rear: 0.80,
                z_center: -0.05,
            },
            LoftStation {
                y: 1.70,
                half_width: 0.74,
                half_len_front: 0.84,
                half_len_rear: 0.66,
                z_center: -0.08,
            },
            LoftStation {
                y: 1.80,
                half_width: 0.60,
                half_len_front: 0.72,
                half_len_rear: 0.52,
                z_center: -0.11,
            },
            LoftStation {
                y: 1.86,
                half_width: 0.48,
                half_len_front: 0.60,
                half_len_rear: 0.42,
                z_center: -0.12,
            },
        ],
        exponent: 2.8,
        segments: 64,
        cheek_amount: 0.10,
        cheek_azimuth: 0.70,
        cheek_y: 1.48,
        cheek_az_width: 0.42,
        cheek_y_width: 0.18,
        embrasure_amount: -0.10,
        embrasure_y: 1.54,
        embrasure_az_width: 0.50,
        embrasure_y_width: 0.18,
        cupola_center: Vec3::new(-0.34, 1.86, -0.10),
        cupola_radius: 0.24,
        cupola_half_height: 0.11,
    }
}
