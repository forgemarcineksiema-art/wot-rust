//! The shared UI kit (W4, target architecture L5): the military-instrument art direction's
//! visual tokens (`theme`), the flat clip-space drawing primitives (`primitives`), the baked
//! HUD font atlas with its layout engine (`font`), and the raster icon set the atlas embeds
//! (`icons`). Extracted from the client so the editor draws its overlays through the SAME kit
//! instead of pulling the whole client (winit, wgpu, cpal, audio) for four symbols — the last
//! app-to-app edge in the layer rules, burned by this crate's existence.
//!
//! Everything works in clip space and emits plain `renderer_api::HudVertex` triangles; nothing
//! here touches a window, a device, or the simulation.

pub mod font;
pub mod icons;
pub mod primitives;
pub mod sheet;
pub mod theme;
