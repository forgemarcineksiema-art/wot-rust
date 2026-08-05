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
pub use hud::demo_strip::demo_reticle_strip;
pub use hud::font::hud_font_atlas;
pub use hud::{HudVitals, build_hud};
// The UI toolkit surface this used to re-export for the editor moved to `crates/ui/ui_kit`
// when it was extracted (#424); the editor imports it directly and the app-to-app allowlist is
// empty, so the ten forwarding names here had no caller left in the workspace.
pub use look_harness::{render_hangar_review_views, render_review_views};
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
pub use scene_build::grass::{
    GRASS_MESH_HANDLE, GrassSpecies, grass_frame_objects, grass_species_meshes, grass_tuft_mesh,
};
pub use scene_build::grass_cards::grass_card_dressing_mesh;
pub use scene_build::hangar::{
    HERO_FOV_DEGREES, INTERIOR_BACKGROUND, TURNTABLE_TOP_M, hangar_camera_pivot, hangar_scene_mesh,
    hero_orbit_eye,
};
pub use scene_build::review_views::{
    GarageScreen, HangarReviewView, REVIEWED_MAPS, ReviewView, hangar_review_views,
    review_views_for,
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
