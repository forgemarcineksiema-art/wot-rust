//! Q8: the tick profile is off in the game and, armed, accounts for the tick it measures.
//! Q11: arming it is idempotent, because the frame log re-arms it every frame.

use battle_host::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use net::ClientInputCommand;
use sim::TankCommand;

/// The client arms the profile once per frame instead of tracking whether a new battle
/// replaced the host. A second arm that reset the sums would silently zero every frame's
/// sections while the report still printed a confident table of them.
#[test]
fn arming_the_profile_again_keeps_what_the_ticks_already_summed() {
    let mut config =
        RandomBattleConfig::new(BattleSeed::fixed(7), game_core::VehicleKind::T54_1951);
    config.map = terrain::MapId::BystraValley;
    let mut server = LocalAuthoritativeServer::new_ai_battle(ServerTickConfig::default(), config);
    let player = server.player_tank();
    let drive = |client_tick: u64| ClientInputCommand {
        client_tick,
        tank_id: player,
        command: TankCommand::drive(1.0, 0.0),
    };
    server.enable_tick_profile();
    for tick in 0..4 {
        server.tick_with_player_input(drive(tick));
        server.enable_tick_profile();
    }
    let sections = server.take_tick_profile().expect("armed");
    assert_eq!(sections.ticks, 4, "re-arming dropped summed ticks: {sections:?}");
    assert!(sections.total_ms > 0.0, "{sections:?}");
    // Drained means reset; re-arming an already-armed profile does not resurrect the sums.
    server.enable_tick_profile();
    let drained = server.take_tick_profile().expect("still armed");
    assert_eq!(drained.ticks, 0, "{drained:?}");
}

#[test]
fn the_tick_profile_is_off_until_armed_and_its_sections_account_for_the_tick() {
    let mut config =
        RandomBattleConfig::new(BattleSeed::fixed(42), game_core::VehicleKind::T54_1951);
    config.map = terrain::MapId::BystraValley;
    let mut server = LocalAuthoritativeServer::new_ai_battle(ServerTickConfig::default(), config);
    let player = server.player_tank();
    let drive = |client_tick: u64| ClientInputCommand {
        client_tick,
        tank_id: player,
        command: TankCommand::drive(1.0, 0.0),
    };
    server.tick_with_player_input(drive(0));
    assert_eq!(server.take_tick_profile(), None, "the game never pays for the profile");

    server.enable_tick_profile();
    let mut snapshots = 0;
    for tick in 1..=30 {
        snapshots += usize::from(server.tick_with_player_input(drive(tick)).snapshot.is_some());
    }
    let sections = server.take_tick_profile().expect("armed");
    assert_eq!(sections.ticks, 30);
    assert!(snapshots > 0, "the window must carry emitting ticks");
    let summed: f64 = sections.sections().iter().map(|(_, ms)| ms).sum();
    assert!(summed > 0.0, "{sections:?}");
    assert!(
        summed <= sections.total_ms + 1e-6,
        "the sections cannot exceed the tick they are cut from: {sections:?}"
    );
    assert!(
        sections.total_ms - summed < 0.05 * 30.0,
        "the clock reads between sections are noise, not a section: {sections:?}"
    );
    assert!(sections.view_ms > 0.0 && sections.snapshot_ms > 0.0, "{sections:?}");
    // The observer masks: computed ONCE per emitting tick, handed back to the viewer's cut.
    assert_eq!(sections.mask_computations, snapshots as u64, "{sections:?}");
    assert_eq!(sections.mask_reuses, snapshots as u64, "{sections:?}");
    assert!(server.cover_box_count() > 0);
    // Taken means reset. On an emitting tick, asked again, the masks are handed back, not
    // walked; the next emitting tick walks them again — the cache is the tick's, never stale.
    let mut tick = 30;
    let mut emit = |server: &mut LocalAuthoritativeServer| {
        loop {
            tick += 1;
            if server.tick_with_player_input(drive(tick)).snapshot.is_some() {
                break;
            }
        }
    };
    emit(&mut server);
    let _ = server.take_tick_profile();
    assert_eq!(server.observer_masks().len(), 30);
    let again = server.take_tick_profile().expect("still armed");
    assert_eq!(again.mask_computations, 0, "{again:?}");
    assert_eq!(again.mask_reuses, 1, "{again:?}");
    emit(&mut server);
    let next = server.take_tick_profile().expect("still armed");
    assert_eq!(next.mask_computations, 1, "{next:?}");
    assert_eq!(next.mask_reuses, 1, "the viewer's cut read the new masks: {next:?}");
}
