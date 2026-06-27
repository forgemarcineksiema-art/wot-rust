//! Vehicle-pipeline passthroughs for [`super::WindowRenderer`]: registering PBR vehicle meshes and
//! material maps and queuing the vehicle render frame. Split out to keep the core file reviewable.

use renderer_api::{
    MaterialHandle, MeshHandle, RenderFrame, VehicleMaterialFamilies, VehicleMeshAsset,
};

impl super::WindowRenderer {
    pub fn register_vehicle_mesh(&mut self, handle: MeshHandle, mesh: &VehicleMeshAsset) {
        self.scene.register_vehicle_mesh(&self.ctx, handle, mesh);
    }

    pub fn register_vehicle_material(
        &mut self,
        handle: MaterialHandle,
        families: &VehicleMaterialFamilies,
    ) {
        self.scene.register_vehicle_material(&self.ctx, handle, families);
    }

    pub fn set_vehicle_render_frame(&mut self, frame: &RenderFrame) {
        self.scene.set_vehicle_render_frame(&self.ctx, frame);
    }
}
