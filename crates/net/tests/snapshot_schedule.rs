use net::SnapshotSchedule;

#[test]
fn fixed_snapshot_schedule_emits_on_server_tick_intervals() {
    let schedule = SnapshotSchedule::fixed(60, 20);

    assert_eq!(schedule.server_tick_hz(), 60);
    assert_eq!(schedule.snapshot_hz(), 20);
    assert_eq!(schedule.server_tick_interval(), 3);

    let emitted_ticks =
        (1..=9).filter(|server_tick| schedule.should_emit(*server_tick)).collect::<Vec<_>>();

    assert_eq!(emitted_ticks, [3, 6, 9]);
}

#[test]
#[should_panic(expected = "snapshot rate must not exceed server tick rate")]
fn fixed_snapshot_schedule_rejects_faster_snapshots_than_server_ticks() {
    SnapshotSchedule::fixed(20, 60);
}
