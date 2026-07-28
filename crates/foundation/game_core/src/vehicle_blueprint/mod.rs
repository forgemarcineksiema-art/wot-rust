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

// The seven per-vehicle Rust constructors are TEST-ONLY golden fixtures now: the runtime
// source is the RON file per vehicle (see `source.rs` / `data.rs`).
#[cfg(test)]
mod centurion;
mod data;
mod fittings;
mod hybrid;
#[cfg(test)]
mod is3;
#[cfg(test)]
mod jagdtiger;
pub mod lint;
#[cfg(test)]
mod panther_ii;
mod shape_track;
mod source;
mod t54_hybrid;
mod t54_hybrid_turret;
#[cfg(test)]
mod tiger_i;
#[cfg(test)]
mod tiger_ii;

pub use fittings::{DetailVisual, FittingsVisual};
pub use hybrid::{
    BoxVisual, FenderVisual, GunVisual, HullPlatesVisual, HullVisual, HybridVisual, LoftStation,
    TurretLoftVisual, TurretVisual,
};
pub use shape_track::{ShoePattern, SuspensionKind, TrackShape, WheelFace};
pub use source::{BlueprintFile, parse_blueprint};

/// How the turret/superstructure reads, for both the mesh recipe and the fit tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TurretForm {
    /// A rounded cast dome (Soviet mediums).
    CastDome,
    /// A welded box turret (the prototype medium).
    WeldedBox,
    /// A fixed casemate superstructure (tank destroyers); never traverses.
    Casemate,
}

/// A thin side skirt hung outside the track band (Schürzen, the Centurion's full-length
/// bazooka plates): a spaced standoff layer, honest in both directions — it visually hides the
/// upper run AND the armor volumes bake it as a screen a HEAT jet detonates against early.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkirtShape {
    /// Ground-relative top/bottom of the plate (typically sponson down over the wheel tops).
    pub top_y: f32,
    pub bottom_y: f32,
    /// Hull-local Z span of the plate run (front positive).
    pub front_z: f32,
    pub rear_z: f32,
    /// Air gap between the track's outer face and the skirt — the standoff that kills the jet.
    pub standoff_m: f32,
    /// Plate thickness (m). Real skirts are 5–10 mm sheet; the armor facet derives from this.
    pub thickness_m: f32,
}

/// Hull body shape (visual plate extents) plus the gameplay collision box.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HullShape {
    pub half_len: f32,
    pub half_width: f32,
    pub belly_y: f32,
    pub deck_y: f32,
    /// Glacis slope from vertical (degrees) — shared by the visual plate and the armour facet.
    pub glacis_slope_deg: f32,
    /// Plan-view sweep of a PIKE NOSE (degrees): `0` is a single flat glacis plate; a positive
    /// sweep splits the bow into two plates yawed `±pike_sweep_deg` about the central ridge —
    /// the IS-3 "shchuchy nos". The armor volumes bake one plane per pike face, so angling a
    /// pike-nosed hull genuinely flattens one face toward the shooter.
    pub pike_sweep_deg: f32,
    pub nose_rise: f32,
    /// Rear plate slope from vertical (degrees).
    pub rear_slope_deg: f32,
    /// Half-width of the lower hull tub (between the tracks); the upper hull (`half_width`) is wider
    /// and forms the sponson that overhangs and covers the tops of the road wheels.
    pub lower_half_width: f32,
    /// Y at which the hull steps out from the narrow tub to the wide sponson (≈ wheel axle height).
    pub sponson_y: f32,
    /// Side skirts, when the vehicle carries them (`None` for the bare-track fleet).
    pub skirt: Option<SkirtShape>,
    // Gameplay collision box (the visual hull fits inside it; checked by a consistency test).
    pub hitbox_half_width: f32,
    pub hitbox_half_height: f32,
    pub hitbox_half_length: f32,
    pub hitbox_center_y: f32,
    pub hitbox_turret_min_y: f32,
}

/// Turret/casemate placement and plate geometry, plus mantlet-fit parameters shared by the turret
/// socket and the gun mantlet so they always meet.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// How far the commander's cupola stands PROUD of `roof_y`, authored from the vehicle's
    /// dossier. `None` falls back to the legacy derivation "fill the gap up to the hitbox apex",
    /// which silently launders any roof-height error into the drum: too low a roof grows the
    /// cupola by exactly the deficit, so a silhouette-apex test can never fail on it
    /// (model-logic finding, Tiger I). Author it and the two heights are independently locked.
    #[serde(default)]
    pub cupola_height: Option<f32>,
    pub plan_half_width: f32,
    pub plan_half_length: f32,
    /// Extra gameplay length AHEAD of the plan, for armor that stands proud of the turret face —
    /// a cast collar around the gun is 250 mm of steel a shell must go through, so the hit volume
    /// has to reach it (honesty doctrine: the collision box IS the visual footprint). It extends
    /// the hitbox only; the lofted shape still ends at `plan_half_length`. Zero on a vehicle whose
    /// face carries nothing proud.
    #[serde(default)]
    pub plan_front_pad: f32,
    /// Mantlet radius and its back/front Z on the barrel axis (shared with the gun).
    pub mantlet_radius: f32,
    pub mantlet_back_z: f32,
    pub mantlet_front_z: f32,
    /// How finely a [`TurretForm::CastDome`] is tessellated into tagged armour planes: one
    /// plane per sector, so the impact normal SWEEPS around the casting instead of snapping
    /// between a few flat faces. Ignored by the prism forms, whose walls really are flat.
    ///
    /// Authored per vehicle because it is a property of the casting, not of the engine: the
    /// T-54's low hemispherical dome earns twice the default because its curvature is what the
    /// whole vehicle's armour argument rests on. This lived as `if kind == T54_1951` inside the
    /// shared bake — a content decision hiding in code, where no author could see or change it.
    #[serde(default = "default_turret_sectors")]
    pub sector_count: u32,
}

/// Sectors a cast dome is tessellated into when the blueprint does not say. Twelve puts a plane
/// every 30°, which is enough that a shell meeting the shoulder of a casting does not resolve
/// against the same normal as one meeting its centre.
pub const DEFAULT_TURRET_SECTORS: u32 = 12;

const fn default_turret_sectors() -> u32 {
    DEFAULT_TURRET_SECTORS
}

impl TurretShape {
    /// How far the commander's cupola stands above `roof_y`: the authored dossier height when
    /// the vehicle has one, else the legacy derivation that fills the gap up to the hitbox apex.
    /// Every consumer (mesh recipe, part table) must ask HERE, so the drum and the roof cannot
    /// disagree between the model and its semantic parts.
    pub fn cupola_proud_m(&self, hull: &HullShape) -> f32 {
        self.cupola_height.unwrap_or(hull.hitbox_center_y + hull.hitbox_half_height - self.roof_y)
    }
}

/// Gun placement and barrel shape.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// Full visual dimensions for the hybrid mesh generators. `Some` only for the hybrid benchmark
    /// (T-54); other vehicles bake through the legacy `vehicle_geometry` path and carry `None`.
    pub hybrid: Option<HybridVisual>,
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
            // The pad grows the plan forward ONLY: half the pad on the length, half on the
            // centre, so the rear face stays exactly where the shape puts it.
            self.turret.plan_half_length + self.turret.plan_front_pad * 0.5,
            self.turret.ring_z + self.turret.plan_front_pad * 0.5,
        )
    }

    /// The hybrid visual dimensions, if this vehicle bakes through the hybrid generator path.
    pub fn hybrid(&self) -> Option<&HybridVisual> {
        self.hybrid.as_ref()
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

    /// The T-54 carries full hybrid visual data (the single source the mesh generators read); a
    /// vehicle still on the legacy path carries none.
    #[test]
    fn t54_blueprint_carries_hybrid_visual_data() {
        let t54 = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        let hybrid = t54.hybrid().expect("T-54 is the hybrid benchmark");
        assert_eq!(t54.track.wheel_count, 5, "five road wheels per side");
        assert!(hybrid.turret.budget > 0, "the cast turret meshes to a triangle budget");
    }

    /// SSOT guard for the hybrid path: [`HybridVisual`] may *add* visual-only dimensions, but every
    /// field that merely restates a gameplay-shape number must equal that number — otherwise the
    /// hybrid mesh and the production (legacy `vehicle_geometry`) mesh would describe two different
    /// tanks. This is the test the duplicated literals previously had no guard for: `half_len` had
    /// already drifted (2.90 vs 3.00) and the cupola radius (0.23 vs 0.24) before this locked them.
    ///
    /// The one documented exception is the gun barrel radius: [`GunVisual`] is intentionally distinct
    /// from the legacy [`GunShape`], so it is not asserted here.
    #[test]
    fn t54_hybrid_visual_does_not_reduplicate_blueprint_shape_dimensions() {
        let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        let h = bp.hybrid().expect("T-54 carries hybrid visual data");
        let (hull, track, turret, gun) = (&bp.hull, &bp.track, &bp.turret, &bp.gun);

        let same = |label: &str, hybrid: f32, shape: f32| {
            assert!(
                (hybrid - shape).abs() < 1.0e-6,
                "{label}: hybrid {hybrid} must equal the blueprint shape {shape} (no second source)"
            );
        };

        // Hull body extents.
        same("hull.half_width", h.hull.half_width, hull.half_width);
        same("hull.belly_y", h.hull.belly_y, hull.belly_y);
        same("hull.half_len", h.hull.half_len, hull.half_len);
        same("hull.roof_y", h.hull.roof_y, hull.deck_y);

        // The fender shelf rides the blueprint track lane (the running gear itself has no hybrid
        // copy — the animated path reads `TrackShape` directly).
        same("fender.side_x", h.fender.side_x, track.center_x);

        // Turret machined planes and the commander's cupola placement (carried in three places).
        same("turret.roof_plane_y", h.turret.roof_plane_y, turret.roof_y);
        same("turret.ring_plane_y", h.turret.ring_plane_y, turret.ring_y);
        same("turret.cupola_radius", h.turret.cupola_radius, turret.cupola_radius);
        same("turret.cupola_center.x", h.turret.cupola_center.x, turret.cupola_x);
        same("turret.cupola_center.z", h.turret.cupola_center.z, turret.cupola_z);
        same("fittings.cupola_hatch_center.x", h.fittings.cupola_hatch_center.x, turret.cupola_x);
        same("fittings.cupola_hatch_center.z", h.fittings.cupola_hatch_center.z, turret.cupola_z);

        // The PRODUCTION turret. Everything above guards the metaball `TurretVisual`, which the
        // game no longer meshes — the shipped casting is this loft, and until now not one of its
        // numbers was tied to the blueprint. That is precisely where a shape edit would drift:
        // raise `roof_y` in the RON and the visible dome would have stayed where it was.
        let loft = &h.turret_loft;
        let stations = &loft.stations;
        let first = stations.first().expect("the loft starts at the ring");
        let last = stations.last().expect("the loft ends at the roof");
        same("turret_loft.first_station.y", first.y, turret.ring_y);
        same("turret_loft.first_station.z_center", first.z_center, turret.ring_z);
        same("turret_loft.last_station.y", last.y, turret.roof_y);
        same("turret_loft.last_station.half_width", last.half_width, turret.roof_radius);
        same("turret_loft.cupola_center.x", loft.cupola_center.x, turret.cupola_x);
        same("turret_loft.cupola_center.z", loft.cupola_center.z, turret.cupola_z);
        same("turret_loft.cupola_radius", loft.cupola_radius, turret.cupola_radius);
        // The cheeks flank the gun at the fire line, and the embrasure is centred ON it.
        same("turret_loft.cheek_y", loft.cheek_y, gun.trunnion_y);
        same("turret_loft.embrasure_y", loft.embrasure_y, gun.trunnion_y);

        // Containment, not equality: the casting may be narrower than its authored plan box and
        // MUST overhang the ring (that overhang is the T-54's armour argument).
        let widest = stations.iter().map(|station| station.half_width).fold(f32::MIN, f32::max);
        assert!(
            widest <= turret.plan_half_width + 1.0e-6,
            "the casting ({widest}) must fit inside its authored plan half-width ({})",
            turret.plan_half_width
        );
        assert!(
            first.half_width > turret.ring_radius,
            "the casting ({}) must overhang the ring ({}) — the undercut IS the armour",
            first.half_width,
            turret.ring_radius
        );
    }

    /// Factory detailing is visual-only: it adds nothing to the gameplay-bearing data (hitbox,
    /// mounts, armour all still resolve unchanged from the shape source) and every detail descriptor
    /// sits inside the already-validated hull/turret volumes. This is the Task-2 guard that the new
    /// grille/exhaust/periscope/fender-lip/weld greeble cannot quietly grow the tank or its hitbox.
    #[test]
    fn t54_detail_fittings_are_visual_only_and_fit_inside_validated_volumes() {
        let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        // Adding DetailVisual leaves the collision hitbox, mount frames and armour untouched.
        assert_eq!(bp.hitbox(), HitboxProfile::for_vehicle(VehicleKind::T54_1951));
        assert_eq!(bp.mount_frames(), MountFrames::for_vehicle(VehicleKind::T54_1951));

        let h = bp.hybrid().expect("hybrid visual");
        let d = &h.detail;
        let hull = &bp.hull;
        let inside_hull = |p: Vec3| {
            p.x.abs() <= hull.hitbox_half_width + 1.0e-3
                && p.z.abs() <= hull.hitbox_half_length + 1.0e-3
                && (p.y - hull.hitbox_center_y).abs() <= hull.hitbox_half_height + 1.0e-3
        };
        assert!(inside_hull(d.grille_center), "deck grille sits within the hull collision volume");
        assert!(
            inside_hull(d.exhaust_center),
            "exhaust cover sits within the hull collision volume"
        );
        // Turret periscopes sit inside the cast turret's meshing box and below its cupola-hatch apex,
        // so they never raise the silhouette or the turret-height proportion.
        let t = &h.turret;
        assert!(d.periscope_center.x.abs() + d.periscope_half.x < t.bbox_max.x);
        assert!(d.periscope_center.z > t.bbox_min.z && d.periscope_center.z < t.bbox_max.z);
        assert!(
            d.periscope_center.y + d.periscope_half.y < h.fittings.cupola_hatch_center.y,
            "periscopes stay below the turret apex"
        );
        // Restrained, factory-clean detail: lips and weld beads are small.
        assert!(d.fender_lip_drop < 0.20 && d.weld_seam_half_thickness < 0.05);

        // The fender lip curtain must clear the track's top run (wrap top + link seat + link
        // body, ~0.08 over the wrap): a lip that dips into the belt band has the scrolling
        // shoes cutting through it on every frame.
        let lip_bottom = h.fender.center_y - h.fender.half.y - d.fender_lip_drop;
        let belt_top = bp.track.end_y + bp.track.end_radius + 0.08;
        assert!(
            lip_bottom > belt_top,
            "fender lip bottom {lip_bottom} must clear the belt's top run {belt_top}"
        );

        // The grille must ride proud of the engine-deck top, never coplanar with it — a coplanar
        // top z-fights the slats into a flickering mess.
        let deck_top = h.deck.center.y + h.deck.half.y;
        assert!(
            d.grille_center.y + d.grille_half.y > deck_top + 0.01,
            "deck grille top {} must clear the engine-deck top {deck_top} to avoid z-fighting",
            d.grille_center.y + d.grille_half.y
        );
    }
}
