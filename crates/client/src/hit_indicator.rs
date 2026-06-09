use game_core::{DamageCause, DamageEvent, ModuleSlot, TankId};
use glam::Vec3;
use renderer_api::{HudVertex, SceneVertex};

mod hit_indicator_draw;

use crate::reticle::world_to_clip_xy;
use crate::tank_mesh::push_box;
use hit_indicator_draw::{GRN, RED, color_for, fade, push_marker, push_module_icon, push_pen_bar};

const FEEDBACK_TTL: f32 = 2.5;
const FADE_DURATION: f32 = 0.8;
const FLOAT_UP: f32 = 0.06;

#[derive(Debug, Clone)]
struct HitFeedback {
    hit_position: Vec3,
    damage_hp: u32,
    penetrated: bool,
    ricocheted: bool,
    shell_pen_mm: f32,
    armor_mm: f32,
    module: Option<ModuleSlot>,
    cause: DamageCause,
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
            shell_pen_mm: e.shell_penetration_mm,
            armor_mm: e.effective_armor_mm,
            module: e.module,
            cause: e.cause,
            age: 0.0,
        }));
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
            let num_digits = digits(entry.damage_hp.min(9_999));
            let num_h = 0.065;
            let num_w = num_digits as f32 * num_h * 0.6;
            crate::hud_number::push_number(
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

            if let Some(module) = entry.module {
                push_module_icon(
                    &mut verts,
                    [clip[0] + num_w * 0.5 + 0.014, clip[1] + 0.02],
                    module,
                    alpha,
                    aspect,
                );
            }

            let by = clip[1] - 0.015;
            push_pen_bar(
                &mut verts,
                clip[0],
                by,
                entry.shell_pen_mm,
                entry.armor_mm,
                alpha,
                aspect,
            );

            let pc = if entry.penetrated { GRN } else { RED };
            crate::hud_number::push_number(
                &mut verts,
                entry.shell_pen_mm.round().min(9_999.0) as u32,
                clip[0] - 0.065,
                by - 0.028,
                0.03,
                aspect,
                fade(pc, alpha),
            );
            crate::hud_number::push_number(
                &mut verts,
                entry.armor_mm.round().min(9_999.0) as u32,
                clip[0] + 0.075,
                by - 0.028,
                0.03,
                aspect,
                fade(RED, alpha),
            );
        }
        verts
    }

    pub(crate) fn append_world_marks(
        &self,
        vertices: &mut Vec<SceneVertex>,
        indices: &mut Vec<u32>,
    ) {
        for entry in self.entries.iter().filter(|entry| entry.cause == DamageCause::Shell) {
            push_box(
                vertices,
                indices,
                entry.hit_position + Vec3::Y * 0.035,
                Vec3::new(0.16, 0.025, 0.16),
                0.0,
                mark_color(entry.penetrated, entry.ricocheted),
            );
        }
    }
}

fn mark_color(penetrated: bool, ricocheted: bool) -> [f32; 3] {
    if penetrated {
        [0.05, 0.045, 0.035]
    } else if ricocheted {
        [0.95, 0.62, 0.18]
    } else {
        [0.22, 0.20, 0.16]
    }
}

fn digits(mut n: u32) -> u32 {
    if n == 0 {
        return 1;
    }
    let mut c = 0;
    while n > 0 {
        n /= 10;
        c += 1;
    }
    c
}
