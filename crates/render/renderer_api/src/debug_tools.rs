#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugToolKind {
    DebugDrawLine,
    DebugDrawBox,
    DebugDrawSphere,
    RaycastVisualizer,
    HitboxArmorOverlay,
    PenetrationNormalOverlay,
    ServerSnapshotInspector,
    EntityInspector,
    GpuFrameTiming,
    CpuProfilerMarkers,
    AssetReload,
    FreeCamera,
    NetworkStatsOverlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugToolPlan;

impl DebugToolPlan {
    pub fn first_week() -> Self {
        Self
    }

    pub fn tools(self) -> [DebugToolKind; 13] {
        [
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
        ]
    }

    pub fn has_tool(self, tool: DebugToolKind) -> bool {
        self.tools().contains(&tool)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaDebugColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl RgbaDebugColor {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugDrawKind {
    Line,
    BoxBounds,
    Sphere,
    Raycast,
    ArmorPlate,
    PenetrationNormal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DebugDrawCommand {
    pub label: String,
    pub kind: DebugDrawKind,
    pub origin: [f32; 3],
    pub vector: [f32; 3],
    pub scalar: f32,
    pub color: RgbaDebugColor,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DebugDrawBatch {
    commands: Vec<DebugDrawCommand>,
}

impl DebugDrawBatch {
    pub fn line(
        &mut self,
        label: impl Into<String>,
        start: [f32; 3],
        end: [f32; 3],
        color: RgbaDebugColor,
    ) {
        self.push(label, DebugDrawKind::Line, start, end, 0.0, color);
    }

    pub fn box_bounds(
        &mut self,
        label: impl Into<String>,
        min: [f32; 3],
        max: [f32; 3],
        color: RgbaDebugColor,
    ) {
        self.push(label, DebugDrawKind::BoxBounds, min, max, 0.0, color);
    }

    pub fn sphere(
        &mut self,
        label: impl Into<String>,
        center: [f32; 3],
        radius_m: f32,
        color: RgbaDebugColor,
    ) {
        self.push(label, DebugDrawKind::Sphere, center, [0.0; 3], radius_m, color);
    }

    pub fn raycast(
        &mut self,
        label: impl Into<String>,
        origin: [f32; 3],
        direction: [f32; 3],
        length_m: f32,
        color: RgbaDebugColor,
    ) {
        self.push(label, DebugDrawKind::Raycast, origin, direction, length_m, color);
    }

    pub fn armor_plate(
        &mut self,
        label: impl Into<String>,
        center: [f32; 3],
        normal: [f32; 3],
        color: RgbaDebugColor,
    ) {
        self.push(label, DebugDrawKind::ArmorPlate, center, normal, 0.0, color);
    }

    pub fn penetration_normal(
        &mut self,
        label: impl Into<String>,
        point: [f32; 3],
        normal: [f32; 3],
        color: RgbaDebugColor,
    ) {
        self.push(label, DebugDrawKind::PenetrationNormal, point, normal, 0.0, color);
    }

    pub fn commands(&self) -> &[DebugDrawCommand] {
        &self.commands
    }

    fn push(
        &mut self,
        label: impl Into<String>,
        kind: DebugDrawKind,
        origin: [f32; 3],
        vector: [f32; 3],
        scalar: f32,
        color: RgbaDebugColor,
    ) {
        self.commands.push(DebugDrawCommand {
            label: label.into(),
            kind,
            origin,
            vector,
            scalar,
            color,
        });
    }
}
