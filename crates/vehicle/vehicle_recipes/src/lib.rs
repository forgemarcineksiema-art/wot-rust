//! Per-vehicle procedural recipes plus the shared family components they are built from.
//!
//! Each recipe authors its submeshes in a single **local space**: the origin sits on the ground
//! plane under the hull centre, `+Y` is up, and `+Z` is forward (the gun points `+Z`). The hull,
//! turret, and gun all use this same space; the renderer later pivots the turret about the
//! turret-ring frame and the gun about the trunnion frame, so authoring everything in one space
//! keeps the mount maths trivial and the fit tests honest.
//!
//! Shared shapes live in [`chassis`] (hulls, tracks, wheels), [`armament`] (guns), and
//! [`turret_fittings`] (cupolas, mantlet sockets, cast domes); the [`soviet`] family module
//! tunes those into the remaining legacy vehicles, while blueprint-born vehicles carry their
//! own modules ([`tiger_i`], [`tiger_ii`], [`jagdtiger`], [`panther_ii`], [`is3`]).
//!
//! Recipes receive [`MountFrames`] and [`HitboxProfile`] from [`game_core`] as authoritative
//! inputs — hull dimensions are derived as explicit fractions of the hitbox, and mount points
//! (turret ring, trunnion, muzzle) drive turret and gun placement. This keeps the fit test
//! nearly tautological and prevents drift between the visual geometry, the collision box, and
//! the simulation shell spawn.

use std::cell::Cell;

use game_core::{HitboxProfile, MountFrames, VehicleBlueprint, VehicleKind};

use vehicle_build::{
    Fidelity, GeneratorKind, LodStrategy, NamedCavity, PartKey, PartLod, PartShape, PostMerge,
    SurfaceBake, VehicleDescription, VehiclePart,
};
use vehicle_geometry::{
    BakeError, BakedVehicle, CavityBand, GeometryMesh, MaterialRole, SmoothingGroup, Submesh,
    SubmeshKind,
};

thread_local! {
    /// The Forge Studio's live-override blueprint (see [`bake_vehicle_from_blueprint`]):
    /// while set, recipes read THIS blueprint instead of the registered one, so an author can
    /// bake an edited RON file without recompiling the workspace.
    static BLUEPRINT_OVERRIDE: Cell<Option<VehicleBlueprint>> = const { Cell::new(None) };
}

/// The blueprint a recipe should read for `kind`: the live override when one is active (and
/// tagged for this kind), otherwise the registered blueprint. Every recipe fetches through
/// this — a direct `VehicleBlueprint::for_vehicle` in a recipe would silently ignore the
/// studio's override path.
pub(crate) fn active_blueprint(kind: VehicleKind) -> Option<VehicleBlueprint> {
    BLUEPRINT_OVERRIDE
        .get()
        .filter(|blueprint| blueprint.kind == kind)
        .or_else(|| VehicleBlueprint::for_vehicle(kind))
}

/// Bake `blueprint.kind` through its RECIPE path with `blueprint` as the live shape source —
/// the Forge Studio's fast loop (edit a RON on disk, bake it in-process, no rebuild). Note the
/// T-54's authoritative mesh is the hybrid `vehicle_build` path; overriding it there is not
/// supported — the override bakes its legacy recipe, which is fine for shape-tuning.
pub fn bake_vehicle_from_blueprint(
    blueprint: &VehicleBlueprint,
) -> Result<BakedVehicle, BakeError> {
    BLUEPRINT_OVERRIDE.set(Some(*blueprint));
    let result = recipe(blueprint.kind, &blueprint.hitbox(), &blueprint.mount_frames())
        .ok_or(BakeError::MissingRecipe(blueprint.kind));
    BLUEPRINT_OVERRIDE.set(None);
    result
}

mod armament;
mod blueprint_cavity;
mod budgets;
mod centurion;
mod chassis;
mod chassis_blueprint;
mod deck_details;
mod is3;
mod is3_hull;
mod jagdtiger;
mod panther_ii;
mod soviet;
mod t34_85;
mod t54;
mod tiger_i;
mod tiger_ii;
mod turret_fittings;

pub use budgets::{
    FAR_MUST_SAVE_FRACTION, GEAR_BUDGETS, GOLDEN_BAKE_HASHES, GearBudgets, VEHICLE_BUDGETS,
    VehicleBudgets, golden_bake_hash,
};

pub(crate) use armament::{GunPlan, gun_group, gun_group_with_mantlet_scale};
pub(crate) use chassis::shade_hull;
pub(crate) use chassis_blueprint::{blueprint_prism_hull, blueprint_skirts};
pub(crate) use t54::{t54_hull, t54_turret_front};

pub(crate) use turret_fittings::{
    add_british_cupola, add_broad_mantlet_socket, add_commander_periscope, add_cupola,
    add_flush_ring_hatch, add_german_cast_cupola, add_mantlet_socket, add_oval_mantlet_socket,
    add_soviet_slit_cupola, add_t54_mantlet_socket, add_turret_ring, cast_turret_shell,
    vision_prism,
};

/// Bake the procedural geometry for `kind`.
///
/// The result is fallible for API stability and forward-compatibility: [`recipe`] matches every
/// current [`VehicleKind`] exhaustively (so adding a wire variant is a compile error until a
/// recipe exists), and the [`BakeError::MissingRecipe`] path stays wired up for that future gap.
pub fn bake_vehicle(kind: VehicleKind) -> Result<BakedVehicle, BakeError> {
    let hitbox = HitboxProfile::for_vehicle(kind);
    let mounts = MountFrames::for_vehicle(kind);
    recipe(kind, &hitbox, &mounts).ok_or(BakeError::MissingRecipe(kind))
}

/// The description the game ships for `kind` — ONE rule for the fleet (Forge 2.0 K1): the part
/// library builds it when the blueprint carries a complete visual
/// ([`vehicle_build::description_for`]); otherwise this crate's recipe is wrapped, submesh by
/// submesh, as a `Sketch` description that reduces whole-mesh. Byte-exact against
/// [`bake_vehicle`] for every sketch (`vehicle_forge/tests/seam_lock.rs`).
pub fn describe(kind: VehicleKind) -> Option<VehicleDescription> {
    if let Some(description) = vehicle_build::description_for(kind) {
        return Some(description);
    }
    let hitbox = HitboxProfile::for_vehicle(kind);
    let mounts = MountFrames::for_vehicle(kind);
    // The library's parts for what the visual file authors (Forge 2.0 K3, one class at a time);
    // the recipe's pieces leave those out.
    let library =
        active_blueprint(kind).and_then(|bp| vehicle_build::fitting_parts_for_blueprint(&bp));
    let omit = deck_details::DeckOmit { fittings: library.is_some() };
    match recipe_pieces(kind, &hitbox, &mounts, omit) {
        Some(pieces) => {
            let mut description = pieces_description(kind, pieces);
            description.parts.extend(library.unwrap_or_default());
            Some(description)
        }
        None => bake_vehicle(kind).ok().map(recipe_description),
    }
}

/// [`describe`] over a LIVE blueprint (the Studio's `--blueprint-file` fast loop): the override
/// is set for the whole composition, so the pieces AND the library parts read the edited file.
pub fn describe_with_blueprint(blueprint: &VehicleBlueprint) -> Option<VehicleDescription> {
    BLUEPRINT_OVERRIDE.set(Some(*blueprint));
    let description = describe(blueprint.kind);
    BLUEPRINT_OVERRIDE.set(None);
    description
}

/// A recipe split into the builders it is made of, each a `Recipe` part with a name — the
/// shape a sketch takes on its way to the part library (Forge 2.0 K3): the library replaces a
/// piece at a time, and every piece still standing is the recipe's own geometry. Only the
/// vehicles whose recipes have been split return `Some`; the rest wrap whole submeshes.
pub(crate) struct RecipePieces {
    pub hull: Vec<(&'static str, GeometryMesh)>,
    pub turret: Vec<(&'static str, GeometryMesh)>,
    pub gun: Vec<(&'static str, GeometryMesh)>,
    pub mounts: MountFrames,
}

fn recipe_pieces(
    kind: VehicleKind,
    hitbox: &HitboxProfile,
    mounts: &MountFrames,
    omit: deck_details::DeckOmit,
) -> Option<RecipePieces> {
    match kind {
        VehicleKind::TigerI => Some(tiger_i::tiger_i_pieces(hitbox, mounts, omit)),
        _ => None,
    }
}

/// The pieces as a description: the recipe's cavity bands become the surface bake and the
/// submesh welds after the merge, exactly as [`assemble`] does — `seam_lock` proves the bytes.
fn pieces_description(kind: VehicleKind, pieces: RecipePieces) -> VehicleDescription {
    let (hull_bands, turret_bands, gun_bands) = match active_blueprint(kind) {
        Some(blueprint) => blueprint_cavity::blueprint_cavity_bands(&blueprint),
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    let piece = |submesh: SubmeshKind, (name, mesh): (&'static str, GeometryMesh)| VehiclePart {
        key: PartKey::new(name),
        submesh,
        material: MaterialRole::RolledArmor,
        smoothing: SG_HARD,
        shape: PartShape::Mesh(mesh),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::Recipe,
    };
    let mut parts = Vec::new();
    parts.extend(pieces.hull.into_iter().map(|p| piece(SubmeshKind::Hull, p)));
    parts.extend(pieces.turret.into_iter().map(|p| piece(SubmeshKind::Turret, p)));
    parts.extend(pieces.gun.into_iter().map(|p| piece(SubmeshKind::Gun, p)));
    // `assemble` applies the three band sets per submesh; the description keeps them apart by
    // scoping each band to its submesh (a hull band would otherwise shade turret vertices that
    // stand in the same recess — 48 hull, 49 turret and 99 gun vertices moved when they were
    // unioned).
    let scoped = |signal: &'static str, scope: SubmeshKind, bands: Vec<CavityBand>| {
        bands.into_iter().map(move |band| NamedCavity { signal, band, scope: Some(scope) })
    };
    let cavities = scoped("recipe_hull_band", SubmeshKind::Hull, hull_bands)
        .chain(scoped("recipe_turret_band", SubmeshKind::Turret, turret_bands))
        .chain(scoped("recipe_gun_band", SubmeshKind::Gun, gun_bands))
        .collect();
    VehicleDescription {
        kind,
        parts,
        mounts: pieces.mounts,
        surface_bake: SurfaceBake { cavities },
        fidelity: Fidelity::Sketch,
        lod: LodStrategy::WholeMesh,
        post_merge: PostMerge::WeldAndSmooth,
    }
}

/// The fidelity `kind` ships at, without building it.
pub fn describe_fidelity(kind: VehicleKind) -> Fidelity {
    match VehicleBlueprint::for_vehicle(kind) {
        Some(bp) if bp.complete_visual().is_some() => Fidelity::Benchmark,
        _ => Fidelity::Sketch,
    }
}

/// [`describe`] over a LIVE blueprint (the Studio's `--blueprint-file` fast loop).
pub fn describe_from_blueprint(
    blueprint: &VehicleBlueprint,
) -> Result<VehicleDescription, BakeError> {
    match vehicle_build::description_from_blueprint(blueprint) {
        Some(description) => Ok(description),
        None => describe_with_blueprint(blueprint).ok_or(BakeError::MissingRecipe(blueprint.kind)),
    }
}

/// A recipe's three submeshes as three parts: what a description looks like before a vehicle
/// has been built from the part library. The mesh is carried as-is (`PartShape::Mesh`), the
/// surface bake is empty (the recipe already baked its contact cavities in `assemble`), and the
/// LOD strategy is whole-mesh — the reduction the recipes always ran.
fn recipe_description(baked: BakedVehicle) -> VehicleDescription {
    let kind = baked.kind();
    let mounts = *baked.mounts();
    let parts = baked
        .submeshes()
        .iter()
        .map(|submesh| VehiclePart {
            key: PartKey::new(match submesh.kind {
                SubmeshKind::Hull => "recipe_hull",
                SubmeshKind::Turret => "recipe_turret",
                SubmeshKind::Gun => "recipe_gun",
            }),
            submesh: submesh.kind,
            material: MaterialRole::RolledArmor,
            smoothing: SG_HARD,
            shape: PartShape::Mesh(submesh.mesh.clone()),
            lod: PartLod::Silhouette,
            generator: GeneratorKind::Recipe,
        })
        .collect();
    VehicleDescription {
        kind,
        parts,
        mounts,
        surface_bake: SurfaceBake::default(),
        fidelity: Fidelity::Sketch,
        lod: LodStrategy::WholeMesh,
        post_merge: PostMerge::None,
    }
}

/// Bake `kind` and reduce it to the requested LOD in one call.
pub fn bake_vehicle_lod(
    kind: VehicleKind,
    level: vehicle_geometry::LodLevel,
) -> Result<BakedVehicle, BakeError> {
    Ok(vehicle_geometry::reduce_vehicle(&bake_vehicle(kind)?, level))
}

fn recipe(kind: VehicleKind, hitbox: &HitboxProfile, mounts: &MountFrames) -> Option<BakedVehicle> {
    Some(match kind {
        VehicleKind::T54_1951 => soviet::t54_1951(hitbox, mounts),
        VehicleKind::TigerI => tiger_i::tiger_i(hitbox, mounts),
        VehicleKind::TigerII => tiger_ii::tiger_ii(hitbox, mounts),
        VehicleKind::Jagdtiger => jagdtiger::jagdtiger(hitbox, mounts),
        VehicleKind::PantherII => panther_ii::panther_ii(hitbox, mounts),
        VehicleKind::IS3 => is3::is3(hitbox, mounts),
        VehicleKind::Centurion => centurion::centurion(hitbox, mounts),
        VehicleKind::T34_85 => t34_85::t34_85(hitbox, mounts),
    })
}

// Smoothing groups shared across families. Group 0 (`hard_edges`) keeps welded plates crisp;
// higher groups mark cast/round surfaces that should read as smooth.
pub(crate) const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();
pub(crate) const SG_CAST: SmoothingGroup = SmoothingGroup(2);
pub(crate) const SG_CUPOLA: SmoothingGroup = SmoothingGroup(3);
pub(crate) const SG_BARREL: SmoothingGroup = SmoothingGroup(4);
pub(crate) const SG_MANTLET: SmoothingGroup = SmoothingGroup(6);
pub(crate) const SG_RING: SmoothingGroup = SmoothingGroup(7);

/// Assemble the three submeshes and mount frames into a baked vehicle. `turret_ring` doubles as
/// the casemate frame for fixed-superstructure tank destroyers (their `turret` submesh simply
/// never traverses, since the sim holds casemate turret yaw at zero).
pub(crate) fn assemble(
    kind: VehicleKind,
    hull: GeometryMesh,
    turret: GeometryMesh,
    gun: GeometryMesh,
    mounts: MountFrames,
) -> BakedVehicle {
    // Program C: every blueprint vehicle bakes the generic contact cavities (ring seam,
    // mantlet seat, gear recess, deck shadow, weld line) the T-54's bespoke bake pioneered —
    // the fleet's ambient look stops being flat the moment a blueprint exists.
    let (hull_bands, turret_bands, gun_bands) = match active_blueprint(kind) {
        Some(blueprint) => blueprint_cavity::blueprint_cavity_bands(&blueprint),
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    BakedVehicle::new(
        kind,
        vec![
            Submesh {
                kind: SubmeshKind::Hull,
                mesh: hull.weld_and_smooth().with_contact_cavity(&hull_bands),
            },
            Submesh {
                kind: SubmeshKind::Turret,
                mesh: turret.weld_and_smooth().with_contact_cavity(&turret_bands),
            },
            Submesh {
                kind: SubmeshKind::Gun,
                mesh: gun.weld_and_smooth().with_contact_cavity(&gun_bands),
            },
        ],
        mounts,
    )
}
