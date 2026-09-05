//! When a lost surface stops being "reconfigure and carry on" and becomes a lost device.
//!
//! `get_current_texture` answers `Lost` for two very different reasons. A display change or a
//! driver hiccup loses the surface for a frame or two, and reconfiguring it is the whole cure. A
//! driver reset (a Windows TDR, a GPU change) loses the DEVICE, and reconfiguring the surface on
//! a dead device answers `Lost` forever — which used to be a black window pumping a WARN per
//! frame with no way out. The two are told apart the only way they can be from the surface's
//! side: by how long the loss lasts.

/// Counts consecutive `Lost` acquires and says when the streak means the device is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SurfaceLossPolicy {
    consecutive: u32,
}

impl SurfaceLossPolicy {
    /// Frames of consecutive `Lost` before the surface is presumed gone with its device: half a
    /// second at 60 Hz. A display change recovers in one or two frames; a TDR never does. Long
    /// enough not to tear down a renderer over a monitor being replugged, short enough that
    /// nobody watches a black window wondering.
    pub const GIVE_UP_AFTER: u32 = 30;

    pub const fn new() -> Self {
        Self { consecutive: 0 }
    }

    /// A frame was acquired: whatever the streak was, the surface is back.
    pub fn presented(&mut self) {
        self.consecutive = 0;
    }

    /// One more `Lost` acquire. `true` when the streak has reached the give-up line and the
    /// caller should report a lost device instead of reconfiguring once more.
    pub fn lost(&mut self) -> bool {
        self.consecutive = self.consecutive.saturating_add(1);
        self.consecutive >= Self::GIVE_UP_AFTER
    }

    pub fn consecutive_losses(&self) -> u32 {
        self.consecutive
    }
}

#[cfg(test)]
mod tests {
    use super::SurfaceLossPolicy;

    /// A display change is a few lost frames and then a good one: never a lost device. A TDR is
    /// lost frames without end: a lost device exactly at the line, not a frame earlier.
    #[test]
    fn a_short_loss_is_reconfigured_and_a_persistent_one_is_a_lost_device() {
        let mut policy = SurfaceLossPolicy::new();
        for _ in 0..SurfaceLossPolicy::GIVE_UP_AFTER - 1 {
            assert!(!policy.lost(), "under the line the surface is only reconfigured");
        }
        assert!(policy.lost(), "at the line the device is presumed gone");
        assert_eq!(policy.consecutive_losses(), SurfaceLossPolicy::GIVE_UP_AFTER);

        // A good acquire anywhere in the streak resets it: a monitor replug that took ten
        // frames to settle costs nothing.
        let mut settled = SurfaceLossPolicy::new();
        for _ in 0..10 {
            settled.lost();
        }
        settled.presented();
        assert_eq!(settled.consecutive_losses(), 0);
        assert!(!settled.lost());
    }
}
