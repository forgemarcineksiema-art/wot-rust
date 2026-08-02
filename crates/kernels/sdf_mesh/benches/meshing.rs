//! Benchmark the SDF meshing hot path at bake-time scale. The subject is a SYNTHETIC
//! three-sphere smooth union: the metaball T-54 turret that used to stand here left with the
//! dead composition module (2026-08-02) - what this bench keeps honest is the production
//! mesher (`vehicle_build::part` calls `mesh_within_budget`), and a kernel cost gauge needs a
//! workload, not a vehicle.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use glam::Vec3;
use sdf::Sdf;
use sdf_mesh::mesh_within_budget;
use vehicle_geometry::{MaterialRole, SmoothingGroup};

fn sphere_at(center: Vec3, radius: f32) -> Sdf {
    Sdf::Rigid {
        rotation: glam::Quat::IDENTITY,
        translation: center,
        node: Box::new(Sdf::Sphere { radius }),
    }
}

fn bench_synthetic_mesh(c: &mut Criterion) {
    let blob = Sdf::SmoothUnion {
        a: Box::new(Sdf::SmoothUnion {
            a: Box::new(sphere_at(Vec3::new(0.0, 1.6, 0.1), 1.1)),
            b: Box::new(sphere_at(Vec3::new(0.0, 1.5, -0.3), 0.9)),
            radius: 0.5,
        }),
        b: Box::new(sphere_at(Vec3::new(0.45, 1.75, 0.55), 0.55)),
        radius: 0.55,
    };
    let (min, max) = (Vec3::new(-1.4, 0.4, -1.4), Vec3::new(1.6, 2.9, 1.7));
    c.bench_function("mesh_synthetic_blob_9k", |b| {
        b.iter(|| {
            mesh_within_budget(
                black_box(&blob),
                min,
                max,
                9_000,
                MaterialRole::CastArmor,
                SmoothingGroup(2),
            )
        })
    });
}

criterion_group!(benches, bench_synthetic_mesh);
criterion_main!(benches);
