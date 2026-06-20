//! Authoritative *visual* dimensions for the hybrid generator path (currently the T-54 benchmark).
//!
//! The flat [`VehicleBlueprint`](super::VehicleBlueprint) fields carry the gameplay shape (hitbox,
//! mounts, armour slopes). [`HybridVisual`] carries everything the hybrid mesh generators
//! (`solid`, `sdf_mesh`, `revolve`) need on top of that — the convex hull block, the cast-turret SDF
//! composition, the barrel and mantlet profiles, the engine deck, the fenders, and the running gear.
//!
//! These types live here, in the lowest crate, so the generators read one source of truth rather
//! than each holding its own copy of a dimension. A generator takes the relevant sub-struct by
//! reference; nothing outside this struct is allowed to invent a T-54 dimension.

use glam::Vec3;

/// The convex hull block plus its two-plate front. The plate slopes (glacis/side/rear) are *not*
/// stored here — they are read from [`ArmorShape`](super::ArmorShape) so the visible rake is the same
/// number the penetration model uses ("what you see is what you shoot").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HullVisual {
    pub half_width: f32,
    pub belly_y: f32,
    pub roof_y: f32,
    pub half_len: f32,
    /// Plane offset (distance from origin along the glacis normal) of the upper glacis plate.
    pub glacis_offset: f32,
    /// Lower nose plate: bevels the bottom-front edge into the T-54's two-plate front.
    pub nose_normal: Vec3,
    pub nose_offset: f32,
}

/// Visual parameters for the multi-plate hull (Stage 3). The plate *extents* come from the gameplay
/// [`HullShape`](super::HullShape) — the lower tub width, the sponson step, the deck height, the hull
/// length — and the plate *slopes* from [`ArmorShape`](super::ArmorShape), so the visible hull is the
/// reconciled-to-gameplay form. These fields add only what the shape model does not already carry:
/// where the two-plate front folds, and the small thickness/bevel/seam cues that make the plates read
/// as plates rather than one block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HullPlatesVisual {
    /// Z where the upper glacis meets the lower nose plate, taken at the sponson step height — the
    /// fold line of the T-54 two-plate front.
    pub glacis_base_z: f32,
    /// Z of the lower nose plate at the belly: tucked back behind the fold, so the nose rakes under.
    pub nose_base_z: f32,
    /// Chamfer on the deck's front edge where it meets the glacis, so the lip reads as plate.
    pub deck_bevel: f32,
    /// How far the wide sponson sits proud of the lower tub at the side (visual plate seam).
    pub sponson_overhang: f32,
}

/// The cast turret as a Surface-Nets SDF: two offset spheres for the flattened dome, a seating ring,
/// flat roof/ring planes, the commander's cupola, and the recessed mantlet socket. Positions are in
/// vehicle-local space; the gun's moving mantlet belongs to the gun submesh, not the casting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurretVisual {
    pub dome_radius: f32,
    pub dome_front: Vec3,
    pub dome_rear: Vec3,
    /// Rear dome radius, smaller than the front, so the casting tapers to a lower narrower bustle.
    pub dome_rear_radius: f32,
    pub dome_blend: f32,
    /// Front cheek bulge flanking the mantlet (right side; mirrored to the left). The cast cheeks are
    /// the T-54 turret's signature front mass, distinct from the rounded rear.
    pub cheek_radius: f32,
    pub cheek_center: Vec3,
    pub cheek_blend: f32,
    pub ring_radius: f32,
    pub ring_half_height: f32,
    pub ring_center: Vec3,
    pub ring_blend: f32,
    /// Flat machined planes capping the casting: roof (upper) and ring seat (lower).
    pub roof_plane_y: f32,
    pub ring_plane_y: f32,
    pub cupola_radius: f32,
    pub cupola_half_height: f32,
    pub cupola_center: Vec3,
    pub cupola_blend: f32,
    pub socket_radius: f32,
    pub socket_center: Vec3,
    pub socket_blend: f32,
    /// Tight world-space meshing box for the casting.
    pub bbox_min: Vec3,
    pub bbox_max: Vec3,
    /// Triangle budget the casting meshes to.
    pub budget: usize,
}

/// The gun: a revolved steel barrel (driven by the installed module's length) and the moving cast
/// mantlet mask. The barrel dimensions are the hybrid visual ones — distinct from the legacy-recipe
/// `GunShape`, which feeds the older `vehicle_geometry` path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GunVisual {
    pub barrel_radius: f32,
    pub muzzle_radius: f32,
    pub muzzle_taper: f32,
    pub barrel_segments: usize,
    /// Mantlet side profile as `(z, radius)` points, revolved about Z then scaled to a flat oval.
    pub mantlet_profile: [(f32, f32); 3],
    pub mantlet_segments: usize,
    pub mantlet_scale: Vec3,
    /// How much of a gun module's length delta the muzzle moves by (visual modularity scale).
    pub module_delta_scale: f32,
}

/// An axis-aligned box part (engine deck), as centre + half-extents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxVisual {
    pub center: Vec3,
    pub half: Vec3,
}

/// A fender (mudguard) plate riding above one track run, mirrored to both sides at `±side_x`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FenderVisual {
    pub side_x: f32,
    pub center_y: f32,
    pub half: Vec3,
}

/// The road-wheel train: rubber-tyred wheels with proud steel hubs, repeated along each side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunningGearVisual {
    pub wheel_radius: f32,
    pub wheel_half_width: f32,
    pub hub_radius: f32,
    pub hub_overhang: f32,
    pub wheel_segments: usize,
    pub hub_segments: usize,
    pub wheel_count: usize,
    pub first_z: f32,
    pub spacing: f32,
    /// The T-54's characteristic large gap between the first and second road wheels. The remaining
    /// stations are spaced evenly to keep the train within `first_z .. first_z + (count-1)*spacing`.
    pub first_gap: f32,
    pub side_x: f32,
}

/// The track belt: a rounded loop swept as a rectangular band, wrapping the wheels on each side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackBeltVisual {
    pub front_z: f32,
    pub rear_z: f32,
    pub axle_y: f32,
    pub radius: f32,
    pub side_x: f32,
    pub half_thickness: f32,
    pub half_width: f32,
    pub straight_segments: usize,
    pub arc_segments: usize,
    /// Number of link cues spaced along the ground (bottom) run, so the belt reads as tracked links
    /// rather than a smooth rubber band. Fine tread is left to the material layer.
    pub link_count: usize,
}

/// The full hybrid visual description for one vehicle. `Some` only for the hybrid benchmark (T-54);
/// other vehicles bake through the legacy `vehicle_geometry` path and carry `None`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HybridVisual {
    pub hull: HullVisual,
    pub hull_plates: HullPlatesVisual,
    pub turret: TurretVisual,
    pub gun: GunVisual,
    pub deck: BoxVisual,
    pub fender: FenderVisual,
    pub running_gear: RunningGearVisual,
    pub track_belt: TrackBeltVisual,
}
