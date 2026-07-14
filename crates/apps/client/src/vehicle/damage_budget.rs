//! Representative Honest Steel budget capture: a deterministic battle-shaped hit sequence on
//! production T-54s, driven through the real bounded worker with at most one integration per
//! simulated frame. This is the measurement behind the phase-8 performance gate (worker build
//! p95 < 8 ms, per-frame main-thread damage work < 0.5 ms, at most one damage-mesh upload per
//! frame); the budget itself is a review gate read off `cargo run --release --example
//! damage_budget_capture`, never a flaky timing assert in CI.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use game_core::{
    ApertureLobe, ArmorBreach, ArmorBreachDescriptor, ArmorBreachSet, ArmorFrame, ArmorMaterial,
    ArmorSurfaceId, ArmorZone, BreachContour, BreachFace, ShellType, TankId, VehicleKind,
};
use glam::Vec3;
use vehicle_geometry::{MeshContactIndex, SubmeshKind};

use super::asset_catalog::VehicleAssetCatalog;
use super::damage_worker::{DamageMeshBudgetReport, percentile_95_ms};

const KIND: VehicleKind = VehicleKind::T54_1951;
const TANKS: u32 = 12;
const HITS: usize = 150;
const DRAIN_TIMEOUT: Duration = Duration::from_secs(60);

/// The outcome of one capture run. `report` carries the rolling worker/integration p95 exactly as
/// the game reads it; `main_thread_p95_ms` is the per-simulated-frame main-thread damage cost
/// (one bounded integration plus one hit's schedule, including the source-mesh clone into the job).
#[derive(Debug, Clone, Copy)]
pub struct DamageBudgetCapture {
    pub report: DamageMeshBudgetReport,
    pub hits: usize,
    pub scheduled: usize,
    pub completed: usize,
    pub main_thread_p95_ms: f32,
}

/// Deterministic anchor probes spread over the plates a battle actually hits. Every probe snaps
/// to the real production surface, so the capture survives any future bake change.
fn anchor_probes() -> [(ArmorFrame, ArmorZone, SubmeshKind, Vec3); 12] {
    [
        (ArmorFrame::Hull, ArmorZone::UpperGlacis, SubmeshKind::Hull, Vec3::new(0.35, 1.2, 2.7)),
        (ArmorFrame::Hull, ArmorZone::UpperGlacis, SubmeshKind::Hull, Vec3::new(-0.35, 1.2, 2.7)),
        (ArmorFrame::Hull, ArmorZone::LowerPlate, SubmeshKind::Hull, Vec3::new(0.0, 0.6, 2.9)),
        (ArmorFrame::Hull, ArmorZone::HullSide, SubmeshKind::Hull, Vec3::new(1.2, 1.0, 0.4)),
        (ArmorFrame::Hull, ArmorZone::HullSide, SubmeshKind::Hull, Vec3::new(-1.2, 1.0, -0.4)),
        (ArmorFrame::Hull, ArmorZone::HullRear, SubmeshKind::Hull, Vec3::new(0.0, 1.0, -2.7)),
        (
            ArmorFrame::Turret,
            ArmorZone::TurretFront,
            SubmeshKind::Turret,
            Vec3::new(0.42, 1.86, 0.98),
        ),
        (
            ArmorFrame::Turret,
            ArmorZone::TurretFront,
            SubmeshKind::Turret,
            Vec3::new(-0.42, 1.86, 0.98),
        ),
        (ArmorFrame::Turret, ArmorZone::TurretSide, SubmeshKind::Turret, Vec3::new(0.9, 1.9, 0.1)),
        (
            ArmorFrame::Turret,
            ArmorZone::TurretSide,
            SubmeshKind::Turret,
            Vec3::new(-0.9, 1.9, -0.3),
        ),
        (ArmorFrame::Mantlet, ArmorZone::Mantlet, SubmeshKind::Gun, Vec3::new(0.12, 1.83, 1.45)),
        (ArmorFrame::Mantlet, ArmorZone::Mantlet, SubmeshKind::Gun, Vec3::new(-0.12, 1.83, 1.45)),
    ]
}

fn fragment(
    hit: u64,
    frame: ArmorFrame,
    zone: ArmorZone,
    entry: Vec3,
    normal: Vec3,
) -> ArmorBreach {
    let seed = game_core::math::splitmix64(hit.wrapping_mul(0x9e37_79b9));
    let radius = 0.04 + game_core::math::hash_unit(seed) * 0.03;
    let thickness = 0.12;
    ArmorBreach::new(
        ArmorBreachDescriptor {
            breach_id: hit,
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
            created_tick: hit,
            impact_angle_degrees: 12.0,
            impact_energy_kj: 1_000.0,
            projectile_diameter_m: radius * 2.0,
            residual_penetration_mm: 60.0,
        },
        ApertureLobe {
            entry_local: entry,
            exit_local: entry - normal * thickness,
            entry_normal_local: normal,
            exit_normal_local: -normal,
            direction_local: -normal,
            thickness_m: thickness,
            outer: BreachContour::new(radius, radius * 0.85, 0.3, 0.11),
            inner: BreachContour::new(radius * 1.4, radius * 1.2, 0.4, 0.14),
            fracture_seed: seed,
        },
    )
}

/// Run the representative capture on a fresh catalog and return the measured budgets.
pub fn capture_damage_mesh_budget() -> DamageBudgetCapture {
    let mut catalog = VehicleAssetCatalog::default();
    let baked = catalog.cached_bake(KIND).expect("T-54 bakes");
    let indices: HashMap<SubmeshKind, MeshContactIndex> =
        [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun]
            .into_iter()
            .map(|submesh| {
                let mesh = &baked.submesh(submesh).expect("submesh").mesh;
                (submesh, MeshContactIndex::from_mesh(mesh, Vec3::ZERO))
            })
            .collect();

    let probes = anchor_probes();
    let mut sets: HashMap<TankId, ArmorBreachSet> = HashMap::new();
    let mut main_thread = VecDeque::new();
    let mut scheduled = 0_usize;
    let mut completed = 0_usize;

    for hit in 0..HITS {
        let frame_started = Instant::now();
        if catalog.integrate_one_damage_mesh() {
            completed += 1;
        }
        let tank = TankId(u64::from(1 + (hit as u32) % TANKS));
        let (frame, zone, submesh, probe) = probes[(hit * 5 + hit / probes.len()) % probes.len()];
        let seed = game_core::math::splitmix64(hit as u64);
        let jitter = Vec3::new(
            (game_core::math::hash_unit(seed) - 0.5) * 0.12,
            (game_core::math::hash_unit(seed.rotate_left(17)) - 0.5) * 0.12,
            (game_core::math::hash_unit(seed.rotate_left(31)) - 0.5) * 0.12,
        );
        let contact = indices[&submesh]
            .nearest_point(probe + jitter, 2.0)
            .expect("every probe snaps to the armor");
        let set = sets.entry(tank).or_default();
        let before = super::aperture_rim::frame_hash(set, frame);
        set.add(fragment(hit as u64, frame, zone, contact.position, contact.normal.normalize()));
        if super::aperture_rim::frame_hash(set, frame) != before {
            scheduled += 1;
            let _ = catalog.damaged_frame_mesh(KIND, tank, frame, set, 0, 0);
        }
        main_thread.push_back(frame_started.elapsed());
    }

    let drain_started = Instant::now();
    while completed < scheduled {
        assert!(
            drain_started.elapsed() < DRAIN_TIMEOUT,
            "the worker lost {} of {scheduled} scheduled bakes",
            scheduled - completed
        );
        let frame_started = Instant::now();
        if catalog.integrate_one_damage_mesh() {
            completed += 1;
            main_thread.push_back(frame_started.elapsed());
        } else {
            std::thread::sleep(Duration::from_micros(200));
        }
    }

    DamageBudgetCapture {
        report: catalog.damage_mesh_budget_report(),
        hits: HITS,
        scheduled,
        completed,
        main_thread_p95_ms: percentile_95_ms(&main_thread),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locks the capture plumbing, never the timings: every scheduled bake completes, the rolling
    /// telemetry window fills, and the measured numbers are real (finite, positive). The 8 ms /
    /// 0.5 ms budget itself is a review gate on the release-mode example, not a CI assert.
    #[test]
    fn the_capture_completes_every_scheduled_bake_and_fills_the_window() {
        let capture = capture_damage_mesh_budget();
        assert_eq!(capture.completed, capture.scheduled);
        assert!(
            capture.scheduled >= 128,
            "a representative capture must fill the 128-sample telemetry window, \
             got {} scheduled bakes",
            capture.scheduled
        );
        assert_eq!(capture.report.sample_count, 128);
        assert!(capture.report.worker_p95_ms.is_finite() && capture.report.worker_p95_ms > 0.0);
        assert!(capture.report.integration_p95_ms.is_finite());
        assert!(capture.main_thread_p95_ms.is_finite() && capture.main_thread_p95_ms > 0.0);
    }
}
