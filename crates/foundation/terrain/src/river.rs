//! A map's river as DATA: the parametric centerline every consumer follows — the terrain
//! carve, the water mesh, the minimap, the backdrop skirt, bot water probes, and the map
//! compiler's own river-relative ops (`map_forge`). One spec, one evaluation, no forked
//! copies of the meander math.
//!
//! The centerline is an even function of `z − axis_z` (a Gaussian bow plus a cosine wiggle),
//! which is what makes a river course mirror-fair about the map's central axis.

use serde::{Deserialize, Serialize};

/// Parametric river centerline: `center_x(z) = base_x − bow(z) + wiggle(z)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RiverSpec {
    /// Nominal centerline x east of mid-map.
    pub base_x_m: f32,
    /// The map's symmetry axis on z (the bow/wiggle are even about it).
    pub axis_z_m: f32,
    /// Gaussian bow: sigma and amplitude of the westward deflection through the center.
    pub bow_sigma_m: f32,
    pub bow_amp_m: f32,
    /// Cosine wiggle: amplitude and wavelength of the small meander.
    pub wiggle_amp_m: f32,
    pub wiggle_wave_m: f32,
    /// Corridor within which water may exist at all (channel + banks + margin) — the
    /// "no accidental puddles" contract radius.
    pub corridor_half_width_m: f32,
}

impl RiverSpec {
    /// The Bystra: nominal line at x=585 bowing 45 m westward through the center of the
    /// 1000 m valley, with a 14 m / 70 m wiggle; the water corridor spans 31 m to each side.
    pub const BYSTRA: RiverSpec = RiverSpec {
        base_x_m: 585.0,
        axis_z_m: 500.0,
        bow_sigma_m: 200.0,
        bow_amp_m: 45.0,
        wiggle_amp_m: 14.0,
        wiggle_wave_m: 70.0,
        corridor_half_width_m: RIVER_CORRIDOR_HALF_WIDTH_M,
    };

    /// The centerline's x at a given z. Even about `z = axis_z_m`, which is what makes the
    /// whole water course mirror-fair.
    pub fn center_x(&self, z: f32) -> f32 {
        let dz = z - self.axis_z_m;
        self.base_x_m - crate::math::gaussian1(z, self.axis_z_m, self.bow_sigma_m, self.bow_amp_m)
            + self.wiggle_amp_m * (dz / self.wiggle_wave_m).cos()
    }
}

/// Corridor within which water may exist at all on the Bystra (channel + banks + margin) —
/// the "no accidental puddles" contract in `tests/bystra_water.rs` checks everything outside
/// it. Public: the water mesh, minimap, and bot water probes all reason about the same
/// corridor. Kept as a plain constant for the callers that predate the spec.
pub const RIVER_CORRIDOR_HALF_WIDTH_M: f32 = 13.0 + 12.0 + 6.0;

/// The Bystra's centerline x at a given z — the compatibility shim for callers that only
/// know the river, not the map (bot water probes, contract tests). New code should read
/// [`BattlefieldMap::river`](crate::BattlefieldMap::river) instead.
pub fn bystra_river_center_x(z: f32) -> f32 {
    RiverSpec::BYSTRA.center_x(z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_centerline_is_even_about_the_axis() {
        let river = RiverSpec::BYSTRA;
        for dz in [0.0_f32, 13.5, 77.0, 180.0, 320.0, 700.0] {
            assert_eq!(
                river.center_x(500.0 + dz),
                river.center_x(500.0 - dz),
                "mirror-fair broke at dz {dz}"
            );
        }
    }

    #[test]
    fn the_shim_matches_the_spec() {
        for z in [-400.0_f32, 0.0, 250.0, 500.0, 820.0, 1400.0] {
            assert_eq!(bystra_river_center_x(z), RiverSpec::BYSTRA.center_x(z));
        }
    }
}
