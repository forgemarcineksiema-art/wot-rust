use std::collections::BTreeMap;

use renderer_api::{MaterialHandle, MeshAsset, MeshHandle, RenderFrame};
use wgpu::util::DeviceExt;

use crate::GpuContext;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneInstance {
    pub model: [[f32; 4]; 4],
    /// Per-instance team tint (rgb + unused w for 16-byte alignment).
    pub tint: [f32; 4],
    /// One-based index into the analytical armor-damage header buffer; zero means undamaged.
    pub damage_index: u32,
    /// The screen-door window (`RenderObject::dither`), read by every pass that cuts foliage.
    pub dither: [f32; 2],
    pub _padding: u32,
}

impl SceneInstance {
    pub const fn identity() -> Self {
        Self {
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            tint: [1.0, 1.0, 1.0, 1.0],
            damage_index: 0,
            dither: [0.0, 1.0],
            _padding: 0,
        }
    }
}

pub struct GpuSceneMesh {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub index_count: u32,
}

#[derive(Default)]
pub struct SceneMeshRegistry {
    meshes: BTreeMap<u32, GpuSceneMesh>,
}

impl SceneMeshRegistry {
    pub fn register(&mut self, ctx: &GpuContext, handle: MeshHandle, asset: &MeshAsset) {
        let vertices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_static_mesh_v"),
            contents: bytemuck::cast_slice(asset.vertices()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_static_mesh_i"),
            contents: bytemuck::cast_slice(asset.indices()),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.meshes.insert(
            handle.0,
            GpuSceneMesh { vertices, indices, index_count: asset.index_count() as u32 },
        );
    }

    pub fn get(&self, handle: MeshHandle) -> Option<&GpuSceneMesh> {
        self.meshes.get(&handle.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneObjectDraw {
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
    pub instance_start: u32,
    pub instance_count: u32,
}

/// Reusable working set for [`frame_instances_into`].
///
/// Every buffer here is cleared and refilled rather than reallocated, because this runs TWICE per
/// frame — once for the scene, once for the fleet — over thousands of objects, and it was
/// measured at 0.95 ms of a 23 ms frame: 0.56 ms of it grass (it falls to 0.004 ms when the
/// meadow is off) and 0.39 ms the fleet. Allocator churn was most of that.
#[derive(Default)]
pub struct InstanceScratch {
    batch_of: std::collections::HashMap<(u32, u32), usize>,
    keys: Vec<(MeshHandle, MaterialHandle)>,
    counts: Vec<u32>,
    cursor: Vec<u32>,
    /// Which batch each object landed in, so pass two does not hash a second time.
    object_batch: Vec<u32>,
    damage_of: std::collections::HashMap<renderer_api::VehicleId, u32>,
    instances: Vec<SceneInstance>,
    draws: Vec<SceneObjectDraw>,
}

impl InstanceScratch {
    pub fn instances(&self) -> &[SceneInstance] {
        &self.instances
    }

    pub fn draws(&self) -> &[SceneObjectDraw] {
        &self.draws
    }

    /// Trim to what the GPU buffer holds. A method rather than two `&mut` accessors, because the
    /// caller cannot borrow both halves of the scratch at once.
    pub fn clip_to_capacity(&mut self, capacity: usize) -> bool {
        clip_instances_to_capacity(&mut self.instances, &mut self.draws, capacity)
    }
}

/// Batch a frame's objects by (mesh, material) into instanced draws, in place.
///
/// Two passes over the objects, one hash lookup each. The first assigns batches and counts them;
/// a prefix sum then gives every batch its span, so the second pass writes each instance STRAIGHT
/// into its final slot. The previous shape collected per-batch `Vec`s and copied them into a flat
/// buffer afterwards — a second full pass over ~430 KB of instances, plus an allocation per batch,
/// every frame.
///
/// First-seen batch order is preserved, so the draw list is byte-identical to the old one.
pub fn frame_instances_into(scratch: &mut InstanceScratch, frame: &RenderFrame) {
    let InstanceScratch {
        batch_of,
        keys,
        counts,
        cursor,
        object_batch,
        damage_of,
        instances,
        draws,
    } = scratch;
    batch_of.clear();
    keys.clear();
    counts.clear();
    cursor.clear();
    object_batch.clear();
    damage_of.clear();
    draws.clear();
    instances.clear();

    for (index, damage) in frame
        .armor_damage
        .iter()
        .take(crate::scene_renderer::armor_damage::MAX_DAMAGE_HEADERS - 1)
        .enumerate()
    {
        damage_of.insert(damage.tank_id, index as u32 + 1);
    }

    for object in &frame.objects {
        let key = (object.mesh.0, object.material.0);
        let batch = match batch_of.get(&key) {
            Some(&batch) => batch,
            None => {
                let batch = keys.len();
                batch_of.insert(key, batch);
                keys.push((object.mesh, object.material));
                counts.push(0);
                batch
            }
        };
        counts[batch] += 1;
        object_batch.push(batch as u32);
    }

    let mut start = 0u32;
    for (batch, &(mesh, material)) in keys.iter().enumerate() {
        cursor.push(start);
        draws.push(SceneObjectDraw {
            mesh,
            material,
            instance_start: start,
            instance_count: counts[batch],
        });
        start += counts[batch];
    }

    instances.resize(start as usize, bytemuck::Zeroable::zeroed());
    for (object, &batch) in frame.objects.iter().zip(object_batch.iter()) {
        let slot = &mut cursor[batch as usize];
        let [r, g, b] = object.tint;
        let damage_index =
            object.tank_id.and_then(|tank_id| damage_of.get(&tank_id).copied()).unwrap_or(0);
        instances[*slot as usize] = SceneInstance {
            model: object.transform,
            tint: [r, g, b, 1.0],
            damage_index,
            dither: object.dither,
            _padding: 0,
        };
        *slot += 1;
    }
}

/// The allocating form. Test-only: the frame path reuses a scratch, and a second allocating
/// entry point on the hot path is how the reuse would quietly stop happening.
#[cfg(test)]
pub fn frame_instances(frame: &RenderFrame) -> (Vec<SceneInstance>, Vec<SceneObjectDraw>) {
    let mut scratch = InstanceScratch::default();
    frame_instances_into(&mut scratch, frame);
    (scratch.instances, scratch.draws)
}

/// Clip an instance upload to the buffer budget: keep the first `capacity` instances and trim the
/// draw list to match (draws past the cut are dropped, the boundary draw is shortened). Returns
/// whether anything was clipped. Losing the last-submitted objects keeps the frame LIVE — dropping
/// the whole oversized upload instead leaves the previous frame's instances on screen, freezing
/// every vehicle in its last uploaded pose.
pub fn clip_instances_to_capacity(
    instances: &mut Vec<SceneInstance>,
    draws: &mut Vec<SceneObjectDraw>,
    capacity: usize,
) -> bool {
    if instances.len() <= capacity {
        return false;
    }
    instances.truncate(capacity);
    draws.retain_mut(|draw| {
        let start = draw.instance_start as usize;
        if start >= capacity {
            return false;
        }
        draw.instance_count = draw.instance_count.min((capacity - start) as u32);
        true
    });
    true
}

#[cfg(test)]
mod tests {
    use game_core::TankId;
    use renderer_api::{ArmorDamageInstance, MaterialHandle, RenderObject};

    use super::*;

    fn damage(tank: u64) -> ArmorDamageInstance {
        ArmorDamageInstance { tank_id: TankId(tank), apertures: Vec::new() }
    }

    /// The scratch is reused across frames, which buys the speed and introduces the one bug that
    /// buys back: something left over from the frame before. A smaller frame following a larger
    /// one must show no trace of it — not one extra instance, not one extra draw, and not a
    /// damage index belonging to a tank that is no longer on screen.
    #[test]
    fn a_reused_scratch_carries_nothing_over_from_the_previous_frame() {
        let mut scratch = InstanceScratch::default();

        let crowded = RenderFrame {
            objects: vec![object(1, 10, 2), object(2, 11, 2), object(3, 10, 2)],
            armor_damage: vec![damage(1)],
            ..Default::default()
        };
        frame_instances_into(&mut scratch, &crowded);
        assert_eq!(scratch.instances().len(), 3);
        assert_eq!(scratch.draws().len(), 2);
        assert_eq!(scratch.instances()[0].damage_index, 1, "tank 1 is damaged in this frame");

        let sparse = RenderFrame {
            objects: vec![object(7, 11, 2)],
            armor_damage: Vec::new(),
            ..Default::default()
        };
        frame_instances_into(&mut scratch, &sparse);
        assert_eq!(scratch.instances().len(), 1, "the crowded frame's instances outlived it");
        assert_eq!(scratch.draws().len(), 1, "the crowded frame's draws outlived it");
        assert_eq!(scratch.draws()[0].mesh, MeshHandle(11));
        assert_eq!(scratch.draws()[0].instance_start, 0, "the batch must start at zero again");
        assert_eq!(scratch.instances()[0].damage_index, 0, "a stale damage index survived");

        // And an empty frame empties it, rather than leaving the last one on screen.
        frame_instances_into(&mut scratch, &RenderFrame::default());
        assert!(scratch.instances().is_empty());
        assert!(scratch.draws().is_empty());
    }

    #[test]
    fn frame_instances_batch_same_mesh_and_material_into_one_draw() {
        let frame = RenderFrame {
            objects: vec![object(1, 10, 2), object(2, 11, 2), object(3, 10, 2)],
            ..Default::default()
        };

        let (instances, draws) = frame_instances(&frame);

        assert_eq!(instances.len(), 3);
        assert_eq!(draws.len(), 2);
        assert_eq!(draws[0].mesh, MeshHandle(10));
        assert_eq!(draws[0].material, MaterialHandle(2));
        assert_eq!(draws[0].instance_start, 0);
        assert_eq!(draws[0].instance_count, 2);
        assert_eq!(draws[1].mesh, MeshHandle(11));
        assert_eq!(draws[1].material, MaterialHandle(2));
        assert_eq!(draws[1].instance_start, 2);
        assert_eq!(draws[1].instance_count, 1);
        assert_eq!(instances[0].tint[0], 1.0);
        assert_eq!(instances[1].tint[0], 3.0);
        assert_eq!(instances[2].tint[0], 2.0);
    }

    #[test]
    fn clipping_to_budget_shortens_the_boundary_draw_and_drops_the_rest() {
        let frame = RenderFrame {
            objects: vec![object(1, 10, 2), object(2, 10, 2), object(3, 11, 2), object(4, 12, 2)],
            ..Default::default()
        };
        let (mut instances, mut draws) = frame_instances(&frame);
        assert_eq!((instances.len(), draws.len()), (4, 3));

        let clipped = clip_instances_to_capacity(&mut instances, &mut draws, 3);

        assert!(clipped);
        assert_eq!(instances.len(), 3);
        // First draw (2 instances) survives whole, the boundary draw is shortened to what fits,
        // and the draw past the cut is gone — no draw may index instances beyond the upload.
        assert_eq!(draws.len(), 2);
        assert_eq!((draws[0].instance_start, draws[0].instance_count), (0, 2));
        assert_eq!((draws[1].instance_start, draws[1].instance_count), (2, 1));
        for draw in &draws {
            assert!((draw.instance_start + draw.instance_count) as usize <= instances.len());
        }
    }

    #[test]
    fn clipping_under_budget_is_a_no_op() {
        let frame =
            RenderFrame { objects: vec![object(1, 10, 2), object(2, 11, 2)], ..Default::default() };
        let (mut instances, mut draws) = frame_instances(&frame);

        assert!(!clip_instances_to_capacity(&mut instances, &mut draws, 2));
        assert_eq!(instances.len(), 2);
        assert_eq!(draws.len(), 2);
    }

    fn object(tank_id: u64, mesh: u32, material: u32) -> RenderObject {
        RenderObject {
            tank_id: Some(TankId(tank_id)),
            mesh: MeshHandle(mesh),
            material: MaterialHandle(material),
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [tank_id as f32, 0.0, 0.0, 1.0],
            ],
            tint: [tank_id as f32, 1.0, 1.0],
            dither: [0.0, 1.0],
        }
    }
}
