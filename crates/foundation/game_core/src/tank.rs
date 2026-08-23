use serde::{Deserialize, Serialize};

use crate::{
    AmmoLoadout, ArmorProfile, ContactFootprint, DamageLayout, GunSpec, ModuleHealth, MountFrames,
    VehicleKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HitboxProfile {
    pub half_width_m: f32,
    pub half_height_m: f32,
    pub half_length_m: f32,
    pub center_y_m: f32,
    /// Local-space Y threshold above which hits are resolved against turret/casemate armor.
    pub turret_min_y_m: f32,
    /// Plan half-width of the turret/casemate volume above `turret_min_y_m`. Above the split a
    /// shot only connects inside this turret-frame box (which traverses with the turret); the
    /// rest of the deck plan is open air over a thin hull roof. Sized to the visual turret and
    /// gated by `vehicle_geometry`'s turret fit/fill test.
    pub turret_half_width_m: f32,
    /// Plan half-length of the turret/casemate volume above the split.
    pub turret_half_length_m: f32,
    /// Local-space Z of the turret volume centre (at zero traverse).
    pub turret_center_z_m: f32,
}

impl HitboxProfile {
    /// A plain single-box profile: the turret volume spans the full plan, which makes every hit
    /// above `turret_min_y_m` a turret hit — the pre-turret-box behaviour. Production vehicles
    /// narrow it via [`Self::with_turret_plan`].
    pub fn new(
        half_width_m: f32,
        half_height_m: f32,
        half_length_m: f32,
        center_y_m: f32,
        turret_min_y_m: f32,
    ) -> Self {
        Self {
            half_width_m,
            half_height_m,
            half_length_m,
            center_y_m,
            turret_min_y_m,
            turret_half_width_m: half_width_m,
            turret_half_length_m: half_length_m,
            turret_center_z_m: 0.0,
        }
    }

    pub fn with_turret_plan(
        mut self,
        turret_half_width_m: f32,
        turret_half_length_m: f32,
        turret_center_z_m: f32,
    ) -> Self {
        self.turret_half_width_m = turret_half_width_m;
        self.turret_half_length_m = turret_half_length_m;
        self.turret_center_z_m = turret_center_z_m;
        self
    }

    /// Per-vehicle collision box. Heights are realistic full-vehicle heights (`2 * half_height`):
    /// the Soviet mediums are famously low (~2.4 m) while the German heavies are genuinely ~3 m.
    /// `center_y` is set so the floor sits ~5 cm below ground (`center_y = half_height - 0.05`),
    /// and `turret_min_y` (local Y, relative to `center_y`) is the hull/turret split — chosen to
    /// keep the *world-space* split height unchanged from the earlier taller boxes, so hit
    /// resolution (which armor a shot meets) is preserved. The visible mesh is sized to fill this
    /// box; see `client::vehicle_visual_specs` and its `body_fits_within_hitbox` test.
    ///
    /// The turret plan (`with_turret_plan`) is sized to the *full* plan extents of the visual
    /// turret submesh — including the cast shoulder where it bulges below the split — so the
    /// gameplay turret volume is a tight, slightly generous box around the visible turret instead
    /// of the whole deck. `vehicle_geometry::every_vehicle_turret_fits_and_fills_its_turret_plan`
    /// keeps these numbers honest against the baked geometry.
    pub fn for_vehicle(kind: VehicleKind) -> Self {
        // Migrated vehicles derive the hitbox from their blueprint (the single shape source); the
        // rest fall back to the hand-authored boxes below.
        crate::VehicleBlueprint::for_vehicle(kind)
            .expect("every VehicleKind is blueprint-migrated")
            .hitbox()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TankSpec {
    pub name: String,
    /// Stable identity used by networking and the renderer to tell vehicles apart.
    #[serde(default)]
    pub kind: VehicleKind,
    pub mass_kg: f32,
    pub engine_power_kw: f32,
    pub max_forward_speed_mps: f32,
    pub max_reverse_speed_mps: f32,
    pub turn_rate_rad_s: f32,
    pub turret_rotation_rad_s: f32,
    pub hull: ArmorProfile,
    pub gun: GunSpec,
    /// Rounds carried into battle per ammo slot (`AmmoLoadout`); the garage clamps the sum to
    /// `ammo_capacity`. `serde(default)` keeps older spec fixtures loading with the stock fill.
    #[serde(default)]
    pub ammo: AmmoLoadout,
    /// Total rounds the rack holds.
    #[serde(default = "crate::ammo::default_ammo_capacity")]
    pub ammo_capacity: u16,
    pub hit_points: u32,
    pub module_health: ModuleHealth,
    pub hitbox: HitboxProfile,
    #[serde(default)]
    pub damage_layout: DamageLayout,
    #[serde(default)]
    pub mounts: MountFrames,
    /// Running-gear ground-contact stations (see [`ContactFootprint`]). `serde(default)` keeps
    /// older spec fixtures loading; consumers read [`TankSpec::contact_footprint`], which falls
    /// back to a hitbox estimate when the field is the empty placeholder.
    #[serde(default)]
    pub contact: ContactFootprint,
}

impl TankSpec {
    /// The gun's arc in radians, as the sim clamps it: `(min_pitch, max_pitch)`, negative down.
    ///
    /// Per VEHICLE, from the installed gun: depression is a mount property (how much room the
    /// breech has under the roof), and it is one of the sharpest balance levers a tank has. The
    /// fleet used to share one hard-coded -8/+20 pair, which handed the T-54 — a tank whose -5
    /// is famous — a ridge-fighting ability it never had.
    pub fn gun_pitch_limits_rad(&self) -> (f32, f32) {
        (-self.gun.depression_deg.to_radians(), self.gun.elevation_deg.to_radians())
    }

    /// How far this vehicle's commander spots. Frozen on the vehicle so dropping eras is not a
    /// silent rebalance: the late-war park keeps 400 m, the postwar park keeps 440 m.
    pub fn view_range_m(&self) -> f32 {
        match self.kind {
            VehicleKind::T34_85
            | VehicleKind::TigerI
            | VehicleKind::TigerII
            | VehicleKind::Jagdtiger
            | VehicleKind::PantherII => 400.0,
            VehicleKind::T54_1951 | VehicleKind::IS3 | VehicleKind::Centurion => 440.0,
        }
    }

    pub fn medium_test_tank() -> Self {
        VehicleKind::BENCHMARK.spec()
    }

    /// The running-gear contact footprint, with the hitbox-estimate fallback for specs that
    /// predate the authored field. `ContactFootprint` is `Copy`, so this is free per tick.
    /// The rectangle this hull MOVES as — the metal, not the shell volume. See [`HullPlan`].
    pub fn hull_plan(&self) -> crate::HullPlan {
        crate::HullPlan::for_vehicle(self.kind)
    }

    pub fn contact_footprint(&self) -> ContactFootprint {
        if self.contact.is_empty() {
            ContactFootprint::from_hitbox(&self.hitbox)
        } else {
            self.contact
        }
    }

    pub fn has_fixed_casemate(&self) -> bool {
        self.turret_rotation_rad_s == 0.0
    }

    pub fn effective_turret_yaw_rad(&self, turret_yaw_rad: f32) -> f32 {
        if self.has_fixed_casemate() { 0.0 } else { turret_yaw_rad }
    }
}
