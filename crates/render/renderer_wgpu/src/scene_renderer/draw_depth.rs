//! Depth-only passes: the sun-shadow occluder pass and the SSAO camera depth prepass. Split from
//! `draw.rs` to keep each module within the reviewability budget. Each pass culls the terrain
//! chunks by ITS OWN frustum — the shadow passes by their focused light boxes (most of a
//! 1000 m map sits outside a ~128 m box), the SSAO prepass by the camera.

use renderer_api::Frustum;

use crate::frame_graph::PassId;
use crate::pass_recorder::PassRecorder;

impl super::SceneRenderer {
    /// Depth-only occluder pass: render the world from the sun's point of view into the shadow map
    /// so the scene and vehicle shaders can shade the key light. The **whole** static world casts —
    /// terrain, and the buildings/trees baked into the same buffer — alongside the dynamic mesh and
    /// the running fleet, so hillsides self-shadow under a raking sun and buildings ground on the
    /// field instead of floating. The pass always clears the map (so it holds no stale depth) and
    /// draws occluders whenever shadows are on.
    pub(super) fn encode_shadow_pass(
        &self,
        recorder: &mut PassRecorder<'_>,
        encoder: &mut wgpu::CommandEncoder,
        light_frustum: &Frustum,
    ) {
        let mut pass = recorder.begin(
            encoder,
            PassId::ShadowNear,
            &[],
            Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
        );
        if self.shadow.strength <= 0.0 {
            return;
        }
        // Static world + dynamic mesh: the scene vertex stride, driven by the identity/frame
        // instance transforms — exactly the caster set the SSAO prepass walks.
        pass.set_pipeline(&self.shadow.pipeline_scene);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_bind_group(1, &self.foliage_atlas.bind_group, &[]);
        pass.set_vertex_buffer(1, self.identity_instance.slice(..));
        self.draw_visible_ground(&mut pass, light_frustum);
        if self.terrain_index_count > 0 {
            pass.set_vertex_buffer(0, self.terrain_vertices.slice(..));
            pass.set_index_buffer(self.terrain_indices.slice(..), wgpu::IndexFormat::Uint32);
            self.draw_visible_terrain(&mut pass, light_frustum);
        }
        if self.dynamic_index_count > 0 {
            pass.set_vertex_buffer(0, self.dynamic_vertices.slice(..));
            pass.set_index_buffer(self.dynamic_indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.dynamic_index_count, 0, 0..1);
        }
        if self.frame_instance_count > 0 {
            pass.set_vertex_buffer(1, self.frame_instances.slice(..));
            for draw in &self.frame_draws {
                if draw.mesh.0 >= renderer_api::SHADOWLESS_DRESSING_MESH_BASE {
                    continue;
                }
                let Some(mesh) = self.static_meshes.get(draw.mesh) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(
                    0..mesh.index_count,
                    0,
                    draw.instance_start..draw.instance_start + draw.instance_count,
                );
            }
        }
        // The running fleet: the vehicle vertex stride and its own instance transforms.
        if self.vehicle_instance_count > 0 {
            pass.set_pipeline(&self.shadow.pipeline_vehicle);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(1, self.vehicle_instances.slice(..));
            for draw in &self.vehicle_draws {
                let Some(mesh) = self.vehicle_meshes.get(draw.mesh) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(
                    0..mesh.index_count,
                    0,
                    draw.instance_start..draw.instance_start + draw.instance_count,
                );
            }
        }
    }

    /// The far-cascade occluder pass: terrain, the dynamic mesh and the static scene meshes from
    /// the far cascade's point of view — NO vehicle fleet. At the far map's ~0.56 m texels a
    /// tank's shadow does not resolve (and vehicles past the near box cast nothing today either),
    /// so the far pass stays nearly free while hillsides and the far town finally ground.
    pub(super) fn encode_far_shadow_pass(
        &self,
        recorder: &mut PassRecorder<'_>,
        encoder: &mut wgpu::CommandEncoder,
        light_frustum: &Frustum,
    ) {
        let mut pass = recorder.begin(
            encoder,
            PassId::ShadowFar,
            &[],
            Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow.far_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
        );
        if self.shadow.strength <= 0.0 || self.shadow.cascade_count < 2 {
            return;
        }
        pass.set_pipeline(&self.shadow.pipeline_scene_far);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_bind_group(1, &self.foliage_atlas.bind_group, &[]);
        pass.set_vertex_buffer(1, self.identity_instance.slice(..));
        self.draw_visible_ground(&mut pass, light_frustum);
        if self.terrain_index_count > 0 {
            pass.set_vertex_buffer(0, self.terrain_vertices.slice(..));
            pass.set_index_buffer(self.terrain_indices.slice(..), wgpu::IndexFormat::Uint32);
            self.draw_visible_terrain(&mut pass, light_frustum);
        }
        if self.dynamic_index_count > 0 {
            pass.set_vertex_buffer(0, self.dynamic_vertices.slice(..));
            pass.set_index_buffer(self.dynamic_indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.dynamic_index_count, 0, 0..1);
        }
        if self.frame_instance_count > 0 {
            pass.set_vertex_buffer(1, self.frame_instances.slice(..));
            for draw in &self.frame_draws {
                if draw.mesh.0 >= renderer_api::SHADOWLESS_DRESSING_MESH_BASE {
                    continue;
                }
                let Some(mesh) = self.static_meshes.get(draw.mesh) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(
                    0..mesh.index_count,
                    0,
                    draw.instance_start..draw.instance_start + draw.instance_count,
                );
            }
        }
    }

    /// Camera depth prepass for SSAO: terrain, static scene meshes and vehicles rendered
    /// depth-only into the screen-sized prepass texture the SSAO pass evaluates.
    pub(super) fn encode_ssao_prepass(
        &self,
        recorder: &mut PassRecorder<'_>,
        encoder: &mut wgpu::CommandEncoder,
        camera_frustum: &Frustum,
    ) {
        let targets = self.ssao.targets.borrow();
        let Some(targets) = targets.as_ref() else {
            return;
        };
        let mut pass = recorder.begin(
            encoder,
            PassId::SsaoPrepass,
            &[],
            Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
        );
        pass.set_pipeline(&self.ssao.prepass_scene_pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_bind_group(1, &self.foliage_atlas.bind_group, &[]);
        pass.set_vertex_buffer(1, self.identity_instance.slice(..));
        self.draw_visible_ground(&mut pass, camera_frustum);
        if self.terrain_index_count > 0 {
            pass.set_vertex_buffer(0, self.terrain_vertices.slice(..));
            pass.set_index_buffer(self.terrain_indices.slice(..), wgpu::IndexFormat::Uint32);
            self.draw_visible_terrain(&mut pass, camera_frustum);
        }
        if self.frame_instance_count > 0 {
            pass.set_vertex_buffer(1, self.frame_instances.slice(..));
            for draw in &self.frame_draws {
                if draw.mesh.0 >= renderer_api::SHADOWLESS_DRESSING_MESH_BASE {
                    continue;
                }
                let Some(mesh) = self.static_meshes.get(draw.mesh) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(
                    0..mesh.index_count,
                    0,
                    draw.instance_start..draw.instance_start + draw.instance_count,
                );
            }
        }
        if self.vehicle_instance_count > 0 {
            pass.set_pipeline(&self.ssao.prepass_vehicle_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(1, self.vehicle_instances.slice(..));
            for draw in &self.vehicle_draws {
                let Some(mesh) = self.vehicle_meshes.get(draw.mesh) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(
                    0..mesh.index_count,
                    0,
                    draw.instance_start..draw.instance_start + draw.instance_count,
                );
            }
        }
    }
}
