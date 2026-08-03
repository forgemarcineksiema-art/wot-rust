//! The FX pass frame budget, locked as one executable number. Every capped FX pool — the
//! particle pool, the terrain-scar pool, and the per-tank decal lists — is filled to its cap
//! and rendered through the REAL append paths, and the summed worst-case vertex count must
//! equal [`FX_FRAME_VERTEX_BUDGET`]. A Honest Steel phase that adds stamps or raises a
//! cap updates the locked number in the same diff (`docs/honest-steel-policy.md`) — the budget
//! moves consciously, never by drift. Shell tracers are the one uncapped source: they are
//! bounded by live shells (4 s lifetime x fire cadence), not by a pool, so they stay outside
//! this lock.

use game_core::{TankId, TeamId, VehicleKind};
use glam::Vec3;
use net::TankSnapshot;
use terrain::HeightMap;

use super::FxSystem;
use super::decals::append_decal_quads;
use super::particle::{MAX_PARTICLES, Particle};
use super::terrain_scars::{MAX_TERRAIN_SCARS, TerrainScars};
use crate::vehicle::variation::{
    DecalFrame, DecalKind, HitDecal, MAX_HIT_DECALS, VehicleVariation,
};

/// The battle roster the budget is sized for: 7v7.
const BATTLE_TANKS: usize = 14;

/// The locked worst case: a full particle pool + a full crater pool. Penetrations deliberately
/// emit no FX quads; analytical clipping and rim meshes carry them without covering the opening.
const FX_FRAME_VERTEX_BUDGET: usize = 35_070;

fn snapshot() -> TankSnapshot {
    let spec = VehicleKind::BENCHMARK.spec();
    TankSnapshot {
        tank_id: TankId(9),
        team: TeamId(2),
        vehicle: spec.kind,
        position: [40.0, 2.0, 60.0],
        yaw_rad: 0.0,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 1.0,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
        rack_fire_remaining_s: None,
    }
}

fn puff(position: Vec3) -> Particle {
    Particle {
        position,
        velocity_mps: Vec3::ZERO,
        gravity_factor: 0.0,
        drag_per_s: 0.0,
        age_s: 0.0,
        ttl_s: 10.0,
        size_begin_m: 1.0,
        size_end_m: 1.0,
        color_begin: [1.0; 4],
        color_end: [0.0; 4],
        stretch_s: 0.0,
    }
}

/// A tank battered to its penetration cap. The decal pass must still emit zero fake-hole vertices.
fn battered_variation() -> VehicleVariation {
    let mut variation = VehicleVariation::default();
    for index in 0..MAX_HIT_DECALS {
        variation.record_hit(HitDecal {
            local_position: [0.3 + index as f32 * 0.05, 1.0, 2.0],
            local_normal: [0.0, 0.0, 1.0],
            radius: 0.13,
            age_s: 0.0,
            kind: DecalKind::Penetration,
            frame: DecalFrame::Hull,
            // Penetration state is retained for gameplay/persistence but not rendered as a decal.
            patch: None,
        });
    }
    variation
}

#[test]
fn the_fx_frame_vertex_budget_is_locked() {
    // Particle pool at cap, rendered as this frame's batch.
    let mut fx = FxSystem::default();
    for index in 0..MAX_PARTICLES {
        fx.spawn(puff(Vec3::new(index as f32 * 0.5, 1.0, 30.0)));
    }
    let particle_vertices = fx.vertices(Vec3::ZERO, Vec3::Z).len();

    // Crater pool at cap, every mark fresh and fully opaque.
    let map = HeightMap::flat(65, 65, 5.0, 0.0).expect("flat map");
    let mut scars = TerrainScars::default();
    for index in 0..MAX_TERRAIN_SCARS {
        // Worst case alternates the two mark families (furrow stamps vs crater stamps).
        scars.record(
            &game_core::ShellImpact {
                owner: game_core::TankId(1),
                position: Vec3::new(5.0 + index as f32 * 2.0, 0.0, 40.0),
                surface: game_core::ImpactSurface::Terrain,
                shell_type: if index % 2 == 0 {
                    game_core::ShellType::HighExplosive
                } else {
                    game_core::ShellType::ArmorPiercing
                },
                direction: Vec3::new(0.0, -0.4, 0.9),
                caliber_mm: 122.0,
                ..Default::default()
            },
            &map,
        );
    }
    let mut scar_batch = Vec::new();
    scars.append_quads(&mut scar_batch);

    // One tank at its decal cap; the fleet worst case is every tank equally battered.
    let tank = snapshot();
    let variation = battered_variation();
    let mut decal_batch = Vec::new();
    append_decal_quads(&mut decal_batch, variation.decals(), &tank);
    let fleet_decal_vertices = decal_batch.len() * BATTLE_TANKS;

    let worst_case = particle_vertices + scar_batch.len() + fleet_decal_vertices;
    assert_eq!(
        worst_case,
        FX_FRAME_VERTEX_BUDGET,
        "the FX frame worst case moved (particles {particle_vertices} + craters {} + fleet \
         decals {fleet_decal_vertices}); if the change is intentional, update \
         FX_FRAME_VERTEX_BUDGET in the same diff and note it in docs/honest-steel-policy.md",
        scar_batch.len()
    );
}
