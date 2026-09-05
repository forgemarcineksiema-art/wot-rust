//! Small accessors over the raw [`InputState`] flags: drive/steer/brake axes and the mouse-look
//! delta reset. Kept beside the event handling in `input.rs` but split out for reviewability.

use super::InputState;

impl InputState {
    pub(super) fn clear_mouse_look(&mut self) {
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
    }

    pub(super) fn set_shift(&mut self, shift: bool) {
        self.shift = shift;
    }

    /// Drop every held drive key. Used when a modal takes over the keyboard: the keys the player
    /// was holding will never send their release to the battle, so latching them would drive the
    /// hull for as long as the menu is up.
    pub(super) fn release_driving(&mut self) {
        self.forward = false;
        self.back = false;
        self.left = false;
        self.right = false;
        self.brake = false;
        self.fire_pending = false;
    }

    /// Everything the keyboard and wheel can leave latched, dropped at once — the drive keys and
    /// trigger of [`Self::release_driving`] plus the modifier mirror, the fractional wheel carry
    /// and the mouse-look delta. For a focus edge: after this the input is exactly what a fresh
    /// battle starts with, whatever was held when the window went away. The free-look and
    /// Shift-hold LATCHES are not fields cleared here but modes ended by the app
    /// (`end_free_look`, `end_sniper_hold`), because ending them moves the camera.
    pub(super) fn release_all_latches(&mut self) {
        self.release_driving();
        self.shift = false;
        self.wheel_pending_lines = 0.0;
        self.clear_mouse_look();
        debug_assert!(!self.free_look && self.sniper_hold_return.is_none(), "app ends the holds");
    }

    pub(super) fn throttle(&self) -> f32 {
        axis(self.forward, self.back)
    }

    pub(super) fn steer(&self) -> f32 {
        axis(self.right, self.left)
    }

    pub(super) fn brake_value(&self) -> f32 {
        if self.brake { 1.0 } else { 0.0 }
    }
}

fn axis(positive: bool, negative: bool) -> f32 {
    f32::from(u8::from(positive)) - f32::from(u8::from(negative))
}
