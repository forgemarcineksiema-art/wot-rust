use game_core::{DamageEvent, ShellImpact, TankId, TankSpec, VehicleKind};
use net::{ClientInputCommand, Snapshot};
use sim::SimulationState;
use terrain::BattlefieldMap;

use crate::RandomBattleConfig;
use crate::ServerTickConfig;
use crate::battle::{BattleMode, BattleOutcome};
use crate::bots::BotRoster;
use crate::setup::{BattleSetup, practice_duel_setup, random_7v7_setup};

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
    mode: BattleMode,
    player_tank: TankId,
    target_tank: TankId,
    bots: BotRoster,
    outcome: Option<BattleOutcome>,
    latest_snapshot: Snapshot,
    pending_damage_events: Vec<DamageEvent>,
    pending_shell_impacts: Vec<ShellImpact>,
}

impl LocalAuthoritativeServer {
    pub fn new(config: ServerTickConfig) -> Self {
        Self::new_with_player_vehicle(config, VehicleKind::T54_1951)
    }

    pub fn new_with_player_vehicle(config: ServerTickConfig, player_vehicle: VehicleKind) -> Self {
        Self::from_setup(config, practice_duel_setup(player_vehicle))
    }

    pub fn new_random_7v7(config: ServerTickConfig, battle: RandomBattleConfig) -> Self {
        Self::from_setup(config, random_7v7_setup(battle))
    }

    fn from_setup(config: ServerTickConfig, setup: BattleSetup) -> Self {
        let latest_snapshot = Snapshot::from(&setup.sim);
        Self {
            config,
            sim: setup.sim,
            battlefield: setup.battlefield,
            mode: setup.mode,
            player_tank: setup.player_tank,
            target_tank: setup.target_tank,
            bots: setup.bots,
            outcome: None,
            latest_snapshot,
            pending_damage_events: Vec::new(),
            pending_shell_impacts: Vec::new(),
        }
    }

    pub fn change_player_vehicle(&mut self, requested_vehicle: VehicleKind) -> Snapshot {
        self.change_player_vehicle_with_spec(requested_vehicle.spec())
    }

    /// Respawn the player's tank from a fully assembled [`TankSpec`] — the garage builds a custom
    /// loadout (modules + ammo + crew) into a spec and installs it here, so a non-stock build
    /// actually drives and fights with its chosen stats.
    pub fn change_player_vehicle_with_spec(&mut self, spec: TankSpec) -> Snapshot {
        if let Some(new_player_tank) = self.sim.replace_tank_with_spec(self.player_tank, spec) {
            self.player_tank = new_player_tank;
        }
        self.pending_damage_events.clear();
        self.pending_shell_impacts.clear();
        self.outcome = None;
        self.sim
            .refresh_spotting(Some(&self.battlefield.heightmap), &self.battlefield.static_cover);
        self.latest_snapshot = Snapshot::from(&self.sim);
        self.latest_snapshot.clone()
    }

    pub fn change_player_vehicle_with_spec_for_player(&mut self, spec: TankSpec) -> Snapshot {
        self.change_player_vehicle_with_spec(spec).filtered_for_viewer(self.player_tank)
    }

    pub fn player_tank(&self) -> TankId {
        self.player_tank
    }

    pub fn target_tank(&self) -> TankId {
        self.target_tank
    }

    pub fn battle_mode(&self) -> BattleMode {
        self.mode
    }

    pub fn battle_outcome(&self) -> Option<BattleOutcome> {
        self.outcome
    }

    pub fn authoritative_tick(&self) -> u64 {
        self.sim.tick()
    }

    pub fn latest_snapshot(&self) -> Snapshot {
        self.latest_snapshot.clone()
    }

    pub fn current_snapshot(&self) -> Snapshot {
        Snapshot::from(&self.sim)
    }

    pub fn latest_snapshot_for_player(&self) -> Snapshot {
        self.latest_snapshot.filtered_for_viewer(self.player_tank)
    }

    pub fn tick_with_input(&mut self, input: ClientInputCommand) -> AuthoritativeTick {
        let battle_over = self.outcome.is_some();
        let mut commands = Vec::with_capacity(1 + self.sim.tanks().len());
        commands.push((
            input.tank_id,
            if battle_over { sim::TankCommand::idle() } else { input.command },
        ));
        commands.extend(self.bots.commands(self.sim.tanks(), &self.battlefield, battle_over));

        self.sim.apply_commands_on_battlefield(
            &commands,
            self.config.timestep(),
            &self.battlefield.heightmap,
            &self.battlefield.static_cover,
        );
        if self.outcome.is_none() {
            self.outcome = BattleOutcome::from_tanks(self.sim.tanks());
        }
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

    pub fn tick_with_player_input(&mut self, input: ClientInputCommand) -> AuthoritativeTick {
        let viewer = self.player_tank;
        let tick = self.tick_with_input(input);
        AuthoritativeTick {
            server_tick: tick.server_tick,
            snapshot: tick.snapshot.map(|snapshot| snapshot.filtered_for_viewer(viewer)),
        }
    }
}
