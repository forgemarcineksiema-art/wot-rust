//! Runtime variation layer (Armored Vehicle Forge layer 6).
//!
//! The baked benchmark owns the *model*: geometry, UVs, and material maps. This layer owns the
//! per-instance *state* a tank carries in battle — hit decals, dirt/snow/camo overlays, broken
//! tracks, destroyed modules, and where optional equipment can attach. It deliberately does **not**
//! re-model anything: it only produces parameters (tints, decal lists, attachment transforms) that
//! the renderer layers on top of the shared baked asset. The layer is pure and deterministic so it
//! can be unit-tested without a GPU.

use game_core::{ModuleSlot, VehicleKind};
use net::TankSnapshot;
use vehicle_geometry::{SubmeshKind, bake_vehicle};

/// Hit decals beyond this are the oldest and get recycled — keeps per-tank state bounded.
pub const MAX_HIT_DECALS: usize = 16;
/// Seconds a hit decal takes to fully fade and be dropped.
pub const DECAL_FADE_S: f32 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CamoPattern {
    None,
    Summer,
    Winter,
    Desert,
}

/// A single impact mark in the hull/turret local space, ageing toward [`DECAL_FADE_S`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitDecal {
    pub local_position: [f32; 3],
    pub radius: f32,
    pub age_s: f32,
}

impl HitDecal {
    /// 1.0 when fresh, fading to 0.0 at [`DECAL_FADE_S`].
    pub fn opacity(&self) -> f32 {
        (1.0 - self.age_s / DECAL_FADE_S).clamp(0.0, 1.0)
    }
}

/// Where an optional stowage/equipment item rides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentAnchor {
    Hull,
    Turret,
}

/// A named attachment slot for optional equipment (spare track, toolbox, antenna, …). Positions are
/// derived from the baked geometry bounds so they sit on the real hull/turret, not on magic values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquipmentPoint {
    pub name: &'static str,
    pub anchor: EquipmentAnchor,
    pub local_position: [f32; 3],
}

/// Per-instance runtime state applied on top of the shared baked asset.
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleVariation {
    dirt: f32,
    snow: f32,
    camo: CamoPattern,
    decals: Vec<HitDecal>,
    tracks_broken: bool,
    destroyed_modules_mask: u8,
}

impl Default for VehicleVariation {
    fn default() -> Self {
        Self {
            dirt: 0.0,
            snow: 0.0,
            camo: CamoPattern::None,
            decals: Vec::new(),
            tracks_broken: false,
            destroyed_modules_mask: 0,
        }
    }
}

impl VehicleVariation {
    /// Derive the cosmetic state implied by a network snapshot: destroyed modules and — since the
    /// suspension drives the running gear — whether the tracks read as broken.
    pub fn from_snapshot(snapshot: &TankSnapshot) -> Self {
        let mask = snapshot.destroyed_modules_mask;
        Self {
            tracks_broken: mask & ModuleSlot::Suspension.destroyed_mask_bit() != 0,
            destroyed_modules_mask: mask,
            ..Self::default()
        }
    }

    pub fn with_camo(mut self, camo: CamoPattern) -> Self {
        self.camo = camo;
        self
    }

    pub fn with_dirt(mut self, dirt: f32) -> Self {
        self.dirt = dirt.clamp(0.0, 1.0);
        self
    }

    pub fn with_snow(mut self, snow: f32) -> Self {
        self.snow = snow.clamp(0.0, 1.0);
        self
    }

    pub fn set_tracks_broken(&mut self, broken: bool) {
        self.tracks_broken = broken;
    }

    pub fn tracks_broken(&self) -> bool {
        self.tracks_broken
    }

    pub fn decals(&self) -> &[HitDecal] {
        &self.decals
    }

    /// Record a fresh impact mark, recycling the oldest decal once [`MAX_HIT_DECALS`] is reached.
    pub fn record_hit(&mut self, local_position: [f32; 3], radius: f32) {
        if self.decals.len() >= MAX_HIT_DECALS {
            self.decals.remove(0);
        }
        self.decals.push(HitDecal { local_position, radius: radius.max(0.0), age_s: 0.0 });
    }

    /// Age decals and drop any that have fully faded.
    pub fn tick(&mut self, dt_s: f32) {
        for decal in &mut self.decals {
            decal.age_s += dt_s.max(0.0);
        }
        self.decals.retain(|decal| decal.opacity() > 0.0);
    }

    /// Whether a given module slot is destroyed.
    pub fn module_destroyed(&self, slot: ModuleSlot) -> bool {
        self.destroyed_modules_mask & slot.destroyed_mask_bit() != 0
    }

    /// The base hull colour after camo + dirt + snow overlays. Defaults (no camo, no weather) return
    /// `base` unchanged, so a clean tank looks exactly like its baked albedo tint.
    pub fn surface_tint(&self, base: [f32; 3]) -> [f32; 3] {
        let mut c = camo_tint(self.camo, base);
        let mud = [0.34, 0.28, 0.20];
        for i in 0..3 {
            c[i] = lerp(c[i], mud[i], self.dirt * 0.6);
            c[i] = lerp(c[i], 0.92, self.snow * 0.7);
        }
        c
    }

    /// The tint for a submesh whose listed modules can knock it out (e.g. a dead engine darkens the
    /// rear hull). Applied on top of [`surface_tint`].
    pub fn module_tint(&self, base: [f32; 3], slots: &[ModuleSlot]) -> [f32; 3] {
        if slots.iter().any(|slot| self.module_destroyed(*slot)) {
            [base[0] * 0.42, base[1] * 0.40, base[2] * 0.36]
        } else {
            base
        }
    }
}

fn camo_tint(camo: CamoPattern, base: [f32; 3]) -> [f32; 3] {
    match camo {
        CamoPattern::None => base,
        CamoPattern::Summer => [base[0] * 0.78, base[1] * 0.92, base[2] * 0.70],
        CamoPattern::Winter => [lerp(base[0], 0.9, 0.5), lerp(base[1], 0.92, 0.5), lerp(base[2], 0.95, 0.5)],
        CamoPattern::Desert => [lerp(base[0], 0.82, 0.5), lerp(base[1], 0.72, 0.5), lerp(base[2], 0.50, 0.5)],
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The standard equipment attachment points for `kind`, derived from its baked hull and turret
/// bounds: spare track on the glacis, a toolbox on the right fender, a stowage log on the rear hull,
/// and a radio antenna at the turret rear.
pub fn equipment_points(kind: VehicleKind) -> Vec<EquipmentPoint> {
    let Ok(baked) = bake_vehicle(kind) else {
        return Vec::new();
    };
    let hull = baked.submesh(SubmeshKind::Hull).and_then(|s| s.mesh.bounds());
    let turret = baked.submesh(SubmeshKind::Turret).and_then(|s| s.mesh.bounds());
    let mut points = Vec::new();
    if let Some(h) = hull {
        points.push(EquipmentPoint {
            name: "spare_track",
            anchor: EquipmentAnchor::Hull,
            local_position: [0.0, mid(h.min.y, h.max.y), h.max.z * 0.92],
        });
        points.push(EquipmentPoint {
            name: "toolbox",
            anchor: EquipmentAnchor::Hull,
            local_position: [h.max.x * 0.96, mid(h.min.y, h.max.y), h.min.z * 0.2],
        });
        points.push(EquipmentPoint {
            name: "stowage_log",
            anchor: EquipmentAnchor::Hull,
            local_position: [0.0, mid(h.min.y, h.max.y), h.min.z * 0.96],
        });
    }
    if let Some(t) = turret {
        points.push(EquipmentPoint {
            name: "antenna",
            anchor: EquipmentAnchor::Turret,
            local_position: [t.min.x * 0.7, t.max.y, t.min.z * 0.8],
        });
    }
    points
}

fn mid(a: f32, b: f32) -> f32 {
    0.5 * (a + b)
}
