use game_core::{MatchWeather, TankId, TeamId, VehicleKind};
use glam::Vec3;
use sim::SimulationState;
use terrain::{BattlefieldMap, MapId, SpawnZone};

use crate::battle::{BattleMode, BattleSeed, RandomBattleConfig};
use crate::bots::BotRoster;
use crate::match_info::pick_weather;

pub(crate) struct BattleSetup {
    pub mode: BattleMode,
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
    sim.set_water(battlefield.water);
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
        sim,
        map_id,
        battlefield,
        weather: MatchWeather::default(),
        player_tank,
        target_tank,
        bots: BotRoster::empty(),
    }
}

pub(crate) fn random_7v7_setup(config: RandomBattleConfig) -> BattleSetup {
    random_7v7_setup_for_humans(config, 1).0
}

/// The dedicated server's variant (N2): reserve the first `humans` team-one slots for connected
/// players (returned in slot order); everything else is a bot. `humans == 1` is exactly the
/// desktop battle.
pub(crate) fn random_7v7_setup_for_humans(
    config: RandomBattleConfig,
    humans: usize,
) -> (BattleSetup, Vec<TankId>) {
    let battlefield = map_forge::battlefield(config.map);
    let mut sim = SimulationState::new();
    sim.set_water(battlefield.water);
    // The map's ground rule, resolved once: what the surface under every track IS. Same call the
    // client makes from the same battlefield, so the predictor grips the road where the authority
    // grips it — it is derived from the map, so it never rides the wire.
    sim.set_ground(Some(terrain::GroundClassifier::new(&battlefield)));
    let mut bot_ids = Vec::new();
    let team_one = random_battle_spawn_zone(&battlefield, 1);
    let team_two = random_battle_spawn_zone(&battlefield, 2);

    let humans = humans.clamp(1, 7);
    let mut human_tanks = Vec::with_capacity(humans);
    for slot in 0..7 {
        let vehicle = if slot == 0 {
            config.player_vehicle
        } else {
            random_battle_bot_vehicle(config.seed, 10 + slot as u64, config.player_vehicle.era())
        };
        let id = random_battle_spawn(&mut sim, &battlefield, team_one, slot, vehicle, config.seed);
        if slot < humans {
            human_tanks.push(id);
        } else {
            bot_ids.push(id);
        }
    }
    let player_tank = human_tanks[0];
    let target_tank =
        random_battle_spawn_enemy_team(&mut sim, &battlefield, team_two, config, &mut bot_ids);
    sim.refresh_spotting(Some(&battlefield.heightmap), &battlefield.static_cover);

    (
        BattleSetup {
            mode: BattleMode::Random7v7,
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

fn random_battle_spawn_enemy_team(
    sim: &mut SimulationState,
    map: &BattlefieldMap,
    zone: &SpawnZone,
    config: RandomBattleConfig,
    bot_ids: &mut Vec<TankId>,
) -> TankId {
    let mut target_tank = TankId(0);
    for slot in 0..7 {
        let vehicle =
            random_battle_bot_vehicle(config.seed, 30 + slot as u64, config.player_vehicle.era());
        let id = random_battle_spawn(sim, map, zone, slot, vehicle, config.seed);
        if slot == 0 {
            target_tank = id;
        }
        bot_ids.push(id);
    }
    target_tank
}

fn random_battle_spawn_zone(map: &BattlefieldMap, team: u16) -> &SpawnZone {
    map.spawn_zones.iter().find(|zone| zone.team == team).expect("team spawn zone")
}

/// Bots deploy from the player's era only — a battle is one technology bracket, never a museum
/// mix (the era system replaces tier spread entirely). An era too thin to field two designs
/// falls back to the full playable park rather than cloning one hull fourteen times.
fn random_battle_bot_vehicle(seed: BattleSeed, salt: u64, era: game_core::Era) -> VehicleKind {
    let roster: Vec<VehicleKind> = era.playable().collect();
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
    seed: BattleSeed,
) -> TankId {
    let position = random_battle_spawn_position(map, zone, slot, seed);
    sim.spawn_tank_with_yaw(TeamId(zone.team), vehicle.spec(), position, zone.facing_yaw_rad)
}

fn random_battle_spawn_position(
    map: &BattlefieldMap,
    zone: &SpawnZone,
    slot: usize,
    seed: BattleSeed,
) -> Vec3 {
    const OFFSETS: [(f32, f32); 7] = [
        (0.0, -8.0),
        (-22.0, -22.0),
        (0.0, -22.0),
        (22.0, -22.0),
        (-22.0, -40.0),
        (0.0, -40.0),
        (22.0, -40.0),
    ];
    let (local_x, local_z) = OFFSETS[slot % OFFSETS.len()];
    let jitter_x = (seed.random_battle_unit(100 + slot as u64) - 0.5) * 3.0;
    let jitter_z = (seed.random_battle_unit(200 + slot as u64) - 0.5) * 3.0;
    let yaw = zone.facing_yaw_rad;
    let forward = Vec3::new(yaw.sin(), 0.0, yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let center = Vec3::from_array(zone.center);
    let flat = center + right * (local_x + jitter_x) + forward * (local_z + jitter_z);
    random_battle_ground_position(map, flat.x, flat.z)
}

fn random_battle_ground_position(map: &BattlefieldMap, x: f32, z: f32) -> Vec3 {
    Vec3::new(x, map.heightmap.sample_height(x, z).unwrap_or(0.0), z)
}
