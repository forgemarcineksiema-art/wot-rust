//! Visual showcase of the "Honest Steel" destruction program, rendered offscreen through the same
//! PBR path the game uses. Writes:
//!   `_cover_intact.png`   — a farm town, whole.
//!   `_cover_wounded.png`  — the same building wounded but standing: a kinetic inset in its
//!                            plaster burst, an HE bite with rubble at the wall's foot (v32).
//!   `_cover_wrecked.png`  — the same town after HE: rubble mounds where buildings stood, a tree
//!                            line cleared, its trees gone with it.
//!   `_cover_felled.png`   — the cleared tree line up close: stumps and fallen trunks (P11).
//!   `_battle_damage.png`  — a live tank pocked with penetration holes seated on the armor.
//!   `_surface_wounds.png` — the same tank two metres off: a bounce's dish and a ricochet's gouge
//!                            as material on the plate (Z5).
//!   `_breach_close.png`   — the glacis penetration from 1.4 m: curled torn tongues with bright
//!                            edges around a dark cavity (Z6).
//!   `_interior_detail.png` — a close side perforation exposing physical internal assemblies.
//!   `_wreck.png`          — a knocked-out hull: charred, gun drooped off the aim, scarred.
//!   `_turret_popoff.png`  — an ammo-rack kill: the turret flung off and settled beside the hull.
//!
//! `cargo run -p client --example probe -- destruction_showcase -- target/destruction`

use std::f32::consts::FRAC_PI_2;
use std::fs::File;
use std::io::BufWriter;

use client::{
    HitDecal, TrackRibbon, TurretPopoff, VehicleAssetCatalog, append_hit_wounds_to,
    battlefield_scene_mesh, battlefield_scene_mesh_with_cover_states, render_frame_from_objects,
    tank_vehicle_render_objects,
};
use game_core::{
    ArmorZone, DamageCause, DamageEvent, ModuleSlot, MountFrames, TankId, TeamId, VehicleKind,
};
use glam::{Mat4, Vec3};
use net::TankSnapshot;
use renderer_api::{
    Camera, CameraProjectionPolicy, FxVertex, RenderFrame, SceneLighting, view_projection_matrix,
};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::{BattlefieldMap, StaticCoverKind};

const KIND: VehicleKind = VehicleKind::T54_1951;

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let prefix = crate::sub_arg(1).unwrap_or_else(|| "target/destruction".to_string());
    let (width, height) = (1100u32, 640u32);

    let mut catalog = VehicleAssetCatalog::default();
    if let Err(error) = catalog.load_forge_artifact_tree("target/forge") {
        eprintln!("note: no Forge artifacts ({error}); neutral material");
    }

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);

    // ---- Cover: a farm town before and after HE ----------------------------------------------
    cover_shots(&ctx, &target, &battlefield, width, height, &prefix)?;

    // ---- Vehicle destruction: damaged / wreck / turret pop-off -------------------------------
    vehicle_shots(&ctx, &target, &mut catalog, &battlefield, width, height, &prefix)?;

    Ok(())
}

/// The town intact, then the same cover states stepped to rubble/gone — the phase-aware scene
/// builder does the rest.
fn cover_shots(
    ctx: &GpuContext,
    target: &OffscreenTarget,
    battlefield: &BattlefieldMap,
    width: u32,
    height: u32,
    prefix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Frame tight on the first farm building, with the ground under it.
    let building = battlefield
        .static_cover
        .iter()
        .position(|cover| cover.kind == StaticCoverKind::FarmBuilding)
        .expect("map has a farm building");
    let center = Vec3::from_array(battlefield.static_cover[building].center);
    let half = Vec3::from_array(battlefield.static_cover[building].half_extents_m);
    let ground = center.y - half.y;
    let reach = half.x.max(half.z) + 9.0;
    let focus = Vec3::new(center.x, ground + 1.0, center.z);
    let eye = [center.x + reach, ground + half.y + 5.0, center.z + reach];
    let look = focus.to_array();

    // Intact.
    let (verts, indices) = battlefield_scene_mesh(battlefield);
    render_scene(ctx, target, &verts, &indices, focus, eye, look, width, height)?;
    write_png(ctx, target, width, height, &format!("{prefix}_cover_intact.png"))?;

    // Wounded but standing (protocol v32): a kinetic inset with its plaster burst and an HE
    // bite with rubble at the wall's foot, on the face toward the camera.
    let scars = [
        terrain::CoverScar {
            cover: building as u16,
            face: 0,
            u_q: 90,
            v_q: 110,
            radius_q: 3,
            kind: terrain::COVER_SCAR_KIND_KINETIC,
        },
        terrain::CoverScar {
            cover: building as u16,
            face: 0,
            u_q: 175,
            v_q: 90,
            radius_q: 18,
            kind: terrain::COVER_SCAR_KIND_HIGH_EXPLOSIVE,
        },
    ];
    let (mut verts, mut indices) = client::battlefield_ground_mesh(battlefield);
    let (sv, si) = client::battlefield_statics_mesh_with_scars(battlefield, &[], &scars);
    let base = verts.len() as u32;
    verts.extend(sv);
    indices.extend(si.into_iter().map(|index| index + base));
    render_scene(ctx, target, &verts, &indices, focus, eye, look, width, height)?;
    write_png(ctx, target, width, height, &format!("{prefix}_cover_wounded.png"))?;

    // Bring the whole town down: farm buildings -> rubble, tree lines -> gone.
    let mut states = vec![0u8; battlefield.static_cover.len()];
    for (index, cover) in battlefield.static_cover.iter().enumerate() {
        states[index] = match cover.kind {
            StaticCoverKind::FarmBuilding => 1, // rubble
            StaticCoverKind::TreeLine => 2,     // gone
            _ => 0,
        };
    }
    let (verts, indices) = battlefield_scene_mesh_with_cover_states(battlefield, &states);
    render_scene(ctx, target, &verts, &indices, focus, eye, look, width, height)?;
    write_png(ctx, target, width, height, &format!("{prefix}_cover_wrecked.png"))?;

    // The felled tree line up close (P11): stumps where the trees stood, trunks along the run.
    let tree_line = battlefield
        .static_cover
        .iter()
        .position(|cover| cover.kind == StaticCoverKind::TreeLine)
        .expect("map has a tree line");
    let tl_center = Vec3::from_array(battlefield.static_cover[tree_line].center);
    let tl_half = Vec3::from_array(battlefield.static_cover[tree_line].half_extents_m);
    let tl_ground = tl_center.y - tl_half.y;
    let tl_focus = Vec3::new(tl_center.x, tl_ground + 0.5, tl_center.z);
    let tl_eye = [tl_center.x + 9.0, tl_ground + 4.5, tl_center.z + 11.0];
    render_scene(
        ctx,
        target,
        &verts,
        &indices,
        tl_focus,
        tl_eye,
        tl_focus.to_array(),
        width,
        height,
    )?;
    write_png(ctx, target, width, height, &format!("{prefix}_cover_felled.png"))?;
    Ok(())
}

fn vehicle_shots(
    ctx: &GpuContext,
    target: &OffscreenTarget,
    catalog: &mut VehicleAssetCatalog,
    battlefield: &BattlefieldMap,
    width: u32,
    height: u32,
    prefix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cx, cz) = (340.0_f32, 300.0_f32);
    let ground = battlefield.heightmap.sample_height(cx, cz).unwrap_or(0.0);
    let (terrain_vertices, terrain_indices) = battlefield_scene_mesh(battlefield);
    let tc = [cx, ground + 1.15, cz];
    let color = [0.42, 0.46, 0.34];
    let gun_dead = ModuleSlot::Gun.destroyed_mask_bit();

    // Build all three object sets up front — the first call registers the T-54's meshes in the
    // catalog, so one renderer (registered once, below) serves every shot.
    let live = tank_snapshot(cx, ground, cz, 1000, 0);
    let _ = tank_vehicle_render_objects(catalog, &live, color);
    catalog.finish_damage_mesh_jobs();
    let damage_budget = catalog.damage_mesh_budget_report();
    if damage_budget.sample_count > 0 {
        eprintln!(
            "damage mesh samples={} worker_p95={:.3}ms integration_p95={:.3}ms",
            damage_budget.sample_count,
            damage_budget.worker_p95_ms,
            damage_budget.integration_p95_ms,
        );
    }
    let live_objects = tank_vehicle_render_objects(catalog, &live, color);
    let mut live_damage: Vec<renderer_api::ArmorDamageInstance> =
        client::armor_damage_instance(&live, 30).into_iter().collect();
    // The bounce's dish and the ricochet's gouge are material on the plate (Z5): wound records
    // beside the breaches, nothing in the FX batch.
    append_hit_wounds_to(&mut live_damage, &battle_decals(catalog, &live, cx, ground, cz), &live);
    let live_fx: Vec<FxVertex> = Vec::new();

    let wreck = tank_snapshot(cx, ground, cz, 0, gun_dead);
    let wreck_objects = tank_vehicle_render_objects(catalog, &wreck, color);
    let mut wreck_damage: Vec<renderer_api::ArmorDamageInstance> =
        client::armor_damage_instance(&wreck, 30).into_iter().collect();
    append_hit_wounds_to(
        &mut wreck_damage,
        &battle_decals(catalog, &wreck, cx, ground, cz),
        &wreck,
    );
    let wreck_fx: Vec<FxVertex> = Vec::new();

    // The de-tracked flank (Fizyczny Świat): the live snapshot above already carries the
    // thrown LEFT track, so its side renders bare wheels — and the loop itself lies beside
    // the hull as the shed band, instanced from the same unit link mesh.
    let mut thrown_objects = tank_vehicle_render_objects(catalog, &live, color);
    let ribbon = TrackRibbon::shed(
        TankId(1),
        KIND,
        game_core::TrackSide::Left,
        Vec3::new(cx, ground, cz),
        FRAC_PI_2,
        Some(&battlefield.heightmap),
    );
    thrown_objects.extend(client::ribbon_render_objects(catalog, &ribbon));
    // And the fresh remnant still hanging over the sprocket (phase 2), before it slides off.
    let hull = Mat4::from_translation(Vec3::new(cx, ground, cz)) * Mat4::from_rotation_y(FRAC_PI_2);
    thrown_objects.extend(client::thrown_remnant_objects(
        catalog,
        TankId(1),
        KIND,
        game_core::TrackSide::Left,
        hull,
        0.0,
    ));

    let mut popoff_objects = tank_vehicle_render_objects(catalog, &wreck, color);
    let ring_local = MountFrames::for_vehicle(KIND).turret_ring.translation;
    // yaw = FRAC_PI_2: nose +X, so authoring +Z maps to world +X, +X to world -Z.
    let ring_world = Vec3::new(cx + ring_local.z, ground + ring_local.y, cz - ring_local.x);
    let mut popoff =
        TurretPopoff::launch(TankId(1), KIND, ring_world, Some(&battlefield.heightmap));
    for _ in 0..400 {
        popoff.tick(0.05); // fly and settle
    }
    // Objects are [hull, turret, gun, ...gear]; drive the turret and gun off with the arc.
    popoff_objects[1].transform = popoff.turret_transform().to_cols_array_2d();
    popoff_objects[2].transform = popoff.gun_transform().to_cols_array_2d();

    // One renderer, meshes registered once.
    let mut renderer = SceneRenderer::for_offscreen(ctx, &terrain_vertices, &terrain_indices)?;
    // The leaf atlas, exactly as the battle binds it — see `bind_battle_foliage_atlas`.
    crate::bind_battle_foliage_atlas(&mut renderer, ctx);
    renderer.scene_lighting = SceneLighting::battlefield_default();
    renderer.shadow_focus = Some([cx, ground, cz]);
    for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
        renderer.register_vehicle_mesh(ctx, handle, &mesh);
    }
    for (handle, maps) in catalog.take_pending_vehicle_materials() {
        renderer.register_vehicle_material(ctx, handle, &maps);
    }

    // The thrown-track shot: from the LEFT flank (nose +X, so the left side faces -Z... the
    // left flank of a +X-facing hull is +Z world) — bare wheels and the band on the ground.
    let thrown_eye = [cx - 5.2, ground + 2.1, cz + 4.6];
    let thrown_look = [cx - 2.3, ground + 1.0, cz];
    draw_vehicle(
        ctx,
        target,
        &mut renderer,
        thrown_objects,
        Vec::new(),
        &[],
        thrown_eye,
        thrown_look,
        width,
        height,
    )?;
    write_png(ctx, target, width, height, &format!("{prefix}_thrown_track.png"))?;

    let close = [cx + 6.0, ground + 3.4, cz + 5.2];
    // Look through the side egress into the fighting compartment, not at the aperture plane. The
    // small longitudinal offset makes the breech, recoil gear and turret drive read as separate
    // volumes instead of collapsing into one dark rectangle.
    let interior_close = [cx + 0.85, ground + 2.15, cz + 2.80];
    let wide = [cx + 7.5, ground + 3.6, cz + 6.5];
    draw_vehicle(
        ctx,
        target,
        &mut renderer,
        live_objects.clone(),
        live_damage.clone(),
        &live_fx,
        close,
        tc,
        width,
        height,
    )?;
    write_png(ctx, target, width, height, &format!("{prefix}_battle_damage.png"))?;
    // The surface wounds up close (Z5): the bounce's dish on the glacis and the ricochet's
    // gouge on the turret front, two metres off — the frame the owner judges them in.
    let wounds_eye = [cx + 5.4, ground + 2.6, cz - 2.1];
    let wounds_look = [cx + 2.4, ground + 1.45, cz - 0.4];
    draw_vehicle(
        ctx,
        target,
        &mut renderer,
        live_objects.clone(),
        live_damage.clone(),
        &live_fx,
        wounds_eye,
        wounds_look,
        width,
        height,
    )?;
    write_png(ctx, target, width, height, &format!("{prefix}_surface_wounds.png"))?;
    // The breach up close (Z6): the glacis penetration from under a metre, ten seconds old so
    // the thermal glow has cooled and the torn tongues and the dark cavity are what is seen —
    // the frame the owner judged the old flaps in.
    let mut cold_damage: Vec<renderer_api::ArmorDamageInstance> =
        client::armor_damage_instance(&live, 30 + 600).into_iter().collect();
    append_hit_wounds_to(&mut cold_damage, &battle_decals(catalog, &live, cx, ground, cz), &live);
    let breach_eye = [cx + 3.45, ground + 1.62, cz - 0.55];
    let breach_look = [cx + 2.7, ground + 1.17, cz - 0.15];
    draw_vehicle(
        ctx,
        target,
        &mut renderer,
        live_objects.clone(),
        cold_damage,
        &live_fx,
        breach_eye,
        breach_look,
        width,
        height,
    )?;
    write_png(ctx, target, width, height, &format!("{prefix}_breach_close.png"))?;
    draw_vehicle(
        ctx,
        target,
        &mut renderer,
        live_objects,
        live_damage,
        &live_fx,
        interior_close,
        [cx + 0.35, ground + 1.80, cz - 0.10],
        width,
        height,
    )?;
    write_png(ctx, target, width, height, &format!("{prefix}_interior_detail.png"))?;
    draw_vehicle(
        ctx,
        target,
        &mut renderer,
        wreck_objects,
        wreck_damage,
        &wreck_fx,
        close,
        tc,
        width,
        height,
    )?;
    write_png(ctx, target, width, height, &format!("{prefix}_wreck.png"))?;
    draw_vehicle(
        ctx,
        target,
        &mut renderer,
        popoff_objects,
        Vec::new(),
        &[],
        wide,
        tc,
        width,
        height,
    )?;
    write_png(ctx, target, width, height, &format!("{prefix}_turret_popoff.png"))?;
    Ok(())
}

/// A few penetration holes on the front, side and turret of the tank at `(cx, ground, cz)`.
fn battle_decals(
    catalog: &VehicleAssetCatalog,
    tank: &TankSnapshot,
    cx: f32,
    ground: f32,
    cz: f32,
) -> Vec<HitDecal> {
    // Seated on the visual mesh through the catalog's contact index, exactly as the battle
    // seats them (a fallback seat lands on the hitbox plane, centimetres inside the casting).
    let contact = catalog.contact_index(KIND);
    // yaw = FRAC_PI_2: nose points +X, right side is -Z-ish; pick surface points and outward normals.
    // Two penetrations, one bounce (the hull side: a dished scuff) and one ricochet (the
    // turret side: a gouge along the departure) — the four marks of game-design §13.1.
    let hits = [
        (
            Vec3::new(cx + 2.6, ground + 1.25, cz),
            ArmorZone::UpperGlacis,
            Vec3::new(0.7, 0.6, 0.0),
            true,
            false,
        ),
        (
            Vec3::new(cx + 1.9, ground + 1.9, cz + 0.2),
            ArmorZone::TurretFront,
            Vec3::new(0.6, 0.5, 0.3),
            true,
            false,
        ),
        (
            Vec3::new(cx + 2.7, ground + 1.18, cz - 0.55),
            ArmorZone::UpperGlacis,
            Vec3::new(0.866, 0.5, 0.0),
            false,
            false,
        ),
        (
            Vec3::new(cx + 0.98, ground + 1.86, cz - 0.25),
            ArmorZone::TurretFront,
            Vec3::new(0.9, 0.35, -0.25),
            false,
            true,
        ),
    ];
    hits.into_iter()
        .filter_map(|(hit, zone, normal, penetrated, ricocheted)| {
            let event = DamageEvent {
                source: TankId(2),
                target: tank.tank_id,
                hit_position: hit,
                damage_hp: 200,
                penetrated,
                ricocheted,
                cause: DamageCause::Shell,
                armor_zone: zone,
                plate_normal: normal.normalize(),
                // A raking approach, so a ricochet has a departure to gouge along.
                shell_direction: (-normal.normalize() * 0.6 + Vec3::new(0.0, -0.35, 0.7))
                    .normalize(),
                ..Default::default()
            };
            client::decal_from_damage_event(&event, tank, contact.as_deref())
        })
        .collect()
}

fn tank_snapshot(
    cx: f32,
    ground: f32,
    cz: f32,
    hp: u32,
    destroyed_modules_mask: u8,
) -> TankSnapshot {
    let spec = KIND.spec();
    TankSnapshot {
        tank_id: TankId(1),
        team: TeamId(1),
        vehicle: KIND,
        position: [cx, ground, cz],
        yaw_rad: FRAC_PI_2,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: hp,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask,
        track_damage_mask: game_core::TrackDamageMask::LEFT.bits(),
        track_hp: [0, game_core::TRACK_HP_MAX],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: demo_breaches(),
        track_break_t: [Some(0.62), None],
        engine_fire: false,
        fuel_fire: false,
        rack_fire_remaining_s: None,
        crew_unconscious_mask: 0,
        crew_weakened_mask: 0,
        crew_down_remaining_s: Default::default(),
        hull_pitch_velocity_rad_s: 0.0,
        hull_roll_velocity_rad_s: 0.0,
    }
}

fn demo_breaches() -> game_core::ArmorBreachSet {
    let mut set = game_core::ArmorBreachSet::default();
    for (id, frame, zone, face, entry, normal, direction, radii) in [
        (
            1,
            game_core::ArmorFrame::Hull,
            ArmorZone::UpperGlacis,
            game_core::BreachFace::Ingress,
            Vec3::new(0.15, 1.18, 2.70),
            Vec3::new(0.0, 0.5, 0.866).normalize(),
            Vec3::new(0.0, -0.5, -0.866).normalize(),
            (0.065, 0.052),
        ),
        (
            2,
            game_core::ArmorFrame::Turret,
            ArmorZone::TurretFront,
            game_core::BreachFace::Ingress,
            Vec3::new(-0.42, 1.86, 0.98),
            Vec3::new(0.0, 0.35, 0.94).normalize(),
            Vec3::new(0.0, -0.35, -0.94).normalize(),
            (0.075, 0.055),
        ),
        (
            3,
            game_core::ArmorFrame::Hull,
            ArmorZone::HullSide,
            game_core::BreachFace::Ingress,
            Vec3::new(-1.32, 1.12, 0.30),
            Vec3::NEG_X,
            Vec3::X,
            (0.072, 0.056),
        ),
        (
            4,
            game_core::ArmorFrame::Turret,
            ArmorZone::TurretSide,
            game_core::BreachFace::Egress,
            Vec3::new(-0.88, 1.92, 0.42),
            Vec3::NEG_X,
            Vec3::X,
            (0.16, 0.12),
        ),
    ] {
        let thickness = 0.18;
        let seed = game_core::math::splitmix64(id);
        let lobe = game_core::ApertureLobe {
            entry_local: entry,
            exit_local: entry + direction * thickness,
            entry_normal_local: normal,
            exit_normal_local: -normal,
            direction_local: direction,
            thickness_m: thickness,
            outer: game_core::BreachContour::new(radii.0, radii.1, id as f32 * 0.37, 0.13),
            inner: game_core::BreachContour::new(
                radii.0 * 1.5,
                radii.1 * 1.4,
                id as f32 * 0.41,
                0.17,
            ),
            fracture_seed: seed,
        };
        let mut breach = game_core::ArmorBreach::new(
            game_core::ArmorBreachDescriptor {
                breach_id: id,
                surface: game_core::ArmorSurfaceId::new(frame, zone),
                frame,
                zone,
                material: if frame == game_core::ArmorFrame::Hull {
                    game_core::ArmorMaterial::RolledSteel
                } else {
                    game_core::ArmorMaterial::CastSteel
                },
                face: game_core::BreachFace::Ingress,
                shell_type: game_core::ShellType::ArmorPiercing,
                created_tick: 1,
                impact_angle_degrees: 18.0,
                impact_energy_kj: 1_200.0,
                projectile_diameter_m: 0.1,
                residual_penetration_mm: 90.0,
            },
            lobe,
        );
        if face == game_core::BreachFace::Egress {
            breach.reshape_as_egress();
        }
        set.add(breach);
    }
    set
}

#[expect(clippy::too_many_arguments)]
fn render_scene(
    ctx: &GpuContext,
    target: &OffscreenTarget,
    verts: &[renderer_api::SceneVertex],
    indices: &[u32],
    focus: Vec3,
    eye: [f32; 3],
    look: [f32; 3],
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut renderer = SceneRenderer::for_offscreen(ctx, verts, indices)?;
    renderer.scene_lighting = SceneLighting::battlefield_default();
    renderer.shadow_focus = Some(focus.to_array());
    let camera = Camera { eye, target: look, vertical_fov_degrees: 40.0 };
    let vp = camera_vp(&camera, width, height);
    renderer.render(ctx, target.render_target(), vp, camera.eye)?;
    Ok(())
}

#[expect(clippy::too_many_arguments)]
fn draw_vehicle(
    ctx: &GpuContext,
    target: &OffscreenTarget,
    renderer: &mut SceneRenderer,
    objects: Vec<renderer_api::RenderObject>,
    armor_damage: Vec<renderer_api::ArmorDamageInstance>,
    fx: &[FxVertex],
    eye: [f32; 3],
    look: [f32; 3],
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut frame: RenderFrame = render_frame_from_objects(objects);
    frame.armor_damage = armor_damage;
    renderer.set_vehicle_render_frame(ctx, &frame);
    renderer.set_fx(ctx, fx);
    let camera = Camera { eye, target: look, vertical_fov_degrees: 34.0 };
    let vp = camera_vp(&camera, width, height);
    renderer.render(ctx, target.render_target(), vp, camera.eye)?;
    Ok(())
}

fn camera_vp(camera: &Camera, width: u32, height: u32) -> [[f32; 4]; 4] {
    let projection = CameraProjectionPolicy::webgpu_default();
    view_projection_matrix(
        camera,
        width as f32 / height as f32,
        projection.near_plane_m(),
        projection.far_plane_m(),
    )
}

fn write_png(
    ctx: &GpuContext,
    target: &OffscreenTarget,
    width: u32,
    height: u32,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pixels = target.read_rgba8(ctx)?;
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("wrote {path}");
    Ok(())
}
