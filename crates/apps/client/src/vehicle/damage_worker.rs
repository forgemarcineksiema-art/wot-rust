//! Single bounded worker for per-instance armor topology. Gameplay and analytical clipping never
//! wait for it; a completed result is integrated at most once per rendered frame.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use game_core::{ArmorBreachSet, ArmorFrame};
use glam::Vec3;
use renderer_api::VehicleMeshAsset;
use vehicle_geometry::{GeometryMesh, remesh_aperture};

#[derive(Debug)]
pub(crate) struct DamageMeshJob {
    pub label: String,
    pub source: GeometryMesh,
    pub breaches: ArmorBreachSet,
    pub frame: ArmorFrame,
    pub pivot: Vec3,
}

#[derive(Debug)]
pub(crate) struct DamageMeshResult {
    pub label: String,
    pub asset: Option<VehicleMeshAsset>,
    pub build_time: Duration,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DamageMeshBudgetReport {
    pub sample_count: usize,
    pub worker_p95_ms: f32,
    pub integration_p95_ms: f32,
}

#[derive(Debug, Default)]
pub(crate) struct DamageMeshTelemetry {
    worker: VecDeque<Duration>,
    integration: VecDeque<Duration>,
}

impl DamageMeshTelemetry {
    const WINDOW: usize = 128;

    pub fn record(&mut self, worker: Duration, integration: Duration) {
        push_bounded(&mut self.worker, worker, Self::WINDOW);
        push_bounded(&mut self.integration, integration, Self::WINDOW);
    }

    pub fn report(&self) -> DamageMeshBudgetReport {
        DamageMeshBudgetReport {
            sample_count: self.worker.len().min(self.integration.len()),
            worker_p95_ms: percentile_95_ms(&self.worker),
            integration_p95_ms: percentile_95_ms(&self.integration),
        }
    }
}

#[derive(Debug)]
pub(crate) struct DamageMeshWorker {
    jobs: Sender<DamageMeshJob>,
    results: Receiver<DamageMeshResult>,
}

impl Default for DamageMeshWorker {
    fn default() -> Self {
        let (job_tx, job_rx) = mpsc::channel::<DamageMeshJob>();
        let (result_tx, result_rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("damage-mesh-worker".into())
            .spawn(move || {
                while let Ok(job) = job_rx.recv() {
                    let started = Instant::now();
                    let asset = build_damage_mesh(&job).map(|mesh| {
                        super::asset_catalog::vehicle_mesh_asset_from_geometry(&mesh, job.pivot)
                    });
                    if result_tx
                        .send(DamageMeshResult {
                            label: job.label,
                            asset,
                            build_time: started.elapsed(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("damage mesh worker thread");
        Self { jobs: job_tx, results: result_rx }
    }
}

fn push_bounded(samples: &mut VecDeque<Duration>, sample: Duration, capacity: usize) {
    if samples.len() == capacity {
        samples.pop_front();
    }
    samples.push_back(sample);
}

fn percentile_95_ms(samples: &VecDeque<Duration>) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<Duration> = samples.iter().copied().collect();
    sorted.sort_unstable();
    let index = (sorted.len() * 95).div_ceil(100) - 1;
    sorted[index].as_secs_f32() * 1_000.0
}

impl DamageMeshWorker {
    pub fn schedule(&self, job: DamageMeshJob) {
        let _ = self.jobs.send(job);
    }

    pub fn try_result(&self) -> Option<DamageMeshResult> {
        self.results.try_recv().ok()
    }

    pub fn wait_result(&self) -> Option<DamageMeshResult> {
        self.results.recv_timeout(std::time::Duration::from_secs(2)).ok()
    }
}

fn build_damage_mesh(job: &DamageMeshJob) -> Option<GeometryMesh> {
    let mut mesh = job.source.clone();
    let mut applied = 0_usize;
    for lobe in job
        .breaches
        .breaches()
        .iter()
        .filter(|breach| breach.frame == job.frame)
        .flat_map(|breach| breach.lobes())
    {
        if let Ok(remeshed) = remesh_aperture(&mesh, *lobe)
            && patch_is_bounded(&mesh, &remeshed, *lobe)
        {
            mesh = remeshed;
            applied += 1;
        }
    }
    if applied == 0 {
        return None;
    }
    if let Some(rim) = super::aperture_rim::build_rim_mesh(&job.breaches, job.frame) {
        mesh = merge_geometry(&mesh, &rim);
    }
    Some(mesh)
}

fn patch_is_bounded(
    source: &GeometryMesh,
    candidate: &GeometryMesh,
    lobe: game_core::ApertureLobe,
) -> bool {
    let first_new = source.vertex_count() as u32;
    let radius = lobe.outer.major_radius_m.max(lobe.outer.minor_radius_m);
    let max_edge = radius * 2.8 + 0.10;
    let max_projected = radius * 2.4 + 0.08;
    let normal = lobe.entry_normal_local.normalize_or_zero();
    let (u, v) = game_core::armor_surface_basis(normal, lobe.direction_local);
    if normal == Vec3::ZERO || u == Vec3::ZERO || v == Vec3::ZERO {
        return false;
    }
    candidate.indices().chunks_exact(3).all(|triangle| {
        if !triangle.iter().any(|index| *index >= first_new) {
            return true;
        }
        let points = [triangle[0], triangle[1], triangle[2]]
            .map(|index| candidate.vertices()[index as usize].position);
        let edges = [
            points[0].distance(points[1]),
            points[1].distance(points[2]),
            points[2].distance(points[0]),
        ];
        let face = (points[1] - points[0]).cross(points[2] - points[0]);
        let locally_bounded = points.into_iter().all(|point| {
            let delta = point - lobe.entry_local;
            Vec3::new(delta.dot(u), delta.dot(v), 0.0).length() <= max_projected
                && delta.dot(normal).abs() <= 0.11
        });
        edges.into_iter().all(|edge| edge.is_finite() && edge <= max_edge)
            && locally_bounded
            && face.length_squared().is_finite()
            && face.length_squared() > 1.0e-10
            && face.normalize().dot(normal) > 0.12
    })
}

fn merge_geometry(base: &GeometryMesh, addition: &GeometryMesh) -> GeometryMesh {
    let mut vertices = base.vertices().to_vec();
    let offset = vertices.len() as u32;
    vertices.extend_from_slice(addition.vertices());
    let mut indices = base.indices().to_vec();
    indices.extend(addition.indices().iter().map(|index| index + offset));
    GeometryMesh::new(vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_reports_a_bounded_nearest_rank_p95() {
        let mut telemetry = DamageMeshTelemetry::default();
        for millis in 1..=100 {
            telemetry.record(Duration::from_millis(millis), Duration::from_micros(millis * 10));
        }
        let report = telemetry.report();
        assert_eq!(report.sample_count, 100);
        assert_eq!(report.worker_p95_ms, 95.0);
        assert!((report.integration_p95_ms - 0.95).abs() < 1.0e-4);

        for millis in 101..=240 {
            telemetry.record(Duration::from_millis(millis), Duration::ZERO);
        }
        assert_eq!(telemetry.report().sample_count, DamageMeshTelemetry::WINDOW);
    }
}
