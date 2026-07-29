//! The T-54 lofted cast-turret stations and shaping — split out of `t54_hybrid` to keep each file
//! within the reviewability budget. Stations run from the ring seat (1.58, the LOW hull-roof
//! plane) up to the flat roof (2.27): the tall ~0.7 m hemispherical casting of the references,
//! widest LOW (station 2) for the ring overhang at the ~2.25 m casting diameter, front-heavy
//! (front > rear half-length) with a rear-pulled bustle (negative z_center climbing with height),
//! rounding continuously into the roof. All within the ±1.125 / ±1.17 turret plan. Cheeks and the
//! front gun embrasure ride as localized radial modulations of the one surface.

use glam::Vec3;

use super::{LoftStation, TurretLoftVisual};

/// The lofted casting. `cupola` is the drum the roof carries: centre, external radius and
/// half-height come from the ONE place they are authored (`t54_hybrid`), because a cupola
/// written down three times is three cupolas that happen to agree.
pub(super) fn turret_loft(cupola: (Vec3, f32, f32)) -> TurretLoftVisual {
    TurretLoftVisual {
        // MEASURED, not guessed. Blender session S1 (2026-07-28) built a master dome over
        // multi-view line drawings, sliced it into horizontal sections and fitted a superellipse
        // per station. Two findings carry the shape:
        //
        //   * the roof is 2.40 m, not 2.27 — book sources and the extracted silhouette agree,
        //     and 2.218 is ruled out;
        //   * the casting is 2.363 m long, and it is NOT symmetric: its widest cut sits 43%
        //     of that length from the front. The model had 1.05 both ways — 2.10 m of turret.
        //
        // Where that asymmetry goes is the one place S1 is NOT followed. Turning "43% from the
        // front" into "1.016 forward of the RING, 1.347 aft" needs the widest cut to BE the ring
        // plane, and S1 flags that registration as an assumption. On a cast dome the widest cut
        // is at the CHEEKS, which sit forward of the ring — so measured from the ring the same
        // casting comes out front-heavy, which is what the kernel contract has said all along.
        // The measured LENGTH and the measured rearward drift (`z_center`) are kept; only the
        // assumed registration is dropped.
        //
        // The SHAPE is S1's; the absolute heights stay on the dossier, because the drawing sheet
        // is internally inconsistent by 4-7% (its own projections disagree) and the crown widths
        // came from closure rather than measurement, with roof furniture in the way.
        stations: [
            LoftStation {
                y: 1.58,
                // The S1 master's flank band is VERTICAL: 1.125 from the ring seat to 2.00,
                // and the Blender section-diff proved the model was not honouring it — the
                // casting waisted to 2.138/2.126 at 1.58/2.00 because the width was split
                // between a 1.038 base and separate cheek bumps. That split was a
                // DOUBLE-COUNT: S1's exponent-2.8 superellipse fit already contains the front
                // fullness (it fitted the whole outline), so bolting extra cheek lobes onto a
                // narrowed base reproduced the width only at one height and hollowed it
                // everywhere else. The stations carry the full silhouette now and the bumps
                // carry nothing.
                half_width: 1.125,
                half_len_front: 1.240,
                half_len_rear: 1.123,
                z_center: 0.00,
            },
            LoftStation {
                y: 1.68,
                // A SUBDIVISION of the casting's straight run from the ring seat to 2.00, not a
                // new measurement: the two stations it sits between carry these same numbers.
                // It exists so the gun aperture has rings to be cut into. A 0.40 m feature
                // resolved by three rings is a diamond; resolved by five it is a hole.
                half_width: 1.125,
                half_len_front: 1.240,
                half_len_rear: 1.123,
                z_center: 0.00,
            },
            // The cheek station. The swell is a per-RING modulation, so it needs a ring at its
            // own height to appear on at all — with the nearest sections at 1.58 and 2.00 the
            // T-54's signature front mass had nowhere to form.
            LoftStation {
                y: 1.78,
                // S1 measured the SILHOUETTE, and the silhouette includes the cheek swell —
                // so 1.125 is the casting's widest point, not its base section. The base runs
                // narrower by the swell it carries, and the two together make the documented
                // 2.25 m width over the turret.
                half_width: 1.125,
                half_len_front: 1.240,
                half_len_rear: 1.123,
                z_center: 0.00,
            },
            LoftStation {
                y: 1.88,
                // A SUBDIVISION of the casting's straight run from the ring seat to 2.00, not a
                // new measurement: the two stations it sits between carry these same numbers.
                // It exists so the gun aperture has rings to be cut into. A 0.40 m feature
                // resolved by three rings is a diamond; resolved by five it is a hole.
                half_width: 1.125,
                half_len_front: 1.240,
                half_len_rear: 1.123,
                z_center: 0.00,
            },
            // Vertical to 2.00: the casting's skirt does not begin narrowing until above the
            // ring overhang.
            LoftStation {
                y: 2.00,
                // S1 measured the SILHOUETTE, and the silhouette includes the cheek swell —
                // so 1.125 is the casting's widest point, not its base section. The base runs
                // narrower by the swell it carries, and the two together make the documented
                // 2.25 m width over the turret.
                half_width: 1.125,
                half_len_front: 1.240,
                half_len_rear: 1.123,
                z_center: 0.00,
            },
            LoftStation {
                y: 2.12,
                half_width: 1.062,
                half_len_front: 1.219,
                half_len_rear: 1.104,
                z_center: -0.011,
            },
            LoftStation {
                y: 2.22,
                half_width: 0.898,
                half_len_front: 1.104,
                half_len_rear: 1.001,
                z_center: -0.116,
            },
            // Above 2.22 the S1 sheet is CLOSING rather than measuring — its own note says the
            // crown came from the closure and had roof furniture in the way. So the neck runs to
            // an authored flat roof plate instead of the sheet's near-point apex: a T-54 has a
            // roof you can stand hatches on, and `turret.roof_radius` is that plate.
            LoftStation {
                y: 2.30,
                half_width: 0.720,
                half_len_front: 0.891,
                half_len_rear: 0.789,
                z_center: -0.140,
            },
            LoftStation {
                y: 2.40,
                half_width: 0.420,
                half_len_front: 0.537,
                half_len_rear: 0.423,
                z_center: -0.180,
            },
        ],
        exponent: 2.8,
        // The CASTING's resolution. The gun aperture is a feature on it, and `cast_loft` refines
        // its own grid over a bump too narrow for this one — so the hole is sharp without the
        // whole dome paying for it.
        segments: 64,
        // ZERO, deliberately, since the flank fix. The cheek swell was a separate Gaussian lobe
        // bolted onto a narrowed base — but S1's superellipse fit measured the WHOLE outline, so
        // the exponent-2.8 fullness of the stations already IS the cast front mass, and a bump on
        // top double-counted it: the documented 2.25 appeared only at the bump's own height and
        // the casting waisted everywhere else. The dossier's form rule says it plainly: a full
        // hemispherical dome — one continuous surface, not a dome wearing lobes.
        cheek_amount: 0.0,
        cheek_azimuth: 0.95,
        cheek_y: 1.78,
        cheek_az_width: 0.50,
        cheek_y_width: 0.24,
        // The documented aperture is ~0.40 m across. For this super-Gaussian the half-depth
        // point sits at 0.941 w, so the vertical opening wants w = 0.17 (the gun needs less
        // travel up and down than side to side, and a taller pocket would cut into the ring
        // seat 0.20 m below the axis).
        //
        // Azimuth is NOT arc length here. The superellipse is flat across its nose: at exponent
        // 2.8 the x it reaches is 1.038 * (sin daz)^0.714, so 0.20 m of x is 0.0997 rad of
        // azimuth, not 0.161. Converting by arc length — which is the obvious thing to do and
        // is wrong — makes the aperture half again too wide.
        // It was 0.48 rad x 0.22 m — a metre-wide soft dish, which is why a ball mantlet had to
        // be invented to fill it.
        //
        // The depth is the wall it is cut through, not a dent pressed into it.
        embrasure_amount: -0.16,
        // ON the gun axis (`gun.trunnion_y`). It used to sit 20 mm above it — a drift nothing
        // measured, in the one feature whose whole job is to be centred on the barrel.
        embrasure_y: 1.78,
        embrasure_az_width: 0.106,
        embrasure_y_width: 0.17,
        // A pocket with a floor and a rim. Six is where the wall stops reading as a slope.
        embrasure_falloff: 6.0,
        // Rooted deep into the curved dome (base ~2.02, under the local shell surface) so the
        // drum grows out of the casting instead of levitating over the sloping roof.
        // Seated so exactly 131 mm of drum stands above the 2.40 m roof — the documented
        // exposure. It was an absolute 2.20, which the raised roof simply swallowed: the
        // commander's cupola disappeared INTO the casting.
        cupola_center: cupola.0,
        cupola_radius: cupola.1,
        cupola_half_height: cupola.2,
    }
}
