use engine::PresentationTank;
use game_core::{TankId, TeamId};
use glam::{Mat4, Vec3};
use net::TankSnapshot;
use renderer_api::{ArmorApertureRender, ArmorDamageInstance, RenderFrame, RenderObject};
use terrain::HeightMap;
use vehicle_geometry::{GearDynamics, RunningGearKinematics};

use super::asset_render::tank_vehicle_render_objects_posed;
use super::variation::VehicleVariation;
use crate::{VehicleAssetCatalog, VehicleMeshCatalog, tank_render_objects};

#[derive(Debug, Clone, PartialEq)]
pub struct VehicleRenderFrame {
    pub objects: Vec<RenderObject>,
    pub armor_damage: Vec<ArmorDamageInstance>,
}

/// The player's own paint, and the enemy's. Named once so the two render paths cannot drift.
const FRIENDLY_HULL: [f32; 3] = [0.30, 0.40, 0.28];
const ENEMY_HULL: [f32; 3] = [0.46, 0.29, 0.25];

/// A vehicle's render objects are laid out `[hull, turret, gun, ...running gear]`, contiguous
/// per tank.
///
/// Five passes reach straight into that layout — the recoil stroke, the player's barrel scale,
/// the garage's gun stretch, the wreck's turret pop-off and the battle-scar override — and each
/// used to spell the position as a bare `2`. That is a convention, and this repository has
/// already written down what happens to conventions. Naming the slots does not make the layout
/// safe on its own, which is why `every_vehicle_lays_its_objects_out_hull_turret_gun` checks the
/// roster against the catalog's own named handles.
pub const VEHICLE_HULL_OBJECT: usize = 0;
pub const VEHICLE_TURRET_OBJECT: usize = 1;
pub const VEHICLE_GUN_OBJECT: usize = 2;

/// The team the player is fighting for, read off the presentation list.
///
/// `None` only if the player's own tank is absent from the frame, in which case there is no team
/// to be friendly TO and the caller falls back to the identity rule.
fn player_team_of(tanks: &[PresentationTank], player_tank: TankId) -> Option<TeamId> {
    tanks.iter().find(|tank| tank.id == player_tank).map(|tank| tank.team)
}

/// Friend or foe paint — keyed on TEAM.
///
/// It used to key on `tank.id == player_tank`, which meant the player was the only vehicle in the
/// world wearing friendly green and **every ally rendered in the enemy's red-brown**. In a 7v7
/// that is six of the thirteen other tanks on the field mis-identified, and no amount of looking
/// at the picture fixes a player shooting at the wrong colour. `PresentationTank` has carried
/// `team` all along.
fn hull_color(
    tank: &PresentationTank,
    player_tank: TankId,
    player_team: Option<TeamId>,
) -> [f32; 3] {
    let friendly = match player_team {
        Some(team) => tank.team == team,
        None => tank.id == player_tank,
    };
    if friendly { FRIENDLY_HULL } else { ENEMY_HULL }
}

pub fn split_vehicle_render_frame(
    catalog: &mut VehicleMeshCatalog,
    tanks: Vec<PresentationTank>,
    player_tank: TankId,
    player_gun_scale: f32,
) -> VehicleRenderFrame {
    let mut objects = Vec::new();
    let player_team = player_team_of(&tanks, player_tank);
    for tank in tanks {
        let is_player = tank.id == player_tank;
        let hull_color = hull_color(&tank, player_tank, player_team);
        let snapshot = render_snapshot(&tank);
        let mut tank_objects = tank_render_objects(catalog, &snapshot, hull_color);
        // The player's installed gun may have a longer/shorter barrel than the baked stock mesh;
        // stretch its gun submesh (index 2: [hull, turret, gun]) along the barrel axis to match the
        // muzzle the sim fires from. Enemies are always stock (scale 1.0).
        scale_player_gun(&mut tank_objects, is_player, player_gun_scale);
        objects.append(&mut tank_objects);
    }
    VehicleRenderFrame { objects, armor_damage: Vec::new() }
}

pub fn split_pbr_vehicle_render_frame(
    catalog: &mut VehicleAssetCatalog,
    tanks: Vec<PresentationTank>,
    player_tank: TankId,
    player_gun_scale: f32,
) -> VehicleRenderFrame {
    split_pbr_vehicle_render_frame_on_terrain(
        catalog,
        tanks,
        player_tank,
        player_gun_scale,
        None,
        0,
        // No camera: this wrapper serves the harnesses and tests that pose a vehicle rather than
        // stand in a battle, and they are all close looks.
        None,
    )
}

/// As [`split_pbr_vehicle_render_frame`], with the local heightmap so each tank's road wheels can
/// ride the ground under them (per-wheel suspension travel) and its track tension can follow the
/// hull's drive state.
/// `now_tick` is the latest authoritative tick: fresh perforations cool their thermal glow
/// against it. Callers without a battle clock pass 0 (glow reads as long cold).
pub fn split_pbr_vehicle_render_frame_on_terrain(
    catalog: &mut VehicleAssetCatalog,
    tanks: Vec<PresentationTank>,
    player_tank: TankId,
    player_gun_scale: f32,
    terrain: Option<&HeightMap>,
    now_tick: u64,
    eye: Option<[f32; 3]>,
) -> VehicleRenderFrame {
    let mut objects = Vec::new();
    let mut armor_damage = Vec::new();
    let player_team = player_team_of(&tanks, player_tank);
    for tank in tanks {
        // Identity still decides the GUN: only the player's installed barrel may differ from the
        // baked stock mesh. Paint is the team's business; the barrel is this tank's.
        let is_player = tank.id == player_tank;
        let hull_color = hull_color(&tank, player_tank, player_team);
        let snapshot = render_snapshot(&tank);
        if let Some(damage) = armor_damage_instance(&snapshot, now_tick) {
            armor_damage.push(damage);
        }
        let variation = VehicleVariation::from_snapshot(&snapshot);
        // How finely this tank's running gear is built THIS frame. The camera is the only thing
        // that decides it, and it is the only thing that can: 204 instances of full-detail gear
        // on a tank four pixels tall is the cost the one-look policy has no room for. Callers
        // with no camera (the garage, the probes, the offscreen lineups) pass none and get the
        // authored construction.
        let gear_detail = eye.map_or(vehicle_geometry::GearDetail::Near, |eye| {
            let distance =
                glam::Vec3::from_array(eye).distance(glam::Vec3::from_array(tank.translation));
            vehicle_geometry::gear_detail_for_distance(distance)
        });
        let (left_travel, right_travel) = wheel_travel(&tank, terrain);
        // A driven track pulls its top run tight; braking or coasting lets it hang. The gain is
        // gentle: the P/v launch hits ~8 m/sÂ˛, and a sag that slams to its clamp on a throttle
        // tap reads as the track convulsing rather than tensioning. A hard landing (the sprung
        // hull dips below the replicated height) throws extra slack into both runs for a beat,
        // and a THROWN track loses its tension entirely â€” that side hangs deep and dead.
        let landing_slack = (-tank.attitude_heave_m).max(0.0) * 2.5;
        let sag_scale = (1.0 - tank.accel_long_mps2 * 0.05 + landing_slack).clamp(0.72, 1.5);
        let damage = game_core::TrackDamageMask::from_bits(tank.track_damage_mask);
        let side_sag = |broken: bool| if broken { 2.2 } else { sag_scale };
        let dynamics = GearDynamics {
            left_travel: &left_travel,
            right_travel: &right_travel,
            left_sag_scale: side_sag(damage.is_broken(game_core::TrackSide::Left)),
            right_sag_scale: side_sag(damage.is_broken(game_core::TrackSide::Right)),
            left_break_t: tank.track_break_t[0],
            right_break_t: tank.track_break_t[1],
        };
        let mut tank_objects = tank_vehicle_render_objects_posed(
            catalog,
            &snapshot,
            hull_color,
            &variation,
            tank.track_left_m,
            tank.track_right_m,
            [tank.attitude_pitch_rad, tank.attitude_roll_rad, tank.attitude_heave_m],
            dynamics,
            gear_detail,
        );
        recoil_gun(&mut tank_objects, tank.gun_recoil_m);
        scale_player_gun(&mut tank_objects, is_player, player_gun_scale);
        objects.append(&mut tank_objects);
    }
    VehicleRenderFrame { objects, armor_damage }
}

/// Whether a vehicle's visual skin is truth-aligned with its armor volumes, so an aperture may
/// really OPEN it (analytic discard + remeshed skin). True for the part-aware hybrid T-54 today;
/// the rest of the fleet joins with its hybrid migration (W2c), and until then a hole into
/// nothing would be dishonest — those hulls scorch instead.
pub(crate) fn vehicle_has_cut_truth(kind: game_core::VehicleKind) -> bool {
    // The capability IS the data: a vehicle whose blueprint carries the part-aware visual detail
    // block has an analytic breach path; one without it scorches. When the fleet migration gives
    // another hull that block (W4 F5), it joins here with no client edit — which is the whole
    // point of the dispatch rule that flushed out the old `kind == T54_1951` version.
    game_core::VehicleBlueprint::for_vehicle(kind).is_some_and(|bp| bp.visual_detail().is_some())
}

pub fn armor_damage_instance(
    snapshot: &TankSnapshot,
    now_tick: u64,
) -> Option<ArmorDamageInstance> {
    let cut = vehicle_has_cut_truth(snapshot.vehicle);
    let pose = super::pose::VehiclePose::from_snapshot(snapshot);
    let mut apertures = Vec::new();
    for breach in snapshot.armor_breaches.breaches() {
        let age_s = now_tick.saturating_sub(breach.created_tick) as f32
            / sim::DEFAULT_SIMULATION_TICK_HZ as f32;
        let glow = breach_glow(breach.shell_type, age_s);
        for lobe in breach.lobes() {
            let (point, basis) = match breach.frame {
                game_core::ArmorFrame::Hull => {
                    (pose.hull_point(lobe.entry_local), pose.hull_basis())
                }
                game_core::ArmorFrame::Turret => {
                    (pose.turret_point(lobe.entry_local), pose.turret_basis())
                }
                game_core::ArmorFrame::Mantlet => {
                    (pose.gun_point(lobe.entry_local), pose.gun_basis())
                }
            };
            let (tangent, _) =
                game_core::armor_surface_basis(lobe.entry_normal_local, lobe.direction_local);
            let phase_a = hash_phase(lobe.fracture_seed);
            let phase_b = hash_phase(lobe.fracture_seed.rotate_left(29));
            apertures.push(ArmorApertureRender {
                center: point.to_array(),
                normal: (basis * lobe.entry_normal_local).normalize_or_zero().to_array(),
                tangent: (basis * tangent).normalize_or_zero().to_array(),
                major_radius_m: lobe.outer.major_radius_m,
                minor_radius_m: lobe.outer.minor_radius_m,
                rotation_rad: lobe.outer.rotation_rad,
                irregularity: lobe.outer.irregularity,
                phase_a,
                phase_b,
                half_depth_m: (lobe.thickness_m + 0.025).clamp(0.04, 0.45),
                glow,
                glow_tightness: breach_glow_tightness(breach.shell_type),
                cut,
            });
        }
    }
    (!apertures.is_empty()).then_some(ArmorDamageInstance { tank_id: snapshot.tank_id, apertures })
}

/// Thermal rim intensity of a perforation `age_s` after impact. The glow is ENERGY speaking:
/// kinetic AP/APCR bore heat runs short and modest, a HEAT jet's melt burns hottest and
/// lingers, HE barely marks the plate. Pure exponential cooling — never a permanent edge.
/// The physics of the timescale (Fizyczny Świat P6): a torn steel lip is thin, and thin steel
/// dumps its heat in a FRACTION of a second — what stays is the soot, not the light show.
pub fn breach_glow(shell_type: game_core::ShellType, age_s: f32) -> f32 {
    let (peak, tau_s) = match shell_type {
        game_core::ShellType::ArmorPiercing => (0.55, 0.45),
        game_core::ShellType::Apcr => (0.45, 0.35),
        game_core::ShellType::Heat => (1.0, 1.2),
        game_core::ShellType::HighExplosive => (0.22, 0.3),
    };
    peak * (-age_s.max(0.0) / tau_s).exp()
}

/// How tightly the glow hugs the contour: a HEAT jet's melt is a narrow bright ring, kinetic
/// rounds heat a softer, wider rim.
pub fn breach_glow_tightness(shell_type: game_core::ShellType) -> f32 {
    match shell_type {
        game_core::ShellType::Heat => 2.4,
        _ => 1.0,
    }
}

fn hash_phase(seed: u64) -> f32 {
    let mixed = game_core::math::splitmix64(seed);
    ((mixed >> 40) as f32) / ((1_u64 << 24) as f32) * std::f32::consts::TAU
}

/// Slide the gun submesh back along its own barrel axis by the live recoil stroke. Applied
/// BEFORE the player barrel scale so the stroke stays in real meters â€” a long gun recoils the
/// same distance as its stock sibling, it does not stretch the recoil with the mesh.
fn recoil_gun(objects: &mut [RenderObject], recoil_m: f32) {
    if recoil_m > 1.0e-4
        && let Some(gun) = objects.get_mut(VEHICLE_GUN_OBJECT)
    {
        let recoiled = Mat4::from_cols_array_2d(&gun.transform)
            * Mat4::from_translation(Vec3::new(0.0, 0.0, -recoil_m));
        gun.transform = recoiled.to_cols_array_2d();
    }
}

/// Per-wheel vertical travel from the terrain under each road wheel: the residual between the
/// ground height at the wheel and the sprung hull's ground plane there. Wheels drop into dips and
/// ride over bumps the hull attitude has already averaged out.
fn wheel_travel(tank: &PresentationTank, terrain: Option<&HeightMap>) -> (Vec<f32>, Vec<f32>) {
    let (Some(map), Some(kin)) = (terrain, RunningGearKinematics::for_vehicle(tank.vehicle)) else {
        return (Vec::new(), Vec::new());
    };
    let mut left = vec![0.0; kin.wheel_zs.len()];
    let mut right = vec![0.0; kin.wheel_zs.len()];
    let (sin, cos) = tank.hull_yaw_rad.sin_cos();
    let (bx, bz) = (tank.translation[0], tank.translation[2]);
    let hull_y = tank.translation[1];
    let (pitch, roll) = (tank.attitude_pitch_rad, tank.attitude_roll_rad);
    for (index, &wz) in kin.wheel_zs.iter().enumerate() {
        for (lane, side) in [(&mut left, -1.0_f32), (&mut right, 1.0)] {
            let lx = side * kin.wheel_x;
            // Hull-local (lx, wz) into the world; the sprung ground plane tilts with the attitude.
            let wx = bx + cos * lx + sin * wz;
            let wzw = bz - sin * lx + cos * wz;
            let plane = hull_y + pitch.tan() * wz + roll.tan() * lx;
            let ground = map.sample_height(wx, wzw).unwrap_or(plane);
            lane[index] = (ground - plane).clamp(-0.08, 0.20);
        }
    }
    (left, right)
}

fn scale_player_gun(objects: &mut [RenderObject], is_player: bool, player_gun_scale: f32) {
    if is_player
        && (player_gun_scale - 1.0).abs() > 1.0e-3
        && let Some(gun) = objects.get_mut(VEHICLE_GUN_OBJECT)
    {
        let scaled = Mat4::from_cols_array_2d(&gun.transform)
            * Mat4::from_scale(Vec3::new(1.0, 1.0, player_gun_scale));
        gun.transform = scaled.to_cols_array_2d();
    }
}

/// Adapt a presentation entity into the pose-only `TankSnapshot` the procedural mesh kernels
/// consume. The fields the meshes never read (`reload_remaining_s`, `aim_dispersion_mrad`) are
/// zeroed â€” they belong to the player's HUD path, not vehicle geometry.
pub(crate) fn render_snapshot(tank: &PresentationTank) -> TankSnapshot {
    TankSnapshot {
        tank_id: tank.id,
        team: tank.team,
        vehicle: tank.vehicle,
        position: tank.translation,
        yaw_rad: tank.hull_yaw_rad,
        // The PRESENTATION attitude (authoritative base + weight-transfer spring): the pose
        // built from this snapshot tilts exactly like the sprung hull the player sees.
        hull_pitch_rad: tank.attitude_pitch_rad,
        hull_roll_rad: tank.attitude_roll_rad,
        turret_yaw_rad: tank.turret_yaw_rad,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: tank.gun_pitch_rad,
        hit_points: tank.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: tank.module_hit_points,
        destroyed_modules_mask: tank.destroyed_modules_mask,
        track_damage_mask: tank.track_damage_mask,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        // Ammo is HUD state, not geometry: the mesh kernels never read it (like reload above).
        ammo_counts: [0; game_core::MAX_AMMO_SLOTS],
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: tank.armor_breaches.clone(),
        track_break_t: tank.track_break_t,
        engine_fire: tank.engine_fire,
        fuel_fire: tank.fuel_fire,
    }
}

pub fn render_frame_from_objects(objects: Vec<RenderObject>) -> RenderFrame {
    RenderFrame { objects, ..RenderFrame::default() }
}

#[cfg(test)]
mod tests {
    use game_core::{
        ApertureLobe, ArmorBreach, ArmorBreachDescriptor, ArmorFrame, ArmorMaterial,
        ArmorSurfaceId, ArmorZone, BreachContour, BreachFace, ShellType, TankId, TeamId,
        VehicleKind,
    };
    use glam::Vec3;
    use net::TankSnapshot;

    use super::{
        ENEMY_HULL, FRIENDLY_HULL, armor_damage_instance, breach_glow, breach_glow_tightness,
        hull_color, player_team_of,
    };

    fn presentation_tank(id: u64, team: u16) -> engine::PresentationTank {
        engine::PresentationTank {
            id: TankId(id),
            team: TeamId(team),
            vehicle: VehicleKind::T54_1951,
            translation: [0.0, 0.0, 0.0],
            hull_yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: 1000,
            destroyed_modules_mask: 0,
            spotted_by_teams_mask: 0,
            module_hit_points: [1; game_core::MODULE_SLOT_COUNT],
            track_damage_mask: 0,
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            armor_breaches: Default::default(),
            track_left_m: 0.0,
            track_right_m: 0.0,
            attitude_pitch_rad: 0.0,
            attitude_roll_rad: 0.0,
            attitude_heave_m: 0.0,
            accel_long_mps2: 0.0,
            gun_recoil_m: 0.0,
        }
    }

    /// Friend or foe is a TEAM question. Keying it on the player's own id painted every ally in
    /// the enemy's red-brown — six of the thirteen other tanks on a 7v7 field mis-identified,
    /// which is a readability bug, not a look one.
    #[test]
    fn allies_wear_the_players_paint_and_only_the_other_team_wears_the_enemys() {
        let player = TankId(7);
        let tanks = vec![presentation_tank(7, 1), presentation_tank(3, 1), presentation_tank(9, 2)];
        let team = player_team_of(&tanks, player);
        assert_eq!(team, Some(TeamId(1)), "the player's team is read off the frame");

        assert_eq!(hull_color(&tanks[0], player, team), FRIENDLY_HULL, "the player");
        assert_eq!(hull_color(&tanks[1], player, team), FRIENDLY_HULL, "an ALLY, not the player");
        assert_eq!(hull_color(&tanks[2], player, team), ENEMY_HULL, "the other team");
    }

    /// With no player tank in the frame there is no team to be friendly to, so the rule falls
    /// back to identity rather than inventing an allegiance and painting the field wrong.
    #[test]
    fn a_frame_without_the_player_falls_back_to_identity() {
        let player = TankId(7);
        let tanks = vec![presentation_tank(3, 1), presentation_tank(9, 2)];
        assert_eq!(player_team_of(&tanks, player), None);
        assert_eq!(hull_color(&tanks[0], player, None), ENEMY_HULL);
        assert_eq!(hull_color(&tanks[1], player, None), ENEMY_HULL);
    }

    fn breached_snapshot(kind: VehicleKind) -> TankSnapshot {
        let entry = Vec3::new(0.1, 1.2, 1.5);
        let normal = Vec3::new(0.0, 0.3, 0.954);
        let mut breaches = game_core::ArmorBreachSet::default();
        breaches.add(ArmorBreach::new(
            ArmorBreachDescriptor {
                breach_id: 5,
                surface: ArmorSurfaceId::new(ArmorFrame::Hull, ArmorZone::UpperGlacis),
                frame: ArmorFrame::Hull,
                zone: ArmorZone::UpperGlacis,
                material: ArmorMaterial::RolledSteel,
                face: BreachFace::Ingress,
                shell_type: ShellType::ArmorPiercing,
                created_tick: 60,
                impact_angle_degrees: 10.0,
                impact_energy_kj: 900.0,
                projectile_diameter_m: 0.1,
                residual_penetration_mm: 40.0,
            },
            ApertureLobe {
                entry_local: entry,
                exit_local: entry - normal * 0.1,
                entry_normal_local: normal,
                exit_normal_local: -normal,
                direction_local: -normal,
                thickness_m: 0.1,
                outer: BreachContour::new(0.06, 0.05, 0.2, 0.1),
                inner: BreachContour::new(0.08, 0.07, 0.3, 0.12),
                fracture_seed: 9,
            },
        ));
        TankSnapshot {
            tank_id: TankId(3),
            team: TeamId(2),
            vehicle: kind,
            position: [4.0, 0.0, 9.0],
            yaw_rad: 0.4,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.1,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: 900,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 1.0,
            module_hit_points: [100; game_core::MODULE_SLOT_COUNT],
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: breaches,
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
        }
    }

    /// Fleet honesty: EVERY vehicle's penetration is visible. The part-aware T-54 really opens
    /// (cut); the rest of the fleet scorches without a hole into nothing until its hybrid
    /// migration — but no vehicle silently swallows a perforation any more.
    #[test]
    fn every_vehicle_kind_shows_its_perforation() {
        for kind in VehicleKind::ALL {
            let instance = armor_damage_instance(&breached_snapshot(kind), 120)
                .unwrap_or_else(|| panic!("{kind:?} must render its perforation"));
            assert!(!instance.apertures.is_empty());
            let expect_cut = kind == VehicleKind::T54_1951;
            for aperture in &instance.apertures {
                assert_eq!(
                    aperture.cut, expect_cut,
                    "{kind:?}: only cut-truth vehicles may open the mesh"
                );
                assert!(aperture.glow > 0.0, "a 1-second-old breach still glows");
            }
        }
    }

    /// `[hull, turret, gun, ...gear]` stops being a convention here.
    ///
    /// Five passes index straight into that layout — the recoil stroke, the player's barrel
    /// scale, the garage's gun stretch, the wreck's turret pop-off and the battle-scar override.
    /// A vehicle whose bake emitted its submeshes in another order would silently recoil its
    /// turret and stretch its hull, on that vehicle only, with nothing failing. So every vehicle
    /// the player can field is checked against the catalog's OWN named handles.
    #[test]
    fn every_vehicle_lays_its_objects_out_hull_turret_gun() {
        let mut catalog = VehicleMeshCatalog::default();
        for kind in game_core::VehicleKind::PLAYABLE {
            let entry = catalog.vehicle_entry(kind).expect("a playable vehicle bakes geometry");
            let objects =
                tank_render_objects(&mut catalog, &breached_snapshot(kind), FRIENDLY_HULL);
            assert!(
                objects.len() > VEHICLE_GUN_OBJECT,
                "{kind:?} emitted only {} objects",
                objects.len()
            );
            assert_eq!(objects[VEHICLE_HULL_OBJECT].mesh, entry.hull, "{kind:?}: hull slot");
            assert_eq!(objects[VEHICLE_TURRET_OBJECT].mesh, entry.turret, "{kind:?}: turret slot");
            assert_eq!(objects[VEHICLE_GUN_OBJECT].mesh, entry.gun, "{kind:?}: gun slot");
        }
    }

    /// The plan's acceptance line verbatim: glow follows ENERGY (short and weak for AP/APCR,
    /// hotter and local for HEAT), cools monotonically, and never leaves a permanent edge.
    #[test]
    fn breach_glow_cools_by_energy_and_never_stays() {
        for shell in
            [ShellType::ArmorPiercing, ShellType::Apcr, ShellType::Heat, ShellType::HighExplosive]
        {
            let fresh = breach_glow(shell, 0.0);
            assert!(fresh > 0.0 && fresh <= 1.0);
            // Monotonic cooling.
            let mut previous = fresh;
            for step in 1..=30 {
                let value = breach_glow(shell, step as f32 * 0.5);
                assert!(value < previous, "{shell:?} must cool monotonically");
                previous = value;
            }
            // No permanent white edges: effectively dark inside half a minute.
            assert!(breach_glow(shell, 30.0) < 0.005, "{shell:?} must go cold");
            // P6: torn steel is THIN — the light show is over in seconds; soot is what stays.
            if shell != ShellType::Heat {
                assert!(breach_glow(shell, 1.5) < 0.05, "{shell:?} kinetic lip cools in a blink");
            }
            assert!(breach_glow(shell, 4.0) < 0.05, "{shell:?} even the HEAT melt is dark by 4 s");
        }
        // Energy speaks: HEAT burns hottest and longest, APCR shortest of the kinetics.
        assert!(breach_glow(ShellType::Heat, 0.0) > breach_glow(ShellType::ArmorPiercing, 0.0));
        assert!(breach_glow(ShellType::Heat, 6.0) > breach_glow(ShellType::ArmorPiercing, 6.0));
        assert!(
            breach_glow(ShellType::ArmorPiercing, 3.0) > breach_glow(ShellType::Apcr, 3.0),
            "APCR bore heat dies before AP"
        );
        // HEAT hugs the contour in a narrower ring.
        assert!(breach_glow_tightness(ShellType::Heat) > breach_glow_tightness(ShellType::Apcr));
        // A clock-less caller (now_tick 0 with a future created_tick) clamps to fresh, not NaN.
        assert!(breach_glow(ShellType::ArmorPiercing, -5.0).is_finite());
    }

    use super::*;

    /// A full 7v7 (14 tanks) of the instance-heaviest playable vehicle must fit the renderer's
    /// vehicle instance buffer. When it does not, `set_vehicle_render_frame` truncates tanks off
    /// the frame — and before it truncated, the whole oversized upload was silently dropped,
    /// freezing every vehicle on screen the moment enough tanks were spotted. This locks the
    /// budget against roster growth and running-gear detail growth alike.
    #[test]
    fn worst_case_7v7_battle_fits_the_vehicle_instance_budget() {
        let per_tank_worst = VehicleKind::PLAYABLE
            .iter()
            .map(|&kind| {
                let gear = RunningGearKinematics::for_vehicle(kind).map_or(0, |kin| {
                    vehicle_geometry::running_gear_placements_dynamic(
                        &kin,
                        0.0,
                        0.0,
                        GearDynamics::default(),
                    )
                    .len()
                });
                3 + gear // hull + turret + gun + every animated gear part
            })
            .max()
            .expect("playable roster is non-empty");

        // Shed track ribbons (D6) ride the same instance buffer, capped by their own pool.
        // Bands on the field plus the 14-link remnant that may hang over each thrown sprocket.
        let ribbons = crate::vehicle::track_ribbon::MAX_TRACK_RIBBONS
            * (crate::vehicle::track_ribbon::MAX_RIBBON_LINKS + 14);
        let battle_worst = 14 * per_tank_worst + ribbons;
        assert!(
            battle_worst <= renderer_wgpu::vehicle_instance_budget(),
            "a 14-tank battle can submit {battle_worst} vehicle instances but the renderer budget \
             holds {}; grow VEHICLE_INSTANCE_CAPACITY before shipping this roster/gear detail",
            renderer_wgpu::vehicle_instance_budget(),
        );
    }

    #[test]
    fn worst_case_7v7_battle_fits_the_grouped_aperture_budget() {
        let per_tank = game_core::MAX_ARMOR_BREACHES
            * game_core::MAX_BREACH_FRAGMENTS_PER_GROUP
            * game_core::MAX_APERTURE_LOBES;
        let battle_worst = 14 * per_tank;
        assert!(
            battle_worst <= renderer_wgpu::armor_damage_aperture_budget(),
            "grouped damage can submit {battle_worst} contours but the renderer holds only {}",
            renderer_wgpu::armor_damage_aperture_budget(),
        );
    }

    #[test]
    fn render_snapshot_carries_every_pose_field_the_meshes_read() {
        let tank = PresentationTank {
            id: TankId(7),
            team: game_core::TeamId(2),
            vehicle: VehicleKind::TigerII,
            translation: [1.0, 2.0, 3.0],
            hull_yaw_rad: 0.4,
            turret_yaw_rad: 0.5,
            gun_pitch_rad: -0.1,
            hit_points: 1200,
            destroyed_modules_mask: 0b101,
            spotted_by_teams_mask: 0,
            module_hit_points: [11, 22, 33, 44, 55, 66],
            track_damage_mask: 0,
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            armor_breaches: Default::default(),
            track_left_m: 0.0,
            track_right_m: 0.0,
            attitude_pitch_rad: 0.0,
            attitude_roll_rad: 0.0,
            attitude_heave_m: 0.0,
            accel_long_mps2: 0.0,
            gun_recoil_m: 0.0,
        };

        let snapshot = render_snapshot(&tank);

        assert_eq!(snapshot.tank_id, TankId(7));
        assert_eq!(snapshot.vehicle, VehicleKind::TigerII);
        assert_eq!(snapshot.position, [1.0, 2.0, 3.0]);
        assert_eq!(snapshot.yaw_rad, 0.4);
        assert_eq!(snapshot.turret_yaw_rad, 0.5);
        assert_eq!(snapshot.gun_pitch_rad, -0.1);
        assert_eq!(snapshot.destroyed_modules_mask, 0b101);
        // Live module HP rides through so partial damage can drive mesh-side cues.
        assert_eq!(snapshot.module_hit_points, [11, 22, 33, 44, 55, 66]);
    }

    #[test]
    fn terrain_travel_samples_all_nine_tiger_family_wheels() {
        let terrain = HeightMap::flat(64, 64, 1.0, 0.15).expect("valid test terrain");
        for vehicle in [VehicleKind::TigerII, VehicleKind::Jagdtiger] {
            let tank = PresentationTank {
                id: TankId(7),
                team: TeamId(1),
                vehicle,
                translation: [30.0, 0.0, 30.0],
                hull_yaw_rad: 0.0,
                turret_yaw_rad: 0.0,
                gun_pitch_rad: 0.0,
                hit_points: 1,
                destroyed_modules_mask: 0,
                spotted_by_teams_mask: 0,
                module_hit_points: [1; game_core::MODULE_SLOT_COUNT],
                track_damage_mask: 0,
                track_break_t: [None, None],
                engine_fire: false,
                fuel_fire: false,
                armor_breaches: Default::default(),
                track_left_m: 0.0,
                track_right_m: 0.0,
                attitude_pitch_rad: 0.0,
                attitude_roll_rad: 0.0,
                attitude_heave_m: 0.0,
                accel_long_mps2: 0.0,
                gun_recoil_m: 0.0,
            };

            let (left, right) = wheel_travel(&tank, Some(&terrain));

            assert_eq!(left.len(), 9, "{vehicle:?} must sample every road-wheel station");
            assert_eq!(right.len(), 9, "{vehicle:?} must sample every road-wheel station");
            assert!((left[8] - 0.15).abs() < 1.0e-5, "{vehicle:?} left rear wheel");
            assert!((right[8] - 0.15).abs() < 1.0e-5, "{vehicle:?} right rear wheel");
        }
    }
}
