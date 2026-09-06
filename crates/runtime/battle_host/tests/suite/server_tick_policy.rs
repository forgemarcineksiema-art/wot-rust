use battle_host::ServerTickConfig;

#[test]
fn server_tick_config_controls_authoritative_tick_and_snapshot_rate() {
    let config = ServerTickConfig::new(60, 20);

    assert_eq!(config.server_tick_hz(), 60);
    assert_eq!(config.timestep().hz(), 60);
    assert_eq!(config.snapshot_schedule().server_tick_interval(), 3);
}
