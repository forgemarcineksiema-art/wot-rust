//! Screen-space ambient occlusion (atmosphere phase 2): a camera depth prepass of the terrain and
//! vehicles, an SSAO pass sampling it with a world-scaled spiral kernel, and a box blur. The main
//! pipelines sample the blurred target at group 2 (see `shadow.rs`); strength 0 is the capability
//! fallback that leaves every surface fully open.

use std::cell::RefCell;

use renderer_api::CameraProjectionPolicy;

use super::ssao_pipelines::{build_prepass_pipeline, fullscreen_pipeline, texture_bgl};

pub fn ssao_shader_source() -> String {
    crate::shader_library::compose_shader(&[
        crate::shader_library::CAMERA_COMMON_WGSL,
        include_str!("../shaders/ssao.wgsl"),
    ])
}

/// The screen-sized SSAO chain: the prepass depth, the raw AO target and its blurred copy, plus
/// the bind groups the passes consume. Recreated whenever the render-target size changes.
pub(crate) struct SsaoTargets {
    pub width: u32,
    pub height: u32,
    pub depth_view: wgpu::TextureView,
    pub ao_view: wgpu::TextureView,
    pub blur_view: wgpu::TextureView,
    pub depth_bind_group: wgpu::BindGroup,
    pub blur_src_bind_group: wgpu::BindGroup,
}

/// SSAO pipelines and settings. `strength` 0 disables (capability fallback); `near`/`far` default
/// to the shared projection policy and only need changing for exotic cameras. `scale` is the
/// SSAO render scale from the lighting-quality table: 0.5 on integrated GPUs quarters the AO
/// pixels — including the depth prepass rasterization, the real cost — for a soft effect the
/// 3×3 blur was smearing anyway.
pub(crate) struct SsaoResources {
    pub strength: f32,
    pub scale: f32,
    pub near: f32,
    pub far: f32,
    pub prepass_vehicle_pipeline: wgpu::RenderPipeline,
    pub prepass_scene_pipeline: wgpu::RenderPipeline,
    ssao_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    depth_bgl: wgpu::BindGroupLayout,
    src_bgl: wgpu::BindGroupLayout,
    pub targets: RefCell<Option<SsaoTargets>>,
}

impl SsaoResources {
    pub fn new(device: &wgpu::Device, camera_bgl: &wgpu::BindGroupLayout, scale: f32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ssao_shader"),
            source: wgpu::ShaderSource::Wgsl(ssao_shader_source().into()),
        });
        let prepass_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ssao_prepass_shader"),
            source: wgpu::ShaderSource::Wgsl(super::shadow::shadow_shader_source().into()),
        });
        let depth_bgl = texture_bgl(device, "ssao_depth_bgl", wgpu::TextureSampleType::Depth);
        let src_bgl = texture_bgl(
            device,
            "ssao_src_bgl",
            wgpu::TextureSampleType::Float { filterable: false },
        );
        let ssao_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ssao_layout"),
            bind_group_layouts: &[Some(camera_bgl), Some(&depth_bgl)],
            immediate_size: 0,
        });
        let blur_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ssao_blur_layout"),
            bind_group_layouts: &[Some(camera_bgl), Some(&depth_bgl), Some(&src_bgl)],
            immediate_size: 0,
        });
        let policy = CameraProjectionPolicy::webgpu_default();
        Self {
            // Scale 0 is the WOT_SSAO=off override: the whole chain stays valid, strength 0 skips
            // the passes and the shaders read fully open.
            strength: if scale > 0.0 { 1.0 } else { 0.0 },
            scale: scale.clamp(0.0, 1.0),
            near: policy.near_plane_m(),
            far: policy.far_plane_m(),
            prepass_vehicle_pipeline: build_prepass_pipeline(
                device,
                &prepass_shader,
                camera_bgl,
                std::mem::size_of::<renderer_api::VehicleVertex>() as u64,
                "ssao_prepass_vehicle",
            ),
            prepass_scene_pipeline: build_prepass_pipeline(
                device,
                &prepass_shader,
                camera_bgl,
                std::mem::size_of::<renderer_api::SceneVertex>() as u64,
                "ssao_prepass_scene",
            ),
            ssao_pipeline: fullscreen_pipeline(device, &shader, &ssao_layout, "fs_ssao"),
            blur_pipeline: fullscreen_pipeline(device, &shader, &blur_layout, "fs_blur"),
            depth_bgl,
            src_bgl,
            targets: RefCell::new(None),
        }
    }

    /// Ensure the AO chain matches the `width`×`height` RENDER target (textures are created at
    /// `scale` times that size); returns `true` when it was (re)created and the group-2
    /// environment bind group must be re-pointed at the new blur view.
    pub fn ensure_targets(&self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        let current = self.targets.borrow();
        if current.as_ref().is_some_and(|t| t.width == width && t.height == height) {
            return false;
        }
        drop(current);

        let scaled = |edge: u32| ((edge as f32 * self.scale.max(0.25)).round() as u32).max(1);
        let (ao_width, ao_height) = (scaled(width), scaled(height));
        let make = |label: &str, format: wgpu::TextureFormat| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: ao_width,
                        height: ao_height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let depth_view = make("ssao_prepass_depth", super::ssao_pipelines::PREPASS_DEPTH_FORMAT);
        let ao_view = make("ssao_raw", super::ssao_pipelines::AO_FORMAT);
        let blur_view = make("ssao_blurred", super::ssao_pipelines::AO_FORMAT);
        let bind = |bgl: &wgpu::BindGroupLayout, view: &wgpu::TextureView, label: &str| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                }],
            })
        };
        let depth_bind_group = bind(&self.depth_bgl, &depth_view, "ssao_depth_bg");
        let blur_src_bind_group = bind(&self.src_bgl, &ao_view, "ssao_blur_src_bg");
        *self.targets.borrow_mut() = Some(SsaoTargets {
            width,
            height,
            depth_view,
            ao_view,
            blur_view,
            depth_bind_group,
            blur_src_bind_group,
        });
        true
    }

    /// Encode the SSAO evaluation + blur (the depth prepass is encoded by the caller, which owns
    /// the geometry buffers).
    pub fn encode_ao_passes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        let targets = self.targets.borrow();
        let Some(targets) = targets.as_ref() else {
            return;
        };
        for (label, pipeline, output, extra_src) in [
            ("ssao_pass", &self.ssao_pipeline, &targets.ao_view, None),
            (
                "ssao_blur_pass",
                &self.blur_pipeline,
                &targets.blur_view,
                Some(&targets.blur_src_bind_group),
            ),
        ] {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, camera_bind_group, &[]);
            pass.set_bind_group(1, &targets.depth_bind_group, &[]);
            if let Some(src) = extra_src {
                pass.set_bind_group(2, src, &[]);
            }
            pass.draw(0..3, 0..1);
        }
    }
}
