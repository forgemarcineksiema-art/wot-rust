//! The T-54 lofted cast-turret stations and shaping — split out of `t54_hybrid` to keep each file
//! within the reviewability budget. Stations run from the skirt lip (1.54, BELOW the hull-roof
//! plane — the casting overhangs its race) up to the flat roof (2.40): the low ~0.86 m
//! hemispherical casting of the references, widest LOW at the ~2.25 m casting diameter, its
//! waist (widest cut) registered 0.20 m FORWARD of the ring axis (S1's "43% from the front") so
//! the plan reads as the T-54's egg — blunt wide front, long tapering rear — with the crown
//! biased toward the cheeks. Cheeks and the front gun embrasure ride as localized radial
//! modulations of the one surface.

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
            // PHOTO-DERIVED (pilot module 1, 2026-07-29), RE-REGISTERED 2026-08-12. Module 1's
            // heights, widths and length sums stand — what moves is WHERE the egg sits over the
            // ring, and it moves back to S1's own measurement. S1 measured the widest cut at
            // 43% of the casting's length from the front and flagged only its registration to
            // the ring as an assumption; module 1 kept the kernel-contract's "front-heavy from
            // the ring axis" registration instead, and in trimming the beak the asymmetry
            // shrank to 8% — a symmetric OVAL, which the plan overlay against the calibrated
            // three-view shows plainly (the drawing's egg: waist ~0.2-0.4 m FORWARD of the
            // hull centre, rear taper the long one; the museum and WG references read the
            // same). Re-registration, with S1's L = 2.363 and the overlay-validated rear tip:
            //
            //   * rear tip -1.128 (validated: the drawing overlay matches it to ~3 cm);
            //   * front tip 1.233 -> authored 1.28 at the waist band (the sheet reads the
            //     collar zone out to ~1.4-1.5; hedged toward S1's casting-only tip);
            //   * waist (widest cut) at z = +0.20 = front tip - 0.43 x 2.363, S1's number;
            //   * the crown holds toward the cheeks (z_center +0.06 at the roof) and the rear
            //     falls in one long sweep - the photo's read, now expressed in registration
            //     instead of being flattened out of it;
            //   * a SKIRT station below the ring plane: the casting overhangs its race and the
            //     lip drops ~4 cm below the 1.58 roofline with a slight tuck - the shadow line
            //     every reference photograph shows around the base of the dome.
            //
            // The blunt nose stands: the front tips fall steeply above the window band.
            // Front-heaviness now lives where the metal has it - the casting still reaches
            // farther forward of the RING AXIS than rearward (1.28 vs 1.128), but each
            // station's own rear half-length exceeds its front one because the waist sits
            // forward; `t54_kernel_contract` measures the registration, not the split.
            LoftStation {
                y: 1.54,
                half_width: 1.074,
                half_len_front: 1.065,
                half_len_rear: 1.275,
                z_center: 0.18,
            },
            LoftStation {
                y: 1.58,
                half_width: 1.090,
                half_len_front: 1.075,
                half_len_rear: 1.305,
                z_center: 0.19,
            },
            LoftStation {
                y: 1.66,
                half_width: 1.125,
                half_len_front: 1.080,
                half_len_rear: 1.328,
                z_center: 0.20,
            },
            LoftStation {
                y: 1.76,
                half_width: 1.125,
                half_len_front: 1.075,
                half_len_rear: 1.320,
                z_center: 0.20,
            },
            LoftStation {
                y: 1.88,
                half_width: 1.105,
                half_len_front: 1.055,
                half_len_rear: 1.265,
                z_center: 0.19,
            },
            LoftStation {
                y: 2.00,
                half_width: 1.060,
                half_len_front: 1.015,
                half_len_rear: 1.170,
                z_center: 0.17,
            },
            LoftStation {
                y: 2.12,
                half_width: 0.980,
                half_len_front: 0.915,
                half_len_rear: 0.995,
                z_center: 0.15,
            },
            LoftStation {
                y: 2.22,
                half_width: 0.865,
                half_len_front: 0.770,
                half_len_rear: 0.815,
                z_center: 0.13,
            },
            LoftStation {
                y: 2.30,
                half_width: 0.700,
                half_len_front: 0.625,
                half_len_rear: 0.625,
                z_center: 0.11,
            },
            LoftStation {
                y: 2.36,
                half_width: 0.495,
                half_len_front: 0.410,
                half_len_rear: 0.440,
                z_center: 0.09,
            },
            // The roof plate: a small near-flat crown the photo shows, big enough to root the
            // hatch coamings - the cupola and furniture root INTO the dome, not onto this
            // plate. Forward-biased with the crown, not drifted rearward.
            LoftStation {
                y: 2.40,
                half_width: 0.420,
                half_len_front: 0.385,
                half_len_rear: 0.390,
                z_center: 0.06,
            },
        ],
        // 2.35: the plan reads as an EGG. The S1 fit said 2.8, and 2.8 is a squircle - the
        // top-down silhouette (the game's default camera) read as a rounded square, not a
        // T-54. Judged against REF 45/top photos; the fit's own sheet was the untrusted input.
        exponent: 2.35,
        // The CASTING's resolution. The gun aperture is a feature on it, and `cast_loft` refines
        // its own grid over a bump too narrow for this one — so the hole is sharp without the
        // whole dome paying for it.
        // 56, deliberately UNDER 60: the base step (0.112 rad) must stay above the window's
        // wanted step (0.32/3 = 0.107) so cast_loft refines the window band — at 60 the base
        // step squeaked under and refinement shut off, leaving the wall between bare columns
        // (x 0.415 -> 0.524, a 0.11 m hole the instruments fell straight through).
        segments: 54,
        // The FACE PLATE, not a cheek pair — third placement, and the one the casting *has*.
        // Under exponent 2.8 the squircle nose carried this mass and a bump double-counted it
        // (the flank fix zeroed it). Under the photo-derived egg (2.35) the bare base curls
        // away from the window's flanks — and REF 22/23 show the opposite: the forward-most
        // structure is the broad face AROUND the window, walls topping out at the front. One
        // central plateau raises floor, shelf and walls together, so the rebate keeps its
        // depth; outboard gaussians (0.75±0.36) sank the wall ring below the bare nose, and a
        // centred pair (0.55±0.55) filled the rebate to a millimetre. `cheek_azimuth` is
        // retired by the plateau (kept for the wire layout).
        cheek_amount: 0.050,
        cheek_azimuth: 0.0,
        cheek_y: 1.78,
        // Width 0.60: the plateau must stay FLAT past the window's walls (az 0.32 + the wall
        // band to ~0.45) — at 0.40 it faded at the wall and the ring sank below the shelf
        // again. Above the loft's refinement threshold either way (0.105 base step / 3).
        cheek_az_width: 0.60,
        cheek_y_width: 0.34,
        // The documented aperture is ~0.40 m across — the INNER armour hole, cut through the
        // window's floor. For this super-Gaussian the half-depth point sits at 0.941 w, so the
        // vertical opening wants w = 0.17 (the gun needs less travel up and down than side to
        // side, and a taller pocket would cut into the ring seat 0.20 m below the axis).
        //
        // Azimuth is NOT arc length here. The superellipse is flat across its nose: at exponent
        // 2.35 the x it reaches is w * (sin daz)^0.851, so 0.20 m of x is 0.132 rad of
        // azimuth. Converting by arc length — which is the obvious thing to do and
        // is wrong — makes the aperture half again too wide.
        // It was 0.48 rad x 0.22 m — a metre-wide soft dish, which is why a ball mantlet had to
        // be invented to fill it.
        //
        // The depth is the wall it is cut through, not a dent pressed into it.
        // Deepened 25 mm with the egg reshape: the depth instrument reads against the
        // cheeks' global reach, and the curved nose sits lower than the old plateau did.
        embrasure_amount: -0.135,
        // ON the gun axis (`gun.trunnion_y`). It used to sit 20 mm above it — a drift nothing
        // measured, in the one feature whose whole job is to be centred on the barrel.
        embrasure_y: 1.78,
        embrasure_az_width: 0.155,
        embrasure_y_width: 0.17,
        // A pocket with a floor and a rim. Six is where the wall stops reading as a slope.
        embrasure_falloff: 6.0,
        // The outer WINDOW, MEASURED (pilot module 2): ~0.50 m wide, ~0.42 m tall. Two museum
        // tanks agree — the dead-front reference (turret width 2.25 in frame gives 2.217 mm/px;
        // window 230 px = 0.51 m; barrel 95 px = 0.211 m as the scale cross-check) and the park
        // tank's bare window (2.0-2.3 barrel diameters). The first build's ~0.85 m 2:1 letterbox
        // was the author's eyeball, not a source. The real read is a compact pillow: the 0.40 m
        // aperture plus ~50 mm of wall each side. x = 1.125 * sin(daz)^0.851 -> 0.25 m = 0.172.
        // 85 mm: on the egg the base itself falls ~35 mm across the window span, eating the
        // rebate's visible step — the cut must out-run the curl for the wall to read.
        window_amount: -0.085,
        window_az_width: 0.172,
        window_y_width: 0.21,
        window_falloff: 8.0,
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
