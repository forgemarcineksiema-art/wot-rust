use renderer_api::{
    FxVertex, HudVertex, MaterialHandle, MeshAsset, MeshHandle, RenderFrame, SceneVertex,
    VehicleMaterialFamilies, VehicleMeshAsset,
};

use crate::GpuContext;
use crate::scene_resources::{SceneInstance, frame_instances_into};

impl super::SceneRenderer {
    pub fn set_dynamic_mesh(
        &mut self,
        ctx: &GpuContext,
        vertices: &[SceneVertex],
        indices: &[u32],
    ) {
        let vbytes: &[u8] = bytemuck::cast_slice(vertices);
        let ibytes: &[u8] = bytemuck::cast_slice(indices);
        if vbytes.len() as u64 > super::DYNAMIC_VERTEX_CAPACITY
            || ibytes.len() as u64 > super::DYNAMIC_INDEX_CAPACITY
        {
            // Refusing the upload keeps the previous mesh; never do it silently — a dropped
            // dynamic mesh reads as "the tanks vanished" with no trace of why.
            tracing::warn!(
                vertices = vertices.len(),
                indices = indices.len(),
                "dynamic mesh exceeds its buffer budget; keeping the previous mesh"
            );
            return;
        }
        ctx.queue.write_buffer(&self.dynamic_vertices, 0, vbytes);
        ctx.queue.write_buffer(&self.dynamic_indices, 0, ibytes);
        self.dynamic_index_count = indices.len() as u32;
    }

    pub fn register_mesh(&mut self, ctx: &GpuContext, handle: MeshHandle, mesh: &MeshAsset) {
        self.static_meshes.register(ctx, handle, mesh);
    }

    pub fn register_vehicle_mesh(
        &mut self,
        ctx: &GpuContext,
        handle: MeshHandle,
        mesh: &VehicleMeshAsset,
    ) {
        self.vehicle_meshes.register(ctx, handle, mesh);
    }

    pub fn register_vehicle_material(
        &mut self,
        ctx: &GpuContext,
        handle: MaterialHandle,
        families: &VehicleMaterialFamilies,
    ) {
        self.vehicle_materials.register(ctx, handle, families);
    }

    /// An oversized frame is truncated to the buffer budget (with a warning), never dropped
    /// whole: a dropped upload leaves the previous instances on screen, so every object would
    /// freeze in its last uploaded pose while the simulation drives on without it.
    pub fn set_render_frame(&mut self, ctx: &GpuContext, frame: &RenderFrame) {
        let capacity = buffer_instance_capacity(&self.frame_instances);
        frame_instances_into(&mut self.instance_scratch, frame);
        if self.instance_scratch.clip_to_capacity(capacity) {
            tracing::warn!(kept = capacity, "scene instance budget exceeded; truncating frame");
        }
        ctx.queue.write_buffer(
            &self.frame_instances,
            0,
            bytemuck::cast_slice(self.instance_scratch.instances()),
        );
        self.frame_instance_count = self.instance_scratch.instances().len() as u32;
        // Reuses this Vec's allocation too; the draw list is rebuilt every frame either way.
        self.frame_draws.clear();
        self.frame_draws.extend_from_slice(self.instance_scratch.draws());
    }

    /// See [`Self::set_render_frame`]: truncate-and-warn, never a silent whole-frame drop — that
    /// is exactly the failure that froze every tank on screen once a 7v7 exceeded the old budget.
    pub fn set_vehicle_render_frame(&mut self, ctx: &GpuContext, frame: &RenderFrame) {
        self.armor_damage.upload(ctx, &frame.armor_damage);
        self.vehicle_frame_has_damage = !frame.armor_damage.is_empty();
        self.collect_grass_crushers(frame);
        let capacity = buffer_instance_capacity(&self.vehicle_instances);
        frame_instances_into(&mut self.instance_scratch, frame);
        if self.instance_scratch.clip_to_capacity(capacity) {
            tracing::warn!(kept = capacity, "vehicle instance budget exceeded; truncating frame");
        }
        ctx.queue.write_buffer(
            &self.vehicle_instances,
            0,
            bytemuck::cast_slice(self.instance_scratch.instances()),
        );
        self.vehicle_instance_count = self.instance_scratch.instances().len() as u32;
        self.vehicle_draws.clear();
        self.vehicle_draws.extend_from_slice(self.instance_scratch.draws());
    }

    /// Where the vehicles stand, for the meadow to be pressed by (Jedna Trawa P9).
    ///
    /// Nothing new crosses the API for this: a vehicle frame already carries which objects
    /// belong to which tank, so the renderer reads the truth it is handed instead of asking
    /// the client to send tank positions a second time (and to keep them in sync). A tank is
    /// many objects — hull, turret, tracks — and the FIRST one seen fixes its position; the
    /// parts sit within a metre of each other, well inside a 3.6 m crush radius.
    fn collect_grass_crushers(&mut self, frame: &RenderFrame) {
        self.grass_crushers.clear();
        for object in &frame.objects {
            let Some(tank) = object.tank_id else { continue };
            if self.grass_crushers.iter().any(|(id, _)| *id == tank) {
                continue;
            }
            let t = object.transform;
            self.grass_crushers.push((tank, [t[3][0], t[3][1], t[3][2]]));
        }
    }

    /// The crusher slots this frame would hand the shader (test/diagnostic hook — the exact
    /// selection the uniform carries).
    pub fn grass_crusher_slots_for_test(
        &self,
        eye: [f32; 3],
    ) -> [[f32; 4]; renderer_api::MAX_GRASS_CRUSHERS] {
        self.grass_crusher_slots(eye)
    }

    /// The crusher slots for one frame: the nearest tanks to the eye, because grass only
    /// exists around the eye — a tank flattening a meadow 300 m away is flattening nothing
    /// anyone can see. Disabled (radius 0) slots pad the rest and cost one compare each.
    pub(super) fn grass_crusher_slots(
        &self,
        eye: [f32; 3],
    ) -> [[f32; 4]; renderer_api::MAX_GRASS_CRUSHERS] {
        let mut slots = [[0.0f32; 4]; renderer_api::MAX_GRASS_CRUSHERS];
        if self.grass_crushers.is_empty() {
            return slots;
        }
        let mut nearest: Vec<([f32; 3], f32)> = self
            .grass_crushers
            .iter()
            .map(|(_, position)| {
                let flat = (position[0] - eye[0]).hypot(position[2] - eye[2]);
                (*position, flat)
            })
            .collect();
        nearest.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (slot, (position, _)) in slots.iter_mut().zip(nearest) {
            *slot = [position[0], position[1], position[2], renderer_api::GRASS_CRUSH_RADIUS_M];
        }
        slots
    }

    /// Upload this frame's battle-FX quads (already world-space, premultiplied colors).
    ///
    /// An oversized upload keeps the first whole triangles that fit, exactly as `set_hud` does,
    /// and warns. It used to answer with a bare `return`, which took EVERY effect off the screen
    /// — tracers, blasts, ruts, the holes in the hulls — for as long as the overrun lasted, and
    /// said nothing. In a battle the overrun lasts from about the fortieth ground impact to the
    /// end of the match, so the layer simply went out mid-game and stayed out.
    ///
    /// Truncation is the floor, not the plan: the client budgets its FX frame against
    /// [`super::fx_vertex_budget`] and gives way in the marks on the ground, so what reaches
    /// here should already fit. This keeps a regression that outgrows the buffer to a few missing
    /// scorch marks instead of a blackout.
    pub fn set_fx(&mut self, ctx: &GpuContext, vertices: &[FxVertex]) {
        let capacity_vertices =
            (super::FX_VERTEX_CAPACITY as usize) / std::mem::size_of::<FxVertex>();
        let vertices = if vertices.len() > capacity_vertices {
            let kept = capacity_vertices - capacity_vertices % 3;
            tracing::warn!(
                submitted = vertices.len(),
                kept,
                "FX vertex budget exceeded; truncating effects"
            );
            &vertices[..kept]
        } else {
            vertices
        };
        ctx.queue.write_buffer(&self.fx_vertices, 0, bytemuck::cast_slice(vertices));
        self.fx_vertex_count = vertices.len() as u32;
    }

    /// Upload this frame's HUD overlay. An oversized upload keeps the first triangles that fit
    /// instead of blanking the whole HUD — losing the last elements beats losing the reticle —
    /// and warns so the regression is visible in logs.
    pub fn set_hud(&mut self, ctx: &GpuContext, vertices: &[HudVertex]) {
        let capacity_vertices =
            (super::HUD_VERTEX_CAPACITY as usize) / std::mem::size_of::<HudVertex>();
        let vertices = if vertices.len() > capacity_vertices {
            let kept = capacity_vertices - capacity_vertices % 3;
            tracing::warn!(
                submitted = vertices.len(),
                kept,
                "HUD vertex budget exceeded; truncating overlay"
            );
            &vertices[..kept]
        } else {
            vertices
        };
        ctx.queue.write_buffer(&self.hud_vertices, 0, bytemuck::cast_slice(vertices));
        self.hud_vertex_count = vertices.len() as u32;
    }

    /// Vertices the HUD pass will draw this frame — what `set_hud` accepted after budgeting.
    pub fn hud_vertex_count(&self) -> u32 {
        self.hud_vertex_count
    }
}

/// How many [`SceneInstance`]s the given GPU buffer holds.
fn buffer_instance_capacity(buffer: &wgpu::Buffer) -> usize {
    (buffer.size() as usize) / std::mem::size_of::<SceneInstance>()
}
