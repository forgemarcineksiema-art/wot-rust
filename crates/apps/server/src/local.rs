use game_core::{DamageEvent, MatchWeather, ShellImpact, TankId, TankSpec, VehicleKind};
use net::{ClientInputCommand, Snapshot};
use sim::SimulationState;
use terrain::{BattlefieldMap, MapId};

use crate::RandomBattleConfig;
use crate::ServerTickConfig;
use crate::battle::{BattleMode, BattleOutcome, DrawReason, RANDOM_BATTLE_TIME_LIMIT_S};
use crate::bots::BotRoster;
use crate::setup::{BattleSetup, practice_duel_setup};

#[derive(Debug, Clone, PartialEq)]
pub struct AuthoritativeTick {
    pub server_tick: u64,
    pub snapshot: Option<Snapshot>,
}

#[derive(Debug, Clone)]
pub struct LocalAuthoritativeServer {
    config: ServerTickConfig,
    sim: SimulationState,
    map_id: MapId,
    battlefield: BattlefieldMap,
    weather: MatchWeather,
    mode: BattleMode,
    player_tank: TankId,
    target_tank: TankId,
    bots: BotRoster,
    outcome: Option<BattleOutcome>,
    /// Battle clock in server ticks; `None` runs untimed (practice duel). When the clock hits
    /// zero with both teams alive the battle ends in a draw — the safety net that guarantees a
    /// random battle always ends even if the last survivors never find each other.
    time_limit_ticks: Option<u64>,
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
        Self::from_setup(config, crate::setup::random_7v7_setup(battle))
    }

    /// The dedicated server's constructor (N2): the first `humans` team-one tanks belong to
    /// connected players (returned in slot order), bots drive the rest.
    pub fn new_random_7v7_for_humans(
        config: ServerTickConfig,
        battle: RandomBattleConfig,
        humans: usize,
    ) -> (Self, Vec<TankId>) {
        let (setup, human_tanks) = crate::setup::random_7v7_setup_for_humans(battle, humans);
        (Self::from_setup(config, setup), human_tanks)
    }

    /// Per-target observer masks (bit = tank index) against the LIVE cover — the per-viewer
    /// filter's second input beside the team masks already on the snapshot.
    pub fn observer_masks(&self) -> Vec<u16> {
        let live_cover: std::borrow::Cow<'_, [terrain::StaticCoverObject]> =
            if self.sim.cover_states().iter().all(|state| state.phase == sim::CoverPhase::Intact) {
                std::borrow::Cow::Borrowed(&self.battlefield.static_cover)
            } else {
                std::borrow::Cow::Owned(sim::live_cover_for_blocking(
                    &self.battlefield.static_cover,
                    self.sim.cover_states(),
                ))
            };
        sim::compute_observer_masks(
            self.sim.tanks(),
            Some(&self.battlefield.heightmap),
            &live_cover,
        )
    }

    fn from_setup(config: ServerTickConfig, setup: BattleSetup) -> Self {
        let latest_snapshot = Snapshot::from(&setup.sim);
        let time_limit_ticks = match setup.mode {
            BattleMode::PracticeDuel => None,
            BattleMode::Random7v7 => {
                Some(u64::from(config.server_tick_hz()) * u64::from(RANDOM_BATTLE_TIME_LIMIT_S))
            }
        };
        Self {
            config,
            sim: setup.sim,
            map_id: setup.map_id,
            battlefield: setup.battlefield,
            weather: setup.weather,
            mode: setup.mode,
            player_tank: setup.player_tank,
            target_tank: setup.target_tank,
            bots: setup.bots,
            outcome: None,
            time_limit_ticks,
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

    /// The map this battle runs on. The client regenerates the identical battlefield from
    /// this id — the world itself never crosses the wire.
    pub fn map_id(&self) -> MapId {
        self.map_id
    }

    /// The match's presentation weather, rolled once from the battle seed at setup.
    pub fn weather(&self) -> MatchWeather {
        self.weather
    }

    pub fn battle_outcome(&self) -> Option<BattleOutcome> {
        self.outcome
    }

    /// Seconds left on the battle clock; `None` when this battle runs untimed.
    pub fn battle_time_remaining_s(&self) -> Option<f32> {
        let limit = self.time_limit_ticks?;
        let remaining_ticks = limit.saturating_sub(self.sim.tick());
        Some(remaining_ticks as f32 / self.config.server_tick_hz().max(1) as f32)
    }

    /// Shrink (or clear) the battle clock. A tuning/testing knob — driving a real 60 Hz battle
    /// to the ten-minute limit tick by tick is not something a test should pay for.
    pub fn override_battle_time_limit_ticks(&mut self, time_limit_ticks: Option<u64>) {
        self.time_limit_ticks = time_limit_ticks;
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
        self.tick_with_inputs(&[(input.tank_id, input.command)])
    }

    /// The battle core's real heartbeat (N2): one authoritative tick fed by ANY number of human
    /// commands — one for the local desktop game, up to seven from the dedicated server's client
    /// table. Bots fill in for every roster tank not driven by a human this tick.
    pub fn tick_with_inputs(&mut self, inputs: &[(TankId, sim::TankCommand)]) -> AuthoritativeTick {
        let battle_over = self.outcome.is_some();
        let mut commands = Vec::with_capacity(inputs.len() + self.sim.tanks().len());
        for (tank_id, command) in inputs {
            commands
                .push((*tank_id, if battle_over { sim::TankCommand::idle() } else { *command }));
        }
        // The bots read LAST tick's damage events (cleared on the next sim step): the honest
        // "we just got hit" signal that lets a blind bot turn toward the fire.
        //
        // Their raycasts must see the cover the battle actually blocks with — otherwise a bot
        // keeps hiding behind a flattened building and refuses to fire through the hole it just
        // made. The pristine common case borrows the authored slice; only a battle that has
        // damaged cover pays for building the live view.
        let live_cover: std::borrow::Cow<'_, [terrain::StaticCoverObject]> =
            if self.sim.cover_states().iter().all(|state| state.phase == sim::CoverPhase::Intact) {
                std::borrow::Cow::Borrowed(&self.battlefield.static_cover)
            } else {
                std::borrow::Cow::Owned(sim::live_cover_for_blocking(
                    &self.battlefield.static_cover,
                    self.sim.cover_states(),
                ))
            };
        commands.extend(self.bots.commands(
            self.sim.tick(),
            self.sim.tanks(),
            &self.battlefield,
            &live_cover,
            battle_over,
            self.sim.damage_events(),
        ));

        self.sim.apply_commands_on_battlefield(
            &commands,
            self.config.timestep(),
            &self.battlefield.heightmap,
            &self.battlefield.static_cover,
        );
        // Fold this tick's crater ledger into the ground the NEXT tick stands on (protocol
        // v31): an unchanged ledger is a cheap compare-and-return inside set_craters.
        self.battlefield.heightmap.set_craters(self.sim.craters());
        if self.outcome.is_none() {
            self.outcome = BattleOutcome::from_tanks(self.sim.tanks());
        }
        // Elimination on the final tick outranks the clock; only a still-live battle times out.
        if self.outcome.is_none()
            && self.time_limit_ticks.is_some_and(|limit| self.sim.tick() >= limit)
        {
            self.outcome = Some(BattleOutcome::Draw { reason: DrawReason::TimeExpired });
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
