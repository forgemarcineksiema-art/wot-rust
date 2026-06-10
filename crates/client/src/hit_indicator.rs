use game_core::{DamageCause, DamageEvent, ImpactSurface, ModuleSlot, ShellImpact, TankId};
use glam::Vec3;
use renderer_api::{HudVertex, SceneVertex};

mod hit_indicator_draw;

use crate::reticle::world_to_clip_xy;
use crate::tank_mesh::push_box;
use hit_indicator_draw::{GRN, RED, color_for, fade, push_marker, push_module_icon, push_pen_bar};

const FEEDBACK_TTL: f32 = 2.5;
const FADE_DURATION: f32 = 0.8;
const FLOAT_UP: f32 = 0.06;
/// Absorbed-shell puffs are short-lived ground/wall feedback, not a HUD readout.
const IMPACT_PUFF_TTL: f32 = 1.4;

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

/// A shell absorbed by terrain/cover/hull: a brief world-space puff where the shot died.
#[derive(Debug, Clone, Copy)]
struct ImpactPuff {
    position: Vec3,
    surface: ImpactSurface,
    age: f32,
}

#[derive(Debug, Default)]
pub(crate) struct HitIndicator {
    entries: Vec<HitFeedback>,
    puffs: Vec<ImpactPuff>,
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

    /// Every absorbed shell on the field gets a puff (yours and theirs alike): visible shellfall
    /// is battlefield information, and your own blocked shot finally has a visible death point.
    pub(crate) fn ingest_shell_impacts(&mut self, impacts: &[ShellImpact]) {
        self.puffs.extend(impacts.iter().map(|impact| ImpactPuff {
            position: impact.position,
            surface: impact.surface,
            age: 0.0,
        }));
    }

    pub(crate) fn tick(&mut self, dt: f32) {
        for e in &mut self.entries {
            e.age += dt;
        }
        self.entries.retain(|e| e.age < FEEDBACK_TTL);
        for puff in &mut self.puffs {
            puff.age += dt;
        }
        self.puffs.retain(|puff| puff.age < IMPACT_PUFF_TTL);
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
            let num_digits = crate::hud_number::digit_count(entry.damage_hp.min(9_999));
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
        for puff in &self.puffs {
            // A small cube that shrinks as it ages — enough to read "the shot died here".
            let life = 1.0 - puff.age / IMPACT_PUFF_TTL;
            let half = 0.14 + 0.26 * life;
            push_box(
                vertices,
                indices,
                puff.position + Vec3::Y * half * 0.5,
                Vec3::splat(half),
                0.0,
                puff_color(puff.surface),
            );
        }
    }
}

fn puff_color(surface: ImpactSurface) -> [f32; 3] {
    match surface {
        ImpactSurface::Terrain => [0.46, 0.40, 0.28],
        ImpactSurface::Cover => [0.52, 0.47, 0.40],
        ImpactSurface::Hull => [0.70, 0.68, 0.62],
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
