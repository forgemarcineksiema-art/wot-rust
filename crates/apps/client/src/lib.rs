mod aim;
mod app;
mod audio_out;
mod camera;
mod color;
mod fx;
mod hit_indicator;
mod hud;
mod loop_policy;
mod predict;
mod render_state;
mod scene;
mod tank_mesh;
mod ui_strings;
mod vehicle;

pub use app::run;
pub use app::{garage_overlay, garage_overlay_option_list};
pub use camera::{
    BattleCameraController, BattleCameraEnvironment, BattleCameraInput, BattleCameraMode,
    BattleCameraSettings, CameraObstacle, CameraSubject,
};
pub use fx::shell_tracer_vertices;
pub use fx::{append_decal_quads, decal_from_damage_event};
pub use hud::demo::demo_battle_hud;
pub use hud::font::hud_font_atlas;
pub use hud::{HudVitals, build_hud};
pub use loop_policy::{
    ClientLoopAction, ClientLoopEvent, ClientLoopPhase, FixedTickAccumulator, WinitLoopDriver,
};
pub use render_state::InterpolatedBattleState;
pub use scene::battlefield::{
    battlefield_scene_mesh, battlefield_scene_mesh_with_cover_states, terrain_scene_mesh,
};
pub use scene::hangar::{TURNTABLE_TOP_M, hangar_camera_pivot, hangar_scene_mesh};
pub use scene::water::battlefield_water_mesh;
pub use vehicle::asset_catalog::VehicleAssetCatalog;
pub use vehicle::asset_render::{
    tank_vehicle_render_objects, tank_vehicle_render_objects_with_variation,
};
pub use vehicle::equipment::{EquipmentAnchor, EquipmentPoint, equipment_points};
pub use vehicle::mesh::{append_tank_mesh, tank_scene_mesh};
pub use vehicle::pbr_mesh::{material_role_id, vehicle_submesh_vertices};
pub use vehicle::render_frame::{
    VehicleRenderFrame, render_frame_from_objects, split_pbr_vehicle_render_frame,
    split_pbr_vehicle_render_frame_on_terrain, split_vehicle_render_frame,
};
pub use vehicle::render_objects::VehicleMeshCatalog;
pub use vehicle::render_objects_draw::tank_render_objects;
pub use vehicle::turret_popoff::TurretPopoff;
pub use vehicle::variation::{
    CamoPattern, DECAL_FADE_S, DecalFrame, DecalKind, HitDecal, MAX_HIT_DECALS, VehicleVariation,
};
