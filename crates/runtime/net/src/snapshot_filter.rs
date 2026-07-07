use game_core::{TankId, TeamId};

use crate::{Snapshot, TankSnapshot};

impl Snapshot {
    pub fn filtered_for_viewer(&self, viewer_tank: TankId) -> Self {
        let Some(viewer_team) = self.viewer_team(viewer_tank) else {
            return Snapshot { server_tick: self.server_tick, ..Snapshot::default() };
        };
        let visible_tanks = self.visible_tanks_for(viewer_tank, viewer_team);
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
            // Wrecks are always visible (the hit_points == 0 rule above), so every detached-turret
            // wreck the viewer can see rides through.
            detached_turrets: self
                .detached_turrets
                .iter()
                .copied()
                .filter(|id| visible_ids.contains(id))
                .collect(),
        }
    }

    fn viewer_team(&self, viewer_tank: TankId) -> Option<TeamId> {
        self.tanks.iter().find(|tank| tank.tank_id == viewer_tank).map(|tank| tank.team)
    }

    fn visible_tanks_for(&self, viewer_tank: TankId, viewer_team: TeamId) -> Vec<TankSnapshot> {
        let viewer_bit = viewer_team.spotting_bit();
        self.tanks
            .iter()
            .filter(|tank| {
                tank.tank_id == viewer_tank
                    || tank.team == viewer_team
                    || tank.hit_points == 0
                    || tank.spotted_by_teams_mask & viewer_bit != 0
            })
            .cloned()
            .collect()
    }
}
