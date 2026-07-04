//! The in-battle ammo panel: three rack slots to the right of the reload bar, each with its
//! shell icon, remaining count and 1/2/3 key hint. The selected slot reads amber; an empty slot
//! prints its zero in signal red. Instrument style: chamfered slots, flat fills, no glow.

use renderer_api::HudVertex;

use super::icons::HudIcon;
use super::theme::{self, color, tagged};
use super::{push_panel, push_quad};

/// Count tints carry unique alpha tags (the HUD tests identify features by exact vertex-color
/// equality); empty slots speak signal red.
pub(crate) const AMMO_COUNT_COLOR: [f32; 4] = tagged(color::VALUE, 0.91);
pub(crate) const AMMO_COUNT_EMPTY: [f32; 4] = tagged(color::SIGNAL, 0.91);
const AMMO_KEY_HINT: [f32; 4] = tagged(color::TEXT_DIM, 0.70);

const SLOT_HALF: [f32; 2] = [0.040, 0.052];
const FIRST_SLOT_X: f32 = 0.27;
const SLOT_STEP_X: f32 = 0.105;
const SLOT_Y: f32 = -0.885;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmmoSlotHud {
    pub icon: HudIcon,
    pub count: u16,
}

/// What the battle HUD knows about the rack this frame: counts from the latest snapshot, the
/// selected slot from the predictor (optimistic on a switch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmmoHudModel {
    pub slots: [AmmoSlotHud; game_core::MAX_AMMO_SLOTS],
    pub selected: u8,
}

/// The icon order mirrors `GunSpec::ammo_options()`: stock AP, APCR, HE.
pub(crate) const AMMO_SLOT_ICONS: [HudIcon; game_core::MAX_AMMO_SLOTS] =
    [HudIcon::AmmoAp, HudIcon::AmmoApcr, HudIcon::AmmoHe];

impl AmmoHudModel {
    /// Rack counts (latest snapshot) + the predictor's selected slot, icons in options order.
    pub(crate) fn new(counts: [u16; game_core::MAX_AMMO_SLOTS], selected: u8) -> Self {
        Self {
            slots: std::array::from_fn(|i| AmmoSlotHud {
                icon: AMMO_SLOT_ICONS[i],
                count: counts[i],
            }),
            selected,
        }
    }
}

pub(crate) fn push_ammo_panel(vertices: &mut Vec<HudVertex>, model: &AmmoHudModel, aspect: f32) {
    for (index, slot) in model.slots.iter().enumerate() {
        let center = [FIRST_SLOT_X + index as f32 * SLOT_STEP_X, SLOT_Y];
        let selected = index == model.selected as usize;
        let fill = if selected { color::SLOT_SELECTED } else { color::SLOT };
        push_panel(vertices, center, SLOT_HALF, theme::CHAMFER_SLOT, aspect, fill);
        if selected {
            // A thin amber base line marks the loaded slot even at a glance-from-the-corner.
            push_quad(
                vertices,
                [center[0], center[1] - SLOT_HALF[1] + 0.004],
                [SLOT_HALF[0] * 0.8, 0.0022],
                color::ACCENT,
            );
        }

        let icon_tint = if slot.count == 0 { color::ICON_DIM } else { color::ICON };
        crate::hud::font::push_icon(
            vertices,
            slot.icon,
            center[0] - 0.030,
            center[1] + 0.036,
            0.052,
            aspect,
            icon_tint,
        );
        let count_color = if slot.count == 0 { AMMO_COUNT_EMPTY } else { AMMO_COUNT_COLOR };
        crate::hud::number::push_number(
            vertices,
            u32::from(slot.count.min(999)),
            center[0] + SLOT_HALF[0] - 0.008,
            center[1] + 0.002,
            0.036,
            aspect,
            count_color,
        );
        // Key hint in the slot's top-right corner: the battle binding is 1/2/3.
        crate::hud::number::push_number(
            vertices,
            index as u32 + 1,
            center[0] + SLOT_HALF[0] - 0.006,
            center[1] + SLOT_HALF[1] - 0.004,
            0.024,
            aspect,
            AMMO_KEY_HINT,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(counts: [u16; 3], selected: u8) -> AmmoHudModel {
        AmmoHudModel {
            slots: [
                AmmoSlotHud { icon: AMMO_SLOT_ICONS[0], count: counts[0] },
                AmmoSlotHud { icon: AMMO_SLOT_ICONS[1], count: counts[1] },
                AmmoSlotHud { icon: AMMO_SLOT_ICONS[2], count: counts[2] },
            ],
            selected,
        }
    }

    #[test]
    fn the_panel_draws_three_slots_with_counts_and_highlights_the_selected() {
        let mut v = Vec::new();
        push_ammo_panel(&mut v, &model([24, 10, 6], 1), 16.0 / 9.0);

        let selected_fill =
            v.iter().filter(|vert| vert.color == crate::hud::theme::color::SLOT_SELECTED).count();
        let rest_fill =
            v.iter().filter(|vert| vert.color == crate::hud::theme::color::SLOT).count();
        assert_eq!(selected_fill, 18, "exactly one chamfered slot reads selected");
        assert_eq!(rest_fill, 36, "the other two slots stay at rest");

        let counts: Vec<_> = v.iter().filter(|vert| vert.color == AMMO_COUNT_COLOR).collect();
        assert!(!counts.is_empty(), "counts print for loaded slots");
        assert!(
            counts.iter().all(|vert| vert.position[0] > 0.15 && vert.position[1] < -0.75),
            "the panel sits bottom-center-right, clear of the reload bar"
        );
    }

    #[test]
    fn an_empty_slot_prints_its_count_in_the_warning_color() {
        let mut v = Vec::new();
        push_ammo_panel(&mut v, &model([24, 0, 6], 0), 16.0 / 9.0);
        assert!(
            v.iter().any(|vert| vert.color == AMMO_COUNT_EMPTY),
            "a dry slot must speak signal red"
        );
    }
}
