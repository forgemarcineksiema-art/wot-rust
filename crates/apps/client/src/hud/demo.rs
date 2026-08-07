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
    let mode = if sniper { ReticleMode::Sniper } else { ReticleMode::ThirdPerson };
    let mut reticle = HudReticle {
        aim_clip: [0.0, 0.0],
        impact_clip: Some([0.06, -0.07]),
        gun_clip: Some([0.035, 0.02]),
        aim_radius_clip: 0.06,
        target_distance_m: Some(214.0),
        block_distance_m: None,
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
    let model = BattleHudModel {
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
        incoming_hits: vec![IncomingHit { bearing_rad: 2.1, age_s: 0.3, penetrated: false }],
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
        minimap: Some(demo_minimap()),
        battle_outcome: None,
        battle_clock_remaining_s: Some(474.0),
        kill_confirm_age_s: None,
        reload_ready_age_s: None,
        fire_denied_age_s: None,
        scope_fade: if sniper { 1.0 } else { 0.0 },
        pause_menu: None,
    };
    build_battle_hud(&model, aspect)
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
