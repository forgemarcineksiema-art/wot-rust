//! The command wheel and the team's word (interface program H16): Z opens the wheel, the
//! mouse picks, the release says; T's mark goes the same way. Every word passes the client's
//! MIRROR of the server's allowance first (W-5, five a minute), so a refusal is knocked at
//! once — `UiReject`, the word REFUSED under the wheel — while the server's own limiter stays
//! the law a modded client cannot talk past. The team's pings and words come back off the
//! relay; the roster says whose they are.

use crate::hud::command_wheel::{self, CommandWheelModel};
use crate::hud::ping_marker::{PING_TTL_S, PingMark, PingModel, TeamWord, WORD_TTL_S};

use super::ClientApp;

/// A teammate's ping, on the map and in the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TeamPing {
    pub xz: [f32; 2],
    pub seat: char,
    pub age_s: f32,
}

/// A ping's disc floats this high over the ground it marks.
const PING_HEIGHT_M: f32 = 2.0;

impl ClientApp {
    /// The newest server tick the client has heard: the clock the mirror runs on.
    pub(super) fn server_tick_now(&self) -> u64 {
        self.render_state.latest_snapshot().map_or(0, |snapshot| snapshot.server_tick)
    }

    /// Z pressed in a live battle: the wheel opens with the mouse at its hub.
    pub(super) fn open_command_wheel(&mut self) {
        if self.garage.has_started() && !self.shell_open() {
            self.command_wheel = Some([0.0, 0.0]);
        }
    }

    /// Z released: the sector under the mouse is said; the hub says nothing. ATTACK names the
    /// hull under the reticle when there is one; PING marks where the sight ray landed.
    pub(super) fn release_command_wheel(&mut self) {
        let Some(travel) = self.command_wheel.take() else {
            return;
        };
        let Some(index) = command_wheel::sector_of(travel) else {
            return;
        };
        let command = net::TeamCommand::ALL[index];
        let (target, map_position) = match command {
            net::TeamCommand::Ping => (None, self.aim_point_xz),
            net::TeamCommand::Attack => (self.hull_under_reticle, None),
            _ => (None, None),
        };
        if command == net::TeamCommand::Ping && map_position.is_none() {
            return;
        }
        self.say(command, target, map_position);
    }

    /// One word to the team: through the mirror of the server's allowance, then the wire. A
    /// refusal knocks and sends nothing.
    pub(super) fn say(
        &mut self,
        command: net::TeamCommand,
        target: Option<game_core::TankId>,
        map_position: Option<[f32; 2]>,
    ) -> bool {
        let now = self.server_tick_now();
        if !self.command_clock.admit(now, sim::DEFAULT_SERVER_TICK_HZ) {
            self.command_knock_age_s = Some(0.0);
            self.queue_audio(audio::AudioEvent::UiReject);
            return false;
        }
        self.session.send_team_command(command, target, map_position)
    }

    /// The wheel for the HUD: the ring while Z is held, the knock while it is fresh.
    pub(super) fn command_wheel_model(&self) -> Option<CommandWheelModel> {
        let open = self.command_wheel.is_some();
        if !open && self.command_knock_age_s.is_none() {
            return None;
        }
        let now = self.server_tick_now();
        let hz = sim::DEFAULT_SERVER_TICK_HZ;
        let remaining = self.command_clock.remaining(now, hz);
        Some(CommandWheelModel {
            open,
            selected: self.command_wheel.and_then(command_wheel::sector_of),
            remaining,
            wait_s: (remaining == 0)
                .then(|| self.command_clock.wait_ticks(now, hz) as f32 / hz as f32),
            knock_age_s: self.command_knock_age_s,
        })
    }

    /// The team's pings still ringing: from a hull on our team (the roster's word, checked
    /// here as well as on the server), placed, and younger than the TTL.
    pub(crate) fn team_pings(&self) -> Vec<TeamPing> {
        self.team_pings_at(self.server_tick_now())
    }

    /// The same, aged against a given server tick.
    pub(crate) fn team_pings_at(&self, now: u64) -> Vec<TeamPing> {
        let roster = self.session.roster();
        let player_team = self.player_team();
        let tick_s = 1.0 / sim::DEFAULT_SERVER_TICK_HZ as f32;
        self.intel
            .team_commands()
            .filter(|relay| relay.command == net::TeamCommand::Ping)
            .filter_map(|relay| {
                let entry = roster.iter().find(|entry| entry.tank_id == relay.from)?;
                if entry.team != player_team {
                    return None;
                }
                let xz = relay.map_position?;
                let age_s = now.saturating_sub(relay.server_tick) as f32 * tick_s;
                (age_s <= PING_TTL_S).then(|| TeamPing { xz, seat: entry.seat_letter(), age_s })
            })
            .collect()
    }

    /// The pings projected into the world: a disc over the pinged ground, when on screen.
    pub(super) fn ping_model(
        &self,
        view_projection: [[f32; 4]; 4],
        viewport_px: [f32; 2],
    ) -> PingModel {
        let player =
            self.render_state.interpolated_tank(self.player_tank).map(|tank| tank.position);
        let marks = self
            .team_pings()
            .into_iter()
            .filter_map(|ping| {
                let ground =
                    self.battlefield.heightmap.sample_height(ping.xz[0], ping.xz[1]).unwrap_or(0.0);
                let world = glam::Vec3::new(ping.xz[0], ground + PING_HEIGHT_M, ping.xz[1]);
                let clip = crate::hud::reticle::world_to_clip_xy(world, view_projection)?;
                let distance_m =
                    player.map_or(0.0, |p| (p[0] - world.x).hypot(p[2] - world.z)).round() as u32;
                Some(PingMark {
                    screen_px: [
                        (clip[0] + 1.0) * 0.5 * viewport_px[0],
                        (1.0 - clip[1]) * 0.5 * viewport_px[1],
                    ],
                    seat: ping.seat,
                    distance_m,
                    age_s: ping.age_s,
                })
            })
            .collect();
        PingModel { marks }
    }

    /// The team's newest word still fresh: who, what, about whom.
    pub(super) fn team_word(&self) -> Option<TeamWord> {
        let roster = self.session.roster();
        let player_team = self.player_team();
        let now = self.server_tick_now();
        let tick_s = 1.0 / sim::DEFAULT_SERVER_TICK_HZ as f32;
        self.intel
            .team_commands()
            .rev()
            .filter(|relay| relay.command != net::TeamCommand::Ping)
            .find_map(|relay| {
                let entry = roster.iter().find(|entry| entry.tank_id == relay.from)?;
                if entry.team != player_team {
                    return None;
                }
                let age_s = now.saturating_sub(relay.server_tick) as f32 * tick_s;
                if age_s >= WORD_TTL_S {
                    return None;
                }
                let target = relay
                    .target
                    .and_then(|id| roster.iter().find(|entry| entry.tank_id == id))
                    .map(|entry| {
                        format!("{} \u{b7} {}", entry.vehicle.short_name(), entry.seat_letter())
                    });
                Some(TeamWord { seat: entry.seat_letter(), command: relay.command, target, age_s })
            })
    }
}
