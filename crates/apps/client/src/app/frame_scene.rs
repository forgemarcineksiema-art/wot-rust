//! Frame scene composition: which tanks render this frame (interpolated + locally predicted),
//! their projection into the presentation world, and the frame's FX vertex batch. Split from
//! `render.rs` (the frame orchestration) for the reviewability budget.

use engine::TankMotion;
use net::TankSnapshot;
use sim::DEFAULT_SNAPSHOT_HZ;

use super::ClientApp;

const SNAPSHOT_INTERVAL_SECONDS: f32 = 1.0 / DEFAULT_SNAPSHOT_HZ as f32;

impl ClientApp {
    /// This frame's rendered tanks, each paired with its tick-domain motion: remote tanks carry
    /// the snapshot-pair motion, the local slot is overwritten by the predicted pose with the
    /// predictor's rigid-body motion.
    pub(super) fn render_tanks(&self, alpha: f32) -> Vec<(TankSnapshot, TankMotion)> {
        let mut tanks: Vec<(TankSnapshot, TankMotion)> = self
            .render_state
            .interpolated_tanks()
            .iter()
            .map(|tank| {
                let motion = self.render_state.motion_of(tank.tank_id);
                (tank.clone(), motion)
            })
            .collect();
        if let Some(local) = self.interpolated_local_tank(alpha)
            && let Some(slot) = tanks.iter_mut().find(|(tank, _)| tank.tank_id == self.player_tank)
        {
            *slot = (local, self.predictor.motion(alpha));
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
    /// vehicle is hidden in sniper view. Everything else draws only if it can possibly reach
    /// the screen: a blueprint vehicle emits ~200+ running-gear instances per frame, and before
    /// this cull all 14 tanks paid that cost even parked behind the camera across the map.
    pub(super) fn visible_render_tanks(
        &self,
        tanks: Vec<engine::PresentationTank>,
        view_proj: [[f32; 4]; 4],
        eye: [f32; 3],
    ) -> Vec<engine::PresentationTank> {
        let sniper = self.camera_controller.mode() == crate::BattleCameraMode::Sniper;
        tanks
            .into_iter()
            .filter(|tank| !(sniper && tank.id == self.player_tank))
            .filter(|tank| tank_can_reach_the_screen(view_proj, eye, tank.translation))
            .collect()
    }

    /// This frame's full FX batch: ground craters first (the farthest surface layer — smoke and
    /// dust must composite over them), then the ticked particle pool, a tracer per in-flight
    /// shell (stateless — rebuilt each frame from the interpolated shell snapshots), and the
    /// on-tank scar decals.
    ///
    /// All of it shares ONE vertex buffer, and a shelled field asks for more than fits: 128
    /// terrain scars alone want ~42k vertices against a budget of 13k, reached about forty
    /// ground impacts into a match. So the batch is budgeted before it is built. The layers that
    /// carry the game — blasts, tracers, the holes punched in hulls — are built first and take
    /// what they need; the marks in the ground get what is left and drop their OLDEST to fit.
    ///
    /// Emission order is unchanged: ruts, scorch marks, particles, tracers, hull decals. The
    /// FX pass blends premultiplied source-over with depth writes off, so order IS compositing
    /// — budgeting decides what is in the batch, never where it sits in it.
    ///
    /// (See `tank_can_reach_the_screen` below for the vehicle cull this batch is NOT subject
    /// to — FX are cheap quads; vehicles are hundreds of gear instances.)
    /// Fill `out` with this frame's composited FX batch, using `live` as working space for the
    /// quads that carry the game (particles, tracers, hull decals). Both buffers are caller-owned
    /// scratch, cleared here and reused across frames, so a ~1 MiB batch costs no allocation — the
    /// caller recovers them the same way the grass frame does.
    pub(super) fn fx_frame_vertices_into(
        &self,
        eye: [f32; 3],
        target: [f32; 3],
        live: &mut Vec<renderer_api::FxVertex>,
        out: &mut Vec<renderer_api::FxVertex>,
    ) {
        let eye = glam::Vec3::from_array(eye);
        let budget = renderer_wgpu::fx_vertex_budget();
        // Built first to learn their size, appended last to keep their place in the composite.
        live.clear();
        live.extend(self.fx.vertices(eye, glam::Vec3::from_array(target)));
        let shells = self.render_state.interpolated_shells(SNAPSHOT_INTERVAL_SECONDS);
        crate::fx::append_shell_tracers(live, &shells, eye);
        self.append_scar_quads(live);

        let ground = budget.saturating_sub(live.len());
        out.clear();
        self.track_marks.append_quads_within(out, ground);
        let left = ground - out.len().min(ground);
        self.terrain_scars.append_quads_within(out, left);
        out.append(live);
    }
}

/// Whether a hull at `translation` can influence this frame's image. Two honest gates:
/// anything within the sun-shadow reach of the camera always draws (an off-screen hull just
/// beside the lens still throws a visible shadow into the frame), and beyond that reach the
/// hull must intersect the view frustum (tested as a sphere against the normalized clip
/// planes). Blips, HP bars and the minimap read the UNculled presentation set - this gate
/// only decides who pays the running-gear instancing bill.
const TANK_CULL_RADIUS_M: f32 = 8.0;
const SHADOW_REACH_M: f32 = 80.0;

fn tank_can_reach_the_screen(
    view_proj: [[f32; 4]; 4],
    eye: [f32; 3],
    translation: [f32; 3],
) -> bool {
    let position = glam::Vec3::from_array(translation);
    if position.distance_squared(glam::Vec3::from_array(eye)) <= SHADOW_REACH_M * SHADOW_REACH_M {
        return true;
    }
    let matrix = glam::Mat4::from_cols_array_2d(&view_proj);
    let point = position.extend(1.0);
    // Gribb-Hartmann frustum planes from the view-projection rows; wgpu clips z to [0, 1].
    let planes = [
        matrix.row(3) + matrix.row(0), // left
        matrix.row(3) - matrix.row(0), // right
        matrix.row(3) + matrix.row(1), // bottom
        matrix.row(3) - matrix.row(1), // top
        matrix.row(2),                 // near (z >= 0)
        matrix.row(3) - matrix.row(2), // far
    ];
    planes.iter().all(|plane| {
        let normal_len = plane.truncate().length().max(1.0e-6);
        plane.dot(point) / normal_len >= -TANK_CULL_RADIUS_M
    })
}

#[cfg(test)]
mod cull_tests {
    use renderer_api::{Camera, CameraProjectionPolicy, view_projection_matrix};

    use super::tank_can_reach_the_screen;

    fn looking_down_z() -> ([[f32; 4]; 4], [f32; 3]) {
        let camera =
            Camera { eye: [0.0, 2.0, 0.0], target: [0.0, 2.0, 100.0], vertical_fov_degrees: 55.0 };
        let policy = CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &camera,
            16.0 / 9.0,
            policy.near_plane_m(),
            policy.far_plane_m(),
        );
        (view_proj, camera.eye)
    }

    #[test]
    fn the_cull_keeps_what_the_frame_can_see_and_what_can_shadow_it() {
        let (view_proj, eye) = looking_down_z();
        // Dead ahead: on screen.
        assert!(tank_can_reach_the_screen(view_proj, eye, [0.0, 0.0, 200.0]));
        // Behind the camera but inside the shadow reach: kept (its shadow can enter the frame).
        assert!(tank_can_reach_the_screen(view_proj, eye, [0.0, 0.0, -30.0]));
        // Behind the camera and far: culled.
        assert!(!tank_can_reach_the_screen(view_proj, eye, [0.0, 0.0, -400.0]));
        // Far off to the side, outside the frustum: culled.
        assert!(!tank_can_reach_the_screen(view_proj, eye, [800.0, 0.0, 120.0]));
        // Near the frustum edge: the sphere radius keeps a clipping hull rendered.
        assert!(tank_can_reach_the_screen(view_proj, eye, [98.0, 0.0, 170.0]));
    }
}
