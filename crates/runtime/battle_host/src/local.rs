use game_core::{DamageEvent, MatchWeather, ShellImpact, TankId, TankSpec, VehicleKind};
use net::{ClientInputCommand, Snapshot};
use sim::SimulationState;
use terrain::{BattlefieldMap, MapId};

use crate::RandomBattleConfig;
use crate::ServerTickConfig;
use crate::battle::{BattleFormat, BattleMode, BattleOutcome};
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
    /// Hulls that died this tick (protocol v51, W-3): one kill per `target_destroyed` damage
    /// event, for EVERY crew — the remote host fans these out on the reliable lane, the local
    /// session reads them straight off the tick.
    pub kills: Vec<game_core::KillEvent>,
    /// Team commands admitted this tick (protocol v51, W-5) — the local host's relay; the
    /// remote host keeps its own limiter per crew and never reads this.
    pub team_commands: Vec<net::TeamCommandRelay>,
}

/// Where one authoritative tick's time goes (Q8): the sections of [`tick_with_inputs`] and
/// the viewer filter of [`tick_with_player_input`], summed over the profiled ticks. Read by
/// the `tick_sections` example against the client's frame log; off unless armed, so the game
/// pays nothing for it.
///
/// [`tick_with_inputs`]: LocalAuthoritativeServer::tick_with_inputs
/// [`tick_with_player_input`]: LocalAuthoritativeServer::tick_with_player_input
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TickSections {
    pub ticks: u64,
    /// `refresh_live_cover`: the sight cover with landed turrets (a no-op while nothing changed).
    pub live_cover_ms: f64,
    /// The bots' commands: every brain's route, target and aim.
    pub bots_ms: f64,
    /// The simulation step: movement, contact, shells, damage, the sim's own spotting refresh.
    pub sim_ms: f64,
    /// The crater ledger copied onto the heightmap, the outcome check.
    pub craters_ms: f64,
    /// The spotting log's observer masks, on emitting ticks.
    pub spotting_log_ms: f64,
    /// `Snapshot::from` and the pending events, on emitting ticks.
    pub snapshot_ms: f64,
    /// The viewer filter (`view_for`, its own observer masks), on emitting ticks.
    pub view_ms: f64,
    /// Everything else in the tick: the event copies, the outcome.
    pub other_ms: f64,
    /// The whole of `tick_with_inputs` (+ `view_for`), wall clock.
    pub total_ms: f64,
    /// How many times the observer masks (30 x 29 lines of sight) were computed — once per
    /// emitting tick, since Q8; the spotting log and every viewer's cut read the same masks.
    pub mask_computations: u64,
    /// How many times a caller got the tick's masks back without computing them again.
    pub mask_reuses: u64,
}

impl TickSections {
    pub fn sections(&self) -> [(&'static str, f64); 8] {
        [
            ("live_cover", self.live_cover_ms),
            ("bots", self.bots_ms),
            ("sim", self.sim_ms),
            ("craters", self.craters_ms),
            ("spotting_log", self.spotting_log_ms),
            ("snapshot", self.snapshot_ms),
            ("view", self.view_ms),
            ("other", self.other_ms),
        ]
    }

    /// The table: mean milliseconds per tick, section by section.
    pub fn table(&self) -> String {
        use std::fmt::Write as _;
        let n = self.ticks.max(1) as f64;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "authoritative tick over {} ticks: {:.3} ms mean",
            self.ticks,
            self.total_ms / n
        );
        for (name, ms) in self.sections() {
            let _ = writeln!(
                out,
                "  {name:<14} {:>8.3} ms  {:>5.1} %",
                ms / n,
                100.0 * ms / self.total_ms.max(1e-9)
            );
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct LocalAuthoritativeServer {
    config: ServerTickConfig,
    /// Armed by [`Self::enable_tick_profile`]; `None` in the game.
    tick_profile: Option<TickSections>,
    /// The observer masks of the tick they were computed on (Q8): the spotting log, the local
    /// viewer's cut and the dedicated host's per-client cuts all read the same masks once,
    /// instead of each walking the 30 x 29 lines of sight again.
    emitted_masks: Option<(u64, Vec<sim::ObserverMask>)>,
    /// Reuse count for the profile (`observer_masks` takes `&self`).
    mask_reuses: std::cell::Cell<u64>,
    sim: SimulationState,
    map_id: MapId,
    battlefield: BattlefieldMap,
    weather: MatchWeather,
    mode: BattleMode,
    format: Option<BattleFormat>,
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
    /// The hulls humans drive (v51, W-1): the roster names them `CrewKind::Human`. The desktop
    /// battle seats one; the dedicated host seats up to seven through
    /// [`Self::new_random_7v7_for_humans`].
    human_tanks: Vec<TankId>,
    /// The one local crew's command allowance (v51, W-5): the same limiter the remote host runs
    /// per client, so local play cannot say more than a remote crew could.
    command_limiter: net::TeamCommandLimiter,
    pending_team_commands: Vec<net::TeamCommandRelay>,
    /// Who saw whom (v52, W-7): from the observer masks, once per snapshot tick; closed at
    /// the end; read per hull after the battle.
    spotting_log: crate::SpottingLog,
}

impl LocalAuthoritativeServer {
    pub fn new(config: ServerTickConfig) -> Self {
        Self::new_with_player_vehicle(config, VehicleKind::BENCHMARK)
    }

    pub fn new_with_player_vehicle(config: ServerTickConfig, player_vehicle: VehicleKind) -> Self {
        Self::from_setup(config, practice_duel_setup(player_vehicle))
    }

    pub fn new_random_7v7(config: ServerTickConfig, battle: RandomBattleConfig) -> Self {
        Self::new_random(config, battle.with_format(BattleFormat::SevenVsSeven))
    }

    pub fn new_random(config: ServerTickConfig, battle: RandomBattleConfig) -> Self {
        Self::from_setup(config, crate::setup::random_battle_setup(battle))
    }

    /// One player, twenty-nine bots, no socket or account. AI battles always use 15v15.
    pub fn new_ai_battle(config: ServerTickConfig, battle: RandomBattleConfig) -> Self {
        let mut setup =
            crate::setup::random_battle_setup(battle.with_format(BattleFormat::FifteenVsFifteen));
        setup.mode = BattleMode::AiBattle;
        Self::from_setup(config, setup)
    }

    /// The dedicated server's constructor (N2): the first `humans` team-one tanks belong to
    /// connected players (returned in slot order), bots drive the rest.
    pub fn new_random_7v7_for_humans(
        config: ServerTickConfig,
        battle: RandomBattleConfig,
        human_vehicles: &[Option<game_core::VehicleKind>],
    ) -> (Self, Vec<TankId>) {
        Self::new_random_for_humans(
            config,
            battle.with_format(BattleFormat::SevenVsSeven),
            human_vehicles,
        )
    }

    /// M7: the battle the matchmaker dealt, seated as dealt — the coordinator's constructor.
    /// Returns every crew's tank in the plan's order.
    pub fn new_from_plan(
        config: ServerTickConfig,
        battle: RandomBattleConfig,
        plan: &matchmaker::BattlePlan,
        wishes: &[(matchmaker::CrewId, Option<game_core::VehicleKind>)],
    ) -> (Self, Vec<(matchmaker::CrewId, TankId)>) {
        let (setup, crews) = crate::setup::planned_battle_setup(battle, plan, wishes);
        let mut server = Self::from_setup(config, setup);
        server.human_tanks = crews.iter().map(|(_, tank)| *tank).collect();
        (server, crews)
    }

    /// Format-aware dedicated setup (M6 deals the crews across both teams).
    pub fn new_random_for_humans(
        config: ServerTickConfig,
        battle: RandomBattleConfig,
        human_vehicles: &[Option<game_core::VehicleKind>],
    ) -> (Self, Vec<TankId>) {
        let (setup, human_tanks) =
            crate::setup::random_battle_setup_for_humans(battle, human_vehicles);
        let mut server = Self::from_setup(config, setup);
        server.human_tanks = human_tanks.clone();
        (server, human_tanks)
    }

    /// The battle's roster (protocol v51, W-1): every hull named — vehicle, team, seat, crew
    /// kind — and no position. The remote host sends it with the seat word; the local session
    /// reads it here. Built from the live board, so a vehicle change in the practice garage
    /// re-seats the new hull under the same rule.
    pub fn roster(&self) -> Vec<net::RosterEntry> {
        net::roster_from_tanks(self.sim.tanks(), &self.human_tanks)
    }

    /// M9 (`docs/game-modes.md` R7): a crew past its reconnect budget hands its hull to the bot
    /// brain — the hull keeps fighting for its side and the roster says who drives it. `true`
    /// when the roster changed.
    pub fn adopt_hull_as_bot(&mut self, tank: TankId) -> bool {
        let was_human = self.human_tanks.contains(&tank);
        self.human_tanks.retain(|human| *human != tank);
        self.bots.adopt(tank) || was_human
    }

    /// M9: a crew — returning, or fresh — claims a hull the brain drives; the brain lets go and
    /// the roster names a crew again. `true` when the roster changed.
    pub fn release_hull_to_crew(&mut self, tank: TankId) -> bool {
        let released = self.bots.release(tank);
        let was_bot = !self.human_tanks.contains(&tank);
        if was_bot {
            self.human_tanks.push(tank);
        }
        released || was_bot
    }

    /// The local crew's word to its team (protocol v51, W-5): admitted by the same limiter the
    /// remote host runs, relayed on the next tick as `AuthoritativeTick::team_commands`. A
    /// refused command returns `false` so the client can knock.
    pub fn send_team_command(
        &mut self,
        command: net::TeamCommand,
        target: Option<TankId>,
        map_position: Option<[f32; 2]>,
    ) -> bool {
        let tick_hz = self.config.server_tick_hz();
        if !self.command_limiter.admit(self.sim.tick(), tick_hz) {
            return false;
        }
        self.pending_team_commands.push(net::TeamCommandRelay {
            from: self.player_tank,
            command,
            target,
            map_position,
            server_tick: self.sim.tick(),
        });
        true
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
        if let Some((tick, masks)) = &self.emitted_masks
            && *tick == self.sim.tick()
        {
            self.mask_reuses.set(self.mask_reuses.get() + 1);
            return masks.clone();
        }
        self.compute_observer_masks()
    }

    fn compute_observer_masks(&self) -> Vec<sim::ObserverMask> {
        let live_cover = sim::live_cover_for_sight_and_shells(
            &self.battlefield.static_cover,
            self.sim.cover_states(),
        );
        sim::compute_observer_masks(
            self.sim.tanks(),
            self.sim.tick(),
            Some(&self.battlefield.heightmap),
            &live_cover,
            &sim::rubble_mounds(&self.battlefield.static_cover, self.sim.cover_states()),
        )
    }

    fn from_setup(config: ServerTickConfig, setup: BattleSetup) -> Self {
        let latest_snapshot = Snapshot::from(&setup.sim);
        let time_limit_ticks = setup
            .format
            .map(|format| u64::from(config.server_tick_hz()) * u64::from(format.time_limit_s()));
        Self {
            config,
            tick_profile: None,
            emitted_masks: None,
            mask_reuses: std::cell::Cell::new(0),
            sim: setup.sim,
            map_id: setup.map_id,
            battlefield: setup.battlefield,
            weather: setup.weather,
            mode: setup.mode,
            format: setup.format,
            player_tank: setup.player_tank,
            target_tank: setup.target_tank,
            bots: setup.bots,
            outcome: None,
            time_limit_ticks,
            latest_snapshot,
            pending_damage_events: Vec::new(),
            pending_shots_fired: Vec::new(),
            pending_shell_impacts: Vec::new(),
            human_tanks: vec![setup.player_tank],
            command_limiter: net::TeamCommandLimiter::default(),
            pending_team_commands: Vec::new(),
            spotting_log: crate::SpottingLog::default(),
        }
    }

    /// The spotting log for one hull (v52, W-7): every enemy that saw it, from how far, from
    /// when to when — empty until the battle is over, so a live client never holds an observer.
    pub fn spotting_log_for(&self, tank: TankId) -> Vec<net::SpottingRecord> {
        self.spotting_log.for_target(tank)
    }

    pub fn change_player_vehicle(&mut self, requested_vehicle: VehicleKind) -> Snapshot {
        self.change_player_vehicle_with_spec(requested_vehicle.spec())
    }

    /// Respawn the player's tank from a fully assembled [`TankSpec`] — the garage builds a custom
    /// loadout (modules + ammo + crew) into a spec and installs it here, so a non-stock build
    /// actually drives and fights with its chosen stats.
    pub fn change_player_vehicle_with_spec(&mut self, spec: TankSpec) -> Snapshot {
        if let Some(new_player_tank) = self.sim.replace_tank_with_spec(self.player_tank, spec) {
            for human in &mut self.human_tanks {
                if *human == self.player_tank {
                    *human = new_player_tank;
                }
            }
            self.player_tank = new_player_tank;
        }
        self.pending_damage_events.clear();
        self.pending_shell_impacts.clear();
        self.pending_shots_fired.clear();
        self.outcome = None;
        self.spotting_log = crate::SpottingLog::default();
        self.sim
            .refresh_spotting(Some(&self.battlefield.heightmap), &self.battlefield.static_cover);
        self.latest_snapshot = Snapshot::from(&self.sim);
        self.latest_snapshot.clone()
    }

    pub fn change_player_vehicle_with_spec_for_player(&mut self, spec: TankSpec) -> Snapshot {
        let snapshot = self.change_player_vehicle_with_spec(spec);
        self.view_for(&snapshot, self.player_tank)
    }

    /// Start summing where the ticks' time goes (Q8). Costs a few clock reads per tick.
    ///
    /// Idempotent (Q11): the client arms this once per frame rather than tracking whether a
    /// fresh battle replaced the host, so a second call must hand back the ticks already
    /// summed instead of dropping them on the floor.
    pub fn enable_tick_profile(&mut self) {
        if self.tick_profile.is_none() {
            self.tick_profile = Some(TickSections::default());
            self.mask_reuses.set(0);
        }
    }

    /// The sums so far, reset to zero; `None` when the profile is not armed.
    pub fn take_tick_profile(&mut self) -> Option<TickSections> {
        let reuses = self.mask_reuses.take();
        self.tick_profile.as_mut().map(|sections| {
            sections.mask_reuses = reuses;
            std::mem::take(sections)
        })
    }

    /// How many cover boxes every line of sight is tested against on this map.
    pub fn cover_box_count(&self) -> usize {
        self.battlefield.static_cover.len()
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

    pub fn battle_format(&self) -> Option<BattleFormat> {
        self.format
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

    /// Test door (N10): shoot out ONE module — the radio, for the own-eyes lock — and nothing
    /// else. As narrow as `knock_out_for_test`: a mask bit, no damage path.
    pub fn destroy_module_for_test(&mut self, tank: TankId, slot: game_core::ModuleSlot) {
        if let Some(state) = self.sim.tank_mut(tank) {
            state.modules.damage(slot, u32::MAX);
        }
    }

    /// Test door (N10): put a hull somewhere on the map, at rest. Position only; the next tick
    /// settles it on the ground like any other hull.
    pub fn place_for_test(&mut self, tank: TankId, position: glam::Vec3) {
        if let Some(state) = self.sim.tank_mut(tank) {
            state.position = position;
            state.velocity_mps = glam::Vec3::ZERO;
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
            hull_pitch_velocity_rad_s: tank.hull_pitch_velocity_rad_s,
            hull_roll_velocity_rad_s: tank.hull_roll_velocity_rad_s,
            hull_dive_pitch_rad: tank.hull_dive_pitch_rad,
            hull_dive_pitch_velocity_rad_s: tank.hull_dive_pitch_velocity_rad_s,
        })
    }

    pub fn latest_snapshot_for_player(&self) -> Snapshot {
        self.view_for(&self.latest_snapshot, self.player_tank)
    }

    /// The player's honest cut of a snapshot (Inny Poziom N10): the SAME rule the remote host
    /// applies — team intel through a working radio, the crew's own eyes always. The local
    /// host used to take the plain team-mask cut, so a player whose radio was shot out went
    /// blind to the hull in front of their own eyes: the sim's team mask already excludes a
    /// radio-dead observer's sightings, and the plain cut has no "own eyes" clause to put
    /// them back. One filter, one truth, whichever host the battle runs on.
    fn view_for(&self, snapshot: &Snapshot, viewer: TankId) -> Snapshot {
        let viewer_index =
            snapshot.tanks.iter().position(|tank| tank.tank_id == viewer).unwrap_or(usize::MAX);
        snapshot.filtered_for_viewer_with_observers(viewer, &self.observer_masks(), viewer_index)
    }

    pub fn tick_with_input(&mut self, input: ClientInputCommand) -> AuthoritativeTick {
        self.tick_with_inputs(&[(input.tank_id, input.command)])
    }

    /// The battle core's real heartbeat (N2): one authoritative tick fed by ANY number of human
    /// commands — one for the local desktop game, up to seven from the dedicated server's client
    /// table. Bots fill in for every roster tank not driven by a human this tick.
    pub fn tick_with_inputs(&mut self, inputs: &[(TankId, sim::TankCommand)]) -> AuthoritativeTick {
        let profiling = self.tick_profile.is_some();
        let tick_started = profiling.then(std::time::Instant::now);
        let mut section_started = tick_started;
        // Close the section that started at `section_started`, open the next one.
        let mut lap = |sections: &mut Option<TickSections>,
                       pick: fn(&mut TickSections) -> &mut f64| {
            if let (Some(sections), Some(started)) = (sections.as_mut(), section_started) {
                let now = std::time::Instant::now();
                *pick(sections) += now.duration_since(started).as_secs_f64() * 1000.0;
                section_started = Some(now);
            }
        };
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
        lap(&mut self.tick_profile, |sections| &mut sections.live_cover_ms);
        let live_cover = self.sim.cached_sight_cover();
        commands.extend(self.bots.commands(
            self.sim.tick(),
            self.sim.tanks(),
            &self.battlefield,
            self.sim.ground(),
            live_cover,
            self.sim.cached_rubble(),
            battle_over,
            self.sim.damage_events(),
        ));

        lap(&mut self.tick_profile, |sections| &mut sections.bots_ms);
        self.sim.apply_commands_on_battlefield(
            &commands,
            self.config.timestep(),
            &self.battlefield.heightmap,
            &self.battlefield.static_cover,
        );
        lap(&mut self.tick_profile, |sections| &mut sections.sim_ms);
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
        // v52 (W-7): who sees whom, from the same observer masks the per-viewer cut reads — one
        // word per snapshot tick; the battle's end closes every open span.
        lap(&mut self.tick_profile, |sections| &mut sections.craters_ms);
        let emitting = self.config.snapshot_schedule().should_emit(self.sim.tick());
        if emitting {
            // Once per emitting tick, for everyone who reads them this tick (Q8: the spotting
            // log and the viewer's cut each walked the lines of sight — 69 % of the tick).
            let masks = self.compute_observer_masks();
            if let Some(sections) = self.tick_profile.as_mut() {
                sections.mask_computations += 1;
            }
            if !self.spotting_log.is_finished() {
                self.spotting_log.observe(self.sim.tanks(), &masks, self.sim.tick());
            }
            self.emitted_masks = Some((self.sim.tick(), masks));
        }
        lap(&mut self.tick_profile, |sections| &mut sections.spotting_log_ms);
        if self.outcome.is_some() {
            self.spotting_log.finish(self.sim.tick());
        }
        self.pending_damage_events.extend_from_slice(self.sim.damage_events());
        self.pending_shots_fired.extend_from_slice(self.sim.shots_fired());
        self.pending_shell_impacts.extend_from_slice(self.sim.shell_impacts());
        let damage_events = self.sim.damage_events().to_vec();
        let shell_impacts = self.sim.shell_impacts().to_vec();
        let armor_breaches = self.sim.armor_breach_events().to_vec();
        // v51: a hull's death is its own event, for everyone — derived from the ONE damage
        // event that took it from alive to dead, so it is counted exactly once.
        let kills = damage_events.iter().filter_map(game_core::KillEvent::from_damage).collect();
        let team_commands = std::mem::take(&mut self.pending_team_commands);

        lap(&mut self.tick_profile, |sections| &mut sections.other_ms);
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
        lap(&mut self.tick_profile, |sections| &mut sections.snapshot_ms);
        if let (Some(sections), Some(started)) = (self.tick_profile.as_mut(), tick_started) {
            sections.ticks += 1;
            sections.total_ms += started.elapsed().as_secs_f64() * 1000.0;
        }

        AuthoritativeTick {
            server_tick: self.sim.tick(),
            snapshot,
            damage_events,
            shell_impacts,
            armor_breaches,
            kills,
            team_commands,
        }
    }

    pub fn tick_with_player_input(&mut self, input: ClientInputCommand) -> AuthoritativeTick {
        let viewer = self.player_tank;
        let tick = self.tick_with_input(input);
        let started = self.tick_profile.is_some().then(std::time::Instant::now);
        let snapshot = tick.snapshot.map(|snapshot| self.view_for(&snapshot, viewer));
        if let (Some(sections), Some(started)) = (self.tick_profile.as_mut(), started) {
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            sections.view_ms += ms;
            sections.total_ms += ms;
        }
        AuthoritativeTick {
            server_tick: tick.server_tick,
            snapshot,
            damage_events: tick.damage_events,
            shell_impacts: tick.shell_impacts,
            armor_breaches: tick.armor_breaches,
            kills: tick.kills,
            team_commands: tick.team_commands,
        }
    }
}
