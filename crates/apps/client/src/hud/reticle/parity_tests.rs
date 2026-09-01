//! The seam between the sight and the server, MEASURED (Inny Poziom A1).
//!
//! The reticle's penetration hint and the server's verdict are one function
//! (`game_core::resolve_traced_impact`) — but the sight feeds it from the SNAPSHOT (pose,
//! belts, the target's spec) and the server from the `TankState`, and the trace outcome carries
//! the struck spot between them. That is a seam, and a seam is measured, never trusted: ten
//! thousand traced impacts over the whole roster, every gun's every round, random attitudes,
//! thrown belts, both flanks — the hint and the verdict must agree on every one, or the sight
//! is lying again ("green, then 0").

use game_core::math::HullPose;
use game_core::{ArmorFacing, ArmorZone, TankId, TeamId, TrackHealth, VehicleKind};
use glam::Vec3;
use net::TankSnapshot;
use sim::SimulationState;
use terrain::HeightMap;

use super::*;
use crate::hud::reticle_sweep::{ReticleTraceQuery, reticle_trace};

const PLACEMENTS: usize = 10_000;

/// A tiny deterministic generator: the test must fail the same way twice.
fn lcg(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 33) as f32) / ((1u64 << 31) as f32)
}

#[test]
fn the_reticle_and_the_server_agree_on_ten_thousand_traced_impacts() {
    let heightmap = HeightMap::flat(65, 65, 20.0, 0.0).expect("a flat 1280 m field");
    let field_centre = Vec3::new(640.0, 0.0, 640.0);
    let mut sim = SimulationState::new();
    let shooter_kind = VehicleKind::T54_1951;
    let shooter_id = sim.spawn_tank_with_yaw(TeamId(1), shooter_kind.spec(), field_centre, 0.0);
    let targets: Vec<(VehicleKind, TankId)> = VehicleKind::PLAYABLE
        .iter()
        .map(|&kind| (kind, sim.spawn_tank_with_yaw(TeamId(2), kind.spec(), field_centre, 0.0)))
        .collect();
    let rounds = shooter_kind.spec().gun.ammo_options();

    let mut seed = 0xA1_5EA4_u64;
    let mut hits = 0usize;
    let mut disagreements: Vec<String> = Vec::new();
    for placement in 0..PLACEMENTS {
        let (kind, target_id) = targets[placement % targets.len()];
        let bearing = lcg(&mut seed) * std::f32::consts::TAU;
        let range = 30.0 + lcg(&mut seed) * 370.0;
        let target_position = field_centre + Vec3::new(bearing.sin(), 0.0, bearing.cos()) * range;
        {
            let target = sim.tank_mut(target_id).expect("spawned above");
            target.position = target_position;
            target.yaw_rad = lcg(&mut seed) * std::f32::consts::TAU;
            target.hull_pitch_rad = (lcg(&mut seed) - 0.5) * 0.30;
            target.hull_roll_rad = (lcg(&mut seed) - 0.5) * 0.30;
            target.turret_yaw_rad = lcg(&mut seed) * std::f32::consts::TAU;
            // A third of the belts lie on the ground: the stack must drop them on both sides.
            let belt = |seed: &mut u64| if lcg(seed) < 0.33 { 0 } else { game_core::TRACK_HP_MAX };
            target.tracks = TrackHealth::from_hp_pair([belt(&mut seed), belt(&mut seed)]);
        }
        let shooter = sim.tank(shooter_id).expect("spawned above");
        let target = sim.tank(target_id).expect("spawned above");
        let snapshots = [TankSnapshot::from(shooter), TankSnapshot::from(target)];

        // Every round the gun carries, in turn.
        let mut player_spec = shooter_kind.spec();
        player_spec.gun.shell = rounds[placement % rounds.len()];
        let shell = player_spec.gun.shell;

        // A point somewhere on the target's body, in its own frame: flanks, bow, deck, turret.
        let hitbox = kind.spec().hitbox;
        let local = Vec3::new(
            (lcg(&mut seed) - 0.5) * 1.8 * hitbox.half_width_m,
            0.35 + lcg(&mut seed) * (hitbox.center_y_m + hitbox.half_height_m + 0.6),
            (lcg(&mut seed) - 0.5) * 1.8 * hitbox.half_length_m,
        );
        let aim = target.position + target.hull_pose().basis() * local;
        let muzzle = shooter.muzzle_world_position();
        let hull_pose = shooter.hull_pose();
        let gun_direction = (aim - muzzle).normalize_or_zero();
        let limits = player_spec.gun_pitch_limits_rad();
        let query = ReticleFeedbackQuery {
            heightmap: &heightmap,
            cover: &[],
            water: terrain::WaterView::DRY,
            gun_pitch_limits_rad: limits,
            hull_pose,
            tanks: &snapshots,
            player_spec: &player_spec,
            owner: shooter_id,
            owner_team: TeamId(1),
            muzzle,
            aim,
            gun_direction,
            muzzle_velocity_mps: shell.muzzle_velocity_mps,
            drag_per_s: shell.drag_per_s(),
        };
        // The shot the sight asks for, exactly as `reticle_report` flies it.
        let solution = crate::aim::firing_solution(
            muzzle,
            aim,
            hull_pose,
            limits,
            shell.muzzle_velocity_mps,
            shell.drag_per_s(),
        );
        let fired = solution.map_or(gun_direction, |s| s.world_direction);
        let outcome = reticle_trace(ReticleTraceQuery {
            heightmap: &heightmap,
            cover: &[],
            water: terrain::WaterView::DRY,
            tanks: &snapshots,
            owner: shooter_id,
            owner_team: TeamId(1),
            muzzle,
            gun_direction: fired,
            muzzle_velocity_mps: shell.muzzle_velocity_mps,
            projectile_radius_m: shell.collision_radius_m(),
            drag_per_s: shell.drag_per_s(),
        });

        let hint = penetration_from_outcome(&query, &outcome);
        let verdict = sim::verdict_for_traced_impact(&shell, target, &outcome);
        match (hint, verdict) {
            (Some(hint), Some(verdict)) => {
                hits += 1;
                let armor_agrees = (hint.armor_mm - verdict.effective_armor_mm).abs() < 1.0e-3;
                let pen_agrees = (hint.shell_pen_mm
                    - (verdict.effective_armor_mm + verdict.remaining_penetration_mm))
                    .abs()
                    < 1.0e-3;
                if hint.penetrates != verdict.penetrated || !armor_agrees || !pen_agrees {
                    disagreements.push(format!(
                        "#{placement} {kind:?} {:?} {:?}: sight says pen={} armor={:.1} vs server pen={} armor={:.1}",
                        shell.shell_type,
                        outcome,
                        hint.penetrates,
                        hint.armor_mm,
                        verdict.penetrated,
                        verdict.effective_armor_mm
                    ));
                }
            }
            (None, None) => {}
            (hint, verdict) => disagreements.push(format!(
                "#{placement} {kind:?}: one side saw a tank hit and the other did not ({hint:?} vs {verdict:?}) on {outcome:?}"
            )),
        }
    }
    assert!(
        hits * 10 >= PLACEMENTS * 4,
        "the sweep must actually strike armour to measure anything: {hits} hits of {PLACEMENTS}"
    );
    assert!(
        disagreements.is_empty(),
        "{} of {hits} traced impacts split the sight from the server; first: {}",
        disagreements.len(),
        disagreements[0]
    );
}

/// The regression itself, in one contact: a belt standing in front of the side plate is a
/// STACK, and the sight prices the stack, not the bare band it used to read — the hint that
/// said "green" over a 20 mm band while the server charged band + belt + side plate.
#[test]
fn a_track_hit_prices_the_belt_and_the_side_plate_behind_it() {
    let heightmap = HeightMap::flat(8, 8, 20.0, 0.0).expect("a flat field");
    let shooter_kind = VehicleKind::T54_1951;
    let player_spec = shooter_kind.spec();
    let shell = player_spec.gun.shell;
    let target_kind = VehicleKind::T54_1951;
    let mut target = super::tests::tank_snapshot(TankId(2), [100.0, 0.0, 300.0]);
    target.vehicle = target_kind;
    target.track_hp = [game_core::TRACK_HP_MAX; 2];
    let tanks = [target];
    let muzzle = Vec3::new(100.0, 1.7, 0.0);
    let query = ReticleFeedbackQuery {
        heightmap: &heightmap,
        cover: &[],
        water: terrain::WaterView::DRY,
        gun_pitch_limits_rad: player_spec.gun_pitch_limits_rad(),
        hull_pose: HullPose::level(0.0),
        tanks: &tanks,
        player_spec: &player_spec,
        owner: TankId(1),
        owner_team: TeamId(1),
        muzzle,
        aim: Vec3::new(100.0, 0.6, 300.0),
        gun_direction: Vec3::Z,
        muzzle_velocity_mps: shell.muzzle_velocity_mps,
        drag_per_s: shell.drag_per_s(),
    };
    // A contact on the left belt, square on, 300 m out — as the trace would report it.
    let contact = |belts: [u8; 2]| {
        let mut tanks = tanks.clone();
        tanks[0].track_hp = belts;
        let query = ReticleFeedbackQuery { tanks: &tanks, ..query };
        let outcome = sim::TraceOutcome::Tank {
            id: TankId(2),
            facing: ArmorFacing::HullSide,
            zone: ArmorZone::LeftTrack,
            impact_angle_degrees: 0.0,
            hit_position: Vec3::new(101.6, 0.6, 300.0),
            distance_m: 300.0,
            thickness_scale: 1.0,
            direction: -Vec3::X,
        };
        penetration_from_outcome(&query, &outcome).expect("a tank contact has a hint")
    };
    let standing = contact([game_core::TRACK_HP_MAX; 2]);
    let thrown = contact([0, 0]);
    let bare_band = game_core::resolve_penetration_at_distance_on_zone(
        &shell,
        &target_kind.spec().hull,
        ArmorZone::LeftTrack,
        0.0,
        300.0,
    );
    assert!(
        standing.armor_mm > bare_band.effective_armor_mm,
        "the sight must price the stack ({} mm), not the bare band ({} mm)",
        standing.armor_mm,
        bare_band.effective_armor_mm
    );
    assert!(
        thrown.armor_mm < standing.armor_mm,
        "a thrown belt stops screening: {} mm must be under {} mm",
        thrown.armor_mm,
        standing.armor_mm
    );
}
