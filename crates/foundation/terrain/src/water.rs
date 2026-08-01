use serde::{Deserialize, Serialize};

/// A map's standing water: one flat table at `surface_level_m`. Water exists exactly where the
/// terrain dips below the level, so **depth = surface_level − heightmap height** and the
/// heightmap stays the single spatial source of truth — physics (wading drag), drowning, shell
/// splashes, the water mesh, and the minimap all derive from these two numbers. Mirror-symmetry
/// fairness of the water is inherited from the heightmap for free.
///
/// A struct (not a bare f32) so future surface behaviour (flow direction, murkiness) can append
/// without a schema break.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaterBody {
    /// World-space elevation of the still water surface.
    pub surface_level_m: f32,
}

impl WaterBody {
    /// Depth of water standing over terrain at height `ground_m`; zero on dry land.
    pub fn depth_over(&self, ground_m: f32) -> f32 {
        (self.surface_level_m - ground_m).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_is_level_minus_ground_and_never_negative() {
        let water = WaterBody { surface_level_m: 5.0 };
        assert_eq!(water.depth_over(3.0), 2.0);
        assert_eq!(water.depth_over(5.0), 0.0);
        assert_eq!(water.depth_over(9.0), 0.0, "dry land has zero depth, not negative");
    }
}
