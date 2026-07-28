//! The T-54 lofted cast-turret stations and shaping — split out of `t54_hybrid` to keep each file
//! within the reviewability budget. Stations run from the ring seat (1.58, the LOW hull-roof
//! plane) up to the flat roof (2.27): the tall ~0.7 m hemispherical casting of the references,
//! widest LOW (station 2) for the ring overhang at the ~2.25 m casting diameter, front-heavy
//! (front > rear half-length) with a rear-pulled bustle (negative z_center climbing with height),
//! rounding continuously into the roof. All within the ±1.125 / ±1.17 turret plan. Cheeks and the
//! front gun embrasure ride as localized radial modulations of the one surface.

use glam::Vec3;

use super::{LoftStation, TurretLoftVisual};

pub(super) fn turret_loft() -> TurretLoftVisual {
    TurretLoftVisual {
        stations: [
            LoftStation {
                y: 1.58,
                half_width: 0.98,
                half_len_front: 1.05,
                half_len_rear: 1.05,
                z_center: 0.00,
            },
            LoftStation {
                y: 1.70,
                half_width: 1.08,
                half_len_front: 1.16,
                half_len_rear: 1.10,
                z_center: -0.02,
            },
            LoftStation {
                y: 1.82,
                half_width: 1.06,
                half_len_front: 1.14,
                half_len_rear: 1.03,
                z_center: -0.04,
            },
            LoftStation {
                y: 1.94,
                half_width: 0.98,
                half_len_front: 1.06,
                half_len_rear: 0.92,
                z_center: -0.06,
            },
            LoftStation {
                y: 2.06,
                half_width: 0.85,
                half_len_front: 0.92,
                half_len_rear: 0.76,
                z_center: -0.09,
            },
            LoftStation {
                y: 2.18,
                half_width: 0.65,
                half_len_front: 0.72,
                half_len_rear: 0.55,
                z_center: -0.12,
            },
            LoftStation {
                y: 2.27,
                half_width: 0.42,
                half_len_front: 0.50,
                half_len_rear: 0.36,
                z_center: -0.14,
            },
        ],
        exponent: 2.8,
        segments: 64,
        // Fuller, wider front cheeks pulled in toward the mantlet (smaller azimuth): the signature
        // T-54 cast front mass must bulge PROUD of the turret sides, not vanish into the superellipse.
        cheek_amount: 0.20,
        cheek_azimuth: 0.95,
        cheek_y: 1.78,
        cheek_az_width: 0.50,
        cheek_y_width: 0.24,
        embrasure_amount: -0.12,
        // ON the gun axis (`gun.trunnion_y`). It used to sit 20 mm above it — a drift nothing
        // measured, in the one feature whose whole job is to be centred on the barrel.
        embrasure_y: 1.78,
        embrasure_az_width: 0.48,
        embrasure_y_width: 0.22,
        // Rooted deep into the curved dome (base ~2.02, under the local shell surface) so the
        // drum grows out of the casting instead of levitating over the sloping roof.
        cupola_center: Vec3::new(-0.34, 2.20, -0.10),
        cupola_radius: 0.24,
        cupola_half_height: 0.18,
    }
}
