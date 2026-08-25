use game_core::{DamageEvent, MatchWeather, ShellImpact, TankId, TankSpec, VehicleKind};
use net::{ClientInputCommand, Snapshot};
use sim::SimulationState;
use terrain::{BattlefieldMap, MapId};

use crate::RandomBattleConfig;
use crate::ServerTickConfig;
use crate::battle::{BattleMode, BattleOutcome, RANDOM_BATTLE_TIME_LIMIT_S};
use crate::bots::BotRoster;
use crate::setup::{BattleSetup, practice_duel_setup};

#[derive(Debug, Clone, PartialEq)]
pub struct AuthoritativeTick {
    pub server_tick: u64,
    pub snapshot: Option<Snapshot>,
    /// This tick's one-shot consequences, exposed independently of snapshot cadence so the
    /// remote host can feed its reliable per-recipient combat lane.
    pub damage_events: Vec<DamageEvent>,
    pub shell_impacts: Vec<ShellImpact>,
    /// Perforations carved this tick (protocol v39). Permanent per-hull state, replicated as a
    /// stream of additions on the same reliable lane instead of riding every snapshot.
    pub armor_breaches: Vec<sim::event_stamp::ArmorBreachRecord>,
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
    /// Battle clock in server ticks; `None` runs untimed (practice duel). When it runs out the
    /// board decides: whoever holds more hulls wins, and only a genuine tie is a draw (see
    /// [`BattleOutcome::from_time_expiry`]). The safety net that guarantees a random battle always
    /// ends even if the last survivors never find each other.
    time_limit_ticks: Option<u64>,
    latest_snapshot: Snapshot,
    pending_damage_events: Vec<DamageEvent>,
    /// Shots fired since the last emitted snapshot. A gun fires on a sim tick; a snapshot goes out
    /// on a slower schedule, so a shot on a non-snapshot tick has to WAIT rather than vanish.
    pending_shots_fired: Vec<game_core::ShotFired>,
    pending_shell_impacts: Vec<ShellImpact>,
}

impl LocalAuthoritativeServer {
    pub fn new(config: ServerTickConfig) -> Self {
        Self::new_with_player_vehicle(config, VehicleKind::BENCHMARK)
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

    /// Every hull's complete perforation set (protocol v39). A crew joining mid-battle gets this
    /// once, before the stream of additions; without it the stream has no baseline to apply to
    /// and a late joiner would see undamaged steel forever.
    pub fn armor_breach_state(&self) -> Vec<sim::event_stamp::ArmorBreachRecord> {
        self.sim.armor_breach_state()
    }

    /// Per-target observer masks (bit = tank index) against the LIVE cover — the per-viewer
    /// filter's second input beside the team masks already on the snapshot.
    pub fn observer_masks(&self) -> Vec<sim::ObserverMask> {
        let live_cover = sim::live_cover_for_sight_and_shells(
            &self.battlefield.static_cover,
            self.sim.cover_states(),
        );
        sim::compute_observer_masks(
            self.sim.tanks(),
            self.sim.tick(),
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
            pending_shots_fired: Vec::new(),
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
        self.pending_shots_fired.clear();
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

    /// The sim tick the clock expires on, or `None` for an untimed battle. Sent once in
    /// `StartBattle` so a remote client can run the countdown locally (v45).
    pub fn time_limit_tick(&self) -> Option<u64> {
        self.time_limit_ticks
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

    /// Knock a hull out directly. The same argument as the clock override: fighting a 7v7 down to
    /// a specific board state, shell by shell, is not something a test about the OUTCOME RULE
    /// should have to pay for. Deliberately the narrowest possible door — it sets hit points to
    /// zero and nothing else, so it cannot be mistaken for a damage path.
    pub fn knock_out_for_test(&mut self, tank: TankId) {
        if let Some(state) = self.sim.tank_mut(tank) {
            state.hit_points = 0;
        }
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

    /// The live authoritative tanks, borrowed. The client's contact predictor reads neighbour poses
    /// from here instead of building a whole `Snapshot` every tick just to see where they are.
    pub fn tanks(&self) -> &[sim::TankState] {
        self.sim.tanks()
    }

    pub fn authoritative_motion(&self, tank_id: TankId) -> Option<net::AuthoritativeMotion> {
        let tank = self.sim.tank(tank_id)?;
        Some(net::AuthoritativeMotion {
            velocity_mps: tank.velocity_mps.to_array(),
            hull_yaw_velocity_rad_s: tank.hull_yaw_velocity_rad_s,
        })
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
        self.sim.refresh_live_cover(&self.battlefield.static_cover);
        let live_cover = self.sim.cached_sight_cover();
        commands.extend(self.bots.commands(
            self.sim.tick(),
            self.sim.tanks(),
            &self.battlefield,
            self.sim.ground(),
            live_cover,
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
            // The clock is a safety net, not a verdict of "nobody won". Whoever is ahead on hulls
            // when it runs out has won the battle — see `BattleOutcome::from_time_expiry`.
            self.outcome = Some(BattleOutcome::from_tanks_at_time_expiry(self.sim.tanks()));
        }
        self.pending_damage_events.extend_from_slice(self.sim.damage_events());
        self.pending_shots_fired.extend_from_slice(self.sim.shots_fired());
        self.pending_shell_impacts.extend_from_slice(self.sim.shell_impacts());
        let damage_events = self.sim.damage_events().to_vec();
        let shell_impacts = self.sim.shell_impacts().to_vec();
        let armor_breaches = self.sim.armor_breach_events().to_vec();

        let snapshot = if self.config.snapshot_schedule().should_emit(self.sim.tick()) {
            let mut snapshot = Snapshot::from(&self.sim);
            snapshot.damage_events = std::mem::take(&mut self.pending_damage_events);
            snapshot.shell_impacts = std::mem::take(&mut self.pending_shell_impacts);
            snapshot.shots_fired = std::mem::take(&mut self.pending_shots_fired);
            self.latest_snapshot = snapshot.clone();
            Some(snapshot)
        } else {
            None
        };

        AuthoritativeTick {
            server_tick: self.sim.tick(),
            snapshot,
            damage_events,
            shell_impacts,
            armor_breaches,
        }
    }

    pub fn tick_with_player_input(&mut self, input: ClientInputCommand) -> AuthoritativeTick {
        let viewer = self.player_tank;
        let tick = self.tick_with_input(input);
        AuthoritativeTick {
            server_tick: tick.server_tick,
            snapshot: tick.snapshot.map(|snapshot| snapshot.filtered_for_viewer(viewer)),
            damage_events: tick.damage_events,
            shell_impacts: tick.shell_impacts,
            armor_breaches: tick.armor_breaches,
        }
    }
}
