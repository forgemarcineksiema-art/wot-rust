//! Frame scene composition: which tanks render this frame (interpolated + locally predicted),
//! their projection into the presentation world, and the frame's FX vertex batch. Split from
//! `render.rs` (the frame orchestration) for the reviewability budget.

use net::TankSnapshot;
use sim::DEFAULT_SNAPSHOT_HZ;

use super::ClientApp;

const SNAPSHOT_INTERVAL_SECONDS: f32 = 1.0 / DEFAULT_SNAPSHOT_HZ as f32;

impl ClientApp {
    pub(super) fn render_tanks(&self, alpha: f32) -> Vec<TankSnapshot> {
        let mut tanks = self.render_state.interpolated_tanks();
        if let Some(local) = self.interpolated_local_tank(alpha)
            && let Some(slot) = tanks.iter_mut().find(|tank| tank.tank_id == self.player_tank)
        {
            *slot = local;
        }
        tanks
    }

    /// Sync this frame's rendered tanks into the persistent presentation world and read the
    /// presentation view back out. The renderer and HUD consume this ECS projection rather than
    /// the snapshot vec.
    pub(super) fn project_render_tanks(&mut self, alpha: f32) -> Vec<engine::PresentationTank> {
        let render_tanks = self.render_tanks(alpha);
        self.presentation.sync_tanks(&render_tanks);
        self.presentation.presentation_tanks()
    }

    /// The sniper eye sits at turret-roof height inside the player's own mesh, so the player's
    /// vehicle is hidden in sniper view; everything else always draws.
    pub(super) fn visible_render_tanks(
        &self,
        tanks: Vec<engine::PresentationTank>,
    ) -> Vec<engine::PresentationTank> {
        if self.camera_controller.mode() == crate::BattleCameraMode::Sniper {
            tanks.into_iter().filter(|tank| tank.id != self.player_tank).collect()
        } else {
            tanks
        }
    }

    /// This frame's full FX batch: ground craters first (the farthest surface layer — smoke and
    /// dust must composite over them), then the ticked particle pool, a tracer per in-flight
    /// shell (stateless — rebuilt each frame from the interpolated shell snapshots), and the
    /// on-tank scar decals.
    pub(super) fn fx_frame_vertices(
        &self,
        eye: [f32; 3],
        target: [f32; 3],
    ) -> Vec<renderer_api::FxVertex> {
        let eye = glam::Vec3::from_array(eye);
        let mut fx_vertices = Vec::new();
        self.terrain_scars.append_quads(&mut fx_vertices);
        fx_vertices.extend(self.fx.vertices(eye, glam::Vec3::from_array(target)));
        let shells = self.render_state.interpolated_shells(SNAPSHOT_INTERVAL_SECONDS);
        crate::fx::append_shell_tracers(&mut fx_vertices, &shells, eye);
        self.append_scar_quads(&mut fx_vertices);
        fx_vertices
    }
}
