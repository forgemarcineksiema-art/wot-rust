//! Runtime variation layer (Armored Vehicle Forge layer 6).
//!
//! The baked benchmark owns the *model*: geometry, UVs, and material maps. This layer owns the
//! per-instance *state* a tank carries in battle — hit decals, dirt/snow/camo overlays, broken
//! tracks, destroyed modules, and where optional equipment can attach. It deliberately does **not**
//! re-model anything: it only produces parameters (tints, decal lists, attachment transforms) that
//! the renderer layers on top of the shared baked asset. The layer is pure and deterministic so it
//! can be unit-tested without a GPU.

use std::sync::Arc;

use game_core::{ModuleSlot, TrackDamageMask, TrackSide};
use net::TankSnapshot;
use vehicle_geometry::DecalPatch;

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

/// What a shell left on the plate — the mark's shape, colour, and lifetime all key off this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecalKind {
    /// A through-hole: black, hard-edged, and PERMANENT — battle scars stay for the battle.
    Penetration,
    /// A non-penetrating smack: bared metal and scorched paint that weathers back over.
    Scuff,
    /// A ricochet gouge: a long scrape along the plate that fades like a scuff.
    Gouge,
}

/// Which rotating frame the mark lives in: hull marks ride the hull, turret marks traverse
/// with the turret (a mantlet hit must not slide off when the turret turns).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecalFrame {
    Hull,
    Turret,
    Mantlet,
}

/// A single impact mark in its local frame; non-permanent kinds age toward [`DECAL_FADE_S`].
#[derive(Debug, Clone, PartialEq)]
pub struct HitDecal {
    pub local_position: [f32; 3],
    /// Outward plate normal in the same local frame — the mark is drawn flat on this plane.
    pub local_normal: [f32; 3],
    pub radius: f32,
    pub age_s: f32,
    pub kind: DecalKind,
    pub frame: DecalFrame,
    /// A conformal patch (mesh triangles clipped to the mark's footprint, in the same local frame)
    /// for a penetration hole that wraps a curved casting instead of hovering across its chord.
    /// `None` for flat marks and for hits that fell back off the mesh — those draw the flat quad.
    pub patch: Option<Arc<DecalPatch>>,
}

impl HitDecal {
    /// 1.0 when fresh; scuffs and gouges fade to 0.0 at [`DECAL_FADE_S`], penetration holes
    /// never fade (they can still be recycled by [`MAX_HIT_DECALS`]).
    pub fn opacity(&self) -> f32 {
        match self.kind {
            DecalKind::Penetration => 1.0,
            DecalKind::Scuff | DecalKind::Gouge => {
                (1.0 - self.age_s / DECAL_FADE_S).clamp(0.0, 1.0)
            }
        }
    }
}

/// Per-instance runtime state applied on top of the shared baked asset.
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleVariation {
    dirt: f32,
    snow: f32,
    camo: CamoPattern,
    decals: Vec<HitDecal>,
    track_damage_mask: TrackDamageMask,
    destroyed_modules_mask: u8,
}

impl Default for VehicleVariation {
    fn default() -> Self {
        Self {
            dirt: 0.0,
            snow: 0.0,
            camo: CamoPattern::None,
            decals: Vec::new(),
            track_damage_mask: TrackDamageMask::healthy(),
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
            track_damage_mask: if mask & ModuleSlot::Suspension.destroyed_mask_bit() != 0 {
                TrackDamageMask::BOTH
            } else {
                TrackDamageMask::from_bits(snapshot.track_damage_mask)
            },
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
        self.track_damage_mask =
            if broken { TrackDamageMask::BOTH } else { TrackDamageMask::healthy() };
    }

    pub fn tracks_broken(&self) -> bool {
        self.track_damage_mask.all_broken()
    }

    pub fn left_track_broken(&self) -> bool {
        self.track_damage_mask.is_broken(TrackSide::Left)
    }

    pub fn right_track_broken(&self) -> bool {
        self.track_damage_mask.is_broken(TrackSide::Right)
    }

    pub fn decals(&self) -> &[HitDecal] {
        &self.decals
    }

    /// Record a fresh impact mark, recycling the oldest decal once [`MAX_HIT_DECALS`] is reached.
    pub fn record_hit(&mut self, decal: HitDecal) {
        if self.decals.len() >= MAX_HIT_DECALS {
            self.decals.remove(0);
        }
        self.decals.push(HitDecal { radius: decal.radius.max(0.0), age_s: 0.0, ..decal });
    }

    /// Age decals and drop any that have fully faded (penetration holes never do).
    pub fn tick(&mut self, dt_s: f32) {
        for decal in &mut self.decals {
            decal.age_s += dt_s.max(0.0);
        }
        self.decals.retain(|decal| decal.opacity() > 0.0);
    }

    /// Adopt the replicated damage state from the latest snapshot while keeping the accumulated
    /// battle scars — the persistent per-tank instance calls this instead of `from_snapshot`.
    pub fn sync_from_snapshot(&mut self, snapshot: &TankSnapshot) {
        let fresh = Self::from_snapshot(snapshot);
        self.track_damage_mask = fresh.track_damage_mask;
        self.destroyed_modules_mask = fresh.destroyed_modules_mask;
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
            c[i] = blend(c[i], mud[i], self.dirt * 0.6);
            c[i] = blend(c[i], 0.92, self.snow * 0.7);
        }
        c
    }

    /// The whole-vehicle tint for a knocked-out hull: burnt out, carbon-dark. This is the ONLY
    /// state that recolours a whole tank — live damage speaks through local cues (decals, smoke),
    /// never through a hull-wide palette swap.
    pub fn wreck_tint(base: [f32; 3]) -> [f32; 3] {
        [base[0] * 0.30, base[1] * 0.28, base[2] * 0.27]
    }
}

fn camo_tint(camo: CamoPattern, base: [f32; 3]) -> [f32; 3] {
    match camo {
        CamoPattern::None => base,
        CamoPattern::Summer => [base[0] * 0.78, base[1] * 0.92, base[2] * 0.70],
        CamoPattern::Winter => {
            [blend(base[0], 0.9, 0.5), blend(base[1], 0.92, 0.5), blend(base[2], 0.95, 0.5)]
        }
        CamoPattern::Desert => {
            [blend(base[0], 0.82, 0.5), blend(base[1], 0.72, 0.5), blend(base[2], 0.50, 0.5)]
        }
    }
}

fn blend(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
