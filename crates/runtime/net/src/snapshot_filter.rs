use game_core::{TankId, TeamId};

use crate::{Snapshot, TankSnapshot};

impl Snapshot {
    /// v29, the honest per-viewer cut: what this crew may know. Team-shared intel flows only
    /// through a WORKING radio; the crew's own eyes always count. `observer_masks` comes from
    /// the sim (`compute_observer_masks`, bit = tank index in this snapshot's order) and
    /// `viewer_index` is the viewer's bit. Passing an empty slice reproduces the plain
    /// team-mask filter (the practice path).
    pub fn filtered_for_viewer_with_observers(
        &self,
        viewer_tank: TankId,
        observer_masks: &[sim::ObserverMask],
        viewer_index: usize,
    ) -> Self {
        let viewer_radio_ok =
            self.tanks.iter().find(|tank| tank.tank_id == viewer_tank).is_some_and(|tank| {
                tank.destroyed_modules_mask & game_core::ModuleSlot::Radio.destroyed_mask_bit() == 0
            });
        // The viewer's bit has to exist before it can be tested. `1 << viewer_index` on an index
        // past the mask's width is an overflow, and the third place this cap used to live
        // unnamed — the other two being the mask type and a bare `.take(16)` in the sim.
        let viewer_bit =
            (viewer_index < sim::MAX_OBSERVERS).then(|| (1 as sim::ObserverMask) << viewer_index);
        let mut filtered = self.filtered_for_viewer_gated(viewer_tank, |index, tank_mask| {
            let own_eyes = viewer_bit
                .is_some_and(|bit| observer_masks.get(index).is_some_and(|mask| mask & bit != 0));
            (viewer_radio_ok && tank_mask) || own_eyes
        });
        filtered.server_tick = self.server_tick;
        filtered
    }

    pub fn filtered_for_viewer(&self, viewer_tank: TankId) -> Self {
        self.filtered_for_viewer_gated(viewer_tank, |_, tank_mask| tank_mask)
    }

    fn filtered_for_viewer_gated(
        &self,
        viewer_tank: TankId,
        admits: impl Fn(usize, bool) -> bool,
    ) -> Self {
        let Some(viewer_team) = self.viewer_team(viewer_tank) else {
            return Snapshot { server_tick: self.server_tick, ..Snapshot::default() };
        };
        let mut visible_tanks = self.visible_tanks_for(viewer_tank, viewer_team, &admits);
        quantize_distant_enemy_hp(&mut visible_tanks, viewer_tank, viewer_team);
        let visible_ids = visible_tanks.iter().map(|tank| tank.tank_id).collect::<Vec<_>>();

        Snapshot {
            server_tick: self.server_tick,
            tanks: visible_tanks,
            // Shells and impacts are world events, not intel: a tracer in the air and dirt
            // thrown by a near-miss are visible to everyone standing there, whatever the
            // spotting state of the gun that fired. Filtering them by OWNER visibility made
            // fire from beyond spotting range fully invisible (no tracer, no impact, no
            // counter-battery read — only the damage), and a shell vanished mid-flight the
            // moment its owner's spotted hold expired. The tank itself stays hidden; the
            // shot it fired does not.
            shells: self.shells.clone(),
            damage_events: self
                .damage_events
                .iter()
                .copied()
                .filter(|event| {
                    event.source == viewer_tank
                        || event.target == viewer_tank
                        || (visible_ids.contains(&event.source)
                            && visible_ids.contains(&event.target))
                })
                .collect(),
            // Like shells (above), impacts are world events everyone standing there sees, not
            // owner-gated intel (see #95); they ride through unfiltered.
            shell_impacts: self.shell_impacts.clone(),
            // A muzzle flash is the loudest, brightest thing on a battlefield. It rides through
            // for the same reason the shell does: an unspotted gun firing from a treeline is a
            // thing you SEE, and hiding it would make counter-battery reading impossible while
            // the shell it fired flew past in plain view.
            shots_fired: self.shots_fired.clone(),
            // Wrecks are always visible (the hit_points == 0 rule above), so every detached-turret
            // wreck the viewer can see rides through.
            detached_turrets: self
                .detached_turrets
                .iter()
                .copied()
                .filter(|id| visible_ids.contains(id))
                .collect(),
            // Cover state is global world state, not intel — everyone sees the same rubble.
            cover_states: self.cover_states.clone(),
            // So are craters (v31): the ground itself is deformed for everyone alike.
            craters: self.craters.clone(),
            // And the wounds on the walls (v32) — world dressing, not intel.
            cover_scars: self.cover_scars.clone(),
        }
    }

    fn viewer_team(&self, viewer_tank: TankId) -> Option<TeamId> {
        self.tanks.iter().find(|tank| tank.tank_id == viewer_tank).map(|tank| tank.team)
    }

    fn visible_tanks_for(
        &self,
        viewer_tank: TankId,
        viewer_team: TeamId,
        admits: &impl Fn(usize, bool) -> bool,
    ) -> Vec<TankSnapshot> {
        let viewer_bit = viewer_team.spotting_bit();
        self.tanks
            .iter()
            .enumerate()
            .filter(|(index, tank)| {
                tank.tank_id == viewer_tank
                    || tank.team == viewer_team
                    || tank.hit_points == 0
                    || admits(*index, tank.spotted_by_teams_mask & viewer_bit != 0)
            })
            .map(|(_, tank)| tank.clone())
            .collect()
    }
}

/// Beyond this range an enemy's EXACT hit points are intel the crew could not have — the HUD
/// bar still moves, but only in coarse steps.
const EXACT_HP_RANGE_M: f32 = 300.0;
/// Quantization step as a fraction of the vehicle's full pool.
const DISTANT_HP_STEP: f32 = 0.20;

/// The old sin was "exact HP at any range" (backlog, 2026-06-10): a sniper across the map read
/// 743/1500 like a medic. Enemies beyond [`EXACT_HP_RANGE_M`] now report their pool rounded UP
/// to a 20% step — never lower than truth (a fake kill reading would be dishonest the other
/// way), never exact. Teammates and wrecks stay exact; so does anyone close enough to SEE the
/// damage.
fn quantize_distant_enemy_hp(tanks: &mut [TankSnapshot], viewer_tank: TankId, viewer_team: TeamId) {
    let Some(viewer_position) =
        tanks.iter().find(|tank| tank.tank_id == viewer_tank).map(|tank| tank.position)
    else {
        return;
    };
    for tank in tanks.iter_mut() {
        if tank.team == viewer_team || tank.hit_points == 0 {
            continue;
        }
        let dx = tank.position[0] - viewer_position[0];
        let dz = tank.position[2] - viewer_position[2];
        if (dx * dx + dz * dz).sqrt() <= EXACT_HP_RANGE_M {
            continue;
        }
        // `spec_ref`, not `spec`: this runs per distant enemy per VIEWER per snapshot, and it
        // reads one number.
        let full = tank.vehicle.spec_ref().hit_points.max(1);
        let step = ((full as f32) * DISTANT_HP_STEP).max(1.0);
        let quantized = ((tank.hit_points as f32 / step).ceil() * step).round() as u32;
        tank.hit_points = quantized.min(full);
    }
}
