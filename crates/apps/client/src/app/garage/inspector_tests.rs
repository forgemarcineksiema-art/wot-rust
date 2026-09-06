//! The inspector's seam, MEASURED (interface program G11, Inny Poziom L1): the inspector reads
//! the plate under a click through the same `resolve_traced_impact` the server's verdict goes
//! through — but the inspector feeds it from the garage's parked hero and the server from a
//! `TankState`. A thousand rays over the hero, every round the crew can load: the two must
//! agree on every one, or the inspector is a picture of the armour and not the armour.

use game_core::{TeamId, VehicleKind};
use glam::Vec3;
use sim::{SimulationState, TraceOutcome};

use super::GarageState;
use super::inspector::{INSPECTOR_RANGE_M, InspectorPoint};

/// xorshift32: a deterministic scatter of cursor positions, so a disagreement repeats.
fn next_unit(state: &mut u32) -> f32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

#[test]
fn the_inspector_equals_the_shell_on_a_thousand_points() {
    let mut state = GarageState::default();
    state.open();
    state.set_inspector(true);
    let pose = state.drive_in_pose();
    let position = Vec3::new(0.0, scene_build::hangar::TURNTABLE_TOP_M, pose.z);
    // The same hull, parked the same way, as the server would hold it.
    let mut sim = SimulationState::new();
    let id =
        sim.spawn_tank_with_yaw(TeamId(1), VehicleKind::BENCHMARK.spec(), position, pose.yaw_rad);
    {
        let tank = sim.tank_mut(id).expect("spawned above");
        tank.position = position;
        tank.yaw_rad = pose.yaw_rad;
        tank.hull_pitch_rad = 0.0;
        tank.hull_roll_rad = 0.0;
        tank.turret_yaw_rad = state.hero_turret_yaw();
    }
    let rounds = state.draft().ammo_options();
    assert!(rounds.len() >= 2, "the benchmark loads more than one round");

    let mut seed = 0x9E37_79B9_u32;
    let mut points = 0;
    let mut attempts = 0;
    let mut disagreements: Vec<String> = Vec::new();
    while points < 1000 {
        attempts += 1;
        assert!(attempts < 40_000, "the hero fills the frame less than the framing promises");
        let clip = [next_unit(&mut seed) * 2.0 - 1.0, next_unit(&mut seed) * 2.0 - 1.0];
        state.set_cursor(clip);
        let Some(hit) = state.hero_hit() else { continue };
        points += 1;
        let point = InspectorPoint::from_hit(&hit);
        let outcome = TraceOutcome::Tank {
            id,
            facing: hit.facing,
            zone: hit.zone,
            impact_angle_degrees: hit.impact_angle_degrees,
            hit_position: hit.hit_position,
            distance_m: INSPECTOR_RANGE_M,
            thickness_scale: hit.thickness_scale,
            direction: hit.direction,
        };
        for shell in &rounds {
            let reading = state.inspector_reading(point, shell);
            let verdict = sim::verdict_for_traced_impact(
                shell,
                sim.tank(id).expect("spawned above"),
                &outcome,
            )
            .expect("a plate hit is a verdict");
            let armour_agrees = (reading.effective_mm - verdict.effective_armor_mm).abs() < 1.0e-3;
            if reading.penetrated != verdict.penetrated
                || reading.ricocheted != verdict.ricocheted
                || !armour_agrees
            {
                disagreements.push(format!(
                    "{:?} {:?} @ {:.1}: inspector pen={} ric={} eff={:.2} vs server pen={} ric={} eff={:.2}",
                    hit.zone,
                    shell.shell_type,
                    hit.impact_angle_degrees,
                    reading.penetrated,
                    reading.ricocheted,
                    reading.effective_mm,
                    verdict.penetrated,
                    verdict.ricocheted,
                    verdict.effective_armor_mm
                ));
            }
            // The reading's own numbers are the shell's and the plate's, not a paraphrase.
            assert!(
                (reading.penetration_mm - shell.penetration_mm_at_distance(INSPECTOR_RANGE_M))
                    .abs()
                    < 1.0e-6
            );
            assert!(reading.nominal_mm > 0.0 && reading.effective_mm > 0.0);
        }
    }
    assert_eq!(points, 1000);
    assert!(
        disagreements.is_empty(),
        "{} of {points} points disagree with the server:\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
}
