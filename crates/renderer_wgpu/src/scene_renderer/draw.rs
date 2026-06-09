use renderer_api::RenderError;

use crate::scene_target::{SceneRenderTarget, store_op_for_target};
use crate::{CameraUniform, GpuContext, GpuMat4, encode_camera_uniform};

impl super::SceneRenderer {
    pub fn render(
        &self,
        ctx: &GpuContext,
        target: SceneRenderTarget<'_>,
        view_proj: [[f32; 4]; 4],
    ) -> Result<(), RenderError> {
        if target.sample_count != self.sample_count {
            return Err(RenderError::new(format!(
                "scene renderer sample count {} does not match render target sample count {}",
                self.sample_count, target.sample_count
            )));
        }
        let camera = CameraUniform { view_proj: GpuMat4(view_proj) };
        ctx.queue.write_buffer(&self.camera_buffer, 0, &encode_camera_uniform(&camera)?);

        let mut encoder =
            ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color_view,
                    resolve_target: target.resolve_target,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.sky),
                        store: store_op_for_target(target.resolve_target),
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(1, self.identity_instance.slice(..));
            if self.terrain_index_count > 0 {
                pass.set_vertex_buffer(0, self.terrain_vertices.slice(..));
                pass.set_index_buffer(self.terrain_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.terrain_index_count, 0, 0..1);
            }
            if self.dynamic_index_count > 0 {
                pass.set_vertex_buffer(0, self.dynamic_vertices.slice(..));
                pass.set_index_buffer(self.dynamic_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.dynamic_index_count, 0, 0..1);
            }
            if self.frame_instance_count > 0 {
                pass.set_vertex_buffer(1, self.frame_instances.slice(..));
                for draw in &self.frame_draws {
                    let Some(mesh) = self.static_meshes.get(draw.mesh) else {
                        self.skipped_mesh_draws
                            .set(self.skipped_mesh_draws.get().saturating_add(1));
                        debug_assert!(
                            false,
                            "render frame references unregistered mesh handle {}",
                            draw.mesh.0
                        );
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
            if self.hud_vertex_count > 0 {
                pass.set_pipeline(&self.hud_pipeline);
                pass.set_bind_group(0, &self.hud_font_bind_group, &[]);
                pass.set_vertex_buffer(0, self.hud_vertices.slice(..));
                pass.draw(0..self.hud_vertex_count, 0..1);
            }
        }
        ctx.queue.submit(Some(encoder.finish()));
        Ok(())
    }
}
