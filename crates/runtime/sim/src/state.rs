use game_core::{BattleEventId, DamageEvent, ShellImpact, TankId, TankSpec, TeamId, TrackSide};
use glam::Vec3;
use physics::{ContactBody, TankFootprint, footprint_overlaps_cover_object};
use serde::{Deserialize, Serialize};
use terrain::{HeightMap, StaticCoverObject, WaterBody};

use crate::aim_dispersion::recover_aim_dispersion;
use crate::combat::{CombatTickContext, fire_click_buffers, try_fire_shell};
use crate::event_stamp::{BattleEventOutput, BattleEventStamp};
use crate::landing::apply_landing_impact;
use crate::ramming::apply_ramming_damage;
use crate::shell::ShellState;
use crate::shell_step::step_shells;
use crate::tank_drive::TankDriveWorld;
use crate::tank_factory::fresh_tank;
use crate::tank_state::TankState;
use crate::{FixedTimestep, TankCommand};

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationState {
    tick: u64,
    next_tank_id: u64,
    tanks: Vec<TankState>,
    shells: Vec<ShellState>,
    damage_events: Vec<DamageEvent>,
    /// Shells absorbed this tick without damaging an enemy (terrain, cover, wreck, or friendly
    /// hull), so the firing client gets impact feedback instead of a silently vanished shot.
    #[serde(default)]
    shell_impacts: Vec<ShellImpact>,
    /// Perforations carved THIS tick, in the order the targets' sets accepted them (protocol
    /// v39). Permanent state replicated as a stream of additions rather than by re-sending each
    /// hull's whole set in every snapshot — see `net`'s v39 note for the wire arithmetic.
    #[serde(default)]
    armor_breach_events: Vec<crate::event_stamp::ArmorBreachRecord>,
    /// Last-fresh-sight memory behind the spotted hold (see `spotting::SpottingMemory`).
    #[serde(default)]
    spotting_memory: crate::spotting::SpottingMemory,
    /// The battle map's standing water, installed once at setup (like the heightmap, it never
    /// changes mid-battle). Drives wading drag, drowning, and shell splashes. `serde(default)`
    /// keeps pre-water fixtures loading dry.
    #[serde(default)]
    water: Option<WaterBody>,
    /// The map's ground rule (`terrain::GroundClassifier`), installed once at battle setup like
    /// the water. It is derived wholly from the map, so it is not replicated: the client builds
    /// the same one from the same battlefield and the predictor reads an identical answer.
    /// `serde(skip)` keeps it out of saved states; `None` is bare grass everywhere.
    #[serde(skip)]
    ground: Option<terrain::GroundClassifier>,
    /// Live structural state of the map's static cover (protocol v21), index-aligned with the
    /// cover slice the battlefield passes in. Rebuilt when the cover count changes (battle setup),
    /// then damaged by HE and crushed by ramming; every blocking consumer sees the resolved live
    /// cover derived from it. `serde(default)` keeps pre-v21 fixtures loading with whole cover.
    #[serde(default)]
    cover_states: Vec<crate::cover_damage::CoverState>,
    /// The battle's crater ledger (protocol v31): quantized records of every high-explosive
    /// ground burst, appended by the shell step and folded into the heightmap overlay by the
    /// battlefield owner. `serde(default)` keeps pre-v31 fixtures loading with virgin ground.
    #[serde(default)]
    craters: Vec<terrain::CraterRecord>,
    /// Shell wounds on static-cover faces (protocol v32): purely visual, replicated whole so a
    /// late joiner dresses the same walls. Capped per cover by the append rule.
    #[serde(default)]
    cover_scars: Vec<terrain::CoverScar>,
    /// Last id assigned in the battle's one authoritative event stream. Zero means that no event
    /// has been emitted yet, including old serialized states that predate this field.
    #[serde(default)]
    last_battle_event_id: BattleEventId,
}

/// Cover damage one absorbed shell deals by its type: a high-explosive round brings structures
/// down, a kinetic round only chips them.
fn cover_damage_hp(shell_type: game_core::ShellType) -> u32 {
    match shell_type {
        game_core::ShellType::HighExplosive => 300,
        _ => 80,
    }
}

/// A hull must be moving at least this fast to flatten a hedgerow it drives into.
const COVER_CRUSH_MIN_SPEED_MPS: f32 = 2.5;
/// The nick a hull takes for bulldozing through cover â€” small, but not free.
const COVER_CRUSH_SELF_HP: u32 = 8;
/// How far ALONG ITS TRAVEL the hull's footprint is carried forward when asking what it is about
/// to flatten — the hedge goes over just before contact, so the same tick's movement drives
/// through instead of being stopped by the (still-blocking) intact hedge and losing the speed the
/// crush needs. It is a reach along the heading, never a radius: the old test put this distance
/// (plus the hull's HALF-LENGTH) around the hull's centre in every direction, so a T-54 driving
/// PARALLEL to a hedgerow flattened it from 2 m off its own flank, and a single `TreeLine` box is
/// tens of metres long — one clean pass deleted the whole run without touching it.
const COVER_CRUSH_APPROACH_M: f32 = 0.8;

impl SimulationState {
    pub fn new() -> Self {
        Self {
            tick: 0,
            next_tank_id: 1,
            tanks: Vec::new(),
            shells: Vec::new(),
            damage_events: Vec::new(),
            shell_impacts: Vec::new(),
            armor_breach_events: Vec::new(),
            spotting_memory: crate::spotting::SpottingMemory::default(),
            water: None,
            ground: None,
            cover_states: Vec::new(),
            craters: Vec::new(),
            cover_scars: Vec::new(),
            last_battle_event_id: BattleEventId::default(),
        }
    }

    /// The battle's replicated crater ledger (protocol v31). The battlefield owner folds it
    /// into the heightmap via `HeightMap::set_craters`; the snapshot re-sends it whole.
    pub fn craters(&self) -> &[terrain::CraterRecord] {
        &self.craters
    }

    /// Shell wounds on static-cover faces (protocol v32), replicated whole every snapshot.
    pub fn cover_scars(&self) -> &[terrain::CoverScar] {
        &self.cover_scars
    }

    /// Live structural state of the static cover (protocol v21), index-aligned with the map's
    /// cover. The client reads it to draw collapsed buildings and cleared foliage.
    pub fn cover_states(&self) -> &[crate::cover_damage::CoverState] {
        &self.cover_states
    }

    /// Install the battle map's standing water (see [`terrain::WaterBody`]). Call once at
    /// battle setup alongside the heightmap; `None` (the default) is a dry map.
    pub fn set_water(&mut self, water: Option<WaterBody>) {
        self.water = water;
    }

    pub fn water(&self) -> Option<WaterBody> {
        self.water
    }

    /// Install the map's ground rule. Call once at battle setup alongside the heightmap and the
    /// water; `None` (the default) drives every surface as grass.
    pub fn set_ground(&mut self, ground: Option<terrain::GroundClassifier>) {
        self.ground = ground;
    }

    /// The map's ground rule, for anything that must reason about the surface a hull is on —
    /// the bots' water escape has to know what it is braking over.
    pub fn ground(&self) -> Option<&terrain::GroundClassifier> {
        self.ground.as_ref()
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn tanks(&self) -> &[TankState] {
        &self.tanks
    }

    /// Entries in the spotting hold-memory — bounded by the live roster (see
    /// `spotting::SpottingMemory::forget_departed`).
    #[cfg(test)]
    pub(crate) fn spotting_memory_len(&self) -> usize {
        self.spotting_memory.len()
    }

    pub fn shells(&self) -> &[ShellState] {
        &self.shells
    }

    pub fn damage_events(&self) -> &[DamageEvent] {
        &self.damage_events
    }

    pub fn shell_impacts(&self) -> &[ShellImpact] {
        &self.shell_impacts
    }

    /// Perforations carved this tick, oldest first (protocol v39). Replication forwards these on
    /// the reliable lane; a client replays them through `ArmorBreachSet::add` and converges.
    pub fn armor_breach_events(&self) -> &[crate::event_stamp::ArmorBreachRecord] {
        &self.armor_breach_events
    }

    /// Every hull's complete perforation set — what a joining crew must be given before the
    /// stream of additions means anything.
    pub fn armor_breach_state(&self) -> Vec<crate::event_stamp::ArmorBreachRecord> {
        self.tanks
            .iter()
            .flat_map(|tank| {
                tank.armor_breaches.breaches().iter().map(|breach| {
                    crate::event_stamp::ArmorBreachRecord { tank: tank.id, breach: breach.clone() }
                })
            })
            .collect()
    }

    pub fn refresh_spotting(&mut self, heightmap: Option<&HeightMap>, cover: &[StaticCoverObject]) {
        if self.cover_states.len() != cover.len() {
            self.cover_states = crate::cover_damage::initial_cover_states(cover);
        }
        let sight_cover =
            crate::cover_damage::live_cover_for_sight_and_shells(cover, &self.cover_states);
        crate::spotting::apply_spotted_masks_with_hold(
            self.tick,
            &mut self.tanks,
            &mut self.spotting_memory,
            heightmap,
            &sight_cover,
        );
    }

    pub fn spawn_tank(&mut self, team: TeamId, spec: TankSpec, position: Vec3) -> TankId {
        self.spawn_tank_with_yaw(team, spec, position, 0.0)
    }

    pub fn spawn_tank_with_yaw(
        &mut self,
        team: TeamId,
        spec: TankSpec,
        position: Vec3,
        yaw_rad: f32,
    ) -> TankId {
        let id = TankId(self.next_tank_id);
        self.next_tank_id += 1;
        self.tanks.push(fresh_tank(id, team, spec, position, yaw_rad));
        id
    }

    pub fn replace_tank_with_spec(&mut self, tank_id: TankId, spec: TankSpec) -> Option<TankId> {
        let index = self.tanks.iter().position(|tank| tank.id == tank_id)?;
        let old = self.tanks.remove(index);
        self.shells.retain(|shell| shell.owner != tank_id);

        let id = TankId(self.next_tank_id);
        self.next_tank_id += 1;
        self.tanks.push(fresh_tank(id, old.team, spec, old.position, old.yaw_rad));
        Some(id)
    }

    pub fn tank(&self, id: TankId) -> Option<&TankState> {
        self.tanks.iter().find(|tank| tank.id == id)
    }

    pub fn tank_mut(&mut self, id: TankId) -> Option<&mut TankState> {
        self.tanks.iter_mut().find(|tank| tank.id == id)
    }

    /// Throw a track outright (test/util helper). Real combat degrades the pool by a
    /// shell-dependent chunk in `combat::apply_shell_impact`; this is the "immediately broken"
    /// shortcut the state tests lean on.
    pub fn damage_track(&mut self, id: TankId, side: TrackSide) {
        if let Some(tank) = self.tank_mut(id) {
            tank.tracks.break_side(side);
        }
    }

    pub fn apply_commands(&mut self, commands: &[(TankId, TankCommand)], timestep: FixedTimestep) {
        self.apply_commands_with_world(commands, timestep, None, &[]);
    }

    pub fn apply_commands_on_terrain(
        &mut self,
        commands: &[(TankId, TankCommand)],
        timestep: FixedTimestep,
        heightmap: &HeightMap,
    ) {
        self.apply_commands_with_world(commands, timestep, Some(heightmap), &[]);
    }

    pub fn apply_commands_on_battlefield(
        &mut self,
        commands: &[(TankId, TankCommand)],
        timestep: FixedTimestep,
        heightmap: &HeightMap,
        cover: &[StaticCoverObject],
    ) {
        self.apply_commands_with_world(commands, timestep, Some(heightmap), cover);
    }

    fn apply_commands_with_world(
        &mut self,
        commands: &[(TankId, TankCommand)],
        timestep: FixedTimestep,
        heightmap: Option<&HeightMap>,
        cover: &[StaticCoverObject],
    ) {
        let dt = timestep.dt_seconds();
        let context = CombatTickContext { dt_seconds: dt, tick: self.tick, water: self.water };
        self.damage_events.clear();
        self.shell_impacts.clear();
        self.armor_breach_events.clear();
        let mut event_stamp = BattleEventStamp::new(self.last_battle_event_id, self.tick);
        // Keep the cover states aligned with the map's cover (rebuilt only when the count changes,
        // i.e. at battle setup). A dry `apply_commands` passes no cover and clears the states.
        if self.cover_states.len() != cover.len() {
            self.cover_states = crate::cover_damage::initial_cover_states(cover);
        }
        // A hull driving into a hedgerow flattens it (and takes a nick), BEFORE the live cover is
        // resolved, so this tick's movement already drives through the gap it just opened.
        {
            let tanks = &mut self.tanks;
            let states = &mut self.cover_states;
            for tank in tanks.iter_mut() {
                if tank.hit_points == 0 || tank.velocity_mps.length() < COVER_CRUSH_MIN_SPEED_MPS {
                    continue;
                }
                // The hull's own oriented footprint, carried one approach-length along the
                // direction it is actually travelling. This is the same SAT movement collides
                // with, so what the hull flattens is what the hull would have hit.
                let heading =
                    Vec3::new(tank.velocity_mps.x, 0.0, tank.velocity_mps.z).normalize_or_zero();
                let probe = tank.position + heading * COVER_CRUSH_APPROACH_M;
                let footprint = TankFootprint::from_hitbox(tank.spec.hitbox);
                for (index, object) in cover.iter().enumerate() {
                    if footprint_overlaps_cover_object(probe, tank.yaw_rad, footprint, object)
                        && crate::cover_damage::crush_cover(states, object, index)
                    {
                        tank.hit_points = tank.hit_points.saturating_sub(COVER_CRUSH_SELF_HP);
                    }
                }
            }
        }
        // The cover this tick resolves against. Two slices, because two different questions:
        // what stops a HULL, and what stops a SHELL or a sight line. They agree on intact boxes
        // and on cleared ground, and part ways over rubble — see `cover_damage::CoverPurpose`.
        let live_cover = crate::cover_damage::LiveCover::resolve(cover, &self.cover_states);
        // ...and the third resolution: what a hull STANDS ON. A collapsed building is debris, and
        // debris is ground — the support envelope and the drive's own slope probe both read it.
        let rubble = crate::cover_damage::rubble_mounds(cover, &self.cover_states);
        for tank in &mut self.tanks {
            tank.reload_remaining_s = (tank.reload_remaining_s - dt).max(0.0);
            recover_aim_dispersion(tank, dt);
            crate::repair::step_crew_repair(tank, dt);
        }
        // Release buffered fire clicks the tick their reload completes â€” one attempt, then the
        // intent drops (if the gun died in the meantime, nothing fires and nothing lingers).
        for index in 0..self.tanks.len() {
            let tank = &mut self.tanks[index];
            if tank.fire_buffered && tank.reload_remaining_s <= 0.0 {
                tank.fire_buffered = false;
                if let Some(shell) = try_fire_shell(tank, self.tick) {
                    self.shells.push(shell);
                }
            }
        }

        // The tick's honest rigid-body order, and the reason it is worth the shape:
        //
        //   1. every hull decides a velocity, and NOBODY moves;
        //   2. hull-to-hull contacts are solved against where those velocities WOULD put them;
        //   3. the surviving velocities are spent and resolved against the world.
        //
        // The old order — drive, move, then refuse the move — deleted the whole point. A blocked
        // hull's momentum simply vanished and the hull it ran into never learned it had been hit,
        // which is why `tank_resolve` needed an interpenetration escape hatch to stop pile-ups
        // deadlocking. Solving after the move does not fix it either: a correction that reaches
        // the velocity only next tick lets a crowd gain ground every tick, and at a river ford
        // that means gaining it into the water.
        let footprint_of = |tank: &TankState| tank.spec.contact_footprint();
        let mut phases: Vec<(usize, TankCommand, crate::tank_drive::DrivePhase)> =
            Vec::with_capacity(commands.len());
        for (tank_id, command) in commands.iter().copied() {
            let Some(index) = self.tanks.iter().position(|tank| tank.id == tank_id) else {
                continue;
            };
            let command = command.clamped();
            let tank = &mut self.tanks[index];
            if tank.hit_points == 0 {
                // A wreck does not drive: the horizontal motion and the spin die here. Its
                // VERTICAL is not this loop's business — `settle_wrecks` below owns it, and
                // owns it for every wreck, commanded or not.
                tank.velocity_mps = Vec3::new(0.0, tank.velocity_mps.y, 0.0);
                tank.hull_yaw_velocity_rad_s = 0.0;
                continue;
            }
            let footprint = footprint_of(tank);
            let world = TankDriveWorld {
                heightmap,
                cover: live_cover.movement(),
                // Hull-to-hull is phase 2's and phase 3's business, not the drive's.
                tank_obstacles: &[],
                footprint: Some(&footprint),
                water: self.water,
                rubble: &rubble,
                ground: self.ground.as_ref(),
            };
            let phase = crate::tank_drive::advance_tank(tank, command, dt, world);
            phases.push((index, command, phase));
        }

        // PHASE 2 — the contacts.
        let contact_pairs = self.exchange_contact_momentum(dt);

        // PHASE 3 — spend what survived, then finish each hull's tick.
        for (index, command, phase) in phases {
            let footprint = footprint_of(&self.tanks[index]);
            let world = TankDriveWorld {
                heightmap,
                cover: live_cover.movement(),
                tank_obstacles: &[],
                footprint: Some(&footprint),
                water: self.water,
                rubble: &rubble,
                ground: self.ground.as_ref(),
            };
            let ground =
                crate::tank_drive::settle_tank(&mut self.tanks[index], command, dt, world, phase);
            let tank = &mut self.tanks[index];
            apply_landing_impact(
                tank,
                ground.landing_impact_mps,
                &mut self.damage_events,
                &mut event_stamp,
            );
            // Ammo switch before the fire check: the honest rule is simple â€” any real switch
            // restarts the full reload (the loader swaps the round out of the breech).
            if let Some(slot) = command.select_ammo
                && (slot as usize) < game_core::MAX_AMMO_SLOTS
                && slot != tank.selected_ammo
            {
                tank.selected_ammo = slot;
                tank.reload_remaining_s = tank.full_reload_seconds();
                // A held click must not survive the switch: the reload it anticipated is gone.
                tank.fire_buffered = false;
            }
            if command.fire {
                if let Some(shell) = try_fire_shell(tank, self.tick) {
                    self.shells.push(shell);
                } else if fire_click_buffers(tank) {
                    tank.fire_buffered = true;
                }
            }
        }

        // PHASE 4 — anything still overlapping is eased apart.
        self.separate_overlapping_hulls(live_cover.movement(), heightmap);

        apply_ramming_damage(
            &contact_pairs,
            &mut self.tanks,
            &mut self.damage_events,
            &mut event_stamp,
        );
        // ...and the wrecks settle onto the ground under them, whether they were killed in
        // mid-air or the ground moved after they died (see `wreck`).
        crate::wreck::settle_wrecks(&mut self.tanks, heightmap, &rubble, dt);
        // Drowning runs for EVERY living hull, commanded or not â€” a dead-engine tank in the
        // river keeps flooding.
        crate::drowning::step_drowning(
            &mut self.tanks,
            heightmap,
            self.water,
            dt,
            &mut self.damage_events,
            &mut event_stamp,
        );
        // Fire burns BEFORE this tick's shells land, so a hull the flames finish is already dead
        // when the next round arrives — the kill belongs to whoever lit it, not to whoever happened
        // to fire into the wreck a frame later.
        crate::fire::step_fire(&mut self.tanks, dt, &mut self.damage_events, &mut event_stamp);
        {
            let mut events = BattleEventOutput::new(
                &mut self.damage_events,
                &mut self.shell_impacts,
                &mut self.armor_breach_events,
                &mut event_stamp,
            );
            step_shells(
                &mut self.shells,
                &mut self.tanks,
                &mut events,
                context,
                heightmap,
                live_cover.sight(),
            );
        }
        // Shells absorbed by cover this tick bring it down: an HE round to rubble/clear, a kinetic
        // round chips it. The impact already carries where it died and what died there.
        for impact in &self.shell_impacts {
            if impact.surface != game_core::ImpactSurface::Cover {
                continue;
            }
            if let Some(index) = crate::cover_damage::cover_index_at(
                impact.position.to_array(),
                cover,
                &self.cover_states,
            ) {
                crate::cover_damage::damage_cover(
                    &mut self.cover_states,
                    cover,
                    index,
                    cover_damage_hp(impact.shell_type),
                );
                // And it leaves a WOUND where it died (protocol v32): a kinetic inset or an
                // HE bite on that face, replicated so every client dresses the same wall.
                crate::cover_damage::record_cover_scar(
                    &mut self.cover_scars,
                    index,
                    &cover[index],
                    impact,
                );
            }
        }
        // A high-explosive burst on OPEN GROUND excavates a real crater (protocol v31): the
        // ledger is replicated state, and once the battlefield owner folds it into the
        // heightmap overlay, physics, spotting, the predictor and the bots all stand in the
        // same hole. Kinetic rounds plough a furrow (presentation) but do not move earth.
        // Filling an old hole back in is only free when it is empty: the ledger evicts around
        // whoever is standing in one (see `crater_ledger`), so it needs the roster's footing.
        let standing_on: Vec<Vec3> = self.tanks.iter().map(|tank| tank.position).collect();
        for impact in &self.shell_impacts {
            if impact.surface == game_core::ImpactSurface::Terrain
                && impact.shell_type == game_core::ShellType::HighExplosive
            {
                crate::crater_ledger::record_high_explosive_burst(
                    &mut self.craters,
                    impact.position,
                    impact.caliber_mm,
                    &standing_on,
                );
            }
        }
        crate::spotting::refresh_spotted_masks(
            self.tick,
            &mut self.tanks,
            &mut self.spotting_memory,
            heightmap,
            live_cover.sight(),
        );
        self.last_battle_event_id = event_stamp.last_event_id();
        self.tick += 1;
    }

    /// PHASE 2: every hull that WOULD meet another this tick exchanges momentum with it.
    ///
    /// Solved against predicted positions — where each velocity is about to put its hull — so a
    /// collision is refused before it happens rather than repaired after. Wrecks take part as
    /// dead weight: they stop a charge without giving ground.
    fn exchange_contact_momentum(&mut self, dt: f32) -> Vec<physics::ContactPair> {
        if self.tanks.len() < 2 {
            return Vec::new();
        }
        let bodies = self.contact_bodies();
        let report = physics::resolve_contacts(&bodies, dt);
        for (index, impulse) in report.bodies.iter().enumerate() {
            if impulse.is_empty() || self.tanks[index].hit_points == 0 {
                continue;
            }
            self.tanks[index].velocity_mps += impulse.delta_velocity;
            self.tanks[index].hull_yaw_velocity_rad_s += impulse.delta_yaw_rate_rad_s;
        }
        report.pairs
    }

    /// PHASE 4: ease apart anything still overlapping — a pivot that swung an oriented footprint
    /// into a neighbour, or a spawn accident. With the contacts solved before the move this is a
    /// mop-up, not the mechanism, and it replaces `tank_resolve`'s "hold the previous position".
    fn separate_overlapping_hulls(
        &mut self,
        cover: &[StaticCoverObject],
        heightmap: Option<&HeightMap>,
    ) {
        if self.tanks.len() < 2 {
            return;
        }
        let bodies = self.contact_bodies();
        for (index, push) in physics::separate_overlaps(&bodies).into_iter().enumerate() {
            if push == Vec3::ZERO {
                continue;
            }
            let tank = &mut self.tanks[index];
            let footprint = TankFootprint::from_hitbox(tank.spec.hitbox);
            // Being pushed out of a neighbour may never push a hull into a building or past the
            // red line: both hard limits are re-applied to the corrected position.
            tank.position = physics::resolve_cover_collision(
                tank.position,
                tank.position + push,
                tank.yaw_rad,
                footprint,
                cover,
            );
            if let Some(heightmap) = heightmap {
                let mut velocity = tank.velocity_mps;
                physics::clamp_to_map_border(&mut tank.position, &mut velocity, heightmap);
                tank.velocity_mps = velocity;
            }
        }
    }

    fn contact_bodies(&self) -> Vec<ContactBody> {
        self.tanks
            .iter()
            .map(|tank| ContactBody {
                position: tank.position,
                velocity: tank.velocity_mps,
                yaw_rad: tank.yaw_rad,
                yaw_rate_rad_s: tank.hull_yaw_velocity_rad_s,
                footprint: TankFootprint::from_hitbox(tank.spec.hitbox),
                mass_kg: tank.spec.mass_kg,
                movable: tank.hit_points > 0,
            })
            .collect()
    }
}

#[cfg(test)]
mod event_truth_tests;
