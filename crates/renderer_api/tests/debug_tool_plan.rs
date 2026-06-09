use renderer_api::{DebugDrawBatch, DebugDrawKind, DebugToolKind, DebugToolPlan, RgbaDebugColor};

#[test]
fn baseline_debug_tool_plan_includes_first_week_vehicle_battle_tools() {
    let plan = DebugToolPlan::first_week();

    for tool in [
        DebugToolKind::DebugDrawLine,
        DebugToolKind::DebugDrawBox,
        DebugToolKind::DebugDrawSphere,
        DebugToolKind::RaycastVisualizer,
        DebugToolKind::HitboxArmorOverlay,
        DebugToolKind::PenetrationNormalOverlay,
        DebugToolKind::ServerSnapshotInspector,
        DebugToolKind::EntityInspector,
        DebugToolKind::GpuFrameTiming,
        DebugToolKind::CpuProfilerMarkers,
        DebugToolKind::AssetReload,
        DebugToolKind::FreeCamera,
        DebugToolKind::NetworkStatsOverlay,
    ] {
        assert!(plan.has_tool(tool), "missing debug tool: {tool:?}");
    }
}

#[test]
fn debug_draw_batch_collects_vehicle_combat_primitives() {
    let mut batch = DebugDrawBatch::default();
    let color = RgbaDebugColor::new(0.2, 0.8, 1.0, 1.0);

    batch.line("aim_line", [0.0, 1.0, 0.0], [10.0, 1.0, 0.0], color);
    batch.box_bounds("tank_hitbox", [-1.0, 0.0, -2.0], [1.0, 2.0, 2.0], color);
    batch.sphere("spotting_radius", [0.0, 0.0, 0.0], 445.0, color);
    batch.raycast("gun_raycast", [0.0, 1.4, 0.0], [1.0, 0.0, 0.0], 1000.0, color);
    batch.armor_plate("upper_glacis", [0.0, 1.0, 2.0], [0.0, 0.4, 0.9], color);
    batch.penetration_normal("penetration_normal", [0.0, 1.0, 2.0], [0.0, 0.0, -1.0], color);

    let kinds = batch.commands().iter().map(|command| command.kind).collect::<Vec<_>>();

    assert_eq!(
        kinds,
        [
            DebugDrawKind::Line,
            DebugDrawKind::BoxBounds,
            DebugDrawKind::Sphere,
            DebugDrawKind::Raycast,
            DebugDrawKind::ArmorPlate,
            DebugDrawKind::PenetrationNormal,
        ]
    );
}
