//! Grouped cross-frame remesh contract on the real T-54: one physical shot that splits at the
//! moving-frame seams owns fragments in the Hull, Turret AND Mantlet pose frames, and every frame
//! bakes its damage skin from its own fragment only, cached under its own label. This is the
//! Honest Steel phase-8 case the synthetic kernel tests cannot cover: real cast/welded production
//! geometry, the bounded worker, and the per-frame cache all in one path.

use game_core::{
    ApertureLobe, ArmorBreach, ArmorBreachDescriptor, ArmorBreachSet, ArmorFrame, ArmorMaterial,
    ArmorSurfaceId, ArmorZone, BreachContour, BreachFace, ShellType, TankId, VehicleKind,
};
use glam::Vec3;
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::{MeshContactIndex, SubmeshKind};

use super::asset_catalog::VehicleAssetCatalog;

const KIND: VehicleKind = VehicleKind::T54_1951;
const GROUP_ID: u64 = 42;

/// A real point on the visual armor: snap a probe to the production submesh so the fragment
/// anchors exactly on the surface the player sees, whatever the current bake looks like.
fn surface_anchor(submesh: SubmeshKind, probe: Vec3) -> (Vec3, Vec3) {
    let baked = authoritative_baked_vehicle(KIND).expect("T-54 bakes");
    let mesh = &baked.submesh(submesh).expect("submesh").mesh;
    let index = MeshContactIndex::from_mesh(mesh, Vec3::ZERO);
    let contact = index
        .nearest_point(probe, 2.0)
        .unwrap_or_else(|| panic!("the probe must find the {submesh:?} armor"));
    (contact.position, contact.normal.normalize())
}

fn fragment(frame: ArmorFrame, zone: ArmorZone, entry: Vec3, normal: Vec3) -> ArmorBreach {
    let thickness = 0.12;
    ArmorBreach::new(
        ArmorBreachDescriptor {
            breach_id: GROUP_ID,
            surface: ArmorSurfaceId::new(frame, zone),
            frame,
            zone,
            material: if frame == ArmorFrame::Hull {
                ArmorMaterial::RolledSteel
            } else {
                ArmorMaterial::CastSteel
            },
            face: BreachFace::Ingress,
            shell_type: ShellType::ArmorPiercing,
            created_tick: 5,
            impact_angle_degrees: 8.0,
            impact_energy_kj: 1_100.0,
            projectile_diameter_m: 0.1,
            residual_penetration_mm: 70.0,
        },
        ApertureLobe {
            entry_local: entry,
            exit_local: entry - normal * thickness,
            entry_normal_local: normal,
            exit_normal_local: -normal,
            direction_local: -normal,
            thickness_m: thickness,
            outer: BreachContour::new(0.055, 0.045, 0.4, 0.10),
            inner: BreachContour::new(0.075, 0.062, 0.5, 0.13),
            fracture_seed: game_core::math::splitmix64(GROUP_ID),
        },
    )
}

/// One shot's fragments: ingress through the glacis, a seam split on the turret face, and the
/// moving-mantlet fragment probed off the real trunnion so it survives any gun re-anchoring.
fn cross_frame_group() -> ArmorBreachSet {
    let trunnion =
        authoritative_baked_vehicle(KIND).expect("T-54 bakes").mounts().gun_trunnion.translation;
    let (hull_entry, hull_normal) = surface_anchor(SubmeshKind::Hull, Vec3::new(0.15, 1.18, 2.70));
    let (turret_entry, turret_normal) =
        // On the face plate's refined band since the egg reshape: x −0.42 sits in the bare
        // stretch between azimuth columns at the 54-segment base grid, and a cut anchored
        // there has no triangles inside its contour.
        surface_anchor(SubmeshKind::Turret, Vec3::new(-0.34, 1.84, 1.05));
    let (mantlet_entry, mantlet_normal) =
        // Probed BEHIND the trunnion plane since the measured-window rebuild: the mantlet is an
        // internal body whose face closes at trunnion −0.06, and everything ahead of it on the
        // gun submesh is canvas — a CastSteel fragment probed at +0.23 landed on fabric and the
        // remesh had no steel to cut.
        surface_anchor(SubmeshKind::Gun, trunnion + Vec3::new(0.12, 0.05, -0.05));
    let mut set = ArmorBreachSet::default();
    for (frame, zone, entry, normal) in [
        (ArmorFrame::Hull, ArmorZone::UpperGlacis, hull_entry, hull_normal),
        (ArmorFrame::Turret, ArmorZone::TurretFront, turret_entry, turret_normal),
        (ArmorFrame::Mantlet, ArmorZone::Mantlet, mantlet_entry, mantlet_normal),
    ] {
        set.add(fragment(frame, zone, entry, normal));
    }
    set
}

#[test]
fn one_shot_split_across_frames_bakes_each_frame_from_its_own_fragment_only() {
    let set = cross_frame_group();
    assert_eq!(set.aperture_group_count(), 1, "the split shot owns ONE bounded aperture slot");
    assert_eq!(set.group_fragments(GROUP_ID).count(), 3);

    let mut catalog = VehicleAssetCatalog::default();
    let tank = TankId(9);
    let frames = [ArmorFrame::Hull, ArmorFrame::Turret, ArmorFrame::Mantlet];
    for frame in frames {
        assert_eq!(
            catalog.damaged_frame_mesh(KIND, tank, frame, &set, 0, 0),
            None,
            "the first request schedules the worker instead of blocking the frame"
        );
    }
    catalog.finish_damage_mesh_jobs();

    let handles = frames.map(|frame| {
        catalog
            .damaged_frame_mesh(KIND, tank, frame, &set, 0, 0)
            .unwrap_or_else(|| panic!("the {frame:?} fragment must remesh on the production bake"))
    });
    assert_ne!(handles[0], handles[1]);
    assert_ne!(handles[1], handles[2]);
    assert_ne!(handles[0], handles[2]);

    // Honesty across the seam: every frame cuts exactly its own single fragment, so each damage
    // skin grows by the same one-lobe contour + rim over its base submesh. A frame that baked a
    // neighbour's fragment too would double its growth.
    let pending = catalog.take_pending_vehicle_meshes();
    let baked = authoritative_baked_vehicle(KIND).expect("T-54 bakes");
    let growth = [
        (handles[0], SubmeshKind::Hull),
        (handles[1], SubmeshKind::Turret),
        (handles[2], SubmeshKind::Gun),
    ]
    .map(|(handle, submesh)| {
        let damaged = pending
            .iter()
            .find(|(pending_handle, _)| *pending_handle == handle)
            .map(|(_, asset)| asset.vertices().len())
            .expect("the damage skin was uploaded through the pending path");
        let base = baked.submesh(submesh).expect("submesh").mesh.vertex_count();
        assert!(damaged > base, "{submesh:?} damage skin must own new contour steel");
        damaged - base
    });
    assert_eq!(growth[0], growth[1], "hull and turret must each bake exactly one fragment");
    assert_eq!(growth[1], growth[2], "the mantlet must bake exactly one fragment");

    // The bake is cached per frame: asking again returns the same handles without new uploads.
    let again = frames.map(|frame| catalog.damaged_frame_mesh(KIND, tank, frame, &set, 0, 0));
    assert_eq!(again, handles.map(Some));
    assert!(
        catalog.take_pending_vehicle_meshes().is_empty(),
        "a cache hit must not re-upload the damage skin"
    );
}

/// W0.6: the interior's Damaged/Burning variants. A destroyed engine chars its bay — the same
/// hull skin baked with the engine slot down carries visibly darker interior vertices than the
/// healthy bake, under a DIFFERENT cache label (module state keys the skin). Shared production
/// meshes never mutate; only this tank's own baked copy darkens.
#[test]
fn a_destroyed_engine_chars_its_bay_in_the_per_instance_skin() {
    let set = cross_frame_group();
    let engine_bit = game_core::ModuleSlot::Engine.destroyed_mask_bit();
    let tank = TankId(11);
    let mut catalog = VehicleAssetCatalog::default();

    let bake = |catalog: &mut VehicleAssetCatalog, destroyed: u8| {
        assert_eq!(
            catalog.damaged_frame_mesh(KIND, tank, ArmorFrame::Hull, &set, 0, destroyed),
            None
        );
        catalog.finish_damage_mesh_jobs();
        let handle = catalog
            .damaged_frame_mesh(KIND, tank, ArmorFrame::Hull, &set, 0, destroyed)
            .expect("hull skin bakes");
        let pending = catalog.take_pending_vehicle_meshes();
        pending
            .into_iter()
            .find(|(pending_handle, _)| *pending_handle == handle)
            .map(|(_, asset)| asset)
            .expect("uploaded")
    };

    let healthy = bake(&mut catalog, 0);
    let charred = bake(&mut catalog, engine_bit);
    let min_shade = |asset: &renderer_api::VehicleMeshAsset| {
        asset.vertices().iter().map(|v| v.shade).fold(f32::MAX, f32::min)
    };
    assert!(
        min_shade(&charred) < 0.30,
        "a destroyed engine must char its bay: min shade {}",
        min_shade(&charred)
    );
    assert!(
        min_shade(&healthy) > min_shade(&charred),
        "the healthy bake stays brighter than the charred one"
    );
}
