use game_core::{DamageEvent, ModuleSlot, TankId};
use glam::Vec3;
use renderer_api::HudVertex;

mod draw;

use crate::hud::reticle::world_to_clip_xy;
use draw::{MarkerOutcome, color_for, fade, push_marker, push_module_icon};

const FEEDBACK_TTL: f32 = 2.5;
const FADE_DURATION: f32 = 0.8;
const FLOAT_UP: f32 = 0.06;

#[derive(Debug, Clone)]
struct HitFeedback {
    hit_position: Vec3,
    damage_hp: u32,
    penetrated: bool,
    ricocheted: bool,
    /// A non-penetration that FAILED by less than the back-face margin — derived from the
    /// pen-vs-armor numbers the shooter already owns (mirror of `shell_spalls_on_nonpen` in the
    /// sim), never from the target's concealed crew state.
    near_pen: bool,
    /// The brittle core's death on the plate (wire v47): the shooter's mark says the round
    /// SHATTERED — stop feeding tungsten to that angle — where a plain ricochet says "it skipped".
    shattered: bool,
    module: Option<ModuleSlot>,
    age: f32,
}

/// The shooter-side mirror of the sim's back-face margin: within 12% of the effective steel
/// (clamped 5–35 mm) of getting in. Same constants as `sim/src/combat.rs` — a divergence here
/// makes the HUD lie about what the sim rewards.
fn near_penetration(event: &DamageEvent) -> bool {
    if event.penetrated
        || event.ricocheted
        || event.shell_type == game_core::ShellType::HighExplosive
    {
        return false;
    }
    let margin_mm = (event.effective_armor_mm * 0.12).clamp(5.0, 35.0);
    event.shell_penetration_mm > event.effective_armor_mm - margin_mm
}

/// What the marker prints beside its glyph (Inny Poziom A6): the damage when there was any,
/// the OUTCOME in a word when there was none. A literal "0" told the shooter nothing —
/// a ricochet, a shattered core, a belt that ate the round and a plain non-penetration all
/// printed the same digit, and five different lessons read as one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HitLabel {
    Damage(u32),
    Word(&'static str),
}

fn hit_label(damage_hp: u32, outcome: MarkerOutcome, module: Option<ModuleSlot>) -> HitLabel {
    use crate::ui_strings::battle as words;
    if damage_hp > 0 {
        return HitLabel::Damage(damage_hp.min(9_999));
    }
    HitLabel::Word(if outcome.shattered {
        words::HIT_SHATTER
    } else if outcome.ric {
        words::HIT_RICOCHET
    } else if outcome.pen {
        // A perforation that dealt nothing does not exist in the armour model today; if one
        // ever does, it still says what happened rather than printing a zero.
        words::HIT_PEN
    } else if module == Some(ModuleSlot::Suspension) {
        words::HIT_TRACKED
    } else {
        words::HIT_NO_PEN
    })
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
            near_pen: near_penetration(e),
            shattered: e.shattered,
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

            let outcome = MarkerOutcome {
                pen: entry.penetrated,
                ric: entry.ricocheted,
                near_pen: entry.near_pen,
                shattered: entry.shattered,
            };
            let dmg_color = color_for(outcome);
            let num_w = match hit_label(entry.damage_hp, outcome, entry.module) {
                HitLabel::Damage(damage) => {
                    let num_digits = crate::hud::number::digit_count(damage);
                    let num_h = 0.065;
                    let num_w = num_digits as f32 * num_h * 0.6;
                    crate::hud::number::push_number(
                        &mut verts,
                        damage,
                        clip[0] + num_w * 0.5,
                        clip[1] + 0.03,
                        num_h,
                        aspect,
                        fade(dmg_color, alpha),
                    );
                    num_w
                }
                HitLabel::Word(word) => {
                    // The outcome in a word, smaller than a damage number: it is a lesson
                    // ("that angle skips"), not a score.
                    let height = 0.042;
                    let width = crate::hud::font::text_width(word, height, aspect);
                    crate::hud::font::push_text(
                        &mut verts,
                        word,
                        clip[0] - width * 0.5,
                        clip[1] + 0.03,
                        height,
                        aspect,
                        fade(dmg_color, alpha),
                    );
                    width
                }
            };

            let mcx = clip[0] - num_w * 0.5 - 0.012;
            let mcy = clip[1] + 0.02;
            push_marker(&mut verts, [mcx, mcy], outcome, alpha, aspect);

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

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(pen: bool, ric: bool, near_pen: bool, shattered: bool) -> MarkerOutcome {
        MarkerOutcome { pen, ric, near_pen, shattered }
    }

    /// Inny Poziom A6: a landed shot that dealt nothing prints WHAT HAPPENED, never "0".
    #[test]
    fn a_zero_damage_hit_prints_its_outcome_in_a_word() {
        use crate::ui_strings::battle as words;
        assert_eq!(
            hit_label(0, outcome(false, true, false, false), None),
            HitLabel::Word(words::HIT_RICOCHET)
        );
        assert_eq!(
            hit_label(0, outcome(false, false, false, true), None),
            HitLabel::Word(words::HIT_SHATTER)
        );
        assert_eq!(
            hit_label(0, outcome(false, false, false, false), Some(ModuleSlot::Suspension)),
            HitLabel::Word(words::HIT_TRACKED)
        );
        assert_eq!(
            hit_label(0, outcome(false, false, true, false), None),
            HitLabel::Word(words::HIT_NO_PEN)
        );
        assert_eq!(
            hit_label(0, outcome(false, false, false, false), None),
            HitLabel::Word(words::HIT_NO_PEN)
        );
    }

    /// The number is for damage, and only damage: every outcome combination at zero yields a
    /// word, every positive damage yields its number (capped at four digits).
    #[test]
    fn a_zero_never_becomes_a_number_and_damage_never_becomes_a_word() {
        for pen in [false, true] {
            for ric in [false, true] {
                for near in [false, true] {
                    for shattered in [false, true] {
                        for module in [None, Some(ModuleSlot::Suspension), Some(ModuleSlot::Gun)] {
                            let o = outcome(pen, ric, near, shattered);
                            assert!(matches!(hit_label(0, o, module), HitLabel::Word(_)));
                            assert_eq!(hit_label(320, o, module), HitLabel::Damage(320));
                            assert_eq!(hit_label(25_000, o, module), HitLabel::Damage(9_999));
                        }
                    }
                }
            }
        }
    }

    /// The word reaches the screen: a zero-damage ricochet at a visible world point renders
    /// glyphs (atlas-sampling vertices), where the old marker rendered a digit's solid quads.
    #[test]
    fn the_word_is_drawn_where_the_zero_used_to_be() {
        let mut indicator = HitIndicator::default();
        indicator.entries.push(HitFeedback {
            hit_position: Vec3::new(0.0, 0.0, 10.0),
            damage_hp: 0,
            penetrated: false,
            ricocheted: true,
            near_pen: false,
            shattered: false,
            module: None,
            age: 0.0,
        });
        let view_proj = glam::Mat4::perspective_rh(1.0, 16.0 / 9.0, 0.1, 100.0)
            * glam::Mat4::look_at_rh(Vec3::ZERO, Vec3::Z * 10.0, Vec3::Y);
        let vertices = indicator.render_vertices(view_proj.to_cols_array_2d(), 16.0 / 9.0);
        assert!(!vertices.is_empty(), "the marker draws at a visible point");
        assert!(
            vertices.iter().any(|vertex| vertex.uv[0] >= 0.0 && vertex.uv != [0.0, 0.0]),
            "a word is glyphs sampling the atlas, not a digit's solid quads"
        );
    }
}
