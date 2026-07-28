mod aim;
mod app;
mod audio_out;
mod camera;
mod color;
mod fx;
mod hit_indicator;
mod hud;
mod look_harness;
mod loop_policy;
mod predict;
mod render_state;
mod ui_strings;
mod vehicle;

pub use app::run;
pub use app::{garage_overlay, garage_overlay_option_list};
pub use camera::{
    BattleCameraController, BattleCameraEnvironment, BattleCameraInput, BattleCameraMode,
    BattleCameraSettings, CameraObstacle, CameraSubject,
};
pub use fx::TerrainScars;
pub use fx::shell_tracer_vertices;
pub use fx::{append_decal_quads, decal_from_damage_event};
pub use hud::demo::demo_battle_hud;
pub use hud::font::hud_font_atlas;
pub use hud::{HudVitals, build_hud};
// The in-house UI toolkit surface (map-editor D3): the editor draws its panels and text
// with the SAME primitives the battle HUD and garage use — one look, one implementation.
pub use hud::font::layout::{push_text, push_text_right, text_width};
pub use hud::primitives::{push_arc, push_bar, push_hairline, push_panel, push_quad, push_segment};
pub use hud::theme;
pub use look_harness::{LookHarnessError, render_hangar_review_views, render_review_views};
pub use loop_policy::{
    ClientLoopAction, ClientLoopEvent, ClientLoopPhase, FixedTickAccumulator, WinitLoopDriver,
};
pub use render_state::InterpolatedBattleState;
pub use scene_build::battlefield::{
    STATICS_BACKDROP_BUCKET, STATICS_BUCKET_COUNT, assemble_statics_mesh,
    battlefield_ground_and_statics_meshes, battlefield_ground_mesh, battlefield_scene_mesh,
    battlefield_scene_mesh_with_cover_states, battlefield_statics_bucket_mesh,
    battlefield_statics_buckets, battlefield_statics_mesh, battlefield_statics_mesh_with_scars,
    statics_buckets_touched_by_cover, terrain_scene_mesh,
};
pub use scene_build::grass::{GRASS_MESH_HANDLE, grass_frame_objects, grass_tuft_mesh};
pub use scene_build::grass_cards::grass_card_dressing_mesh;
pub use scene_build::hangar::{TURNTABLE_TOP_M, hangar_camera_pivot, hangar_scene_mesh};
pub use scene_build::review_views::{
    HangarReviewView, REVIEWED_MAPS, ReviewView, hangar_review_views, review_views_for,
};
pub use scene_build::terrain_maps::{bake_terrain_ground_maps, terrain_material_set_for};
pub use scene_build::water::battlefield_water_mesh;
pub use vehicle::asset_catalog::VehicleAssetCatalog;
pub use vehicle::asset_render::{
    tank_vehicle_render_objects, tank_vehicle_render_objects_with_variation,
};
pub use vehicle::damage_budget::{DamageBudgetCapture, capture_damage_mesh_budget};
pub use vehicle::damage_worker::DamageMeshBudgetReport;
pub use vehicle::equipment::{EquipmentAnchor, EquipmentPoint, equipment_points};
pub use vehicle::mesh::{append_tank_mesh, tank_scene_mesh};
pub use vehicle::pbr_mesh::{material_role_id, vehicle_submesh_vertices};
pub use vehicle::render_frame::{
    VEHICLE_GUN_OBJECT, VEHICLE_HULL_OBJECT, VEHICLE_TURRET_OBJECT, VehicleRenderFrame,
    armor_damage_instance, render_frame_from_objects, split_pbr_vehicle_render_frame,
    split_pbr_vehicle_render_frame_on_terrain, split_vehicle_render_frame,
};
pub use vehicle::render_objects::VehicleMeshCatalog;
pub use vehicle::render_objects_draw::tank_render_objects;
pub use vehicle::track_ribbon::{TrackRibbon, ribbon_render_objects, thrown_remnant_objects};
pub use vehicle::turret_popoff::TurretPopoff;
pub use vehicle::variation::{
    CamoPattern, DECAL_FADE_S, DecalFrame, DecalKind, HitDecal, MAX_HIT_DECALS, VehicleVariation,
};
