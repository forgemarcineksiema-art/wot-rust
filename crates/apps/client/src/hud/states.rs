//! The battle HUD's named states (interface program F8): what the HUD golden instrument
//! renders, byte-exact, over one frozen battlefield frame — one model per state, staged from
//! the same base the `battle_hud` probe stages, so a review frame is the HUD the game draws.
//!
//! A state is appended, never renamed: its name is the golden's file name and the census's key.

use super::demo::demo_model;
use super::{BattleHudModel, BattleHudOutcome};

/// The states under golden. Append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HudState {
    /// Third person, the gun laid, nothing happening: the quiet frame the eye lives in.
    ThirdPersonIdle,
    /// The scope on a hull at 300 m with the verdict and the millimetres.
    SniperAimingHull,
    /// Mid-reload: the arc and the countdown.
    Reloading,
    /// A hit just landed on us: the direction arc and its verdict.
    HitTaken,
    /// A module knocked out: the panel speaks.
    ModuleDestroyed,
    /// The rack is cooking: the fuze counts down.
    OnFire,
    /// Spotted: the lamp (H13) once it exists; today the frame where it would be.
    Spotted,
    /// The kill confirmation beat.
    KillConfirmed,
    /// The battle's outcome banner.
    OutcomeBanner,
    /// The escape modal over the battle.
    PauseMenu,
    /// The HUD editor open (H21) once it exists; today the frame where it would be.
    HudEditorOpen,
    /// The ears at their busiest (H2): dead rows, withheld enemies, a spotted one, bots and
    /// humans — and the clock in its last minute.
    TeamListsMixed,
    /// A round switch in flight (H6): the band over the panel, the reload restarted.
    AmmoSwitching,
    /// The minimap at its large size (H15): the grid's letters, the seats, the circles.
    MinimapLarge,
    /// The command wheel open (H16), a word chosen, a teammate's ping in the world and the
    /// echo strip.
    CommandWheelOpen,
    /// The kill feed at three rows (H3) and the connection readout with a slow wire (H18).
    KillFeed,
    /// A dead crew riding an ally (H19): the intel sat back, the strip, the ally's panel.
    DeadSpectating,
    /// The deuteranopia palette (H22): every semantic pair apart by luminance and the blue axis.
    PaletteDeuteranopia,
}

impl HudState {
    pub const ALL: [HudState; 18] = [
        HudState::ThirdPersonIdle,
        HudState::SniperAimingHull,
        HudState::Reloading,
        HudState::HitTaken,
        HudState::ModuleDestroyed,
        HudState::OnFire,
        HudState::Spotted,
        HudState::KillConfirmed,
        HudState::OutcomeBanner,
        HudState::PauseMenu,
        HudState::HudEditorOpen,
        HudState::TeamListsMixed,
        HudState::AmmoSwitching,
        HudState::MinimapLarge,
        HudState::CommandWheelOpen,
        HudState::KillFeed,
        HudState::DeadSpectating,
        HudState::PaletteDeuteranopia,
    ];

    /// The golden's name stem.
    pub fn name(self) -> &'static str {
        match self {
            HudState::ThirdPersonIdle => "third_person_idle",
            HudState::SniperAimingHull => "sniper_aiming_hull",
            HudState::Reloading => "reloading",
            HudState::HitTaken => "hit_taken",
            HudState::ModuleDestroyed => "module_destroyed",
            HudState::OnFire => "on_fire",
            HudState::Spotted => "spotted",
            HudState::KillConfirmed => "kill_confirmed",
            HudState::OutcomeBanner => "outcome_banner",
            HudState::PauseMenu => "pause_menu",
            HudState::HudEditorOpen => "hud_editor_open",
            HudState::TeamListsMixed => "team_lists_mixed",
            HudState::AmmoSwitching => "ammo_switching",
            HudState::MinimapLarge => "minimap_large",
            HudState::CommandWheelOpen => "command_wheel_open",
            HudState::KillFeed => "kill_feed",
            HudState::DeadSpectating => "dead_spectating",
            HudState::PaletteDeuteranopia => "palette_deuteranopia",
        }
    }

    /// Whether the frame is seen through the scope.
    pub fn sniper(self) -> bool {
        matches!(self, HudState::SniperAimingHull)
    }

    /// The model this state draws: the staged base, with what the state is about switched on
    /// and everything the state is not about switched off, so each golden shows one thing.
    pub(crate) fn model(self) -> BattleHudModel {
        let mut model = demo_model(self.sniper());
        // The quiet base: no transient beats unless the state asks for them.
        model.vitals.reload_remaining_s = 0.0;
        model.reload_ready_age_s = None;
        model.fire_denied_age_s = None;
        model.kill_confirm_age_s = None;
        model.battle_outcome = None;
        model.pause_menu = None;
        model.incoming_hits.clear();
        model.damage = Some(super::demo::quiet_damage_panel());
        match self {
            HudState::ThirdPersonIdle | HudState::SniperAimingHull => {}
            HudState::Reloading => model.vitals.reload_remaining_s = 3.1,
            HudState::HitTaken => {
                model.incoming_hits.push(super::hit_direction::IncomingHit {
                    bearing_rad: 2.1,
                    age_s: 0.3,
                    penetrated: false,
                    effective_armor_mm: 162.0,
                    shell_penetration_mm: 148.0,
                });
            }
            HudState::ModuleDestroyed => {
                model.damage = Some(super::demo::wounded_damage_panel());
            }
            HudState::OnFire => model.damage = Some(super::demo::burning_damage_panel()),
            HudState::Spotted => model.sixth_sense_lit = true,
            HudState::KillConfirmed => model.kill_confirm_age_s = Some(0.4),
            HudState::OutcomeBanner => model.battle_outcome = Some(BattleHudOutcome::Victory),
            HudState::PauseMenu => {
                model.pause_menu = Some(super::pause_menu::PauseMenuModel { hovered: None });
            }
            HudState::HudEditorOpen => {}
            HudState::TeamListsMixed => {
                model.team_lists = Some(super::demo::mixed_team_lists());
                model.top_bar = Some(super::top_bar::TopBarModel {
                    frags: [4, 3],
                    team_hit_points: [2_140, 3_050],
                    team_hit_points_max: [7_450, 7_210],
                });
                model.battle_clock_remaining_s = Some(42.0);
            }
            HudState::AmmoSwitching => {
                model.ammo = Some(super::demo::demo_ammo(1));
                model.vitals.reload_remaining_s = 5.1;
            }
            HudState::PaletteDeuteranopia => {
                model.palette = ui_kit::theme::Palette::Deuteranopia;
                model.sixth_sense_lit = true;
                model.kill_feed = Some(super::demo::demo_kill_feed());
            }
            HudState::DeadSpectating => {
                model.kill_feed = Some(super::demo::demo_kill_feed());
                model.dead = Some(super::spectate::DeadModel {
                    spectating: Some(super::spectate::SpectateStrip {
                        name: "T-54 \u{b7} B".to_string(),
                        index: 1,
                        count: 3,
                        panel: super::demo::quiet_damage_panel(),
                    }),
                });
            }
            HudState::KillFeed => {
                model.kill_feed = Some(super::demo::demo_kill_feed());
                model.net = Some(super::net_readout::NetReadoutModel {
                    local: false,
                    rtt_ms: Some(184),
                    snapshot_age_ms: 61,
                });
            }
            HudState::CommandWheelOpen => {
                model.command_wheel = Some(super::demo::demo_wheel());
                model.pings = Some(super::demo::demo_pings());
                model.team_word = Some(super::demo::demo_team_word());
            }
            HudState::MinimapLarge => {
                if let Some(map) = &mut model.minimap {
                    map.size = super::minimap::MinimapSize::Large;
                }
            }
        }
        model
    }
}

/// The size classes a state is rendered in: the reference, and the largest the settings offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HudSizeClass {
    /// User scale 1.0 — one `u` is one pixel at 1080p.
    Standard,
    /// User scale 1.5 — what a 4K monitor or a player who wants it larger gets.
    Large,
}

impl HudSizeClass {
    pub const ALL: [HudSizeClass; 2] = [HudSizeClass::Standard, HudSizeClass::Large];

    pub fn user_scale(self) -> f32 {
        match self {
            HudSizeClass::Standard => 1.0,
            HudSizeClass::Large => 1.5,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            HudSizeClass::Standard => "",
            HudSizeClass::Large => "_large",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_has_a_name_and_a_model_that_draws() {
        let mut names = std::collections::HashSet::new();
        for state in HudState::ALL {
            assert!(names.insert(state.name()), "{state:?} shares a name");
            let list = super::super::build_battle_hud_list(
                &state.model(),
                &ui_kit::ui::Ui::for_aspect(16.0 / 9.0),
            );
            assert!(!list.is_empty(), "{state:?} draws nothing");
        }
        assert!(HudState::Reloading.model().vitals.reload_remaining_s > 0.0);
        assert!(HudState::OutcomeBanner.model().battle_outcome.is_some());
        assert!(HudState::PauseMenu.model().pause_menu.is_some());
        assert!(HudState::ThirdPersonIdle.model().pause_menu.is_none());
    }
}
