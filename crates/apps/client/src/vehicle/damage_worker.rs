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
    pub kind: game_core::VehicleKind,
    /// Module slots whose interior components render the Damaged variant (scorched paint).
    pub damaged_modules: u8,
    /// Module slots whose interior components render the Destroyed/Burning variant (charred).
    pub destroyed_modules: u8,
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

pub(crate) fn percentile_95_ms(samples: &VecDeque<Duration>) -> f32 {
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
    char_damaged_components(&mut mesh, job);
    if let Some(rim) = super::aperture_rim::build_rim_mesh(&job.breaches, job.frame) {
        mesh = merge_geometry(&mesh, &rim);
    }
    Some(mesh)
}

/// The interior's Damaged/Burning variants: components whose module the battle has hurt darken
/// in the per-instance skin — scorched paint for a damaged module, charred black for a destroyed
/// one. Vertex-level, driven by the authoritative hit volumes, and only ever on this tank's own
/// baked copy (shared production meshes stay pristine, per the Honest Steel contract).
fn char_damaged_components(mesh: &mut GeometryMesh, job: &DamageMeshJob) {
    if job.damaged_modules == 0 && job.destroyed_modules == 0 {
        return;
    }
    let layout = game_core::DamageLayout::for_vehicle(job.kind);
    let center_y = job.kind.spec().hitbox.center_y_m;
    let volumes: Vec<(&game_core::DamageShape, f32)> = layout
        .components()
        .iter()
        .filter_map(|component| {
            if component.frame != job.frame {
                return None;
            }
            let bit = component.slot.destroyed_mask_bit();
            let shade = if job.destroyed_modules & bit != 0 {
                0.28
            } else if job.damaged_modules & bit != 0 {
                0.55
            } else {
                return None;
            };
            Some((&component.shape, shade))
        })
        .collect();
    if volumes.is_empty() {
        return;
    }
    let interior = |material: vehicle_geometry::MaterialRole| {
        matches!(
            material,
            vehicle_geometry::MaterialRole::InteriorPrimer
                | vehicle_geometry::MaterialRole::InteriorMachinery
                | vehicle_geometry::MaterialRole::Ammunition
        )
    };
    let mut vertices = mesh.vertices().to_vec();
    let mut touched = false;
    for vertex in &mut vertices {
        if !interior(vertex.material) {
            continue;
        }
        for (shape, shade) in &volumes {
            if shape_contains(shape, vertex.position, center_y) {
                vertex.surface_shade = vertex.surface_shade.min(*shade);
                touched = true;
                break;
            }
        }
    }
    if touched {
        *mesh = GeometryMesh::new(vertices, mesh.indices().to_vec());
    }
}

/// Point-in-volume for the layout shapes, padded a little so a component's dressing chars with
/// it. Layout volumes live in the hitbox-center frame; the mesh is in vehicle coordinates.
fn shape_contains(shape: &game_core::DamageShape, position: Vec3, center_y: f32) -> bool {
    const PAD: f32 = 0.09;
    let local = position - Vec3::Y * center_y;
    match shape {
        game_core::DamageShape::Obb { center, half_extents, yaw_rad } => {
            let delta = local - *center;
            let (sin, cos) = (-yaw_rad).sin_cos();
            let rotated =
                Vec3::new(delta.x * cos - delta.z * sin, delta.y, delta.x * sin + delta.z * cos);
            rotated.x.abs() <= half_extents.x + PAD
                && rotated.y.abs() <= half_extents.y + PAD
                && rotated.z.abs() <= half_extents.z + PAD
        }
        game_core::DamageShape::Cylinder { center, axis, half_length, radius } => {
            let delta = local - *center;
            let along = delta.dot(*axis);
            along.abs() <= *half_length + PAD && (delta - *axis * along).length() <= radius + PAD
        }
        game_core::DamageShape::Capsule { a, b, radius } => {
            let ab = *b - *a;
            let t = ((local - *a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            (local - (*a + ab * t)).length() <= radius + PAD
        }
        game_core::DamageShape::Convex { planes, bounds_min, bounds_max } => {
            local.cmpge(*bounds_min - Vec3::splat(PAD)).all()
                && local.cmple(*bounds_max + Vec3::splat(PAD)).all()
                && planes.iter().all(|plane| local.dot(plane.normal) <= plane.offset + PAD)
        }
    }
}

/// Sanity gate on a rebuilt patch before it replaces the analytical clip. Only the NEW steel is
/// bounded: contour vertices must hug the aperture, and every rebuilt triangle must be finite and
/// face outward. Pre-existing source vertices are trusted wherever they sit — on production cast
/// lofts the patch boundary ring legitimately reaches past any radius derived from the lobe alone,
/// because its extent follows the source triangle size, not the hole size.
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
    let new_vertex_is_local = candidate.vertices()[first_new as usize..].iter().all(|vertex| {
        let delta = vertex.position - lobe.entry_local;
        let projected = Vec3::new(delta.dot(u), delta.dot(v), 0.0).length();
        delta.is_finite() && projected <= max_projected && delta.dot(normal).abs() <= 0.11
    });
    new_vertex_is_local
        && candidate.indices().chunks_exact(3).all(|triangle| {
            if !triangle.iter().any(|index| *index >= first_new) {
                return true;
            }
            let points = [triangle[0], triangle[1], triangle[2]]
                .map(|index| candidate.vertices()[index as usize].position);
            let contour_edges_bounded = [(0, 1), (1, 2), (2, 0)].into_iter().all(|(a, b)| {
                if triangle[a] < first_new || triangle[b] < first_new {
                    return true;
                }
                let edge = points[a].distance(points[b]);
                edge.is_finite() && edge <= max_edge
            });
            let face = (points[1] - points[0]).cross(points[2] - points[0]);
            // The kernel already orients every rebuilt triangle toward the entry normal; this
            // backstop only rejects backfacing or collapsed output. On a rounded casting rim an
            // honest patch triangle can lie nearly perpendicular to the entry normal, so the
            // threshold is "positive with numerical margin", not a cone.
            contour_edges_bounded
                && face.length_squared().is_finite()
                && face.length_squared() > 1.0e-10
                && face.normalize().dot(normal) > 0.02
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
