//! The crew row: five letter-cells under the module panel telling the player, at a glance, WHO
//! is out of the fight and for how long — a covered station reads red with the first-aid
//! countdown beside it, a scarred man reads amber for the rest of the battle, a whole crew reads
//! as quietly as whole modules do. Same instrument style as the module panel above it: chamfered
//! dark slots, calm green / amber / signal red.

use game_core::{CREW_ROLE_COUNT, CrewRole, CrewVitals};
use renderer_api::HudVertex;

use super::push_panel;
use super::theme::{CHAMFER_SLOT, color, tagged};

/// Distinct byte tags (HUD tests identify features by exact vertex-color equality); the values
/// track the module panel's palette so the two rows read as one instrument.
pub(crate) const CREW_WHOLE: [f32; 4] = tagged([0.34, 0.60, 0.36, 1.0], 0.54);
pub(crate) const CREW_WEAKENED: [f32; 4] = tagged([0.93, 0.72, 0.20, 1.0], 0.95);
pub(crate) const CREW_DOWN: [f32; 4] = tagged([0.90, 0.26, 0.22, 1.0], 0.95);

const CELL_HALF: [f32; 2] = [0.024, 0.032];
const FIRST_CELL_X: f32 = -0.906;
const CELL_STEP_X: f32 = 0.062;
/// Directly under the module row (0.72), same left edge — one instrument, two lines.
const ROW_CENTER_Y: f32 = 0.62;
const LETTER_SIZE: f32 = 0.034;

/// One crewman's cell: his letter, his state, and the bandage countdown while he is down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrewCell {
    pub role: CrewRole,
    pub weakened: bool,
    /// Seconds of first aid left; `Some` means the man is DOWN right now.
    pub down_remaining_s: Option<f32>,
}

/// What the battle HUD knows about the player's own crew this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrewPanelModel {
    pub cells: [CrewCell; CREW_ROLE_COUNT],
}

impl CrewPanelModel {
    /// Classify the row straight off the snapshot's replicated crew state (team-private, v46).
    pub(crate) fn new(
        unconscious_mask: u8,
        weakened_mask: u8,
        down_remaining_s: [Option<f32>; CREW_ROLE_COUNT],
    ) -> Self {
        let vitals = CrewVitals::from_wire(unconscious_mask, weakened_mask, down_remaining_s);
        let remaining = vitals.down_remaining_s();
        CrewPanelModel {
            cells: std::array::from_fn(|index| {
                let role = CrewRole::ALL[index];
                CrewCell {
                    role,
                    weakened: weakened_mask & role.mask_bit() != 0,
                    down_remaining_s: remaining[index],
                }
            }),
        }
    }
}

/// The one place a role becomes its HUD letter — the damage log borrows it for hit callouts.
pub(crate) fn role_letter(role: CrewRole) -> &'static str {
    match role {
        CrewRole::Commander => "C",
        CrewRole::Gunner => "G",
        CrewRole::Driver => "D",
        CrewRole::Loader => "L",
        CrewRole::RadioOperator => "R",
    }
}

fn cell_color(cell: &CrewCell) -> [f32; 4] {
    if cell.down_remaining_s.is_some() {
        CREW_DOWN
    } else if cell.weakened {
        CREW_WEAKENED
    } else {
        CREW_WHOLE
    }
}

pub(crate) fn push_crew_panel(vertices: &mut Vec<HudVertex>, model: &CrewPanelModel, aspect: f32) {
    for (index, cell) in model.cells.iter().enumerate() {
        let center = [FIRST_CELL_X + index as f32 * CELL_STEP_X, ROW_CENTER_Y];
        push_panel(vertices, center, CELL_HALF, CHAMFER_SLOT, aspect, color::PANEL);
        let color = cell_color(cell);
        crate::hud::font::push_text(
            vertices,
            role_letter(cell.role),
            center[0] - 0.011,
            center[1] + 0.017,
            LETTER_SIZE,
            aspect,
            color,
        );
        // The bandage countdown, whole seconds, beside the downed man's cell — the same "you can
        // win this" clock the rack fuze earns.
        if let Some(remaining) = cell.down_remaining_s {
            crate::hud::number::push_number(
                vertices,
                remaining.ceil().max(0.0) as u32,
                center[0] + CELL_HALF[0] + 0.030,
                center[1] + 0.014,
                0.028,
                aspect,
                CREW_DOWN,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_row_reads_whole_weakened_and_down_with_the_countdown() {
        let model =
            CrewPanelModel::new(CrewRole::Loader.mask_bit(), CrewRole::Driver.mask_bit(), {
                let mut down = [None; CREW_ROLE_COUNT];
                down[CrewRole::Loader.wire_index()] = Some(9.4);
                down
            });
        let mut v = Vec::new();
        push_crew_panel(&mut v, &model, 16.0 / 9.0);

        let backings = v.iter().filter(|vert| vert.color == crate::hud::theme::color::PANEL);
        assert_eq!(backings.count(), CREW_ROLE_COUNT * 18, "one chamfered backing per man");
        assert!(v.iter().any(|vert| vert.color == CREW_DOWN), "the downed loader speaks red");
        assert!(v.iter().any(|vert| vert.color == CREW_WEAKENED), "the scarred driver reads amber");
        assert!(v.iter().any(|vert| vert.color == CREW_WHOLE), "whole men still draw, quietly");
        assert!(
            v.iter().all(|vert| vert.position[0] < 0.0 && vert.position[1] > 0.5),
            "the row sits top-left, under the module panel"
        );
    }

    #[test]
    fn a_whole_crew_paints_no_alarm_colors() {
        let model = CrewPanelModel::new(0, 0, [None; CREW_ROLE_COUNT]);
        let mut v = Vec::new();
        push_crew_panel(&mut v, &model, 16.0 / 9.0);
        assert!(v.iter().all(|vert| vert.color != CREW_DOWN && vert.color != CREW_WEAKENED));
    }
}
