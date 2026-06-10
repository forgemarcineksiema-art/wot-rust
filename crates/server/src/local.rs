use game_core::{DamageEvent, ShellImpact, TankId, TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use net::{ClientInputCommand, Snapshot};
use sim::SimulationState;
use std::f32::consts::PI;
use terrain::{BattlefieldMap, prokhorovka_hill_252_2};

use crate::ServerTickConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct AuthoritativeTick {
    pub server_tick: u64,
    pub snapshot: Option<Snapshot>,
}

#[derive(Debug, Clone)]
pub struct LocalAuthoritativeServer {
    config: ServerTickConfig,
    sim: SimulationState,
    battlefield: BattlefieldMap,
    player_tank: TankId,
    target_tank: TankId,
    latest_snapshot: Snapshot,
    pending_damage_events: Vec<DamageEvent>,
    pending_shell_impacts: Vec<ShellImpact>,
}

impl LocalAuthoritativeServer {
    pub fn new(config: ServerTickConfig) -> Self {
        Self::new_with_player_vehicle(config, VehicleKind::T55A)
    }

    pub fn new_with_player_vehicle(config: ServerTickConfig, player_vehicle: VehicleKind) -> Self {
        let battlefield = prokhorovka_hill_252_2();
        let mut sim = SimulationState::new();

        // A gentle 40 m duel corridor in the south-central steppe, clear of the central
        // ditch, embankment and the eastern hills: both tanks sit on their own ground and a
        // level shot carries cleanly (locked by the server combat-replication test), while
        // the rest of the map rolls and climbs.
        let player_pos = ground_position(&battlefield, 340.0, 300.0);
        let target_pos = ground_position(&battlefield, 340.0, 340.0);
        let player_tank = sim.spawn_tank(TeamId(1), player_vehicle.spec(), player_pos);
        let target_tank = sim.spawn_tank(TeamId(2), TankSpec::t54_1951(), target_pos);
        sim.tank_mut(target_tank).expect("spawned target tank").yaw_rad = PI;
        let latest_snapshot = Snapshot::from(&sim);

        Self {
            config,
            sim,
            battlefield,
            player_tank,
            target_tank,
            latest_snapshot,
            pending_damage_events: Vec::new(),
            pending_shell_impacts: Vec::new(),
        }
    }

    pub fn change_player_vehicle(&mut self, requested_vehicle: VehicleKind) -> Snapshot {
        if let Some(new_player_tank) =
            self.sim.replace_tank_with_spec(self.player_tank, requested_vehicle.spec())
        {
            self.player_tank = new_player_tank;
        }
        self.pending_damage_events.clear();
        self.pending_shell_impacts.clear();
        self.latest_snapshot = Snapshot::from(&self.sim);
        self.latest_snapshot.clone()
    }

    pub fn player_tank(&self) -> TankId {
        self.player_tank
    }

    pub fn target_tank(&self) -> TankId {
        self.target_tank
    }

    pub fn authoritative_tick(&self) -> u64 {
        self.sim.tick()
    }

    pub fn latest_snapshot(&self) -> Snapshot {
        self.latest_snapshot.clone()
    }

    pub fn tick_with_input(&mut self, input: ClientInputCommand) -> AuthoritativeTick {
        self.sim.apply_commands_on_battlefield(
            &[(input.tank_id, input.command)],
            self.config.timestep(),
            &self.battlefield.heightmap,
            &self.battlefield.static_cover,
        );
        self.pending_damage_events.extend_from_slice(self.sim.damage_events());
        self.pending_shell_impacts.extend_from_slice(self.sim.shell_impacts());

        let snapshot = if self.config.snapshot_schedule().should_emit(self.sim.tick()) {
            let mut snapshot = Snapshot::from(&self.sim);
            snapshot.damage_events = std::mem::take(&mut self.pending_damage_events);
            snapshot.shell_impacts = std::mem::take(&mut self.pending_shell_impacts);
            self.latest_snapshot = snapshot.clone();
            Some(snapshot)
        } else {
            None
        };

        AuthoritativeTick { server_tick: self.sim.tick(), snapshot }
    }
}

fn ground_position(battlefield: &BattlefieldMap, x: f32, z: f32) -> Vec3 {
    Vec3::new(x, battlefield.heightmap.sample_height(x, z).unwrap_or(0.0), z)
}
