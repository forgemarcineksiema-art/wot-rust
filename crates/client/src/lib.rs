mod aim;
mod app;
mod camera;
mod color;
mod health_bar;
mod hit_indicator;
mod hud;
mod hud_font;
mod hud_icons;
mod hud_number;
mod loop_policy;
mod predict;
mod render_state;
mod reticle;
mod reticle_sweep;
mod scene;
mod tank_mesh;
mod vehicle;

pub use app::garage_overlay;
pub use app::run;
pub use camera::{
    BattleCameraController, BattleCameraEnvironment, BattleCameraInput, BattleCameraMode,
    BattleCameraSettings, CameraObstacle, CameraSubject,
};
pub use hud::{HudVitals, build_hud};
pub use hud_font::hud_font_atlas;
pub use loop_policy::{
    ClientLoopAction, ClientLoopEvent, ClientLoopPhase, FixedTickAccumulator, WinitLoopDriver,
};
pub use render_state::InterpolatedBattleState;
pub use scene::battlefield::{battlefield_scene_mesh, terrain_scene_mesh};
pub use scene::hangar::{TURNTABLE_TOP_M, hangar_camera_pivot, hangar_scene_mesh};
pub use tank_mesh::append_shell_markers;
pub use vehicle::asset_catalog::{
    VehicleAssetCatalog, tank_vehicle_render_objects, tank_vehicle_render_objects_with_variation,
};
pub use vehicle::equipment::{EquipmentAnchor, EquipmentPoint, equipment_points};
pub use vehicle::mesh::{append_tank_mesh, tank_scene_mesh};
pub use vehicle::pbr_mesh::{material_role_id, vehicle_submesh_vertices};
pub use vehicle::render_frame::{
    VehicleRenderFrame, render_frame_from_objects, split_pbr_vehicle_render_frame,
    split_vehicle_render_frame,
};
pub use vehicle::render_objects::{VehicleMeshCatalog, tank_render_objects};
pub use vehicle::variation::{
    CamoPattern, DECAL_FADE_S, HitDecal, MAX_HIT_DECALS, VehicleVariation,
};
