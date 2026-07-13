//! Single bounded worker for per-instance armor topology. Gameplay and analytical clipping never
//! wait for it; a completed result is integrated at most once per rendered frame.

use std::sync::mpsc::{self, Receiver, Sender};

use game_core::{ArmorBreachSet, ArmorFrame};
use glam::Vec3;
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
    pub mesh: Option<GeometryMesh>,
    pub pivot: Vec3,
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
                    let result = build_damage_mesh(&job);
                    if result_tx
                        .send(DamageMeshResult { label: job.label, mesh: result, pivot: job.pivot })
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
