//! What the client does when a frame fails to render.
//!
//! Almost every render error is a fact about one frame — a validation slip, a budget — and the
//! next frame tries again; those are logged and nothing else. A LOST DEVICE is a fact about the
//! machine: the GPU driver reset (a Windows TDR, a GPU change, a laptop waking with a different
//! adapter), every buffer and texture is gone, and no frame will ever succeed on it again. Until
//! this module the renderer answered that by reconfiguring its surface forever — a black window
//! with a WARN per frame, no exit, no message. Now the device is rebuilt ONCE, from the state the
//! client already owns (the baked scene, the vehicle catalog, the atlases), and a second loss
//! stops the game with a reason instead of looping.

use renderer_api::RenderError;
use tracing::{error, warn};

use super::{ClientApp, SceneKind};

/// The decision a render failure leads to — pure, so the policy is tested without a GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderFailureAction {
    /// The device is fine and this frame was not: log, the next frame tries again.
    LogAndContinue,
    /// The device is gone for the first time: rebuild the renderer on a fresh one.
    RebuildRenderer,
    /// The device is gone AGAIN after a rebuild: this machine cannot hold one; stop the game.
    Exit,
}

impl ClientApp {
    pub(super) fn render_failure_action(
        error: &RenderError,
        renderer_rebuilt: bool,
    ) -> RenderFailureAction {
        if !error.is_device_lost() {
            RenderFailureAction::LogAndContinue
        } else if renderer_rebuilt {
            RenderFailureAction::Exit
        } else {
            RenderFailureAction::RebuildRenderer
        }
    }

    pub(super) fn on_render_failure(&mut self, error: RenderError) {
        match Self::render_failure_action(&error, self.renderer_rebuilt) {
            RenderFailureAction::LogAndContinue => error!(%error, "frame render failed"),
            RenderFailureAction::RebuildRenderer => {
                warn!(%error, "GPU device lost — rebuilding the renderer on a fresh device");
                self.renderer_rebuilt = true;
                if let Err(rebuild) = self.rebuild_renderer() {
                    self.fatal_error = Some(format!(
                        "the GPU device was lost ({error}) and the renderer could not be rebuilt \
                         ({rebuild})"
                    ));
                }
            }
            RenderFailureAction::Exit => {
                self.fatal_error = Some(format!(
                    "the GPU device was lost again after a rebuild ({error}); this machine cannot \
                     hold a GPU device"
                ));
            }
        }
    }

    /// Drop the dead renderer and build a new one against the same window, then hand it
    /// everything the old one held: the scene bake and atlases go up in `create_renderer`,
    /// every vehicle mesh and material is re-queued from the catalog under its old handle, and
    /// the scene swap is marked stale so the next frame re-uploads whichever scene is showing.
    fn rebuild_renderer(&mut self) -> Result<(), RenderError> {
        let Some(window) = self.window.clone() else {
            return Err(RenderError::new("no window to rebuild the renderer against"));
        };
        // The old device first: two devices on one surface is exactly what wgpu refuses.
        self.renderer = None;
        let (width, height) = self.viewport;
        self.create_renderer(window, width, height)?;
        self.vehicle_asset_catalog.re_pend_all_uploads();
        // `create_renderer` uploads the battle statics; if the garage is showing, the next
        // `ensure_scene` swaps its hall back in.
        self.current_scene = SceneKind::Battle;
        self.scene_upload_dirty = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use renderer_api::RenderError;

    use super::RenderFailureAction;
    use crate::app::ClientApp;

    /// One rebuild is a recovery; a second loss is a machine that cannot hold a device. An
    /// ordinary error is neither.
    #[test]
    fn a_lost_device_earns_one_rebuild_and_then_the_game_stops() {
        let lost = RenderError::device_lost("surface lost for 30 consecutive frames");
        let slip = RenderError::new("validation");
        assert_eq!(
            ClientApp::render_failure_action(&slip, false),
            RenderFailureAction::LogAndContinue
        );
        assert_eq!(
            ClientApp::render_failure_action(&slip, true),
            RenderFailureAction::LogAndContinue,
            "an ordinary error after a rebuild is still only a frame"
        );
        assert_eq!(
            ClientApp::render_failure_action(&lost, false),
            RenderFailureAction::RebuildRenderer
        );
        assert_eq!(ClientApp::render_failure_action(&lost, true), RenderFailureAction::Exit);
    }

    /// Headless there is no window to rebuild against, so the rebuild fails — and a failed
    /// rebuild must leave a FATAL error for the loop to act on, never a silent return to the
    /// black window this module exists to end. A second loss after that is fatal outright.
    #[test]
    fn a_rebuild_that_cannot_happen_is_fatal_and_so_is_a_second_loss() {
        let mut app = ClientApp::new();
        assert!(app.fatal_error.is_none());
        app.on_render_failure(RenderError::new("a slip"));
        assert!(app.fatal_error.is_none(), "an ordinary error is logged, not fatal");

        app.on_render_failure(RenderError::device_lost("lost"));
        assert!(app.renderer_rebuilt, "the rebuild was attempted");
        let reason = app.fatal_error.take().expect("no window: the rebuild fails, fatally");
        assert!(reason.contains("could not be rebuilt"), "{reason}");

        app.on_render_failure(RenderError::device_lost("lost again"));
        let reason = app.fatal_error.take().expect("a second loss is fatal");
        assert!(reason.contains("lost again"), "{reason}");
    }
}
