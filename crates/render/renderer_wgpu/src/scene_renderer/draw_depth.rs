//! Depth-only passes: the sun-shadow occluder pass and the SSAO camera depth prepass. Split from
//! `draw.rs` to keep each module within the reviewability budget.

impl super::SceneRenderer {
    /// Depth-only occluder pass: render the vehicles from the sun's point of view into the shadow
    /// map so the scene and vehicle shaders can shade the key light. Terrain is a receiver, not a
    /// caster, in this focused phase. The pass always clears the map (so it holds no stale depth) and
    /// draws occluders only when shadows are on and there are vehicles to cast.
    pub(super) fn encode_shadow_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shadow_pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if self.shadow.strength <= 0.0 || self.vehicle_instance_count == 0 {
            return;
        }
        pass.set_pipeline(&self.shadow.pipeline);
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

    /// Camera depth prepass for SSAO: terrain, static scene meshes and vehicles rendered
    /// depth-only into the screen-sized prepass texture the SSAO pass evaluates.
    pub(super) fn encode_ssao_prepass(&self, encoder: &mut wgpu::CommandEncoder) {
        let targets = self.ssao.targets.borrow();
        let Some(targets) = targets.as_ref() else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ssao_prepass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.ssao.prepass_scene_pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(1, self.identity_instance.slice(..));
        if self.terrain_index_count > 0 {
            pass.set_vertex_buffer(0, self.terrain_vertices.slice(..));
            pass.set_index_buffer(self.terrain_indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.terrain_index_count, 0, 0..1);
        }
        if self.frame_instance_count > 0 {
            pass.set_vertex_buffer(1, self.frame_instances.slice(..));
            for draw in &self.frame_draws {
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
