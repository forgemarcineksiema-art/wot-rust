//! Standing sheets through the LIVE rules (teren W6): a hull drowns in a pool and not a
//! metre outside it, and a shell splashes on each pool's OWN surface — position decides,
//! not one global table. These are the promises that make two lakes at two levels a
//! gameplay fact rather than a picture.

use game_core::{ImpactSurface, TankSpec, TeamId};
use glam::Vec3;
use sim::{
    DROWN_DEPTH_M, FixedTimestep, SegmentImpact, ShellTraceWorld, SimulationState, TankCommand,
};
use terrain::{HeightMap, StandingWater, WaterField};

/// Flat ground at 0; one sheet holds a drowning-deep pool over the north-east quarter.
/// (The report's shoreline gate governs AUTHORED maps; the sim itself must resolve
/// whatever field it is handed, positionally.)
fn pool_field() -> (HeightMap, WaterField) {
    let heightmap = HeightMap::flat(64, 64, 4.0, 0.0).expect("flat field");
    let field = WaterField {
        table: None,
        sheets: vec![StandingWater {
            rect: [120.0, 120.0, 240.0, 240.0],
            surface_level_m: DROWN_DEPTH_M + 0.5,
        }],
    };
    (heightmap, field)
}

/// Drowning is POSITIONAL: the hull inside the sheet floods and dies, the hull on the same
/// ground height a map-quarter away never wets its tracks.
#[test]
fn a_hull_drowns_inside_the_sheet_and_not_beside_it() {
    let (heightmap, field) = pool_field();
    let mut sim = SimulationState::new();
    sim.set_water(field);
    let wet = sim.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(180.0, 0.0, 180.0));
    let dry = sim.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(40.0, 0.0, 40.0));

    let timestep = FixedTimestep::from_hz(60);
    for _ in 0..60 * 8 {
        sim.apply_commands_on_terrain(
            &[(wet, TankCommand::idle()), (dry, TankCommand::idle())],
            timestep,
            &heightmap,
        );
    }
    let drowned = sim.tank(wet).expect("wet tank");
    assert_eq!(drowned.hit_points, 0, "the pool floods the engine and drains the hull");
    let dry_tank = sim.tank(dry).expect("dry tank");
    assert_eq!(
        dry_tank.hit_points, dry_tank.spec.hit_points,
        "a map-quarter outside the rect there is no water at all"
    );
}

/// The shell splash resolves each surface at ITS level: a lob into the pool dies on the
/// sheet's plane (impact height = the sheet's level), while the same lob outside the rect
/// flies on to the ground.
#[test]
fn a_shell_splashes_on_the_sheets_own_surface() {
    let (heightmap, field) = pool_field();
    let level = field.sheets[0].surface_level_m;
    let world = ShellTraceWorld {
        projectile_radius_m: 0.05,
        tanks: &[],
        blockers: &[],
        heightmap: Some(&heightmap),
        cover: &[],
        water: field.view(),
    };
    let velocity = Vec3::new(0.0, -220.0, 0.0);
    let into_pool = sim::segment_impact(
        Vec3::new(180.0, 12.0, 180.0),
        Vec3::new(180.0, -1.0, 180.0),
        velocity,
        &world,
    );
    match into_pool {
        Some(SegmentImpact::Obstacle { position, surface }) => {
            assert_eq!(surface, ImpactSurface::Water, "the pool eats the shell");
            assert!(
                (position.y - level).abs() < 0.1,
                "the splash sits on the SHEET's surface ({} vs {level})",
                position.y
            );
        }
        other => panic!("expected a water impact in the pool, got {other:?}"),
    }

    let beside_pool = sim::segment_impact(
        Vec3::new(40.0, 12.0, 40.0),
        Vec3::new(40.0, -1.0, 40.0),
        velocity,
        &world,
    );
    match beside_pool {
        Some(SegmentImpact::Obstacle { surface, .. }) => {
            assert_eq!(
                surface,
                ImpactSurface::Terrain,
                "outside the rect the same lob meets dry ground"
            );
        }
        other => panic!("expected a terrain impact beside the pool, got {other:?}"),
    }
}
