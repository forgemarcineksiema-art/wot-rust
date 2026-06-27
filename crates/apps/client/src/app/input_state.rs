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
