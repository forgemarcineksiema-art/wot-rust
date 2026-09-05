//! External fittings and visual-only factory detailing for the hybrid path, split out of
//! `hybrid.rs` to keep each module reviewable. [`FittingsVisual`] holds semantic external parts
//! (hatches, headlight, tow hooks); [`DetailVisual`] holds the clean-build greeble (grille, exhaust,
//! periscopes, DShK mount, fender lips, weld beads). Neither restates a gameplay dimension or feeds
//! collision, armour, the mount frames, or the network snapshot.

use glam::Vec3;

/// Semantic external fittings carried as their own parts (not anonymous greeble): the commander's
/// cupola hatch lid and turret-side vision drum ride the turret; the glacis headlight and the front
/// tow hooks ride the hull. Finer surface detail (welds, grab handles) is left to the material layer.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FittingsVisual {
    pub cupola_hatch_center: Vec3,
    pub cupola_hatch_radius: f32,
    pub cupola_hatch_half_height: f32,
    /// Driver's hatch lid: a round hatch on the hull roof, front-left, ahead of the turret ring and
    /// behind the upper glacis fold. Rides the hull (does not traverse).
    pub driver_hatch_center: Vec3,
    pub driver_hatch_radius: f32,
    pub driver_hatch_half_height: f32,
    /// Loader's hatch lid: a round hatch on the turret roof, loader (right) side. Rides the turret so
    /// it traverses with the vehicle.
    pub loader_hatch_center: Vec3,
    pub loader_hatch_radius: f32,
    pub loader_hatch_half_height: f32,
    pub headlight_center: Vec3,
    pub headlight_radius: f32,
    pub headlight_half_height: f32,
    /// Front tow hook (right side; mirrored to the left).
    pub tow_hook_center: Vec3,
    pub tow_hook_half: Vec3,
    /// A second bow hatch on the hull roof (the German line's radio operator beside the driver),
    /// with the driver's radius and height. `None` on a vehicle with one bow hatch (the T-54).
    #[serde(default)]
    pub second_bow_hatch_center: Option<Vec3>,
}

/// Visual-only factory detailing for the hybrid path. Clean-build intent: a freshly delivered
/// vehicle — *no* mud, rust, battle damage, decals, or heavy weathering. These descriptors add only
/// crisp manufactured greeble (engine-deck grille, exhaust housing, turret periscopes, fender lips
/// and restrained weld beads). Every value is a *new* visual dimension placed inside the already
/// validated hull/turret volumes; none restates a hull, track, ring, cupola, mantlet, trunnion or
/// muzzle dimension, and none feeds collision, armour, the mount frames, or the network snapshot.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DetailVisual {
    /// Louvered engine-deck grille panel (centre + half-extents) and its slat count.
    pub grille_center: Vec3,
    pub grille_half: Vec3,
    pub grille_slats: usize,
    /// Exhaust housing: a boxed cover lying along the left fender run.
    pub exhaust_center: Vec3,
    pub exhaust_half: Vec3,
    /// Turret-roof periscope block (right of the cupola; mirrored to the loader side by the generator).
    pub periscope_center: Vec3,
    pub periscope_half: Vec3,
    /// Loader-side DShK anti-aircraft mount: barrel pivot and exposed barrel length.
    pub dshk_mount_center: Vec3,
    pub dshk_barrel_length: f32,
    /// Fender lip: a thin downturned edge along the outer fender run (drop below + thickness).
    pub fender_lip_drop: f32,
    pub fender_lip_thickness: f32,
    /// Restrained weld bead half-thickness for the glacis/deck joins (kept tiny — a crisp cast seam,
    /// not a weathered weld). Finer surface relief stays in the material/normal layer.
    pub weld_seam_half_thickness: f32,
}
