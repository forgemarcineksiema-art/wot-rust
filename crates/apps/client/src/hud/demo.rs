//! A staged battle HUD for the offscreen `battle_hud` example and QA: one representative frame
//! with every element populated (reticle with a pen hint, ammo panel, dealt/taken damage log, an
//! incoming-hit arc), in either reticle mode. Keeps the HUD's internal types private — the example
//! just asks for the finished vertices.

use game_core::{ArmorFacing, ModuleSlot, VehicleKind};
use renderer_api::HudVertex;

use super::ammo_panel::AmmoHudModel;
use super::damage_log::{DamageLogEntry, LogDirection};
use super::hit_direction::IncomingHit;
use super::reticle::{PenetrationHint, ReticleMode, ReticleStatus};
use super::reticle_overlay::HudReticle;
use super::reticle_readouts::HitConfirm;
use super::{BattleHudModel, HudVitals, build_battle_hud};

/// Build a fully-populated battle HUD in third-person or sniper mode. Used by the offscreen
/// example to show both reticle regimes and every readout in one frame.
pub fn demo_battle_hud(sniper: bool, aspect: f32) -> Vec<HudVertex> {
    let mode = if sniper { ReticleMode::Sniper } else { ReticleMode::ThirdPerson };
    let reticle = HudReticle {
        aim_clip: [0.0, 0.0],
        impact_clip: Some([0.06, -0.07]),
        gun_clip: Some([0.035, 0.02]),
        aim_radius_clip: 0.06,
        target_distance_m: Some(214.0),
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
    };
    let model = BattleHudModel {
        vitals: HudVitals {
            hit_points: 780,
            max_hit_points: 1000,
            reload_remaining_s: 3.1,
            reload_seconds: 5.2,
        },
        reticle: Some(reticle),
        fps: 60.0,
        speed_kmh: 24.0,
        zoom_factor: sniper.then_some(6.9),
        damage_log: vec![
            DamageLogEntry {
                direction: LogDirection::Dealt,
                damage_hp: 240,
                module: Some(ModuleSlot::Gun),
                other_vehicle: Some(VehicleKind::TigerII),
                age_s: 0.5,
            },
            DamageLogEntry {
                direction: LogDirection::Taken,
                damage_hp: 180,
                module: None,
                other_vehicle: Some(VehicleKind::IS3),
                age_s: 1.5,
            },
        ],
        incoming_hits: vec![IncomingHit { bearing_rad: 2.1, age_s: 0.3, penetrated: false }],
        ammo: Some(AmmoHudModel::new([22, 9, 6], 0)),
    };
    build_battle_hud(&model, aspect)
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
