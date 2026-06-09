mod aim;
mod app;
mod camera;
mod color;
mod health_bar;
mod hit_indicator;
mod hud;
mod hud_font;
mod hud_number;
mod loop_policy;
mod predict;
mod render_state;
mod reticle;
mod reticle_sweep;
mod scene_mesh;
mod tank_mesh;
mod vehicle_geometry_mesh;
mod vehicle_mesh;
mod vehicle_render_frame;
mod vehicle_render_objects;

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
pub use scene_mesh::terrain_scene_mesh;
pub use tank_mesh::append_shell_markers;
pub use vehicle_mesh::{append_tank_mesh, tank_scene_mesh};
pub use vehicle_render_frame::{
    VehicleRenderFrame, render_frame_from_objects, split_vehicle_render_frame,
};
pub use vehicle_render_objects::{VehicleMeshCatalog, tank_render_objects};
