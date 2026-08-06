//! THE FLEET'S MOBILITY, AS A NUMBER PER VEHICLE — P0.1 of `docs/contact-and-tracks-program.md`.
//!
//! Wave 4 of that program replaces the scalar drive with two track forces. Everything it touches is
//! a number a player feels, and there is no way to argue about "does it still feel like a T-54"
//! without the T-54's numbers written down FIRST. This is that measurement, taken before anything
//! moves, and then held: the table below is the baseline, and the test re-measures it every run.
//!
//! It is deliberately not a "reasonable value" test. Nothing here asserts that 13.9 m/s is a good
//! top speed — only that it is TODAY's top speed, so a change that moves it is a change somebody
//! chose. Re-blessing is a copy-paste: on a mismatch the test prints the whole table back in the
//! exact shape `BASELINE` holds it.
//!
//! Measured through the shipped drive model, never a copy of it:
//! `step_custom_tank_controller_on_contact` is the same function the server tick and the client
//! predictor call. Flat rows use `TerrainContact::flat` — on level ground the shipped sampler
//! returns exactly that, so the constructor is the same contact rather than a shortcut past it.
//! The gradeability row drives a real constant-grade heightmap through
//! `sample_tank_terrain_contact`, so the slope, roughness and traction the hull meets are the map's
//! rule and not this file's opinion.

use game_core::VehicleKind;
use glam::Vec3;
use physics::{
    GroundScales, TankControlInput, TankControllerSettings, TankKinematicState, TerrainContact,
    sample_tank_terrain_contact, step_custom_tank_controller_on_contact,
};
use terrain::{GroundMaterial, HeightMap};

const DT: f32 = 1.0 / 60.0;

/// The three surfaces the table spans: the reference, the softest thing a hull crosses, and the one
/// that bites hardest. `Straw` sits between `Grass` and `Dirt` and would add a row without adding
/// an axis.
const GROUNDS: [GroundMaterial; 3] =
    [GroundMaterial::Grass, GroundMaterial::Dirt, GroundMaterial::Rock];

/// Recorded 2026-08-06 on master, before the contact and per-track work begins.
///
/// A text table rather than a `const` array of structs on purpose: rustfmt explodes a struct
/// literal this wide into eight lines apiece, and a baseline nobody can read across is a baseline
/// nobody checks. Held as data, one row per line, so a diff shows what moved.
///
/// Read it as a photograph, not as a target. Three things in it are worth an eyebrow, and all three
/// belong to later PRs — the harness's job here is to have SEEN them:
///
/// * **Steady-state gradeability is ~0.42, not the 0.60 the settings name.**
///   `longitudinal_grip_mu` is `MAX_CLIMB_GRADE` = 0.60, but `sample_tank_terrain_contact` folds a
///   plane's grade into `roughness` as well as into `forward_slope` (on a plane `roughness` IS the
///   grade), and `traction` is cut by both — so the grip cap shrinks on exactly the face it is
///   climbing. At 0.417 the arithmetic closes: grip `0.6·g·0.75·cos = 4.98` against slope `4.62`
///   plus rolling `0.34`. This is the STANDING-START number the movement policy defines
///   gradeability as, and the momentum-climb band still carries a moving hull higher — so it is not
///   by itself proof that a map can author unclimbable ground. It is the reason to go and check.
/// * **Gradeability is a fleet constant, not a vehicle property.** Every hull lands on the same
///   0.417/0.394/0.432 because they share one `longitudinal_grip_mu`. The exception is the
///   Jagdtiger (0.355), and it is the interesting one: 69.9 t behind 441 kW gives 4.7 m/s² of `P/v`
///   thrust at a crawl against a 5.3 m/s² grip cap, so it is the only hull in the roster whose
///   climb is ENGINE-limited rather than track-limited. That is the kind of difference per-track
///   forces should produce across the fleet instead of in one corner of it.
/// * **The surfaces barely separate anything.** Grip spans 0.95..1.04 by design (see
///   `GroundMaterial::properties`, which says as much in its own words), so the ground axis is
///   almost entirely a rolling-resistance story today.
const BASELINE: &str = "\
# vehicle   ground  vmax_mps launch_s brake_m pivot_rad_s radius_m gradeability
T54_1951    Grass   13.8777  8.7167   16.9116 0.7800      6.9084   0.4170
T54_1951    Dirt    13.2859  8.4667   15.2397 0.7410      7.0969   0.3935
T54_1951    Rock    13.8690  8.3667   16.9797 0.7800      6.9752   0.4318
TigerI      Grass   10.5464  5.9667   9.6461  0.5800      8.8912   0.4170
TigerI      Dirt    10.1463  5.8167   8.7801  0.5510      9.0502   0.3935
TigerI      Rock    10.5565  5.8000   9.7134  0.5800      8.9877   0.4318
TigerII     Grass   10.5494  7.6500   9.8647  0.4500      11.3060  0.4170
TigerII     Dirt    10.0298  7.3500   8.7620  0.4275      11.3548  0.3935
TigerII     Rock    10.5562  7.3500   9.9290  0.4500      11.4618  0.4318
Jagdtiger   Grass   9.6010   8.3333   8.2684  0.3200      14.9752  0.3551
Jagdtiger   Dirt    9.0334   7.9167   7.1884  0.3040      15.0632  0.3448
Jagdtiger   Rock    9.5997   7.9167   8.3093  0.3200      14.9665  0.3573
PantherII   Grass   12.7702  10.0500  14.5749 0.6400      7.8367   0.4170
PantherII   Dirt    12.0792  9.5833   12.8124 0.6080      7.9551   0.3935
PantherII   Rock    12.7636  9.5500   14.6375 0.6400      7.9321   0.4318
IS3         Grass   11.0882  7.3500   10.8244 0.5800      8.6957   0.4170
IS3         Dirt    10.5905  7.1000   9.7069  0.5510      8.8196   0.3935
IS3         Rock    11.0985  7.1000   10.9000 0.5800      8.7992   0.4318
Centurion   Grass   9.5833   4.4500   7.7853  0.6200      7.7293   0.4170
Centurion   Dirt    9.2906   4.3833   7.1984  0.5890      8.1191   0.3935
Centurion   Rock    9.5773   4.3000   7.8151  0.6200      7.7187   0.4318
T34_85      Grass   14.9879  9.6167   19.7725 0.8000      6.9456   0.4170
T34_85      Dirt    14.3345  9.3167   17.7821 0.7600      7.1476   0.3935
T34_85      Rock    14.9926  9.2500   19.8849 0.8000      7.0110   0.4318
";

/// Relative band the re-measurement must stay inside. On one binary the model is bit-stable, so
/// this is slack for a different compiler or target, not licence to drift: 0.5% is far below any
/// change a hand would make on purpose, and the band Wave 4 calibrates to is ±10%.
const TOLERANCE: f32 = 0.005;

/// The six columns, in the order the table holds them.
const COLUMNS: [&str; 6] =
    ["vmax_mps", "launch_s", "brake_m", "pivot_rad_s", "radius_m", "gradeability"];

/// One vehicle on one surface: the two keys and the six measurements.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Row {
    vehicle: &'static str,
    ground: &'static str,
    values: [f32; COLUMNS.len()],
}

#[test]
fn the_fleet_mobility_table_holds() {
    let measured = measure_fleet();
    let baseline = parse(BASELINE);

    let mut failures = Vec::new();
    for row in &measured {
        let Some(was) =
            baseline.iter().find(|b| b.vehicle == row.vehicle && b.ground == row.ground)
        else {
            failures.push(format!("{} on {}: no baseline row", row.vehicle, row.ground));
            continue;
        };
        for (column, (&before, &now)) in COLUMNS.iter().zip(was.values.iter().zip(&row.values)) {
            if (now - before).abs() > before.abs().max(1.0e-3) * TOLERANCE {
                failures.push(format!(
                    "{} on {}: {column} was {before:.4}, is {now:.4}",
                    row.vehicle, row.ground
                ));
            }
        }
    }
    for was in &baseline {
        if !measured.iter().any(|row| row.vehicle == was.vehicle && row.ground == was.ground) {
            failures.push(format!(
                "{} on {}: baseline row has no vehicle to measure",
                was.vehicle, was.ground
            ));
        }
    }

    println!("{}", render(&measured));
    assert!(
        failures.is_empty(),
        "the fleet's mobility moved. Every line below is a number a player feels; if the change was \
         deliberate, paste the table that follows over `BASELINE` and say WHY in the commit.\n\n\
         {}\n\nconst BASELINE: &str = \"\\\n{}\";\n",
        failures.join("\n"),
        render(&measured)
    );
}

/// Every playable vehicle reaches the table on every measured surface. A skipped vehicle is an
/// unmeasured one, and the point of a baseline is that nothing sits outside it.
#[test]
fn the_table_spans_the_whole_roster() {
    let baseline = parse(BASELINE);
    assert_eq!(baseline.len(), VehicleKind::PLAYABLE.len() * GROUNDS.len());
    for kind in VehicleKind::PLAYABLE {
        for ground in GROUNDS {
            let (vehicle, ground) = (vehicle_name(kind), ground_name(ground));
            assert!(
                baseline.iter().any(|row| row.vehicle == vehicle && row.ground == ground),
                "{vehicle} on {ground} is missing from the baseline"
            );
        }
    }
}

fn measure_fleet() -> Vec<Row> {
    VehicleKind::PLAYABLE
        .into_iter()
        .flat_map(|kind| GROUNDS.into_iter().map(move |ground| measure(kind, ground)))
        .collect()
}

fn measure(kind: VehicleKind, ground: GroundMaterial) -> Row {
    let spec = kind.spec();
    let settings = TankControllerSettings::from_spec(&spec);
    let scales = GroundScales::from(ground.properties());
    let vmax = top_speed(&settings, scales);
    Row {
        vehicle: vehicle_name(kind),
        ground: ground_name(ground),
        values: [
            vmax,
            launch_time(&settings, scales, vmax),
            braking_distance(&settings, scales, vmax),
            pivot_rate(&settings, scales),
            turn_radius(&settings, scales),
            gradeability(&settings, scales),
        ],
    }
}

// ---------------------------------------------------------------------------------------------
// The six measurements
// ---------------------------------------------------------------------------------------------

fn flat(scales: GroundScales) -> TerrainContact {
    TerrainContact { ground: scales, ..TerrainContact::flat(0.0) }
}

fn run_flat(
    state: &mut TankKinematicState,
    settings: &TankControllerSettings,
    scales: GroundScales,
    input: TankControlInput,
    ticks: usize,
) {
    for _ in 0..ticks {
        step_custom_tank_controller_on_contact(state, input, settings, flat(scales), DT);
    }
}

/// Thrust and resistance balance at the top speed; a minute of full throttle is far past that.
fn top_speed(settings: &TankControllerSettings, scales: GroundScales) -> f32 {
    let mut state = TankKinematicState::default();
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    run_flat(&mut state, settings, scales, input, 3_600);
    state.forward_speed()
}

fn launch_time(settings: &TankControllerSettings, scales: GroundScales, vmax: f32) -> f32 {
    let mut state = TankKinematicState::default();
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    for tick in 0..3_600 {
        step_custom_tank_controller_on_contact(&mut state, input, settings, flat(scales), DT);
        if state.forward_speed() >= vmax * 0.9 {
            return (tick + 1) as f32 * DT;
        }
    }
    f32::NAN
}

/// From top speed under full brake until the hull is at walking pace. Distance, because distance is
/// what anything planning around a stop has to read — the bots' water escape already learned that.
fn braking_distance(settings: &TankControllerSettings, scales: GroundScales, vmax: f32) -> f32 {
    let mut state =
        TankKinematicState { velocity: Vec3::new(0.0, 0.0, vmax), ..Default::default() };
    let input = TankControlInput { throttle: 0.0, steer: 0.0, brake: 1.0 };
    let start = state.position;
    for _ in 0..3_600 {
        step_custom_tank_controller_on_contact(&mut state, input, settings, flat(scales), DT);
        if state.forward_speed() < 0.05 {
            break;
        }
    }
    (state.position - start).length()
}

/// Neutral steer: the throttle is released, so the hull turns on counter-rotating tracks alone.
fn pivot_rate(settings: &TankControllerSettings, scales: GroundScales) -> f32 {
    let mut state = TankKinematicState::default();
    let input = TankControlInput { throttle: 0.0, steer: 1.0, brake: 0.0 };
    run_flat(&mut state, settings, scales, input, 600);
    state.yaw_rate_rad_s
}

/// Half throttle, full steer, settled: radius = speed / yaw rate.
fn turn_radius(settings: &TankControllerSettings, scales: GroundScales) -> f32 {
    let mut state = TankKinematicState::default();
    let input = TankControlInput { throttle: 0.5, steer: 1.0, brake: 0.0 };
    run_flat(&mut state, settings, scales, input, 1_800);
    if state.yaw_rate_rad_s.abs() < 1.0e-4 {
        return f32::INFINITY;
    }
    state.speed() / state.yaw_rate_rad_s.abs()
}

/// The steepest constant grade the hull climbs from a standing start ON the slope — the
/// steady-state gradeability `docs/vehicle-movement-policy.md` defines, not a momentum run-up.
fn gradeability(settings: &TankControllerSettings, scales: GroundScales) -> f32 {
    let (mut low, mut high) = (0.0_f32, 0.9_f32);
    for _ in 0..12 {
        let mid = 0.5 * (low + high);
        if climbs(settings, scales, mid) {
            low = mid;
        } else {
            high = mid;
        }
    }
    low
}

/// Ten seconds of full throttle facing up a constant grade: did the hull gain half a metre?
fn climbs(settings: &TankControllerSettings, scales: GroundScales, grade: f32) -> bool {
    let map = ramp(grade);
    let mut state =
        TankKinematicState { position: Vec3::new(300.0, grade * 30.0, 30.0), ..Default::default() };
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    let start_y = state.position.y;
    for _ in 0..600 {
        let Some(sampled) = sample_tank_terrain_contact(
            &map,
            state.position,
            state.yaw_rad,
            settings.ground_probe_length_m,
            &[],
            None,
        ) else {
            // Off the ramp's far end: it climbed further than the ramp is long, which is a yes.
            return true;
        };
        let contact = TerrainContact { ground: scales, ..sampled };
        // The ground carries the hull on a followable slope — what `resolve_vertical` does every
        // grounded tick, said directly so this harness needs no world step around it.
        state.position.y = contact.height_m;
        step_custom_tank_controller_on_contact(&mut state, input, settings, contact, DT);
    }
    state.position.y - start_y > 0.5
}

/// A constant grade rising along +z, wide and long enough that the probe cross never leaves it.
fn ramp(grade: f32) -> HeightMap {
    const CELLS: usize = 200;
    const CELL_M: f32 = 3.0;
    let mut samples = Vec::with_capacity(CELLS * CELLS);
    for z in 0..CELLS {
        for _ in 0..CELLS {
            samples.push(grade * z as f32 * CELL_M);
        }
    }
    HeightMap::new(CELLS, CELLS, CELL_M, samples).expect("the ramp's dimensions are fixed")
}

// ---------------------------------------------------------------------------------------------
// The table as data
// ---------------------------------------------------------------------------------------------

fn parse(table: &'static str) -> Vec<Row> {
    table
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .map(|line| {
            let mut fields = line.split_whitespace();
            let vehicle = fields.next().expect("every row names a vehicle");
            let ground = fields.next().expect("every row names a surface");
            let mut values = [0.0; COLUMNS.len()];
            for (slot, column) in values.iter_mut().zip(COLUMNS) {
                let field = fields
                    .next()
                    .unwrap_or_else(|| panic!("{vehicle}/{ground} is missing {column}"));
                *slot = field.parse().unwrap_or_else(|_| {
                    panic!("{vehicle}/{ground} {column}: {field} is not a number")
                });
            }
            assert!(fields.next().is_none(), "{vehicle}/{ground} has a column the table does not");
            Row { vehicle, ground, values }
        })
        .collect()
}

fn render(rows: &[Row]) -> String {
    let mut out = String::from(
        "# vehicle   ground  vmax_mps launch_s brake_m pivot_rad_s radius_m gradeability\n",
    );
    for row in rows {
        out.push_str(&format!("{:<11} {:<7}", row.vehicle, row.ground));
        for (value, width) in row.values.iter().zip([8, 8, 7, 11, 8, 0]) {
            out.push_str(&format!(" {:<width$}", format!("{value:.4}"), width = width));
        }
        out.push('\n');
    }
    out
}

fn vehicle_name(kind: VehicleKind) -> &'static str {
    match kind {
        VehicleKind::T54_1951 => "T54_1951",
        VehicleKind::TigerI => "TigerI",
        VehicleKind::TigerII => "TigerII",
        VehicleKind::Jagdtiger => "Jagdtiger",
        VehicleKind::PantherII => "PantherII",
        VehicleKind::IS3 => "IS3",
        VehicleKind::Centurion => "Centurion",
        VehicleKind::T34_85 => "T34_85",
        other => panic!("{other:?} joined the roster without joining the mobility table"),
    }
}

fn ground_name(ground: GroundMaterial) -> &'static str {
    match ground {
        GroundMaterial::Grass => "Grass",
        GroundMaterial::Straw => "Straw",
        GroundMaterial::Dirt => "Dirt",
        GroundMaterial::Rock => "Rock",
    }
}
