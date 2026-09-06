//! The dead crew (interface program H19, H20): with the arrows it rides its living allies —
//! the camera follows that hull, the HUD shows that hull's panel off the wire — and when the
//! battle's outcome lands, the banner hands off on Enter or by itself after three seconds.

use game_core::TankId;
use net::TankSnapshot;

use super::ClientApp;
use crate::hud::spectate::{DeadModel, SpectateStrip};

/// The banner hands off by itself after this long (H20).
pub(crate) const OUTCOME_HAND_OFF_S: f32 = 3.0;

impl ClientApp {
    /// The living allies, in roster order: the hulls a dead crew can ride.
    fn living_allies(&self) -> Vec<TankId> {
        let Some(snapshot) = self.render_state.latest_snapshot() else {
            return Vec::new();
        };
        let player_team = self.player_team();
        let alive = |id: TankId| {
            snapshot
                .tanks
                .iter()
                .any(|tank| tank.tank_id == id && tank.team == player_team && tank.hit_points > 0)
        };
        let roster = self.session.roster();
        let mut allies: Vec<TankId> = if roster.is_empty() {
            snapshot.tanks.iter().map(|tank| tank.tank_id).collect()
        } else {
            roster.iter().map(|entry| entry.tank_id).collect()
        };
        allies.retain(|id| *id != self.player_tank && alive(*id));
        allies
    }

    /// An arrow: the next (or previous) living ally; past either end, the own wreck again.
    pub(super) fn spectate_step(&mut self, step: i32) {
        let allies = self.living_allies();
        if allies.is_empty() {
            self.spectate = None;
            return;
        }
        let at = self.spectate.and_then(|id| allies.iter().position(|ally| *ally == id));
        let next = match at {
            None if step > 0 => Some(0),
            None => Some(allies.len() - 1),
            Some(index) => {
                let next = index as i32 + step;
                (0..allies.len() as i32).contains(&next).then_some(next as usize)
            }
        };
        self.spectate = next.map(|index| allies[index]);
    }

    /// Each frame: a living crew rides nobody, and a ridden hull that died is let go.
    pub(super) fn refresh_spectate(&mut self, player_dead: bool) {
        if !player_dead {
            self.spectate = None;
            return;
        }
        if let Some(id) = self.spectate
            && !self.living_allies().contains(&id)
        {
            self.spectate = None;
        }
    }

    /// The ridden hull, blended for the camera; `None` while riding the own wreck.
    pub(super) fn spectated_tank(&self) -> Option<TankSnapshot> {
        self.spectate.and_then(|id| self.render_state.interpolated_tank(id))
    }

    /// The strip and the panel of the ridden hull, off the wire's newest snapshot: its hit
    /// points, modules, tracks, fires and the team's repair clocks (W-4) — never its aim.
    pub(super) fn spectate_model(&self) -> Option<SpectateStrip> {
        let id = self.spectate?;
        let latest = self.render_state.latest_snapshot()?;
        let tank = latest.tanks.iter().find(|tank| tank.tank_id == id)?;
        let clocks = latest.repair_clocks.iter().find(|clocks| clocks.tank_id == id);
        let allies = self.living_allies();
        let index = allies.iter().position(|ally| *ally == id)?;
        let roster = self.session.roster();
        let seat = roster
            .iter()
            .find(|entry| entry.tank_id == id)
            .map_or(' ', net::RosterEntry::seat_letter);
        Some(SpectateStrip {
            name: format!("{} \u{b7} {}", tank.vehicle.short_name(), seat),
            index,
            count: allies.len(),
            panel: crate::hud::damage_panel::DamagePanelModel::from_snapshot(
                tank,
                &tank.vehicle.spec(),
                clocks,
                self.ticks_since_snapshot as f32 * super::prediction::TICK_DT,
                None,
            ),
        })
    }

    pub(super) fn dead_model(&self) -> DeadModel {
        DeadModel { spectating: self.spectate_model() }
    }

    /// The outcome's clock (H20): once the banner is up it hands off by itself after
    /// `OUTCOME_HAND_OFF_S`; a living battle keeps the clock at zero.
    pub(super) fn tick_outcome_hand_off(&mut self, dt: f32) {
        if self.battle_outcome.is_none() {
            self.outcome_age_s = 0.0;
            return;
        }
        self.outcome_age_s += dt;
        if self.outcome_age_s >= OUTCOME_HAND_OFF_S {
            self.hand_off_outcome();
        }
    }

    /// The way on after the banner: the garage today, the results screen when P1 lands.
    pub(super) fn hand_off_outcome(&mut self) {
        if self.battle_outcome.is_some() && self.garage.has_started() && !self.garage.is_open() {
            self.open_garage();
        }
    }
}
