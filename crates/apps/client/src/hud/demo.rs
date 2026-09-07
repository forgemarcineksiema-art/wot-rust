//! A staged battle HUD for the offscreen `battle_hud` example and QA: one representative frame
//! with every element populated (reticle with a pen hint, ammo panel, dealt/taken damage log, an
//! incoming-hit arc), in either reticle mode. Keeps the HUD's internal types private — the example
//! just asks for the finished vertices.

use game_core::{ArmorFacing, ModuleSlot, VehicleKind};
use renderer_api::HudVertex;

use super::ammo_panel::AmmoHudModel;
use super::damage_log::{DamageLogEntry, LogDirection};
use super::hit_direction::IncomingHit;
use super::minimap::{self, MinimapBox, MinimapModel};
use super::reticle::{PenetrationHint, ReticleMode, ReticleStatus};
use super::reticle_overlay::HudReticle;
use super::reticle_readouts::HitConfirm;
use super::{BattleHudModel, HudVitals, build_battle_hud};

/// Build a fully-populated battle HUD in third-person or sniper mode. Used by the offscreen
/// example to show both reticle regimes and every readout in one frame.
pub fn demo_battle_hud(sniper: bool, aspect: f32) -> Vec<HudVertex> {
    build_battle_hud(&demo_model(sniper), aspect)
}

/// The staged model itself: every element populated, in either reticle mode. The HUD golden
/// instrument's states (F8) start from this and switch things on and off.
pub(crate) fn demo_model(sniper: bool) -> BattleHudModel {
    let mode = if sniper { ReticleMode::Sniper } else { ReticleMode::ThirdPerson };
    let mut reticle = HudReticle {
        aim_clip: [0.0, 0.0],
        impact_clip: Some([0.06, -0.07]),
        gun_clip: Some([0.035, 0.02]),
        aim_radius_clip: 0.06,
        target_distance_m: Some(214.0),
        block_distance_m: None,
        arc_limit: None,
        status: ReticleStatus::Clear,
        penetration_hint: Some(PenetrationHint {
            penetrates: true,
            shell_pen_mm: 201.0,
            armor_mm: 120.0,
            facing: ArmorFacing::HullFront,
        }),
        reload_fraction: 0.4,
        hit_confirm: Some(HitConfirm { age_s: 0.1, penetrated: true, ricocheted: false }),
        mode,
        converged: true,
        marker_color: super::reticle_overlay::RETICLE_NEUTRAL,
    };
    // Resolved from the reticle's own hint, exactly as a live frame would (settled optics in
    // sniper), instead of a second copy of the same verdict.
    reticle.marker_color = super::reticle_overlay::marker_color(
        mode,
        reticle.penetration_hint,
        if sniper { 1.0 } else { 0.0 },
    );
    BattleHudModel {
        vitals: HudVitals {
            hit_points: 780,
            max_hit_points: 1000,
            reload_remaining_s: 3.1,
            reload_seconds: 5.2,
        },
        reticle: Some(reticle),
        fps: 60.0,
        frame_p95_ms: 16.7,
        speed_kmh: 24.0,
        cruise_level: 2,
        zoom_factor: sniper.then_some(6.9),
        damage_log: demo_hit_log(),
        hit_log_collapsed: false,
        incoming_hits: vec![IncomingHit {
            bearing_rad: 2.1,
            age_s: 0.3,
            penetrated: false,
            effective_armor_mm: 0.0,
            shell_penetration_mm: 0.0,
        }],
        ammo: Some(demo_ammo(0)),
        // The staged hull: a wounded gun, a thrown left track mid re-seat, a downed loader
        // mid-bandage and a scarred driver — the panel's states at once.
        damage: Some(demo_damage_panel()),
        minimap: Some(demo_minimap()),
        battle_outcome: None,
        battle_clock_remaining_s: Some(474.0),
        top_bar: Some(super::top_bar::TopBarModel {
            frags: [2, 1],
            team_hit_points: [5_120, 4_380],
            team_hit_points_max: [7_450, 7_210],
        }),
        team_lists: Some(demo_team_lists()),
        markers: Some(demo_markers()),
        sixth_sense_lit: false,
        // Moving at 24 km/h against the benchmark and the fleet's heaviest (a Tiger II today,
        // chosen by mass, never by name — vehicles are data): the full 440 m.
        budget: super::budget::BudgetModel::from_battle(
            &super::budget::staged_enemies([VehicleKind::BENCHMARK, heaviest_playable()]),
            game_core::TeamId(1),
            24.0 / 3.6,
            None,
            60.0,
        ),
        kill_confirm_age_s: None,
        reload_ready_age_s: None,
        fire_denied_age_s: None,
        scope_fade: if sniper { 1.0 } else { 0.0 },
        command_wheel: None,
        pings: None,
        team_word: None,
        kill_feed: None,
        net: Some(super::net_readout::NetReadoutModel {
            local: false,
            rtt_ms: Some(48),
            snapshot_age_ms: 32,
        }),
        dead: None,
        palette: ui_kit::theme::Palette::Standard,
        layout: crate::hud::layout::HudLayout::default(),
        editor: None,
        shell: None,
    }
}

/// The heaviest vehicle on the roster, by its spec's mass — the demo's stand-in for "a heavy".
fn heaviest_playable() -> VehicleKind {
    VehicleKind::PLAYABLE
        .into_iter()
        .filter(|kind| *kind != VehicleKind::BENCHMARK)
        .max_by(|a, b| a.spec_ref().mass_kg.total_cmp(&b.spec_ref().mass_kg))
        .unwrap_or(VehicleKind::BENCHMARK)
}

fn row(
    vehicle: game_core::VehicleKind,
    seat: char,
    human: bool,
    hp: Option<(u32, u32)>,
    alive: bool,
    is_player: bool,
    spotted: bool,
) -> super::team_list::TeamRow {
    super::team_list::TeamRow { vehicle, seat, human, hp, alive, is_player, spotted }
}

/// The staged ears: a full 7v7 a few minutes in — one ally down, two enemies down, the rest
/// of the enemy unseen but two, the player in seat A.
pub(crate) fn demo_team_lists() -> super::team_list::TeamListsModel {
    use game_core::VehicleKind as V;
    super::team_list::TeamListsModel {
        allies: vec![
            row(V::T54_1951, 'A', true, Some((780, 1_000)), true, true, true),
            row(V::IS3, 'B', false, Some((1_320, 1_500)), true, false, true),
            row(V::Centurion, 'C', false, Some((410, 1_100)), true, false, true),
            row(V::T34_85, 'D', false, Some((0, 700)), false, false, true),
            row(V::TigerI, 'E', false, Some((900, 1_200)), true, false, true),
            row(V::PantherII, 'F', false, Some((1_090, 1_200)), true, false, true),
            row(V::Jagdtiger, 'G', false, Some((1_620, 1_800)), true, false, true),
        ],
        enemies: vec![
            row(V::TigerII, 'A', false, Some((1_000, 1_500)), true, false, true),
            row(V::T54_1951, 'B', false, None, true, false, false),
            row(V::IS3, 'C', false, None, false, false, false),
            row(V::PantherII, 'D', false, Some((600, 1_200)), true, false, true),
            row(V::Jagdtiger, 'E', false, None, true, false, false),
            row(V::Centurion, 'F', false, None, false, false, false),
            row(V::T34_85, 'G', false, None, true, false, false),
        ],
    }
}

/// The busiest ears (`HudState::TeamListsMixed`): more dead, more withheld, one enemy wreck seen.
pub(crate) fn mixed_team_lists() -> super::team_list::TeamListsModel {
    use game_core::VehicleKind as V;
    super::team_list::TeamListsModel {
        allies: vec![
            row(V::T54_1951, 'A', true, Some((210, 1_000)), true, true, true),
            row(V::IS3, 'B', true, Some((0, 1_500)), false, false, true),
            row(V::Centurion, 'C', false, Some((1_100, 1_100)), true, false, true),
            row(V::T34_85, 'D', false, Some((0, 700)), false, false, true),
            row(V::TigerI, 'E', false, Some((0, 1_200)), false, false, true),
            row(V::PantherII, 'F', false, Some((830, 1_200)), true, false, true),
            row(V::Jagdtiger, 'G', false, Some((0, 1_800)), false, false, true),
        ],
        enemies: vec![
            row(V::TigerII, 'A', true, Some((0, 1_500)), false, false, true),
            row(V::T54_1951, 'B', false, None, true, false, false),
            row(V::IS3, 'C', false, None, false, false, false),
            row(V::PantherII, 'D', false, Some((1_200, 1_200)), true, false, true),
            row(V::Jagdtiger, 'E', false, None, false, false, false),
            row(V::Centurion, 'F', false, None, true, false, false),
            row(V::T34_85, 'G', false, None, true, false, false),
        ],
    }
}

/// The staged hit log (H8): a dealt penetration on a turret front with a gunner knock, a taken
/// track hit, and a taken ricochet at range — the three families the eye must tell apart.
pub(crate) fn demo_hit_log() -> Vec<DamageLogEntry> {
    let others: Vec<VehicleKind> = VehicleKind::PLAYABLE
        .iter()
        .copied()
        .filter(|kind| *kind != VehicleKind::BENCHMARK)
        .collect();
    let base = DamageLogEntry {
        direction: LogDirection::Dealt,
        damage_hp: 0,
        module: None,
        track: None,
        other_vehicle: others.first().copied(),
        crew_hits_mask: 0,
        round: None,
        age_s: 0.5,
        cause: game_core::DamageCause::Shell,
        penetrated: false,
        ricocheted: false,
        shattered: false,
        shell_penetration_mm: 148,
        effective_armor_mm: 162,
        impact_angle_degrees: 31,
        zone: game_core::ArmorZone::TurretFront,
        distance_m: Some(412),
    };
    vec![
        DamageLogEntry {
            damage_hp: 240,
            module: Some(ModuleSlot::Gun),
            crew_hits_mask: game_core::CrewRole::Gunner.mask_bit(),
            round: Some(game_core::RoundId::Pzgr39_42),
            penetrated: true,
            shell_penetration_mm: 194,
            effective_armor_mm: 162,
            ..base
        },
        DamageLogEntry {
            direction: LogDirection::Taken,
            track: Some((game_core::TrackSide::Right, true)),
            other_vehicle: others.get(1).copied(),
            round: Some(game_core::RoundId::Of471),
            age_s: 1.5,
            zone: game_core::ArmorZone::RightTrack,
            impact_angle_degrees: 12,
            shell_penetration_mm: 61,
            effective_armor_mm: 80,
            distance_m: Some(268),
            ..base
        },
        DamageLogEntry {
            direction: LogDirection::Taken,
            other_vehicle: others.get(2).copied(),
            round: Some(game_core::RoundId::Br412D),
            age_s: 3.2,
            ricocheted: true,
            zone: game_core::ArmorZone::UpperGlacis,
            impact_angle_degrees: 71,
            shell_penetration_mm: 171,
            effective_armor_mm: 310,
            distance_m: Some(510),
            ..base
        },
    ]
}

/// The staged markers (H10): the marked target under the reticle's headroom, a second hull known
/// but not spoken to, off to the right.
pub(crate) fn demo_markers() -> super::marker::MarkerModel {
    use super::marker::{HullMarker, MarkerModel};
    let others: Vec<VehicleKind> = VehicleKind::PLAYABLE
        .iter()
        .copied()
        .filter(|kind| *kind != VehicleKind::BENCHMARK)
        .collect();
    MarkerModel {
        hulls: vec![
            HullMarker {
                id: game_core::TankId(9),
                rect_px: ui_kit::rect::Rect::new(900.0, 470.0, 120.0, 64.0),
                vehicle: others.first().copied().unwrap_or(VehicleKind::BENCHMARK),
                seat: 'B',
                hit_points: 640,
                max_hit_points: 1_000,
                distance_m: 214,
                is_target: true,
            },
            HullMarker {
                id: game_core::TankId(11),
                rect_px: ui_kit::rect::Rect::new(1_290.0, 372.0, 70.0, 36.0),
                vehicle: others.get(1).copied().unwrap_or(VehicleKind::BENCHMARK),
                seat: 'E',
                hit_points: 1_180,
                max_hit_points: 1_500,
                distance_m: 468,
                is_target: false,
            },
        ],
    }
}

/// The staged ammunition: the benchmark's three rounds, the server's selection in slot 0 and
/// the crew's request in `requested` (a switch in flight when it differs).
pub(crate) fn demo_ammo(requested: u8) -> AmmoHudModel {
    AmmoHudModel::new(
        &game_core::VehicleKind::BENCHMARK.spec_ref().gun.ammo_options(),
        [22, 9, 6],
        0,
        requested,
    )
}

/// The staged hull's snapshot, with the drama switched on by the caller.
fn demo_hull() -> net::TankSnapshot {
    let vehicle = game_core::VehicleKind::BENCHMARK;
    let spec = vehicle.spec_ref();
    net::TankSnapshot {
        tank_id: game_core::TankId(1),
        team: game_core::TeamId(1),
        vehicle,
        position: [0.0; 3],
        yaw_rad: 0.0,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 780,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: spec.gun.dispersion_mrad,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
        rack_fire_remaining_s: None,
        crew_unconscious_mask: 0,
        crew_weakened_mask: 0,
        crew_down_remaining_s: Default::default(),
        hull_pitch_velocity_rad_s: 0.0,
        hull_roll_velocity_rad_s: 0.0,
        hull_dive_pitch_rad: 0.0,
        hull_dive_pitch_velocity_rad_s: 0.0,
    }
}

fn thrown_left() -> game_core::TrackDamageMask {
    let mut mask = game_core::TrackDamageMask::healthy();
    mask.damage(game_core::TrackSide::Left);
    mask
}

fn left_thrown_callout() -> super::track_feedback::CalloutView {
    super::track_feedback::CalloutView { broke: true, side: game_core::TrackSide::Left, age_s: 0.4 }
}

/// The staged damage panel: a wounded gun, a thrown left track mid re-seat, a downed loader
/// mid-bandage and a scarred driver.
pub(crate) fn demo_damage_panel() -> super::damage_panel::DamagePanelModel {
    let mut tank = demo_hull();
    tank.module_hit_points[game_core::ModuleSlot::Gun.wire_index()] = 60;
    tank.track_damage_mask = thrown_left().bits();
    tank.track_hp[0] = 0;
    tank.crew_unconscious_mask = game_core::CrewRole::Loader.mask_bit();
    tank.crew_weakened_mask = game_core::CrewRole::Driver.mask_bit();
    tank.crew_down_remaining_s[game_core::CrewRole::Loader.wire_index()] = Some(9.0);
    let clocks = net::RepairClocks {
        tank_id: tank.tank_id,
        module_s: [0.0; game_core::MODULE_SLOT_COUNT],
        track_s: [3.5, 0.0],
    };
    super::damage_panel::DamagePanelModel::from_snapshot(
        &tank,
        game_core::VehicleKind::BENCHMARK.spec_ref(),
        Some(&clocks),
        0.0,
        Some(left_thrown_callout()),
    )
}

/// The quiet hull: every module whole, every station up, nothing burning.
pub(crate) fn quiet_damage_panel() -> super::damage_panel::DamagePanelModel {
    super::damage_panel::DamagePanelModel::from_snapshot(
        &demo_hull(),
        game_core::VehicleKind::BENCHMARK.spec_ref(),
        None,
        0.0,
        None,
    )
}

/// The wounded hull (`HudState::ModuleDestroyed`): the engine knocked out three seconds into
/// its patch, the left track thrown mid re-seat, its callout still pulsing.
pub(crate) fn wounded_damage_panel() -> super::damage_panel::DamagePanelModel {
    let mut tank = demo_hull();
    tank.module_hit_points[game_core::ModuleSlot::Engine.wire_index()] = 0;
    tank.track_damage_mask = thrown_left().bits();
    tank.track_hp[0] = 0;
    let mut clocks = net::RepairClocks {
        tank_id: tank.tank_id,
        module_s: [0.0; game_core::MODULE_SLOT_COUNT],
        track_s: [3.5, 0.0],
    };
    clocks.module_s[game_core::ModuleSlot::Engine.wire_index()] = 3.0;
    super::damage_panel::DamagePanelModel::from_snapshot(
        &tank,
        game_core::VehicleKind::BENCHMARK.spec_ref(),
        Some(&clocks),
        0.0,
        Some(left_thrown_callout()),
    )
}

/// The burning hull (`HudState::OnFire`): the engine alight, the rack seven seconds from
/// cooking off, the radio dead.
pub(crate) fn burning_damage_panel() -> super::damage_panel::DamagePanelModel {
    let mut tank = demo_hull();
    tank.engine_fire = true;
    tank.rack_fire_remaining_s = Some(7.0);
    tank.module_hit_points[game_core::ModuleSlot::Radio.wire_index()] = 0;
    super::damage_panel::DamagePanelModel::from_snapshot(
        &tank,
        game_core::VehicleKind::BENCHMARK.spec_ref(),
        None,
        0.0,
        None,
    )
}

/// A synthetic minimap for the staged frame: a diagonal ridge, one cover block, the player with
/// its view wedge, an ally, and one spotted enemy.
fn demo_minimap() -> MinimapModel {
    let res = minimap::RELIEF_RES;
    let relief = (0..res * res)
        .map(|i| {
            let (x, z) = ((i % res) as f32 / res as f32, (i / res) as f32 / res as f32);
            (0.5 + 0.5 * ((x + z - 1.0) * std::f32::consts::PI).sin()).clamp(0.0, 1.0)
        })
        .collect();
    MinimapModel {
        size: minimap::MinimapSize::Standard,
        extent_m: [1000.0, 1000.0],
        relief,
        water: vec![false; res * res],
        roads: vec![vec![[0.0, 500.0], [1000.0, 500.0]], vec![[500.0, 170.0], [250.0, 500.0]]],
        cover: vec![MinimapBox { center_xz: [520.0, 470.0], half_xz: [40.0, 14.0] }],
        player_xz: [420.0, 300.0],
        player_heading_rad: 0.5,
        player_turret_yaw_rad: 1.3,
        view_yaw_rad: 0.5,
        view_half_fov_rad: 0.45,
        view_range_m: 440.0,
        seen_from_m: Some(360.0),
        allies: vec![
            minimap::Blip { xz: [470.0, 360.0], class: game_core::VehicleClass::Heavy, seat: 'B' },
            minimap::Blip { xz: [330.0, 250.0], class: game_core::VehicleClass::Medium, seat: 'C' },
        ],
        enemies: vec![minimap::Blip {
            xz: [640.0, 660.0],
            class: game_core::VehicleClass::Heavy,
            seat: 'A',
        }],
        ghosts: vec![minimap::Ghost {
            xz: [760.0, 540.0],
            class: game_core::VehicleClass::TankDestroyer,
            age_s: 3.0,
        }],
        pings: vec![minimap::Ping { xz: [580.0, 720.0], age_s: 1.2 }],
    }
}

/// The wheel open with AFFIRMATIVE under the mouse and one word already spent (H16).
pub(crate) fn demo_wheel() -> super::command_wheel::CommandWheelModel {
    super::command_wheel::CommandWheelModel {
        open: true,
        selected: Some(3),
        remaining: 4,
        wait_s: None,
        knock_age_s: None,
    }
}

/// A teammate's ping a second old, on the ridge to the right (H16), in reference pixels.
pub(crate) fn demo_pings() -> super::ping_marker::PingModel {
    super::ping_marker::PingModel {
        marks: vec![super::ping_marker::PingMark {
            screen_px: [1430.0, 560.0],
            seat: 'B',
            distance_m: 340,
            age_s: 1.0,
        }],
    }
}

/// The team's newest word: C says ATTACK the Tiger II in seat A (H16).
pub(crate) fn demo_team_word() -> super::ping_marker::TeamWord {
    super::ping_marker::TeamWord {
        seat: 'C',
        command: net::TeamCommand::Attack,
        target: Some("Tiger II \u{b7} A".to_string()),
        age_s: 0.8,
    }
}

/// Three kills on the feed (H3): a kill between two enemies this crew never saw, an ally's
/// kill, and a drowning with no killer to name.
pub(crate) fn demo_kill_feed() -> super::kill_feed::KillFeedModel {
    use super::kill_feed::{FeedName, KillRow};
    let name = |text: &str, enemy: bool| FeedName { text: text.to_string(), enemy };
    super::kill_feed::KillFeedModel {
        rows: vec![
            KillRow {
                killer: None,
                victim: name("T-54 \u{b7} E", false),
                cause: game_core::DamageCause::Drowning,
                age_s: 0.6,
            },
            KillRow {
                killer: Some(name("T-54 \u{b7} B", false)),
                victim: name("Tiger II \u{b7} C", true),
                cause: game_core::DamageCause::Shell,
                age_s: 2.4,
            },
            KillRow {
                killer: Some(name("Jagdtiger \u{b7} D", true)),
                victim: name("Tiger I \u{b7} F", true),
                cause: game_core::DamageCause::Fire,
                age_s: 5.1,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn both_modes_emit_a_populated_hud_and_sniper_speaks_more() {
        let third = super::demo_battle_hud(false, 16.0 / 9.0);
        let sniper = super::demo_battle_hud(true, 16.0 / 9.0);
        assert!(!third.is_empty() && !sniper.is_empty());
        // Sniper adds the pen color, mm readout, impact X and zoom text over the neutral view.
        assert!(sniper.len() > third.len(), "sniper mode speaks penetration");
    }
}
