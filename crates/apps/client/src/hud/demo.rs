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
        zoom_factor: sniper.then_some(6.9),
        damage_log: vec![
            DamageLogEntry {
                direction: LogDirection::Dealt,
                damage_hp: 240,
                module: Some(ModuleSlot::Gun),
                track: None,
                other_vehicle: VehicleKind::PLAYABLE
                    .iter()
                    .copied()
                    .find(|kind| *kind != VehicleKind::BENCHMARK),
                // The staged dealt row also shows a gunner knock — the "G" callout — and the
                // named round (v47).
                crew_hits_mask: game_core::CrewRole::Gunner.mask_bit(),
                round: Some(game_core::RoundId::Pzgr39_42),
                age_s: 0.5,
            },
            DamageLogEntry {
                direction: LogDirection::Taken,
                damage_hp: 0,
                module: None,
                track: Some((game_core::TrackSide::Right, true)),
                other_vehicle: VehicleKind::PLAYABLE
                    .iter()
                    .copied()
                    .filter(|kind| *kind != VehicleKind::BENCHMARK)
                    .nth(1),
                crew_hits_mask: 0,
                round: Some(game_core::RoundId::Of471),
                age_s: 1.5,
            },
        ],
        rack_fire_remaining_s: None,
        track_feedback: crate::hud::track_callout::TrackFeedbackModel {
            callout: Some(crate::hud::track_callout::CalloutView {
                broke: true,
                side: game_core::TrackSide::Left,
                age_s: 0.4,
            }),
            reseat: [Some(0.35), None],
        },
        incoming_hits: vec![IncomingHit {
            bearing_rad: 2.1,
            age_s: 0.3,
            penetrated: false,
            effective_armor_mm: 0.0,
            shell_penetration_mm: 0.0,
        }],
        ammo: Some(AmmoHudModel::new(
            [
                game_core::ShellType::ArmorPiercing,
                game_core::ShellType::Apcr,
                game_core::ShellType::HighExplosive,
            ],
            [22, 9, 6],
            0,
        )),
        // A wounded gun (amber) and a thrown track (red running gear) so the staged frame shows
        // the module panel doing its job. Order: [Engine, Suspension, Turret, Gun, AmmoRack, Radio].
        modules: Some(crate::hud::module_panel::ModulePanelModel::new(
            [400, 300, 300, 60, 225, 60],
            [400, 300, 300, 150, 225, 60],
            game_core::ModuleCondition::Destroyed,
        )),
        // A downed loader mid-bandage and a scarred driver, so the staged frame shows the crew
        // row's three states at once.
        crew: Some(crate::hud::crew_panel::CrewPanelModel::new(
            game_core::CrewRole::Loader.mask_bit(),
            game_core::CrewRole::Driver.mask_bit(),
            {
                let mut down = [None; game_core::CREW_ROLE_COUNT];
                down[game_core::CrewRole::Loader.wire_index()] = Some(9.0);
                down
            },
        )),
        minimap: Some(demo_minimap()),
        battle_outcome: None,
        battle_clock_remaining_s: Some(474.0),
        top_bar: Some(super::top_bar::TopBarModel {
            frags: [2, 1],
            team_hit_points: [5_120, 4_380],
            team_hit_points_max: [7_450, 7_210],
        }),
        team_lists: Some(demo_team_lists()),
        kill_confirm_age_s: None,
        reload_ready_age_s: None,
        fire_denied_age_s: None,
        scope_fade: if sniper { 1.0 } else { 0.0 },
        pause_menu: None,
    }
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
        extent_m: [1000.0, 1000.0],
        relief,
        water: vec![false; res * res],
        roads: vec![vec![[0.0, 500.0], [1000.0, 500.0]], vec![[500.0, 170.0], [250.0, 500.0]]],
        cover: vec![MinimapBox { center_xz: [520.0, 470.0], half_xz: [40.0, 14.0] }],
        player_xz: [420.0, 300.0],
        player_heading_rad: 0.5,
        view_yaw_rad: 0.5,
        view_half_fov_rad: 0.45,
        allies: vec![[470.0, 360.0]],
        enemies: vec![[640.0, 660.0]],
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
