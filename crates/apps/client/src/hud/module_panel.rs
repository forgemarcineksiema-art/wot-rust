//! The module-status panel: a short row of module icons under the health bar telling the player,
//! at a glance, WHY the gun just refused — a destroyed gun or ammo rack reads red here, so a
//! silent fire-refusal becomes a visible wound instead of a mystery. Four cells: Gun, Ammo rack,
//! Engine, Suspension (the suspension cell folds in the worst of the running gear — the tracks
//! carry their own dedicated callout). Instrument style: chamfered dark slots, flat icon fills,
//! a calm green when whole, amber when wounded, signal red when knocked out.

use game_core::{MODULE_SLOT_COUNT, ModuleCondition, ModuleSlot, TRACK_HP_MAX, module_condition};
use renderer_api::HudVertex;

use super::icons::HudIcon;
use super::push_panel;
use super::theme::{CHAMFER_SLOT, color, tagged};

/// Cells drawn, left to right. Gun and ammo rack lead because they answer "why can't I fire".
pub const MODULE_CELL_COUNT: usize = 4;

/// Condition tints. Semantic combat colors live with their feature (not in `theme`), and the HUD
/// tests identify features by exact vertex-color equality, so each carries a distinct byte tag.
/// Whole modules read quiet (low alpha) so the eye only catches the wounded ones.
pub(crate) const MODULE_HEALTHY: [f32; 4] = tagged([0.34, 0.60, 0.36, 1.0], 0.55);
pub(crate) const MODULE_DAMAGED: [f32; 4] = tagged([0.93, 0.72, 0.20, 1.0], 0.96);
pub(crate) const MODULE_DESTROYED: [f32; 4] = tagged([0.90, 0.26, 0.22, 1.0], 0.96);

const CELL_HALF: [f32; 2] = [0.030, 0.038];
const FIRST_CELL_X: f32 = -0.90;
const CELL_STEP_X: f32 = 0.076;
/// Just under the top-left health bar (which sits near clip-y 0.9).
const ROW_CENTER_Y: f32 = 0.72;
const ICON_SIZE: f32 = 0.048;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModuleCell {
    pub icon: HudIcon,
    pub condition: ModuleCondition,
}

/// What the battle HUD knows about the player's own modules this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModulePanelModel {
    pub cells: [ModuleCell; MODULE_CELL_COUNT],
}

impl ModulePanelModel {
    /// Classify the panel from the local player's live module HP (latest snapshot) and full pool
    /// (spec), plus the worst-side track condition. `live`/`full` are in `ModuleSlot::ALL` order.
    /// The suspension cell shows whichever is worse — the suspension module or the tracks — so the
    /// running gear reads as one honest mobility light.
    pub(crate) fn new(
        live: [u32; MODULE_SLOT_COUNT],
        full: [u32; MODULE_SLOT_COUNT],
        track: ModuleCondition,
    ) -> Self {
        let cond =
            |slot: ModuleSlot| module_condition(live[slot.wire_index()], full[slot.wire_index()]);
        let cell = |slot: ModuleSlot, condition: ModuleCondition| ModuleCell {
            icon: HudIcon::for_module(slot),
            condition,
        };
        ModulePanelModel {
            cells: [
                cell(ModuleSlot::Gun, cond(ModuleSlot::Gun)),
                cell(ModuleSlot::AmmoRack, cond(ModuleSlot::AmmoRack)),
                cell(ModuleSlot::Engine, cond(ModuleSlot::Engine)),
                cell(ModuleSlot::Suspension, worse(cond(ModuleSlot::Suspension), track)),
            ],
        }
    }
}

/// The worst-side track condition from the graded per-side pool `[left, right]`: a thrown side
/// (0) reads Destroyed, a degraded side (below full) Damaged, both full Healthy. Folded into the
/// suspension cell so the running gear reads as one mobility light.
pub(crate) fn track_condition(track_hp: [u8; 2]) -> ModuleCondition {
    let side = |hp: u8| module_condition(u32::from(hp), u32::from(TRACK_HP_MAX));
    worse(side(track_hp[0]), side(track_hp[1]))
}

/// The more severe of two conditions (Destroyed > Damaged > Healthy).
fn worse(a: ModuleCondition, b: ModuleCondition) -> ModuleCondition {
    if rank(a) >= rank(b) { a } else { b }
}

fn rank(c: ModuleCondition) -> u8 {
    match c {
        ModuleCondition::Healthy => 0,
        ModuleCondition::Damaged => 1,
        ModuleCondition::Destroyed => 2,
    }
}

fn condition_color(condition: ModuleCondition) -> [f32; 4] {
    match condition {
        ModuleCondition::Healthy => MODULE_HEALTHY,
        ModuleCondition::Damaged => MODULE_DAMAGED,
        ModuleCondition::Destroyed => MODULE_DESTROYED,
    }
}

pub(crate) fn push_module_panel(
    vertices: &mut Vec<HudVertex>,
    model: &ModulePanelModel,
    aspect: f32,
) {
    for (index, cell) in model.cells.iter().enumerate() {
        let center = [FIRST_CELL_X + index as f32 * CELL_STEP_X, ROW_CENTER_Y];
        // Dark chamfered backing so the icon reads over a bright, sunlit field.
        push_panel(vertices, center, CELL_HALF, CHAMFER_SLOT, aspect, color::PANEL);
        crate::hud::font::push_icon(
            vertices,
            cell.icon,
            center[0] - 0.024,
            center[1] + 0.024,
            ICON_SIZE,
            aspect,
            condition_color(cell.condition),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_pool() -> [u32; MODULE_SLOT_COUNT] {
        // Engine, Suspension, Turret, Gun, AmmoRack, Radio — arbitrary but non-zero fulls.
        [400, 300, 300, 150, 225, 60]
    }

    #[test]
    fn a_dead_gun_reads_destroyed_and_a_whole_rack_reads_healthy() {
        let full = full_pool();
        let mut live = full;
        live[ModuleSlot::Gun.wire_index()] = 0; // knocked out
        let model = ModulePanelModel::new(live, full, ModuleCondition::Healthy);
        assert_eq!(model.cells[0].condition, ModuleCondition::Destroyed, "gun cell is first");
        assert_eq!(model.cells[1].condition, ModuleCondition::Healthy, "rack still whole");
    }

    #[test]
    fn a_wounded_gun_reads_damaged() {
        let full = full_pool();
        let mut live = full;
        live[ModuleSlot::Gun.wire_index()] = 37; // crew-patched fraction of 150
        let model = ModulePanelModel::new(live, full, ModuleCondition::Healthy);
        assert_eq!(model.cells[0].condition, ModuleCondition::Damaged);
    }

    #[test]
    fn the_suspension_cell_folds_in_the_worse_of_running_gear_and_tracks() {
        let full = full_pool();
        // Suspension module whole, but a track is thrown -> the mobility light must go red.
        let model = ModulePanelModel::new(full, full, ModuleCondition::Destroyed);
        assert_eq!(model.cells[3].condition, ModuleCondition::Destroyed);
    }

    #[test]
    fn the_panel_draws_four_cells_top_left_and_a_dead_module_speaks_red() {
        let full = full_pool();
        let mut live = full;
        live[ModuleSlot::AmmoRack.wire_index()] = 0;
        let model = ModulePanelModel::new(live, full, ModuleCondition::Healthy);

        let mut v = Vec::new();
        push_module_panel(&mut v, &model, 16.0 / 9.0);

        // Each chamfered backing is one `push_panel` (18 verts, as the ammo panel locks).
        let backings = v.iter().filter(|vert| vert.color == color::PANEL).count();
        assert_eq!(backings, MODULE_CELL_COUNT * 18, "one chamfered backing per cell");

        let dead: Vec<_> = v.iter().filter(|vert| vert.color == MODULE_DESTROYED).collect();
        assert!(!dead.is_empty(), "a knocked-out module must speak signal red");
        assert!(
            dead.iter().all(|vert| vert.position[0] < 0.0 && vert.position[1] > 0.5),
            "the panel sits top-left, under the health bar"
        );
        assert!(
            v.iter().any(|vert| vert.color == MODULE_HEALTHY),
            "whole modules still draw, quietly"
        );
    }
}
