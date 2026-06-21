//! The garage frame: the selected vehicle parked in the hangar under the orbit camera, plus the
//! scene-geometry swap between the garage hangar and the battlefield, and the player's barrel
//! silhouette scale shared by the rendered gun and the reticle muzzle.

use std::time::Instant;

use game_core::{TankId, TeamId, VehicleKind};
use net::TankSnapshot;
use renderer_api::{CameraProjectionPolicy, RenderFrame, SceneLighting, view_projection_matrix};
use renderer_wgpu::WindowRenderer;
use tracing::error;

use super::{ClientApp, SceneKind};
use crate::{battlefield_scene_mesh, render_frame_from_objects, tank_vehicle_render_objects};

impl ClientApp {
    /// Render the static garage hangar: the selected vehicle parked on the turntable under an
    /// orbit camera, with the garage UI overlay. Replaces the battle scene while the garage is open.
    pub(super) fn render_garage(&mut self) {
        self.last_render_time = Instant::now();
        // Apply orbit drag (and clear the cursor delta) — the battle path does this per frame, and
        // the garage needs it too or the inspection camera never moves.
        self.apply_mouse_look();
        self.ensure_scene(SceneKind::Garage);
        let aspect = self.renderer.as_ref().map_or(16.0 / 9.0, WindowRenderer::aspect_ratio);
        let camera = self.garage.orbit_camera();
        let projection = CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &camera,
            aspect,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );

        let snapshot = garage_preview_snapshot(self.garage.selected_vehicle());
        let mut objects = tank_vehicle_render_objects(
            &mut self.vehicle_asset_catalog,
            &snapshot,
            [0.72, 0.76, 0.62],
        );
        // Stretch the gun submesh (objects: [hull, turret, gun]) along its barrel axis so swapping
        // to a longer/shorter gun visibly changes the silhouette. Local +Z is the barrel direction.
        let barrel_scale = self.garage.gun_silhouette_scale();
        if (barrel_scale - 1.0).abs() > 1.0e-3
            && let Some(gun) = objects.get_mut(2)
        {
            let scaled = glam::Mat4::from_cols_array_2d(&gun.transform)
                * glam::Mat4::from_scale(glam::Vec3::new(1.0, 1.0, barrel_scale));
            gun.transform = scaled.to_cols_array_2d();
        }
        let render_frame = render_frame_from_objects(objects);
        let hud = self.garage.overlay_vertices(aspect);

        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        for (handle, mesh) in self.vehicle_asset_catalog.take_pending_vehicle_meshes() {
            renderer.register_vehicle_mesh(handle, &mesh);
        }
        for (handle, maps) in self.vehicle_asset_catalog.take_pending_vehicle_materials() {
            renderer.register_vehicle_material(handle, &maps);
        }
        renderer.set_render_frame(&RenderFrame::default());
        renderer.set_vehicle_render_frame(&render_frame);
        renderer.set_dynamic_mesh(&[], &[]);
        renderer.set_hud(&hud);
        if let Err(error) = renderer.render(view_proj, camera.eye) {
            error!(%error, "garage frame render failed");
        }
    }

    /// Swap the renderer's static geometry to the requested scene if it differs. Cheap because it
    /// only fires on a garage <-> battle transition, not per frame.
    pub(super) fn ensure_scene(&mut self, want: SceneKind) {
        if self.current_scene == want {
            return;
        }
        let (vertices, indices, sky, lighting) = match want {
            SceneKind::Garage => {
                let (v, i) = crate::garage_scene::hangar_scene_mesh();
                // A calibrated studio key/fill/rim keeps the vehicle's olive material readable
                // without casting colour over the whole scene.
                (v, i, (0.07, 0.05, 0.04), SceneLighting::garage_studio())
            }
            SceneKind::Battle => {
                let (v, i) = battlefield_scene_mesh(&self.battlefield);
                (v, i, (0.55, 0.69, 0.87), SceneLighting::battlefield_default())
            }
        };
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_terrain(&vertices, &indices);
            renderer.set_sky(sky.0, sky.1, sky.2);
            renderer.set_scene_lighting(lighting);
            self.current_scene = want;
        }
    }

    /// Barrel-length scale of the player's installed gun vs the vehicle's stock gun. The predictor
    /// holds the real (custom) loadout — the snapshot-derived `player_spec` is stock by kind — so
    /// the rendered gun and the reticle muzzle both track the gun the player actually fitted.
    pub(super) fn player_barrel_scale(&self) -> f32 {
        let spec = self.predictor.spec();
        let stock = spec.kind.stock_barrel_length_m();
        if stock <= 0.0 { 1.0 } else { (spec.gun.barrel_length_m / stock).clamp(0.6, 1.6) }
    }
}

/// A pose-only snapshot of the selected vehicle parked on the garage turntable, angled three-
/// quarters to the camera. Only the fields the mesh kernels read are meaningful.
fn garage_preview_snapshot(kind: VehicleKind) -> TankSnapshot {
    let spec = kind.spec();
    TankSnapshot {
        tank_id: TankId(0),
        team: TeamId(1),
        vehicle: kind,
        position: [0.0, crate::garage_scene::TURNTABLE_TOP_M, 0.0],
        yaw_rad: 0.6,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
    }
}
