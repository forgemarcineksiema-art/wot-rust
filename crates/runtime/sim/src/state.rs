use game_core::{DamageEvent, ShellImpact, TankId, TankSpec, TeamId, TrackSide};
use glam::Vec3;
use physics::TankObstacle;
use serde::{Deserialize, Serialize};
use terrain::{HeightMap, StaticCoverObject, WaterBody};

use crate::aim_dispersion::recover_aim_dispersion;
use crate::combat::{CombatTickContext, try_fire_shell};
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
}

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
        }
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
        crate::spotting::apply_spotted_masks_with_hold(
            self.tick,
            &mut self.tanks,
            &mut self.spotting_memory,
            heightmap,
            cover,
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

    pub fn damage_track(&mut self, id: TankId, side: TrackSide) {
        if let Some(tank) = self.tank_mut(id) {
            tank.tracks.damage(side);
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
        let ramming_before = capture_ramming_snapshots(&self.tanks);
        for tank in &mut self.tanks {
            tank.reload_remaining_s = (tank.reload_remaining_s - dt).max(0.0);
            recover_aim_dispersion(tank, dt);
        }

        for (tank_id, command) in commands.iter().copied() {
            if let Some(index) = self.tanks.iter().position(|tank| tank.id == tank_id) {
                let command = command.clamped();
                let tank_obstacles = self
                    .tanks
                    .iter()
                    .enumerate()
                    .filter(|(other_index, _)| *other_index != index)
                    .map(|(_, tank)| {
                        TankObstacle::from_hitbox(tank.position, tank.yaw_rad, tank.spec.hitbox)
                    })
                    .collect::<Vec<_>>();
                let tank = &mut self.tanks[index];
                if tank.hit_points == 0 {
                    tank.velocity_mps = Vec3::ZERO;
                    tank.hull_yaw_velocity_rad_s = 0.0;
                    continue;
                }
                let ground =
                    step_tank(tank, command, dt, heightmap, cover, &tank_obstacles, self.water);
                apply_landing_impact(tank, ground.landing_impact_mps, &mut self.damage_events);
                // Ammo switch before the fire check: the honest rule is simple — any real switch
                // restarts the full reload (the loader swaps the round out of the breech).
                if let Some(slot) = command.select_ammo
                    && (slot as usize) < game_core::MAX_AMMO_SLOTS
                    && slot != tank.selected_ammo
                {
                    tank.selected_ammo = slot;
                    tank.reload_remaining_s = tank.spec.gun.reload_seconds;
                }
                if command.fire
                    && let Some(shell) = try_fire_shell(tank, self.tick)
                {
                    self.shells.push(shell);
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
            cover,
        );
        crate::spotting::refresh_spotted_masks(
            self.tick,
            &mut self.tanks,
            &mut self.spotting_memory,
            heightmap,
            cover,
        );
        self.tick += 1;
    }
}
