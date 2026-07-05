//! Assemble the battle minimap model from the live presentation state: the battlefield relief and
//! cover, the player's pose, the camera's heading, allied positions, and the enemies the player's
//! team has spotted. Kept out of `render.rs` so the per-frame render path stays lean.

use engine::PresentationTank;
use terrain::HeightMap;

use crate::app::ClientApp;
use crate::hud::minimap::{MinimapBox, MinimapModel, RELIEF_RES};

/// Half-angle of the minimap view wedge (radians) — a readable spread, not the true camera FOV.
const VIEW_HALF_FOV_RAD: f32 = 0.55;

/// Sample the heightmap into a `RELIEF_RES`-square grid, normalised to its own min/max so the
/// relief reads on any map regardless of absolute elevation.
fn sample_relief(heightmap: &HeightMap, extent: [f32; 2]) -> Vec<f32> {
    let n = RELIEF_RES;
    let mut heights = vec![0.0f32; n * n];
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for iz in 0..n {
        for ix in 0..n {
            let x = (ix as f32 + 0.5) / n as f32 * extent[0];
            let z = (iz as f32 + 0.5) / n as f32 * extent[1];
            let y = heightmap.sample_height(x, z).unwrap_or(0.0);
            heights[iz * n + ix] = y;
            lo = lo.min(y);
            hi = hi.max(y);
        }
    }
    let range = (hi - lo).max(0.001);
    for value in &mut heights {
        *value = (*value - lo) / range;
    }
    heights
}

impl ClientApp {
    /// Build the minimap for this frame. Enemy blips are gated to those the player's team has
    /// spotted (LOS v1), the same visibility bit that gates floating HP bars. Returns `None` before
    /// the player tank is present in the presentation set.
    pub(super) fn build_minimap(
        &self,
        tanks: &[PresentationTank],
        camera_forward_xz: [f32; 2],
    ) -> Option<MinimapModel> {
        let player = tanks.iter().find(|tank| tank.id == self.player_tank)?;
        let player_team = self.player_team();
        let player_bit = player_team.spotting_bit();

        let xz = |tank: &PresentationTank| [tank.translation[0], tank.translation[2]];
        let allies = tanks
            .iter()
            .filter(|t| t.id != self.player_tank && t.team == player_team && t.hit_points > 0)
            .map(xz)
            .collect();
        let enemies = tanks
            .iter()
            .filter(|t| {
                t.team != player_team
                    && t.hit_points > 0
                    && t.spotted_by_teams_mask & player_bit != 0
            })
            .map(xz)
            .collect();

        let heightmap = &self.battlefield.heightmap;
        let extent = heightmap.extent_m();
        let extent_m = [extent[0].max(1.0), extent[1].max(1.0)];
        let cover = self
            .battlefield
            .static_cover
            .iter()
            .map(|c| MinimapBox {
                center_xz: [c.center[0], c.center[2]],
                half_xz: [c.half_extents_m[0], c.half_extents_m[2]],
            })
            .collect();

        // Map yaw convention: heading 0 points up (+world z), so yaw = atan2(x, z).
        Some(MinimapModel {
            extent_m,
            relief: sample_relief(heightmap, extent_m),
            cover,
            player_xz: [player.translation[0], player.translation[2]],
            player_heading_rad: player.hull_yaw_rad,
            view_yaw_rad: camera_forward_xz[0].atan2(camera_forward_xz[1]),
            view_half_fov_rad: VIEW_HALF_FOV_RAD,
            allies,
            enemies,
        })
    }
}
