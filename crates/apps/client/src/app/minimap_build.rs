//! Assemble the battle minimap model from the live presentation state: the battlefield relief and
//! cover, the player's pose, the camera's heading, allied positions, and the enemies the player's
//! team has spotted. Kept out of `render.rs` so the per-frame render path stays lean.

use engine::PresentationTank;
use terrain::HeightMap;

use crate::app::ClientApp;
use crate::hud::minimap::{Blip, Ghost, MinimapBox, MinimapModel, Ping, RELIEF_RES};

/// Half-angle of the minimap view wedge (radians) — a readable spread, not the true camera FOV.
const VIEW_HALF_FOV_RAD: f32 = 0.55;

/// Sample the heightmap into a `RELIEF_RES`-square grid, normalised to its own min/max so the
/// relief reads on any map regardless of absolute elevation. Cells whose ground lies under the
/// map's standing water are flagged so the map paints the river instead of a dark lowland.
fn sample_relief(
    heightmap: &HeightMap,
    extent: [f32; 2],
    water: terrain::WaterView<'_>,
) -> (Vec<f32>, Vec<bool>) {
    let n = RELIEF_RES;
    let mut heights = vec![0.0f32; n * n];
    let mut wet = vec![false; n * n];
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for iz in 0..n {
        for ix in 0..n {
            let x = (ix as f32 + 0.5) / n as f32 * extent[0];
            let z = (iz as f32 + 0.5) / n as f32 * extent[1];
            let y = heightmap.sample_height(x, z).unwrap_or(0.0);
            heights[iz * n + ix] = y;
            wet[iz * n + ix] = water.depth_at(y, x, z) > 0.05;
            lo = lo.min(y);
            hi = hi.max(y);
        }
    }
    let range = (hi - lo).max(0.001);
    for value in &mut heights {
        *value = (*value - lo) / range;
    }
    (heights, wet)
}

/// The minimap's static layers — terrain relief, water, roads, and cover boxes — computed
/// ONCE per battlefield. None of them change during a match; resampling and renormalising
/// every rendered frame was pure waste (the audit's cheapest isolated win).
#[derive(Debug, Clone, Default)]
pub(crate) struct MinimapStaticLayers {
    pub relief: Vec<f32>,
    pub water: Vec<bool>,
    pub roads: Vec<Vec<[f32; 2]>>,
    pub cover: Vec<MinimapBox>,
    /// The relief and the water painted once at `MINIMAP_BAKE_PX` square (H0): uploaded into
    /// the material sheet's reserved quarter and drawn as ONE quad instead of 1 296.
    pub relief_bake: Vec<u8>,
}

pub(crate) fn minimap_static_layers(battlefield: &terrain::BattlefieldMap) -> MinimapStaticLayers {
    let extent = battlefield.heightmap.extent_m();
    let extent_m = [extent[0].max(1.0), extent[1].max(1.0)];
    let (relief, water) = sample_relief(&battlefield.heightmap, extent_m, battlefield.water_view());
    let roads = battlefield.roads.iter().map(|road| road.points.clone()).collect();
    let cover = battlefield
        .static_cover
        .iter()
        .map(|c| MinimapBox {
            center_xz: [c.center[0], c.center[2]],
            half_xz: [c.half_extents_m[0], c.half_extents_m[2]],
        })
        .collect();
    let relief_bake = bake_minimap_relief(battlefield);
    MinimapStaticLayers { relief, water, roads, cover, relief_bake }
}

/// How strongly a slope facing the map's top-left lightens and the opposite darkens: a hint of
/// hillshade so ridgelines read as ridges, kept gentle so the ramp still says height.
const BAKE_SHADE_GAIN: f32 = 5.0;

/// The minimap's relief and water as texels (H0): the heightmap sampled at the bake's own
/// resolution — far finer than the 36-cell quad grid it replaces — normalised to the map's
/// own range, on the same ramp the cells wore, water in the river blue, with a touch of
/// hillshade. Row 0 is the map's far edge (+z), so the texture's v runs down the map as the
/// quad's does. Opaque throughout: the map square is the plate's window.
pub(crate) fn bake_minimap_relief(battlefield: &terrain::BattlefieldMap) -> Vec<u8> {
    let n = ui_kit::sheet::MINIMAP_BAKE_PX as usize;
    let extent = battlefield.heightmap.extent_m();
    let extent_m = [extent[0].max(1.0), extent[1].max(1.0)];
    let water = battlefield.water_view();
    let mut heights = vec![0.0f32; n * n];
    let mut wet = vec![false; n * n];
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for row in 0..n {
        for col in 0..n {
            let x = (col as f32 + 0.5) / n as f32 * extent_m[0];
            let z = (1.0 - (row as f32 + 0.5) / n as f32) * extent_m[1];
            let y = battlefield.heightmap.sample_height(x, z).unwrap_or(0.0);
            heights[row * n + col] = y;
            wet[row * n + col] = water.depth_at(y, x, z) > 0.05;
            lo = lo.min(y);
            hi = hi.max(y);
        }
    }
    let range = (hi - lo).max(0.001);
    let mut rgba = vec![0u8; n * n * 4];
    for row in 0..n {
        for col in 0..n {
            let at = row * n + col;
            let tint = if wet[at] {
                // The river blue, opaque: the square is the plate's window, nothing shows through.
                let water = crate::hud::minimap::WATER;
                [water[0], water[1], water[2], 1.0]
            } else {
                let h = (heights[at] - lo) / range;
                let left = heights[row * n + col.saturating_sub(1)];
                let right = heights[row * n + (col + 1).min(n - 1)];
                let up = heights[row.saturating_sub(1) * n + col];
                let down = heights[(row + 1).min(n - 1) * n + col];
                let slope = ((left - right) + (up - down)) / range;
                let shade = (1.0 + BAKE_SHADE_GAIN * slope).clamp(0.72, 1.28);
                let base = crate::hud::minimap::relief_tint(h);
                [base[0] * shade, base[1] * shade, base[2] * shade, 1.0]
            };
            for (channel, value) in tint.iter().enumerate() {
                rgba[at * 4 + channel] = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
    rgba
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
        let roster = self.session.roster();
        let seat_of = |id: game_core::TankId| {
            roster
                .iter()
                .find(|entry| entry.tank_id == id)
                .map_or(' ', net::RosterEntry::seat_letter)
        };

        let xz = |tank: &PresentationTank| [tank.translation[0], tank.translation[2]];
        let blip = |tank: &PresentationTank| Blip {
            xz: xz(tank),
            class: tank.vehicle.class(),
            seat: seat_of(tank.id),
        };
        let allies = tanks
            .iter()
            .filter(|t| t.id != self.player_tank && t.team == player_team && t.hit_points > 0)
            .map(blip)
            .collect();
        let spotted: Vec<&PresentationTank> = tanks
            .iter()
            .filter(|t| {
                t.team != player_team
                    && t.hit_points > 0
                    && t.spotted_by_teams_mask & player_bit != 0
            })
            .collect();
        let enemies: Vec<Blip> = spotted.iter().map(|t| blip(t)).collect();
        // The memory (H15): the ghosts are the hulls seen before and not now.
        let spotted_ids: Vec<game_core::TankId> = spotted.iter().map(|t| t.id).collect();
        let ghosts: Vec<Ghost> = self
            .ghosts
            .ghosts(move |id| spotted_ids.contains(&id))
            .map(|known| Ghost { xz: known.xz, class: known.class, age_s: known.age_s })
            .collect();
        // The team's pings (W-5), aged against the newest server tick.
        let now_tick =
            self.render_state.latest_snapshot().map_or(0, |snapshot| snapshot.server_tick);
        let tick_s = 1.0 / sim::DEFAULT_SERVER_TICK_HZ as f32;
        let pings: Vec<Ping> = self
            .intel
            .team_commands()
            .filter(|relay| relay.command == net::TeamCommand::Ping)
            .filter_map(|relay| {
                let age_s = now_tick.saturating_sub(relay.server_tick) as f32 * tick_s;
                (age_s <= crate::hud::minimap::PING_TTL_S)
                    .then(|| relay.map_position.map(|xz| Ping { xz, age_s }))
                    .flatten()
            })
            .collect();
        let seen_from_m = crate::hud::budget::BudgetModel::from_battle(
            &roster,
            player_team,
            self.predictor.speed_mps(),
            self.own_shot.fire_age_s(),
            sim::DEFAULT_SIMULATION_TICK_HZ as f32,
        )
        .map(|budget| budget.seen_from_m as f32);

        let extent = self.battlefield.heightmap.extent_m();
        let extent_m = [extent[0].max(1.0), extent[1].max(1.0)];

        // Map yaw convention: heading 0 points up (+world z), so yaw = atan2(x, z).
        Some(MinimapModel {
            size: self.input.minimap_size(),
            extent_m,
            relief: self.minimap_static.relief.clone(),
            water: self.minimap_static.water.clone(),
            roads: self.minimap_static.roads.clone(),
            cover: self.minimap_static.cover.clone(),
            player_xz: [player.translation[0], player.translation[2]],
            player_heading_rad: player.hull_yaw_rad,
            player_turret_yaw_rad: player.hull_yaw_rad + player.turret_yaw_rad,
            view_yaw_rad: camera_forward_xz[0].atan2(camera_forward_xz[1]),
            view_half_fov_rad: VIEW_HALF_FOV_RAD,
            view_range_m: self.player_spec().view_range_m(),
            seen_from_m,
            allies,
            enemies,
            ghosts,
            pings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// H0: the bake is the reserved quarter's size, opaque, and not flat — the map's own relief
    /// is in it. Baked from the shipped default map, the same document the goldens stage.
    #[test]
    fn the_relief_bake_fills_its_square_and_is_not_flat() {
        let battlefield = map_forge::battlefield(terrain::MapId::default());
        let bake = bake_minimap_relief(&battlefield);
        let n = ui_kit::sheet::MINIMAP_BAKE_PX as usize;
        assert_eq!(bake.len(), n * n * 4);
        assert!(bake.chunks(4).all(|texel| texel[3] == 255), "opaque throughout");
        let greens: Vec<u8> = bake.chunks(4).map(|texel| texel[1]).collect();
        let (lo, hi) = greens.iter().fold((255u8, 0u8), |(lo, hi), g| (lo.min(*g), hi.max(*g)));
        assert!(hi - lo > 40, "the relief reads: green spans {lo}..{hi}");
    }
}
