use game_core::VehicleKind;
use glam::Vec3;
use renderer_api::{Camera, HudVertex};

use super::ClientApp;
use crate::garage_scene::hangar_camera_pivot;
use crate::hud::push_quad;

const PANEL: [f32; 4] = [0.04, 0.05, 0.06, 0.74];
const ROW: [f32; 4] = [0.12, 0.14, 0.16, 0.86];
const ROW_SELECTED: [f32; 4] = [0.62, 0.78, 0.42, 0.92];
const STAT_HP: [f32; 4] = [0.35, 0.78, 0.36, 0.95];
const STAT_SPEED: [f32; 4] = [0.35, 0.62, 0.92, 0.95];
const STAT_RELOAD: [f32; 4] = [0.90, 0.62, 0.32, 0.95];
const BATTLE: [f32; 4] = [0.78, 0.30, 0.20, 0.95];
const TEXT: [f32; 4] = [0.90, 0.93, 0.88, 0.97];
const TEXT_DIM: [f32; 4] = [0.74, 0.78, 0.74, 0.85];
const TEXT_DARK: [f32; 4] = [0.10, 0.12, 0.10, 0.98];

// Orbit camera limits.
const MIN_PITCH: f32 = -0.05;
const MAX_PITCH: f32 = 1.20;
const MIN_DISTANCE: f32 = 6.0;
const MAX_DISTANCE: f32 = 24.0;
const ORBIT_SENSITIVITY: f32 = 0.005;
const ZOOM_STEP_M: f32 = 1.2;

// List layout, in clip space (these are shared by the overlay and the cursor hit test).
const ROW_CENTER_X: f32 = -0.74;
const ROW_HALF_X: f32 = 0.22;
const ROW_HALF_Y: f32 = 0.066;
const ROW_TOP_Y: f32 = 0.80;
const ROW_PITCH_Y: f32 = 0.158;
const BATTLE_CENTER: [f32; 2] = [0.70, -0.82];
const BATTLE_HALF: [f32; 2] = [0.24, 0.10];

/// What the cursor is over when a click lands in the garage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GarageHit {
    /// A vehicle row in the left list.
    Row(usize),
    /// The "Battle" button.
    Battle,
    /// Empty scene — start orbiting the camera.
    Scene,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct GarageState {
    open: bool,
    started: bool,
    selected_index: usize,
    orbit_yaw: f32,
    orbit_pitch: f32,
    orbit_distance: f32,
    cursor_clip: [f32; 2],
    dragging: bool,
}

impl Default for GarageState {
    fn default() -> Self {
        Self {
            open: true,
            started: false,
            selected_index: 0,
            orbit_yaw: 2.4,
            orbit_pitch: 0.22,
            orbit_distance: 13.0,
            cursor_clip: [2.0, 2.0],
            dragging: false,
        }
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
        self.dragging = false;
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
        self.dragging = false;
        self.selected_vehicle()
    }

    pub(super) fn selected_vehicle(&self) -> VehicleKind {
        VehicleKind::ALL[self.selected_index]
    }

    // --- orbit camera ---------------------------------------------------------------------

    /// The orbit camera circling the parked tank. `view_projection_matrix` applies the aspect.
    pub(super) fn orbit_camera(&self) -> Camera {
        let pivot = hangar_camera_pivot();
        let horizontal = self.orbit_distance * self.orbit_pitch.cos();
        let eye = pivot
            + Vec3::new(
                horizontal * self.orbit_yaw.sin(),
                self.orbit_distance * self.orbit_pitch.sin(),
                horizontal * self.orbit_yaw.cos(),
            );
        Camera { eye: eye.to_array(), target: pivot.to_array(), vertical_fov_degrees: 42.0 }
    }

    pub(super) fn begin_drag(&mut self) {
        self.dragging = true;
    }

    pub(super) fn end_drag(&mut self) {
        self.dragging = false;
    }

    /// Apply accumulated mouse motion to the orbit when a drag is in progress.
    pub(super) fn apply_drag(&mut self, dx: f32, dy: f32) {
        if !self.dragging {
            return;
        }
        self.orbit_yaw += dx * ORBIT_SENSITIVITY;
        self.orbit_pitch = (self.orbit_pitch - dy * ORBIT_SENSITIVITY).clamp(MIN_PITCH, MAX_PITCH);
    }

    pub(super) fn apply_zoom(&mut self, notches: f32) {
        self.orbit_distance =
            (self.orbit_distance - notches * ZOOM_STEP_M).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    // --- cursor / hit testing -------------------------------------------------------------

    pub(super) fn set_cursor(&mut self, clip: [f32; 2]) {
        self.cursor_clip = clip;
    }

    pub(super) fn hit_test(&self) -> GarageHit {
        let [cx, cy] = self.cursor_clip;
        if in_rect([cx, cy], BATTLE_CENTER, BATTLE_HALF) {
            return GarageHit::Battle;
        }
        for index in 0..VehicleKind::ALL.len() {
            let (center, half) = row_rect(index);
            if in_rect([cx, cy], center, half) {
                return GarageHit::Row(index);
            }
        }
        GarageHit::Scene
    }

    // --- overlay --------------------------------------------------------------------------

    pub(super) fn overlay_vertices(&self, aspect: f32) -> Vec<HudVertex> {
        if !self.open {
            return Vec::new();
        }

        let mut vertices = Vec::new();

        // Left list panel backing.
        let list_top = ROW_TOP_Y + ROW_HALF_Y;
        let list_bottom = row_rect(VehicleKind::ALL.len() - 1).0[1] - ROW_HALF_Y;
        let list_center_y = (list_top + list_bottom) / 2.0;
        let list_half_y = (list_top - list_bottom) / 2.0 + 0.03;
        push_quad(&mut vertices, [ROW_CENTER_X, list_center_y], [ROW_HALF_X + 0.03, list_half_y], PANEL);

        for (index, kind) in VehicleKind::ALL.into_iter().enumerate() {
            let (center, half) = row_rect(index);
            let selected = index == self.selected_index;
            push_quad(&mut vertices, center, half, if selected { ROW_SELECTED } else { ROW });
            let label = format!("{}  {}", index + 1, short_name(kind));
            let text_color = if selected { TEXT_DARK } else { TEXT };
            crate::hud_font::push_text(
                &mut vertices,
                &label,
                center[0] - half[0] + 0.02,
                center[1] + 0.035,
                0.05,
                aspect,
                text_color,
            );
        }

        self.push_info_panel(&mut vertices, aspect);
        self.push_battle_button(&mut vertices, aspect);

        vertices
    }

    fn push_info_panel(&self, vertices: &mut Vec<HudVertex>, aspect: f32) {
        let kind = self.selected_vehicle();
        let spec = kind.spec();

        push_quad(vertices, [0.62, 0.46], [0.34, 0.30], PANEL);
        crate::hud_font::push_text(vertices, short_name(kind), 0.32, 0.70, 0.075, aspect, TEXT);

        let max_hp = fleet_max_hp();
        let max_speed = fleet_max_speed();
        let min_reload = fleet_min_reload();

        push_labeled_stat(
            vertices,
            "HP",
            [0.62, 0.56],
            stat_fraction(spec.hit_points as f32 / max_hp),
            STAT_HP,
            aspect,
        );
        push_labeled_stat(
            vertices,
            "SPD",
            [0.62, 0.46],
            stat_fraction(spec.max_forward_speed_mps / max_speed),
            STAT_SPEED,
            aspect,
        );
        push_labeled_stat(
            vertices,
            "RLD",
            [0.62, 0.36],
            stat_fraction(min_reload / spec.gun.reload_seconds),
            STAT_RELOAD,
            aspect,
        );
    }

    fn push_battle_button(&self, vertices: &mut Vec<HudVertex>, aspect: f32) {
        push_quad(vertices, BATTLE_CENTER, BATTLE_HALF, BATTLE);
        let height = 0.08;
        let width = crate::hud_font::text_width("BITWA", height, aspect);
        crate::hud_font::push_text(
            vertices,
            "BITWA",
            BATTLE_CENTER[0] - width / 2.0,
            BATTLE_CENTER[1] + height / 2.0,
            height,
            aspect,
            TEXT,
        );
    }
}

impl ClientApp {
    pub(super) fn open_garage(&mut self) {
        self.garage.open();
        self.input.clear_mouse_look();
        self.set_cursor_captured(false);
    }

    pub(super) fn select_garage_vehicle(&mut self, vehicle: VehicleKind) {
        self.garage.select_vehicle(vehicle);
    }

    /// Route a left-button press in the garage: pick a vehicle, launch battle, or start orbiting.
    pub(super) fn garage_primary_press(&mut self) {
        match self.garage.hit_test() {
            GarageHit::Row(index) => {
                if let Some(kind) = VehicleKind::ALL.get(index).copied() {
                    self.garage.select_vehicle(kind);
                }
            }
            GarageHit::Battle => self.confirm_garage_selection(),
            GarageHit::Scene => self.garage.begin_drag(),
        }
    }

    pub(super) fn garage_primary_release(&mut self) {
        self.garage.end_drag();
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
        self.set_cursor_captured(true);
        if let Some(window) = &self.window {
            window.set_title(&format!("WOT Rust Prototype - {}", requested_vehicle.display_name()));
        }
    }
}

fn row_rect(index: usize) -> ([f32; 2], [f32; 2]) {
    let y = ROW_TOP_Y - index as f32 * ROW_PITCH_Y;
    ([ROW_CENTER_X, y], [ROW_HALF_X, ROW_HALF_Y])
}

fn in_rect(point: [f32; 2], center: [f32; 2], half: [f32; 2]) -> bool {
    (point[0] - center[0]).abs() <= half[0] && (point[1] - center[1]).abs() <= half[1]
}

/// Stat bars never fully empty or saturated: clamp to a readable band so even the worst vehicle
/// shows a sliver and the fleet best does not peg an ambiguous full bar.
fn stat_fraction(raw: f32) -> f32 {
    raw.clamp(0.0, 1.0)
}

fn fleet_max_hp() -> f32 {
    VehicleKind::ALL.iter().map(|kind| kind.spec().hit_points).max().unwrap_or(1) as f32
}

fn fleet_max_speed() -> f32 {
    VehicleKind::ALL
        .iter()
        .map(|kind| kind.spec().max_forward_speed_mps)
        .fold(1.0_f32, f32::max)
}

fn fleet_min_reload() -> f32 {
    VehicleKind::ALL
        .iter()
        .map(|kind| kind.spec().gun.reload_seconds)
        .fold(f32::INFINITY, f32::min)
}

fn push_labeled_stat(
    vertices: &mut Vec<HudVertex>,
    label: &str,
    center: [f32; 2],
    fraction: f32,
    color: [f32; 4],
    aspect: f32,
) {
    crate::hud_font::push_text(vertices, label, center[0] - 0.30, center[1] + 0.022, 0.04, aspect, TEXT_DIM);
    let half = [0.16, 0.014];
    let bar_center = [center[0] + 0.06, center[1]];
    push_quad(vertices, bar_center, half, [0.0, 0.0, 0.0, 0.55]);
    let fill = half[0] * fraction.clamp(0.0, 1.0);
    push_quad(vertices, [bar_center[0] - half[0] + fill, bar_center[1]], [fill, half[1]], color);
}

fn short_name(kind: VehicleKind) -> &'static str {
    match kind {
        VehicleKind::PrototypeMedium => "Prototype",
        VehicleKind::T54_1951 => "T-54",
        VehicleKind::T55A => "T-55A",
        VehicleKind::TigerI => "Tiger I",
        VehicleKind::TigerII => "Tiger II",
        VehicleKind::Jagdtiger => "Jagdtiger",
        VehicleKind::PantherII => "Panther II",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vehicle_stat_fraction_is_within_the_bar() {
        // Locks the earlier garage bug: fixed normalizers must keep every bar in [0, 1].
        let max_hp = fleet_max_hp();
        let max_speed = fleet_max_speed();
        let min_reload = fleet_min_reload();
        for kind in VehicleKind::ALL {
            let spec = kind.spec();
            for raw in [
                spec.hit_points as f32 / max_hp,
                spec.max_forward_speed_mps / max_speed,
                min_reload / spec.gun.reload_seconds,
            ] {
                assert!((0.0..=1.0).contains(&raw), "{kind:?} stat {raw} out of [0,1]");
            }
        }
    }

    #[test]
    fn cursor_over_a_row_selects_that_vehicle_index() {
        let mut garage = GarageState::default();
        let (center, _) = row_rect(3);
        garage.set_cursor(center);
        assert_eq!(garage.hit_test(), GarageHit::Row(3));
    }

    #[test]
    fn cursor_over_the_battle_button_hits_battle() {
        let mut garage = GarageState::default();
        garage.set_cursor(BATTLE_CENTER);
        assert_eq!(garage.hit_test(), GarageHit::Battle);
    }

    #[test]
    fn cursor_over_empty_scene_is_a_scene_hit() {
        let mut garage = GarageState::default();
        garage.set_cursor([0.0, -0.2]);
        assert_eq!(garage.hit_test(), GarageHit::Scene);
    }

    #[test]
    fn drag_only_orbits_while_a_drag_is_active() {
        let mut garage = GarageState::default();
        let yaw = garage.orbit_yaw;
        garage.apply_drag(100.0, 0.0);
        assert_eq!(garage.orbit_yaw, yaw, "no drag in progress, no rotation");

        garage.begin_drag();
        garage.apply_drag(100.0, 0.0);
        assert!((garage.orbit_yaw - yaw).abs() > 1.0e-3, "active drag rotates the orbit");
    }

    #[test]
    fn orbit_pitch_and_zoom_stay_clamped() {
        let mut garage = GarageState::default();
        garage.begin_drag();
        garage.apply_drag(0.0, -100_000.0);
        assert!(garage.orbit_pitch <= MAX_PITCH + 1.0e-6);
        garage.apply_drag(0.0, 100_000.0);
        assert!(garage.orbit_pitch >= MIN_PITCH - 1.0e-6);

        garage.apply_zoom(1_000.0);
        assert!(garage.orbit_distance >= MIN_DISTANCE - 1.0e-6);
        garage.apply_zoom(-1_000.0);
        assert!(garage.orbit_distance <= MAX_DISTANCE + 1.0e-6);
    }

    #[test]
    fn confirm_closes_the_garage_and_keeps_the_selected_vehicle() {
        let mut garage = GarageState::default();
        garage.select_vehicle(VehicleKind::TigerII);
        let chosen = garage.confirm();
        assert_eq!(chosen, VehicleKind::TigerII);
        assert!(!garage.is_open() && garage.has_started());
    }
}
