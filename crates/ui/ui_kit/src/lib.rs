//! The shared UI kit (W4, target architecture L5; interface program F2–F4): the theme as data
//! (`theme`), the material sheet (`sheet`), the font pair's distance-field atlas with its layout
//! engine (`font`), the icon set the atlas embeds (`icons`), the pixel-space `Rect`, the `Ui`
//! context and the row/column flow (`rect`, `ui`, `flow`), the semantic draw list with its one
//! emitter and one hit test (`draw_list`), the interaction machine (`interaction`) — and the
//! legacy clip-space primitives (`primitives`) the reticle stack still draws with. Extracted
//! from the client so the editor draws its overlays through the SAME kit instead of pulling the
//! whole client (winit, wgpu, cpal, audio) for four symbols — the last app-to-app edge in the
//! layer rules, burned by this crate's existence.
//!
//! Everything emits plain `renderer_api::HudVertex` triangles; nothing here touches a window, a
//! device, or the simulation.

pub mod draw_list;
pub mod flow;
pub mod font;
pub mod icons;
pub mod interaction;
pub mod primitives;
pub mod rect;
pub mod sheet;
pub mod theme;
pub mod ui;
