use game_core::{DamageEvent, ModuleSlot, TankId};
use glam::Vec3;
use renderer_api::HudVertex;

mod draw;

use crate::hud::reticle::world_to_clip_xy;
use draw::{color_for, fade, push_marker, push_module_icon};

const FEEDBACK_TTL: f32 = 2.5;
const FADE_DURATION: f32 = 0.8;
const FLOAT_UP: f32 = 0.06;

#[derive(Debug, Clone)]
struct HitFeedback {
    hit_position: Vec3,
    damage_hp: u32,
    penetrated: bool,
    ricocheted: bool,
    module: Option<ModuleSlot>,
    age: f32,
}

#[derive(Debug, Default)]
pub(crate) struct HitIndicator {
    entries: Vec<HitFeedback>,
}

impl HitIndicator {
    pub(crate) fn ingest_damage_events(&mut self, events: &[DamageEvent], player: TankId) {
        self.entries.extend(events.iter().filter(|e| e.source == player).map(|e| HitFeedback {
            hit_position: e.hit_position,
            damage_hp: e.damage_hp,
            penetrated: e.penetrated,
            ricocheted: e.ricocheted,
            module: e.module,
            age: 0.0,
        }));
    }

    /// The freshest own hit still inside the confirm window, echoed as ticks at the reticle.
    pub(crate) fn recent_confirm(&self) -> Option<crate::hud::reticle_readouts::HitConfirm> {
        self.entries
            .iter()
            .filter(|entry| entry.age < crate::hud::reticle_readouts::HIT_CONFIRM_TTL_S)
            .min_by(|a, b| a.age.total_cmp(&b.age))
            .map(|entry| crate::hud::reticle_readouts::HitConfirm {
                age_s: entry.age,
                penetrated: entry.penetrated,
                ricocheted: entry.ricocheted,
            })
    }

    pub(crate) fn tick(&mut self, dt: f32) {
        for e in &mut self.entries {
            e.age += dt;
        }
        self.entries.retain(|e| e.age < FEEDBACK_TTL);
    }

    pub(crate) fn render_vertices(&self, view_proj: [[f32; 4]; 4], aspect: f32) -> Vec<HudVertex> {
        let mut verts = Vec::new();
        for entry in &self.entries {
            let Some(mut clip) = world_to_clip_xy(entry.hit_position, view_proj) else { continue };
            clip[1] += entry.age * FLOAT_UP;
            let alpha = ((FEEDBACK_TTL - entry.age) / FADE_DURATION).clamp(0.0, 1.0);
            if alpha <= 0.0 {
                continue;
            }

            let dmg_color = color_for(entry.penetrated, entry.ricocheted);
            let num_digits = crate::hud::number::digit_count(entry.damage_hp.min(9_999));
            let num_h = 0.065;
            let num_w = num_digits as f32 * num_h * 0.6;
            crate::hud::number::push_number(
                &mut verts,
                entry.damage_hp.min(9_999),
                clip[0] + num_w * 0.5,
                clip[1] + 0.03,
                num_h,
                aspect,
                fade(dmg_color, alpha),
            );

            let mcx = clip[0] - num_w * 0.5 - 0.012;
            let mcy = clip[1] + 0.02;
            push_marker(&mut verts, [mcx, mcy], entry.penetrated, entry.ricocheted, alpha, aspect);

            // Damage number + result glyph + module icon and NOTHING more: the mm duel
            // (pen vs armor bar and both numbers) drowned the read in a fight — the color
            // and glyph already say pen/bounce/ricochet, the world FX say the rest.
            if let Some(module) = entry.module {
                push_module_icon(
                    &mut verts,
                    [clip[0] + num_w * 0.5 + 0.014, clip[1] + 0.02],
                    module,
                    alpha,
                    aspect,
                );
            }
        }
        verts
    }
}
