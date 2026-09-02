use serde::{Deserialize, Serialize};

use super::{EngineModule, GunModule, HullChassis, RadioModule, SuspensionModule, TurretModule};
use crate::{
    ArmorFacet, ArmorProfile, DamageLayout, HitboxProfile, ModuleHealth, MountFrames, TankSpec,
    VehicleKind,
};

/// Why a module could not be installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleError {
    /// The gun's caliber exceeds the turret's mount limit.
    GunTooLargeForTurret { caliber_mm: u32, max_mm: u32 },
    /// The combined module weight exceeds what the running gear can carry.
    OverloadedSuspension { combat_kg: u32, max_load_kg: u32 },
}

/// A complete installed module loadout. Swap a module (respecting compatibility) and call
/// [`VehicleModules::assemble`] to get the flat effective stats the rest of the game uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VehicleModules {
    pub hull: HullChassis,
    pub engine: EngineModule,
    pub suspension: SuspensionModule,
    pub turret: TurretModule,
    pub gun: GunModule,
    pub radio: RadioModule,
}

impl VehicleModules {
    /// Combat weight = chassis + every installed module.
    pub fn total_mass_kg(&self) -> f32 {
        self.hull.mass_kg
            + self.engine.mass_kg
            + self.suspension.mass_kg
            + self.turret.mass_kg
            + self.gun.mass_kg
            + self.radio.mass_kg
    }

    /// Whether `gun` fits the currently installed turret.
    pub fn can_mount_gun(&self, gun: &GunModule) -> bool {
        gun.caliber_mm() <= self.turret.max_gun_caliber_mm
    }

    /// Whether the running gear can carry the loadout once `extra_kg` more is added.
    pub fn within_load_limit(&self, extra_kg: f32) -> bool {
        self.total_mass_kg() + extra_kg <= self.suspension.max_load_kg
    }

    /// Install a gun if it fits the turret, otherwise report why.
    pub fn try_install_gun(&mut self, gun: GunModule) -> Result<(), ModuleError> {
        if !self.can_mount_gun(&gun) {
            return Err(ModuleError::GunTooLargeForTurret {
                caliber_mm: gun.caliber_mm() as u32,
                max_mm: self.turret.max_gun_caliber_mm as u32,
            });
        }
        self.gun = gun;
        Ok(())
    }

    /// Install a turret if the running gear can still carry the loadout, otherwise report why.
    pub fn try_install_turret(&mut self, turret: TurretModule) -> Result<(), ModuleError> {
        let delta = turret.mass_kg - self.turret.mass_kg;
        if !self.within_load_limit(delta) {
            return Err(ModuleError::OverloadedSuspension {
                combat_kg: (self.total_mass_kg() + delta) as u32,
                max_load_kg: self.suspension.max_load_kg as u32,
            });
        }
        self.turret = turret;
        Ok(())
    }

    /// Compose the installed modules into the flat effective [`TankSpec`] the simulation and
    /// penetration model consume. This is the single bridge from "modules" to "stats".
    pub fn assemble(&self, kind: VehicleKind) -> TankSpec {
        TankSpec {
            name: kind.display_name().to_string(),
            kind,
            mass_kg: self.total_mass_kg(),
            engine_power_kw: self.engine.power_kw,
            max_forward_speed_mps: self.hull.max_forward_speed_mps,
            max_reverse_speed_mps: self.hull.max_reverse_speed_mps,
            turn_rate_rad_s: self.suspension.turn_rate_rad_s,
            turret_rotation_rad_s: self.turret.traverse.rate_rad_s(),
            vertical_stabilizer: self.turret.vertical_stabilizer,
            hull: armor_profile_for(kind, self),
            gun: self.gun.spec.clone(),
            ammo: crate::AmmoLoadout::default_for_slots(
                kind.ammo_capacity(),
                self.gun.spec.ammo_options().len(),
            ),
            ammo_capacity: kind.ammo_capacity(),
            hit_points: self.hull.hit_points,
            module_health: ModuleHealth::from_loadout(self),
            hitbox: HitboxProfile::for_vehicle(kind),
            damage_layout: DamageLayout::for_vehicle(kind),
            mounts: MountFrames::for_vehicle(kind),
            contact: crate::ContactFootprint::for_vehicle(kind),
        }
    }
}

fn armor_profile_for(kind: VehicleKind, modules: &VehicleModules) -> ArmorProfile {
    let h = &modules.hull;
    let t = &modules.turret;
    // Migrated vehicles take their plate slopes/weakspots from the blueprint (the same angles the
    // visible plates are built from); thicknesses still come from the installed modules.
    if let Some(bp) = crate::VehicleBlueprint::for_vehicle(kind) {
        let a = &bp.armor;
        let authored_roof = t.roof_mm;
        let profile = ArmorProfile::new_with_facets(
            ArmorFacet::new(h.front_mm, a.hull_front.0, a.hull_front.1),
            ArmorFacet::new(h.side_mm, a.hull_side.0, a.hull_side.1),
            ArmorFacet::new(h.rear_mm, a.hull_rear.0, a.hull_rear.1),
            ArmorFacet::new(t.front_mm, a.turret_front.0, a.turret_front.1),
            ArmorFacet::new(t.side_mm, a.turret_side.0, a.turret_side.1),
            ArmorFacet::new(t.rear_mm, a.turret_rear.0, a.turret_rear.1),
        );
        // An authored roof beats the fleet formula: the T-54's documents say 30 mm where the
        // derivation off the front plate produced 24. The lower front plate follows the same
        // idiom: stated in the dossier, authored in the blueprint, read by armour and metal
        // alike.
        let mut profile = match authored_roof {
            Some(roof_mm) => profile.with_turret_roof_mm(roof_mm),
            None => profile,
        };
        if let Some((deg, thickness_factor)) = a.hull_lower_front {
            profile.lower_front =
                Some(crate::ArmorFacet::new(h.front_mm * thickness_factor, deg, 1.10));
        }
        return profile;
    }
    unreachable!("{kind:?} is blueprint-migrated")
}
