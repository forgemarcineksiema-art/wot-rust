use game_core::{DamageEvent, ShellImpact, TankId, TankSpec, TeamId, TrackSide};
use glam::Vec3;
use physics::TankObstacle;
use serde::{Deserialize, Serialize};
use terrain::{HeightMap, StaticCoverObject, WaterBody};

use crate::aim_dispersion::recover_aim_dispersion;
use crate::combat::{CombatTickContext, fire_click_buffers, try_fire_shell};
use crate::landing::apply_landing_impact;
use crate::ramming::{apply_ramming_damage, capture_ramming_snapshots};
use crate::shell::ShellState;
use crate::shell_step::step_shells;
use crate::tank_drive::step_tank;
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
    /// Last-fresh-sight memory behind the spotted hold (see `spotting::SpottingMemory`).
    #[serde(default)]
    spotting_memory: crate::spotting::SpottingMemory,
    /// The battle map's standing water, installed once at setup (like the heightmap, it never
    /// changes mid-battle). Drives wading drag, drowning, and shell splashes. `serde(default)`
    /// keeps pre-water fixtures loading dry.
    #[serde(default)]
    water: Option<WaterBody>,
    /// Live structural state of the map's static cover (protocol v21), index-aligned with the
    /// cover slice the battlefield passes in. Rebuilt when the cover count changes (battle setup),
    /// then damaged by HE and crushed by ramming; every blocking consumer sees the resolved live
    /// cover derived from it. `serde(default)` keeps pre-v21 fixtures loading with whole cover.
    #[serde(default)]
    cover_states: Vec<crate::cover_damage::CoverState>,
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
/// The nick a hull takes for bulldozing through cover — small, but not free.
const COVER_CRUSH_SELF_HP: u32 = 8;
/// Reach ahead of the hull's own footprint at which a fast approach bowls a hedge over — the hull
/// flattens it just before contact, so the same tick's movement drives through instead of being
/// stopped by the (still-blocking) intact hedge and losing the speed the crush needs.
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
            spotting_memory: crate::spotting::SpottingMemory::default(),
            water: None,
            cover_states: Vec::new(),
        }
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

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn tanks(&self) -> &[TankState] {
        &self.tanks
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

    pub fn refresh_spotting(&mut self, heightmap: Option<&HeightMap>, cover: &[StaticCoverObject]) {
        if self.cover_states.len() != cover.len() {
            self.cover_states = crate::cover_damage::cover_states_for(cover);
        }
        let live_cover = crate::cover_damage::live_cover_for_blocking(cover, &self.cover_states);
        crate::spotting::apply_spotted_masks_with_hold(
            self.tick,
            &mut self.tanks,
            &mut self.spotting_memory,
            heightmap,
            &live_cover,
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
        let context = CombatTickContext { dt_seconds: dt, water: self.water };
        self.damage_events.clear();
        self.shell_impacts.clear();
        // Keep the cover states aligned with the map's cover (rebuilt only when the count changes,
        // i.e. at battle setup). A dry `apply_commands` passes no cover and clears the states.
        if self.cover_states.len() != cover.len() {
            self.cover_states = crate::cover_damage::cover_states_for(cover);
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
                let reach = tank.spec.hitbox.half_length_m + COVER_CRUSH_APPROACH_M;
                for (index, object) in cover.iter().enumerate() {
                    if crate::cover_damage::hull_overlaps_cover_xz(
                        tank.position.to_array(),
                        reach,
                        object,
                    ) && crate::cover_damage::crush_cover(states, object, index)
                    {
                        tank.hit_points = tank.hit_points.saturating_sub(COVER_CRUSH_SELF_HP);
                    }
                }
            }
        }
        // The cover the world collides against this tick: intact as-authored, rubble a low mound,
        // destroyed omitted. Every blocking consumer (movement, shell trace, spotting) uses this.
        let live_cover = crate::cover_damage::live_cover_for_blocking(cover, &self.cover_states);
        let ramming_before = capture_ramming_snapshots(&self.tanks);
        for tank in &mut self.tanks {
            tank.reload_remaining_s = (tank.reload_remaining_s - dt).max(0.0);
            recover_aim_dispersion(tank, dt);
            crate::repair::step_crew_repair(tank, dt);
        }
        // Release buffered fire clicks the tick their reload completes — one attempt, then the
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

        // Tank-vs-tank obstacles in lockstep with the sequential update: built once from the
        // start-of-tick hulls and refreshed per moved hull, so a later command collides against
        // exactly the positions the old per-command rebuild saw — bit-identical (replay-locked)
        // at a fraction of the rebuilds and without a fresh Vec per command.
        let mut all_obstacles: Vec<TankObstacle> = self
            .tanks
            .iter()
            .map(|tank| TankObstacle::from_hitbox(tank.position, tank.yaw_rad, tank.spec.hitbox))
            .collect();
        let mut obstacle_scratch: Vec<TankObstacle> =
            Vec::with_capacity(all_obstacles.len().saturating_sub(1));
        for (tank_id, command) in commands.iter().copied() {
            if let Some(index) = self.tanks.iter().position(|tank| tank.id == tank_id) {
                let command = command.clamped();
                obstacle_scratch.clear();
                obstacle_scratch.extend(
                    all_obstacles
                        .iter()
                        .enumerate()
                        .filter(|(other_index, _)| *other_index != index)
                        .map(|(_, obstacle)| *obstacle),
                );
                let tank = &mut self.tanks[index];
                if tank.hit_points == 0 {
                    tank.velocity_mps = Vec3::ZERO;
                    tank.hull_yaw_velocity_rad_s = 0.0;
                    continue;
                }
                let ground = step_tank(
                    tank,
                    command,
                    dt,
                    heightmap,
                    &live_cover,
                    &obstacle_scratch,
                    self.water,
                );
                all_obstacles[index] =
                    TankObstacle::from_hitbox(tank.position, tank.yaw_rad, tank.spec.hitbox);
                apply_landing_impact(tank, ground.landing_impact_mps, &mut self.damage_events);
                // Ammo switch before the fire check: the honest rule is simple — any real switch
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
        }

        apply_ramming_damage(&ramming_before, &mut self.tanks, &mut self.damage_events, dt);
        // Drowning runs for EVERY living hull, commanded or not — a dead-engine tank in the
        // river keeps flooding.
        crate::drowning::step_drowning(
            &mut self.tanks,
            heightmap,
            self.water,
            dt,
            &mut self.damage_events,
        );
        step_shells(
            &mut self.shells,
            &mut self.tanks,
            &mut self.damage_events,
            &mut self.shell_impacts,
            context,
            heightmap,
            &live_cover,
        );
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
            }
        }
        crate::spotting::refresh_spotted_masks(
            self.tick,
            &mut self.tanks,
            &mut self.spotting_memory,
            heightmap,
            &live_cover,
        );
        self.tick += 1;
    }
}
