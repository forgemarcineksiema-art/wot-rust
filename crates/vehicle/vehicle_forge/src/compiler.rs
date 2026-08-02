use game_core::{MountFrames, TankSpec, VehicleKind, VehicleModules};
use thiserror::Error;
use vehicle_geometry::BakedVehicle;

use crate::{ArtifactError, BakeProfile, ForgeArtifact};

/// Typed authoring request for one gameplay and visual vehicle variant.
#[derive(Debug, Clone)]
pub struct TankCompileRequest {
    pub vehicle: VehicleKind,
    pub modules: VehicleModules,
    pub profile: BakeProfile,
}

/// The immutable result consumed by build tooling and runtime asset caches.
#[derive(Debug, Clone)]
pub struct CompiledTank {
    pub spec: TankSpec,
    pub mounts: MountFrames,
    pub baked_vehicle: BakedVehicle,
    pub artifact: ForgeArtifact,
    pub source_hash: u64,
}

#[derive(Debug, Error)]
pub enum TankCompileError {
    #[error("visual compilation is not registered for {0:?}")]
    UnsupportedVehicle(VehicleKind),
    #[error("loadout validation failed: {0:?}")]
    Validation(Vec<TankValidationError>),
    #[error("damage layout does not fit inside the compiled hitbox")]
    DamageLayoutOutsideHitbox,
    #[error("baked mount frames differ from the compiled mount frames")]
    InconsistentMounts,
    #[error(transparent)]
    Artifact(#[from] ArtifactError),
}

#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum TankValidationError {
    #[error("gun caliber {caliber_mm:.0} mm exceeds turret limit {max_mm:.0} mm")]
    GunTooLarge { caliber_mm: f32, max_mm: f32 },
    #[error("loadout weighs {combat_kg:.0} kg but suspension supports {max_load_kg:.0} kg")]
    OverloadedSuspension { combat_kg: f32, max_load_kg: f32 },
}

pub fn compile_tank(request: TankCompileRequest) -> Result<CompiledTank, TankCompileError> {
    let mut validation = Vec::new();
    if !request.modules.can_mount_gun(&request.modules.gun) {
        validation.push(TankValidationError::GunTooLarge {
            caliber_mm: request.modules.gun.caliber_mm(),
            max_mm: request.modules.turret.max_gun_caliber_mm,
        });
    }
    if !request.modules.within_load_limit(0.0) {
        validation.push(TankValidationError::OverloadedSuspension {
            combat_kg: request.modules.total_mass_kg(),
            max_load_kg: request.modules.suspension.max_load_kg,
        });
    }
    if !validation.is_empty() {
        return Err(TankCompileError::Validation(validation));
    }

    let mut spec = request.modules.assemble(request.vehicle);
    if !spec.damage_layout.fits_within(spec.hitbox) {
        return Err(TankCompileError::DamageLayoutOutsideHitbox);
    }

    // Modules -> geometry, for the whole fleet (Program C: un-T-54'd). The hybrid benchmark
    // keeps its bespoke part-description path; every other vehicle bakes its recipe from a
    // blueprint whose GUN is rescaled to the installed module, so a longer barrel is a longer
    // silhouette everywhere, not just on the T-54.
    let (mounts, baked_vehicle) = if request.vehicle == VehicleKind::T54_1951 {
        let description = vehicle_build::t54_from_modules(&request.modules);
        (description.mounts, description.build_lod(request.profile.lod_level()))
    } else {
        let blueprint = game_core::VehicleBlueprint::for_vehicle(request.vehicle)
            .ok_or(TankCompileError::UnsupportedVehicle(request.vehicle))?;
        let blueprint = blueprint_with_gun_module(blueprint, &request.modules);
        let baked = vehicle_recipes::bake_vehicle_from_blueprint(&blueprint)
            .map_err(|error| TankCompileError::Artifact(ArtifactError::Bake(error)))?;
        (blueprint.mount_frames(), baked)
    };
    spec.mounts = mounts;
    if *baked_vehicle.mounts() != mounts {
        return Err(TankCompileError::InconsistentMounts);
    }
    let artifact = ForgeArtifact::bake_from_baked(request.vehicle, request.profile, baked_vehicle)?;
    let baked_vehicle = artifact.baked_vehicle()?;
    let source_hash = artifact.manifest().source_hash();
    Ok(CompiledTank { spec, mounts, baked_vehicle, artifact, source_hash })
}

/// The installed gun reshapes the blueprint: the barrel keeps its authored trunnion and scales
/// its reach by the module's length against the stock gun, so a swapped gun visibly changes
/// the silhouette (and the muzzle the shared shell trace fires from moves with it).
fn blueprint_with_gun_module(
    mut blueprint: game_core::VehicleBlueprint,
    modules: &VehicleModules,
) -> game_core::VehicleBlueprint {
    let stock_length = blueprint.kind.stock_barrel_length_m();
    let module_length = modules.gun.barrel_length_m();
    if stock_length > 0.1 && (module_length - stock_length).abs() > 1.0e-3 {
        let reach = blueprint.gun.muzzle_z - blueprint.gun.trunnion_z;
        blueprint.gun.muzzle_z = blueprint.gun.trunnion_z + reach * (module_length / stock_length);
    }
    blueprint
}
