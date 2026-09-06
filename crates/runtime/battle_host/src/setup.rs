use game_core::{MatchWeather, TankId, TeamId, VehicleKind};
use glam::Vec3;
use sim::SimulationState;
use terrain::{BattlefieldMap, MapId, SpawnZone};

use crate::battle::{BattleFormat, BattleMode, BattleSeed, RandomBattleConfig};
use crate::bots::BotRoster;
use crate::match_info::pick_weather;

pub(crate) struct BattleSetup {
    pub mode: BattleMode,
    pub format: Option<BattleFormat>,
    pub sim: SimulationState,
    pub map_id: MapId,
    pub battlefield: BattlefieldMap,
    pub weather: MatchWeather,
    pub player_tank: TankId,
    pub target_tank: TankId,
    pub bots: BotRoster,
}

pub(crate) fn practice_duel_setup(player_vehicle: VehicleKind) -> BattleSetup {
    let map_id = MapId::default();
    let battlefield = map_forge::battlefield(map_id);
    let mut sim = SimulationState::new();
    sim.set_water(battlefield.water_field());
    // The map's ground rule, resolved once: what the surface under every track IS. Same call the
    // client makes from the same battlefield, so the predictor grips the road where the authority
    // grips it — it is derived from the map, so it never rides the wire.
    sim.set_ground(Some(terrain::GroundClassifier::new(&battlefield)));
    let player_pos = random_battle_ground_position(&battlefield, 340.0, 300.0);
    let target_pos = random_battle_ground_position(&battlefield, 340.0, 340.0);
    let player_tank = sim.spawn_tank(game_core::TeamId(1), player_vehicle.spec(), player_pos);
    let target_tank = sim.spawn_tank_with_yaw(
        game_core::TeamId(2),
        game_core::TankSpec::t54_1951(),
        target_pos,
        std::f32::consts::PI * 0.5,
    );
    sim.refresh_spotting(Some(&battlefield.heightmap), &battlefield.static_cover);

    BattleSetup {
        mode: BattleMode::PracticeDuel,
        format: None,
        sim,
        map_id,
        battlefield,
        weather: MatchWeather::default(),
        player_tank,
        target_tank,
        bots: BotRoster::empty(),
    }
}

pub(crate) fn random_battle_setup(config: RandomBattleConfig) -> BattleSetup {
    random_battle_setup_for_humans(config, &[None]).0
}

/// The dedicated server's variant (N2), and since M6 (`docs/game-modes.md` R4; the owner,
/// 2026-09-06: "ludzie muszą być w jednej jak i drugiej drużynie, boty mają zapełniać puste
/// miejsca") the deal across BOTH teams: crew `i` of `human_vehicles` sits on the team
/// [`human_team`] names — a snake by seat order (1-2-2-1 …; by rating when M8 lands), so the two
/// sides differ by at most one human — takes its team's next seat, and every seat no human took
/// is a bot. The human tanks come back in crew order. `human_vehicles.len() == 1` is exactly
/// the desktop battle: team one, seat A, bit for bit.
/// Seat=vehicle (netcode block 3, v49): each human seat spawns the crew's GARAGE PICK.
/// `human_vehicles[i]` is that crew's wish; `None` falls back to the host's `player_vehicle`
/// for crew 0 (the historical single-human contract) and to the benchmark for later crews — a
/// predictable hull, never a random bot draw.
pub(crate) fn random_battle_setup_for_humans(
    config: RandomBattleConfig,
    human_vehicles: &[Option<game_core::VehicleKind>],
) -> (BattleSetup, Vec<TankId>) {
    let battlefield = map_forge::battlefield(config.map);
    let mut sim = SimulationState::new();
    sim.set_water(battlefield.water_field());
    // The map's ground rule, resolved once: what the surface under every track IS. Same call the
    // client makes from the same battlefield, so the predictor grips the road where the authority
    // grips it — it is derived from the map, so it never rides the wire.
    sim.set_ground(Some(terrain::GroundClassifier::new(&battlefield)));
    let mut bot_ids = Vec::new();
    let zones =
        [random_battle_spawn_zone(&battlefield, 1), random_battle_spawn_zone(&battlefield, 2)];

    let humans = human_vehicles.len().clamp(1, config.format.total_seats());
    // Which crews sit on which team, in seat order within the team.
    let mut crews_by_team: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
    for crew in 0..humans {
        crews_by_team[human_team(crew)].push(crew);
    }
    let mut human_tanks = vec![TankId(0); humans];
    let mut target_tank = TankId(0);
    // Team one's seats, then team two's: the spawn order every tank id and every 7v7 replay
    // fixture counts on. The bot draws keep their historical salts (10 + seat, 30 + seat).
    for (team, zone) in zones.into_iter().enumerate() {
        let salt = if team == 0 { 10 } else { 30 };
        for seat in 0..config.format.seats_per_team() {
            let crew = crews_by_team[team].get(seat).copied();
            let vehicle = match crew {
                Some(crew) => match human_vehicles.get(crew).copied().flatten() {
                    Some(pick) => pick,
                    None if crew == 0 => config.player_vehicle,
                    None => game_core::VehicleKind::BENCHMARK,
                },
                None => random_battle_bot_vehicle(
                    config.seed,
                    salt + seat as u64,
                    config.player_vehicle,
                ),
            };
            let id = random_battle_spawn(&mut sim, &battlefield, zone, seat, vehicle, config);
            match crew {
                Some(crew) => human_tanks[crew] = id,
                None => bot_ids.push(id),
            }
            if team == 1 && seat == 0 {
                target_tank = id;
            }
        }
    }
    let player_tank = human_tanks[0];
    sim.refresh_spotting(Some(&battlefield.heightmap), &battlefield.static_cover);

    (
        BattleSetup {
            mode: match config.format {
                BattleFormat::SevenVsSeven => BattleMode::Random7v7,
                BattleFormat::FifteenVsFifteen => BattleMode::Random15v15,
            },
            format: Some(config.format),
            sim,
            map_id: config.map,
            battlefield,
            weather: pick_weather(config.map, config.seed),
            player_tank,
            target_tank,
            bots: BotRoster::new(bot_ids, config.seed),
        },
        human_tanks,
    )
}

/// The team (0 or 1) crew `crew` sits on: a snake, 1-2-2-1 — crews 0 and 3 on team one, 1 and 2
/// on team two, then again — so any count of humans splits with at most one more on a side.
/// M8 sorts the crews by rating before this deal; until then the order is the hello order.
pub fn human_team(crew: usize) -> usize {
    usize::from(matches!(crew % 4, 1 | 2))
}

fn random_battle_spawn_zone(map: &BattlefieldMap, team: u16) -> &SpawnZone {
    map.spawn_zones.iter().find(|zone| zone.team == team).expect("team spawn zone")
}

/// Bots deploy from the player's tier ±1 — World of Tanks matchmaking, not a museum mix.
/// A bracket too thin to field two designs falls back to the full playable park rather than
/// cloning one hull fourteen times.
fn random_battle_bot_vehicle(seed: BattleSeed, salt: u64, player: VehicleKind) -> VehicleKind {
    let roster: Vec<VehicleKind> = player.matchmaking_pool().collect();
    if roster.len() < 2 {
        let all = VehicleKind::PLAYABLE;
        return all[seed.random_battle_index(salt, all.len())];
    }
    roster[seed.random_battle_index(salt, roster.len())]
}

fn random_battle_spawn(
    sim: &mut SimulationState,
    map: &BattlefieldMap,
    zone: &SpawnZone,
    slot: usize,
    vehicle: VehicleKind,
    config: RandomBattleConfig,
) -> TankId {
    let position = random_battle_spawn_position(map, zone, slot, config);
    sim.spawn_tank_with_yaw(TeamId(zone.team), vehicle.spec(), position, zone.facing_yaw_rad)
}

fn random_battle_spawn_position(
    map: &BattlefieldMap,
    zone: &SpawnZone,
    slot: usize,
    config: RandomBattleConfig,
) -> Vec3 {
    let seed = config.seed;
    let jitter = game_core::BattleFormat::SPAWN_JITTER_M * 2.0;
    let jitter_x = (seed.random_battle_unit(100 + slot as u64) - 0.5) * jitter;
    let jitter_z = (seed.random_battle_unit(200 + slot as u64) - 0.5) * jitter;
    // The same arithmetic the map report certifies (M4): one seat, one point.
    let [x, z] = config
        .format
        .seat_position(
            slot,
            [zone.center[0], zone.center[2]],
            zone.facing_yaw_rad,
            [jitter_x, jitter_z],
        )
        .expect("seat in battle format");
    random_battle_ground_position(map, x, z)
}

fn random_battle_ground_position(map: &BattlefieldMap, x: f32, z: f32) -> Vec3 {
    Vec3::new(x, map.heightmap.sample_height(x, z).unwrap_or(0.0), z)
}
