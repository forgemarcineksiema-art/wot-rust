//! Surface-of-revolution geometry (renderer-free): the round-part generator of the hybrid kernel.
//!
//! Barrels, road wheels, rollers and sprockets are surfaces of revolution — cheap, clean, and a poor
//! fit for both the SDF (would round their crisp rims away) and CAD convex solids (would facet their
//! roundness). This crate revolves a profile into a `GeometryMesh`, plus translate/merge for
//! repetition (wheel trains). See `[[geometry-foundation-pivot]]` in project notes.

mod gun_parts;
mod parts;
mod profile;
mod revolve;

pub use gun_parts::{gun_barrel, gun_barrel_between, moving_mantlet};
pub use parts::drum;
pub use profile::{ProfilePoint, RevolveCaps, RevolveError, RevolveProfile, try_revolve};
pub use revolve::{merge, revolve, translate};
