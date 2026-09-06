use sim::{DEFAULT_SIMULATION_TICK_HZ, FixedTimestep, SimulationClock};

#[test]
fn fixed_timestep_keeps_declared_tick_rate() {
    let timestep = FixedTimestep::from_hz(60);

    assert_eq!(timestep.hz(), 60);
    assert!((timestep.dt_seconds() - (1.0 / 60.0)).abs() < f32::EPSILON);
}

#[test]
fn simulation_clock_advances_only_by_discrete_ticks() {
    let mut clock = SimulationClock::new(FixedTimestep::from_hz(DEFAULT_SIMULATION_TICK_HZ));

    assert_eq!(clock.tick(), 0);
    assert_eq!(clock.timestep().hz(), DEFAULT_SIMULATION_TICK_HZ);

    clock.advance_tick();
    clock.advance_ticks(2);

    assert_eq!(clock.tick(), 3);
}
