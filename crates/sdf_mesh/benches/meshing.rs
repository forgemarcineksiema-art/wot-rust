//! Benchmark the SDF meshing hot path: the cast T-54 turret to a ~9k-triangle budget. The pivot's
//! geometry is sampled-and-meshed at bake time, so this is the cost a richer roster multiplies.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use game_core::{VehicleBlueprint, VehicleKind};
use sdf_mesh::{mesh_within_budget, t54_turret};
use vehicle_geometry::{MaterialRole, SmoothingGroup};

fn bench_turret_mesh(c: &mut Criterion) {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let (turret, min, max) = t54_turret(&bp.hybrid().expect("T-54 hybrid visual").turret);
    c.bench_function("mesh_t54_turret_9k", |b| {
        b.iter(|| {
            mesh_within_budget(
                black_box(&turret),
                min,
                max,
                9_000,
                MaterialRole::CastArmor,
                SmoothingGroup(2),
            )
        })
    });
}

criterion_group!(benches, bench_turret_mesh);
criterion_main!(benches);
