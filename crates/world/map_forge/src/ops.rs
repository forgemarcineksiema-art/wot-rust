//! Terrain op evaluation — the structural vocabulary of the heightfield program. The
//! arithmetic mirrors the historical generators exactly (sums folded in document order and
//! applied once, masks multiplied in order, `lerp`-to-target carves), so a blueprint
//! reproduces a legacy map bit for bit; that is what lets the baked assets, the replay
//! fixtures and the two ends of the wire keep agreeing after the migration to data.

use terrain::{RiverSpec, band_mask, gaussian1, gaussian2, lerp, polyline_distance, smoothstep01};

use crate::blueprint::{
    BaseSpec, DampMask, MapAxis, RectBand, ReliefTerm, StrokeProfile, TerrainOp,
};

/// Everything an op needs beyond the sample point: the river line (for river-relative ops)
/// and the map's symmetry axis (every `dz` is measured from it).
pub(crate) struct EvalContext<'a> {
    pub river: Option<&'a RiverSpec>,
    pub axis_z: f32,
}

impl EvalContext<'_> {
    /// Lateral distance to the river centerline — `None` on a riverless map, which makes
    /// every river-relative mask vanish (the op is a no-op). The REPORT carries the Error
    /// (`check_river_dependencies`); evaluation itself never panics on author input.
    fn river_d(&self, x: f32, z: f32) -> Option<f32> {
        self.river.map(|river| x - river.center_x(z))
    }

    /// The river band mask, or 0 (no effect) on a riverless map.
    fn river_band(&self, x: f32, z: f32, half_width: f32, falloff: f32) -> f32 {
        self.river_d(x, z).map_or(0.0, |d| band_mask(d, half_width, falloff))
    }
}

impl BaseSpec {
    pub(crate) fn eval(&self, x: f32) -> f32 {
        match self {
            BaseSpec::Constant(h) => *h,
            BaseSpec::SlopeEases { base, eases } => {
                let mut h = *base;
                for [start, span, delta] in eases {
                    h += delta * smoothstep01((x - start) / span);
                }
                h
            }
        }
    }
}

impl TerrainOp {
    pub(crate) fn apply(&self, ctx: &EvalContext, x: f32, z: f32, h: f32) -> f32 {
        let dz = z - ctx.axis_z;
        match self {
            TerrainOp::Relief { terms, dampen } => {
                let mut relief = 0.0_f32;
                for term in terms {
                    relief += match *term {
                        ReliefTerm::SinX { amp, wave } => amp * (x / wave).sin(),
                        ReliefTerm::CosDz { amp, wave } => amp * (dz / wave).cos(),
                        ReliefTerm::SinXCosDz { amp, x_freq, dz_freq } => {
                            amp * (x * x_freq).sin() * (dz * dz_freq).cos()
                        }
                    };
                }
                let mut damping = 1.0_f32;
                for mask in dampen {
                    damping *= match *mask {
                        DampMask::LinearRampX { inner_m, outer_m, strength } => {
                            1.0 - strength * ((outer_m - x) / (outer_m - inner_m)).clamp(0.0, 1.0)
                        }
                        DampMask::TownRect { rect, strength } => {
                            1.0 - strength * rect_mask(rect, x, dz)
                        }
                        DampMask::RiverBand { half_width_m, falloff_m, strength } => {
                            1.0 - strength * ctx.river_band(x, z, half_width_m, falloff_m)
                        }
                    };
                }
                h + relief * damping
            }
            TerrainOp::Gauss2 { apply, terms } => {
                let mut sum = 0.0_f32;
                for t in terms {
                    sum += gaussian2(x, z, t.x, t.z, t.sx, t.sz, t.amp);
                }
                match apply {
                    crate::blueprint::Apply::Add => h + sum,
                    crate::blueprint::Apply::Subtract => h - sum,
                }
            }
            TerrainOp::Gauss1 { axis, apply, terms } => {
                let value = match axis {
                    MapAxis::X => x,
                    MapAxis::Z => z,
                };
                let mut sum = 0.0_f32;
                for t in terms {
                    sum += gaussian1(value, t.center, t.sigma, t.amp);
                }
                match apply {
                    crate::blueprint::Apply::Add => h + sum,
                    crate::blueprint::Apply::Subtract => h - sum,
                }
            }
            TerrainOp::RidgeGated { center, sigma, amp, gates } => {
                let ridge = gaussian1(z, *center, *sigma, *amp);
                let mut openness = 0.0_f32;
                for gate in gates {
                    openness += gaussian1(x, gate.center, gate.sigma, 1.0);
                }
                h + ridge * (1.0 - openness.min(1.0))
            }
            TerrainOp::CrestShelf { crest_x, shelf_x, sigma_x, crest_h, shelf_cut, windows } => {
                let mut sum = 0.0_f32;
                for [window_z, window_sigma] in windows {
                    let z_local = gaussian1(z, *window_z, *window_sigma, 1.0);
                    let crest = gaussian1(x, *crest_x, *sigma_x, *crest_h);
                    let shelf = gaussian1(x, *shelf_x, *sigma_x, *shelf_cut);
                    sum += (crest - shelf) * z_local;
                }
                h + sum
            }
            TerrainOp::FlattenToRamp { mask, ramp_base_m, ramp_from_x_m, ramp_slope, strength } => {
                let ramp = ramp_base_m + (x - ramp_from_x_m).max(0.0) * ramp_slope;
                lerp(h, ramp, strength * rect_mask(*mask, x, dz))
            }
            TerrainOp::FlattenToGauss { target_m, x: gx, z: gz, sx, sz } => {
                lerp(h, *target_m, gaussian2(x, z, *gx, *gz, *sx, *sz, 1.0))
            }
            TerrainOp::RiverBandAdd { half_width_m, falloff_m, delta_m } => {
                h + delta_m * ctx.river_band(x, z, *half_width_m, *falloff_m)
            }
            TerrainOp::CarveChannel {
                half_width_m,
                falloff_m,
                water_level_m,
                channel_depth_m,
                sill_depth_m,
                sills,
            } => {
                let mut ford = 0.0_f32;
                for [sill_z, sill_sigma] in sills {
                    ford += gaussian1(z, *sill_z, *sill_sigma, 1.0);
                }
                let ford = ford.min(1.0);
                let depth = channel_depth_m - (channel_depth_m - sill_depth_m) * ford;
                let bed = water_level_m - depth;
                lerp(h, bed, ctx.river_band(x, z, *half_width_m, *falloff_m))
            }
            TerrainOp::Deck {
                dz_m,
                deck_m,
                half_width_m,
                half_length_m,
                width_falloff_m,
                length_falloff_m,
            } => {
                let mask = band_mask(dz - dz_m, *half_width_m, *width_falloff_m)
                    * ctx.river_band(x, z, *half_length_m, *length_falloff_m);
                // Only ever raise: the deck bridges the channel but never trenches the bank.
                lerp(h, deck_m.max(h), mask)
            }
            TerrainOp::ClampMin(min) => h.max(*min),
            TerrainOp::Stroke(spec) => {
                let mask = band_mask(
                    polyline_distance(&spec.points, x, z),
                    spec.half_width_m,
                    spec.falloff_m,
                );
                match spec.profile {
                    StrokeProfile::Ridge { amp_m } => h + amp_m * mask,
                    StrokeProfile::Valley { depth_m } => h - depth_m * mask,
                    StrokeProfile::Plateau { target_m } => lerp(h, target_m, mask),
                }
            }
            // Every evaluation path walks `effective_terrain_ops`, which resolves this into
            // a Stroke riding the named road. Reaching here means a caller evaluated the RAW
            // document list — a silent identity would hide that bug; fail loudly instead.
            TerrainOp::RoadProfile(spec) => {
                unreachable!(
                    "RoadProfile '{}' must be resolved via effective_terrain_ops before \
                     evaluation",
                    spec.road_id
                )
            }
        }
    }

    /// The rectangle outside which this op provably does nothing — `None` means "never
    /// cull". Only strokes claim one: `band_mask`'s support ends EXACTLY at
    /// `half_width + falloff` (its own locked contract), so skipping samples beyond the
    /// expanded point bbox is bitwise identical to full evaluation.
    pub(crate) fn influence_bounds(&self) -> Option<[f32; 4]> {
        let TerrainOp::Stroke(spec) = self else { return None };
        let reach = spec.half_width_m + spec.falloff_m;
        let mut min_x = f32::MAX;
        let mut min_z = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_z = f32::MIN;
        for [x, z] in &spec.points {
            min_x = min_x.min(*x);
            min_z = min_z.min(*z);
            max_x = max_x.max(*x);
            max_z = max_z.max(*z);
        }
        Some([min_x - reach, min_z - reach, max_x + reach, max_z + reach])
    }
}

fn rect_mask(rect: RectBand, x: f32, dz: f32) -> f32 {
    band_mask(x - rect.x_center_m, rect.x_half_m, rect.x_falloff_m)
        * band_mask(dz, rect.dz_half_m, rect.dz_falloff_m)
}
