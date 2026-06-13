//! The single parametric source of truth for a vehicle's *shape*. One [`VehicleBlueprint`] drives,
//! together, the collision hitbox, the mount frames, and the armour facet slopes — and (via
//! `vehicle_geometry`, which reads the same blueprint) the visual mesh. Because all of these read
//! one struct, the silhouette, what you hit, and what the armour model resolves cannot drift apart.
//!
//! Armour *thicknesses* still come from the installed modules; the blueprint contributes only the
//! plate angles and weakspot multipliers (the geometry), so a sloped glacis you can see is the same
//! angle the penetration model uses.
//!
//! Migration is per-vehicle: [`VehicleBlueprint::for_vehicle`] returns `Some` for vehicles already
//! moved onto the blueprint and `None` for the rest, which stay on the legacy hand-authored path
//! until they are migrated too.

use glam::Vec3;

use crate::{HitboxProfile, MountFrame, MountFrames, VehicleKind};

mod data;

/// How the turret/superstructure reads, for both the mesh recipe and the fit tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurretForm {
    /// A rounded cast dome (Soviet mediums).
    CastDome,
    /// A welded box turret (the prototype medium).
    WeldedBox,
    /// A fixed casemate superstructure (tank destroyers); never traverses.
    Casemate,
}

/// Hull body shape (visual plate extents) plus the gameplay collision box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HullShape {
    pub half_len: f32,
    pub half_width: f32,
    pub belly_y: f32,
    pub deck_y: f32,
    /// Glacis slope from vertical (degrees) — shared by the visual plate and the armour facet.
    pub glacis_slope_deg: f32,
    pub nose_rise: f32,
    /// Rear plate slope from vertical (degrees).
    pub rear_slope_deg: f32,
    /// How far the fender/sponson overhangs past the hull side, per side.
    pub sponson_overhang: f32,
    // Gameplay collision box (the visual hull fits inside it; checked by a consistency test).
    pub hitbox_half_width: f32,
    pub hitbox_half_height: f32,
    pub hitbox_half_length: f32,
    pub hitbox_center_y: f32,
    pub hitbox_turret_min_y: f32,
}

/// Running-gear shape: the wrapped track belt, road wheels, drive sprocket, and idler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackShape {
    pub center_x: f32,
    pub belt_half_thickness: f32,
    pub top_y: f32,
    pub bottom_y: f32,
    pub wheel_radius: f32,
    pub wheel_count: usize,
    pub wheel_first_z: f32,
    pub wheel_last_z: f32,
    pub end_radius: f32,
    pub inner_x: f32,
    pub outer_x: f32,
    pub segments: usize,
}

/// Turret/casemate placement and plate geometry, plus mantlet-fit parameters shared by the turret
/// socket and the gun mantlet so they always meet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurretShape {
    pub form: TurretForm,
    pub ring_y: f32,
    pub ring_z: f32,
    pub ring_radius: f32,
    pub base_radius: f32,
    pub roof_radius: f32,
    pub roof_y: f32,
    pub front_slope_deg: f32,
    pub side_slope_deg: f32,
    pub rear_slope_deg: f32,
    pub cupola_x: f32,
    pub cupola_z: f32,
    pub cupola_radius: f32,
    pub plan_half_width: f32,
    pub plan_half_length: f32,
    /// Mantlet radius and its back/front Z on the barrel axis (shared with the gun).
    pub mantlet_radius: f32,
    pub mantlet_back_z: f32,
    pub mantlet_front_z: f32,
}

/// Gun placement and barrel shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GunShape {
    pub trunnion_y: f32,
    pub trunnion_z: f32,
    pub muzzle_z: f32,
    pub barrel_radius: f32,
    pub evacuator: Option<(f32, f32)>,
    pub muzzle_brake: Option<f32>,
    pub segments: usize,
}

/// Per-facing armour geometry: plate slope (degrees from vertical) and weakspot multiplier. The
/// thickness is supplied by the installed module; this is the shape contribution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmorShape {
    pub hull_front: (f32, f32),
    pub hull_side: (f32, f32),
    pub hull_rear: (f32, f32),
    pub turret_front: (f32, f32),
    pub turret_side: (f32, f32),
    pub turret_rear: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleBlueprint {
    pub kind: VehicleKind,
    pub hull: HullShape,
    pub track: TrackShape,
    pub turret: TurretShape,
    pub gun: GunShape,
    pub armor: ArmorShape,
}

impl VehicleBlueprint {
    /// The blueprint for `kind`, or `None` if it has not been migrated off the legacy path yet.
    pub fn for_vehicle(kind: VehicleKind) -> Option<Self> {
        data::blueprint(kind)
    }

    /// The collision hitbox derived from the hull and turret shape.
    pub fn hitbox(&self) -> HitboxProfile {
        HitboxProfile::new(
            self.hull.hitbox_half_width,
            self.hull.hitbox_half_height,
            self.hull.hitbox_half_length,
            self.hull.hitbox_center_y,
            self.hull.hitbox_turret_min_y,
        )
        .with_turret_plan(
            self.turret.plan_half_width,
            self.turret.plan_half_length,
            self.turret.ring_z,
        )
    }

    /// The mount frames (turret ring, gun trunnion, muzzle) derived from the shape.
    pub fn mount_frames(&self) -> MountFrames {
        MountFrames {
            turret_ring: MountFrame::new(Vec3::new(0.0, self.turret.ring_y, self.turret.ring_z)),
            gun_trunnion: MountFrame::new(Vec3::new(0.0, self.gun.trunnion_y, self.gun.trunnion_z)),
            muzzle: MountFrame::new(Vec3::new(0.0, self.gun.trunnion_y, self.gun.muzzle_z)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The migrated T-54's hitbox, mounts, and armour slopes all flow from its blueprint — the
    /// single source of truth. If any consumer stops reading it, one of these diverges.
    #[test]
    fn t54_blueprint_is_the_single_source_for_hitbox_mounts_and_armor() {
        let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        assert_eq!(bp.hitbox(), HitboxProfile::for_vehicle(VehicleKind::T54_1951));
        assert_eq!(bp.mount_frames(), MountFrames::for_vehicle(VehicleKind::T54_1951));

        // The armour facet the penetration model uses carries the same plate slope the visible
        // glacis is built from.
        let spec = VehicleKind::T54_1951.spec();
        assert!(
            (spec.hull.facets.hull_front.slope_degrees - bp.armor.hull_front.0).abs() < 1.0e-6,
            "glacis armour slope must equal the blueprint glacis angle"
        );
    }
}
