use game_core::VehicleKind;
use renderer_api::HudVertex;

use super::ClientApp;
use crate::hud::push_quad;

const PANEL: [f32; 4] = [0.02, 0.025, 0.025, 0.86];
const SLOT: [f32; 4] = [0.10, 0.12, 0.13, 0.88];
const SELECTED: [f32; 4] = [0.62, 0.78, 0.42, 0.92];
const STAT_HP: [f32; 4] = [0.35, 0.78, 0.36, 0.90];
const STAT_SPEED: [f32; 4] = [0.35, 0.62, 0.92, 0.90];
const STAT_RELOAD: [f32; 4] = [0.90, 0.62, 0.32, 0.90];
const DIGIT: [f32; 4] = [0.88, 0.92, 0.86, 0.95];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GarageState {
    open: bool,
    started: bool,
    selected_index: usize,
}

impl Default for GarageState {
    fn default() -> Self {
        Self { open: true, started: false, selected_index: 0 }
    }
}

impl GarageState {
    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn has_started(&self) -> bool {
        self.started
    }

    pub(super) fn open(&mut self) {
        self.open = true;
    }

    pub(super) fn close_if_started(&mut self) {
        if self.started {
            self.open = false;
        }
    }

    pub(super) fn select_vehicle(&mut self, vehicle: VehicleKind) {
        if let Some(index) = VehicleKind::ALL.iter().position(|kind| *kind == vehicle) {
            self.selected_index = index;
        }
    }

    pub(super) fn cycle(&mut self, delta: isize) {
        let len = VehicleKind::ALL.len() as isize;
        self.selected_index = (self.selected_index as isize + delta).rem_euclid(len) as usize;
    }

    pub(super) fn confirm(&mut self) -> VehicleKind {
        self.started = true;
        self.open = false;
        self.selected_vehicle()
    }

    pub(super) fn selected_vehicle(&self) -> VehicleKind {
        VehicleKind::ALL[self.selected_index]
    }

    pub(super) fn overlay_vertices(&self, aspect: f32) -> Vec<HudVertex> {
        if !self.open {
            return Vec::new();
        }

        let mut vertices = Vec::new();
        push_quad(&mut vertices, [0.0, 0.0], [0.78, 0.30], PANEL);
        for (index, kind) in VehicleKind::ALL.into_iter().enumerate() {
            push_vehicle_slot(&mut vertices, index, kind, index == self.selected_index, aspect);
        }
        vertices
    }
}

impl ClientApp {
    pub(super) fn open_garage(&mut self) {
        self.garage.open();
        self.input.clear_mouse_look();
    }

    pub(super) fn select_garage_vehicle(&mut self, vehicle: VehicleKind) {
        self.garage.select_vehicle(vehicle);
    }

    pub(super) fn confirm_garage_selection(&mut self) {
        let requested_vehicle = self.garage.confirm();
        let selection =
            net::ClientVehicleSelection { client_tick: self.client_tick, requested_vehicle };
        let snapshot = self.local_server.change_player_vehicle(selection.requested_vehicle);
        self.player_tank = self.local_server.player_tank();
        self.predictor.reset_to_spec(&selection.requested_vehicle.spec());
        self.render_state = crate::InterpolatedBattleState::default();
        self.input.fire_pending = false;
        self.input.clear_mouse_look();
        self.accept_and_sync(snapshot);
        if let Some(window) = &self.window {
            window.set_title(&format!("WOT Rust Prototype - {}", requested_vehicle.display_name()));
        }
    }
}

fn push_vehicle_slot(
    vertices: &mut Vec<HudVertex>,
    index: usize,
    kind: VehicleKind,
    selected: bool,
    aspect: f32,
) {
    let x = -0.60 + index as f32 * 0.20;
    push_quad(vertices, [x, 0.0], [0.086, 0.23], if selected { SELECTED } else { SLOT });
    crate::hud_number::push_number(
        vertices,
        (index + 1) as u32,
        x + 0.03 / aspect.max(0.01),
        0.18,
        0.055,
        aspect,
        DIGIT,
    );

    let spec = kind.spec();
    push_stat_bar(vertices, [x, 0.04], spec.hit_points as f32 / 2050.0, STAT_HP);
    push_stat_bar(
        vertices,
        [x, -0.04],
        spec.max_forward_speed_mps / VehicleKind::PantherII.spec().max_forward_speed_mps,
        STAT_SPEED,
    );
    push_stat_bar(vertices, [x, -0.12], 1.0 - spec.gun.reload_seconds / 12.0, STAT_RELOAD);
}

fn push_stat_bar(vertices: &mut Vec<HudVertex>, center: [f32; 2], fraction: f32, color: [f32; 4]) {
    let half = [0.055, 0.012];
    push_quad(vertices, center, half, [0.0, 0.0, 0.0, 0.55]);
    let fill = half[0] * fraction.clamp(0.0, 1.0);
    push_quad(vertices, [center[0] - half[0] + fill, center[1]], [fill, half[1]], color);
}
