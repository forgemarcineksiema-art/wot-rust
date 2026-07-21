//! Locks for the contract report itself: the checks that guard an AUTHOR's mistakes must
//! fire on a synthetic bad blueprint — a shipped map never exercises them (it ships clean).

use map_forge::blueprint::{
    BaseSpec, Gauss2Term, GridSpec, MapBlueprint, MetaSpec, SpawnSpec, SymmetrySpec, TerrainOp,
    TerrainProgram,
};
use map_forge::compile;
use terrain::RiverSpec;

/// A minimal valid document to break in one deliberate way per test.
fn flat_square() -> MapBlueprint {
    MapBlueprint {
        meta: MetaSpec {
            id: "report_probe".into(),
            name: "Report probe".into(),
            historical_basis: "Synthetic test map".into(),
            design_notes: Vec::new(),
        },
        grid: GridSpec { size_m: [300.0, 300.0], cell_m: 5.0, min_height_m: 0.2 },
        symmetry: None,
        river: None,
        horizon: None,
        terrain: TerrainProgram { base: BaseSpec::Constant(5.0), ops: Vec::new() },
        water: None,
        objects: Vec::new(),
        scenery: Vec::new(),
        roads: Vec::new(),
        gameplay: map_forge::blueprint::GameplaySpec {
            spawns: vec![
                SpawnSpec { team: 1, at: [150.0, 150.0], facing_yaw_rad: 0.0, radius_m: None },
                SpawnSpec { team: 2, at: [150.0, 40.0], facing_yaw_rad: 0.0, radius_m: None },
            ],
            strategic_points: Vec::new(),
            features: Vec::new(),
        },
    }
}

fn error_messages(blueprint: &MapBlueprint) -> Vec<String> {
    let (_, report) = compile(blueprint);
    report.errors().map(|entry| format!("{}: {}", entry.check, entry.message)).collect()
}

#[test]
fn a_rectangular_grid_is_refused_not_silently_miscompiled() {
    let mut blueprint = flat_square();
    blueprint.grid.size_m = [300.0, 400.0];
    let errors = error_messages(&blueprint);
    assert!(
        errors.iter().any(|message| message.starts_with("grid:") && message.contains("square")),
        "a rectangular grid must be a report error, got: {errors:?}"
    );
}

#[test]
fn a_river_off_the_symmetry_axis_is_refused() {
    let mut blueprint = flat_square();
    blueprint.symmetry = Some(SymmetrySpec::MirrorZ);
    blueprint.river = Some(RiverSpec {
        base_x_m: 200.0,
        axis_z_m: 100.0, // the grid's symmetry axis is 150.0
        bow_sigma_m: 50.0,
        bow_amp_m: 10.0,
        wiggle_amp_m: 3.0,
        wiggle_wave_m: 40.0,
        corridor_half_width_m: 20.0,
    });
    let errors = error_messages(&blueprint);
    assert!(
        errors.iter().any(|message| message.starts_with("grid:") && message.contains("axis")),
        "a river off the symmetry axis must be a report error, got: {errors:?}"
    );
}

/// The gentle-approach rule holds on DRY maps too — a cliff at the spawn must not hide
/// behind the absence of water (it did once: the probe lived inside the water branch).
#[test]
fn a_steep_spawn_approach_warns_even_on_a_dry_map() {
    let mut blueprint = flat_square();
    blueprint.terrain.ops.push(TerrainOp::Gauss2 {
        apply: map_forge::blueprint::Apply::Add,
        terms: vec![Gauss2Term { x: 150.0, z: 172.0, sx: 8.0, sz: 8.0, amp: 9.0 }],
    });
    let (map, report) = compile(&blueprint);
    assert!(map.water.is_none(), "the probe map must stay dry");
    let warnings: Vec<&str> = report
        .warnings()
        .filter(|entry| entry.check == "spawns")
        .map(|entry| entry.message.as_str())
        .collect();
    assert!(
        warnings.iter().any(|message| message.contains("steep")),
        "a cliff at a dry spawn must warn, got: {warnings:?}"
    );
}
