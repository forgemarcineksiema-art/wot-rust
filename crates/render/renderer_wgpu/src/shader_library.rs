//! Shared WGSL building blocks, composed into pass shaders Rust-side (WGSL has no #include).
//!
//! Four fragments, layered in this order:
//! - `camera_common.wgsl` — the one Camera uniform declaration (group 0, binding 0); every
//!   camera-bound pass composes it, so the uniform grows in exactly one place.
//! - `lighting_common.wgsl` — the lighting model and display transform (hemi ambient, radiance,
//!   fog, env sky, ACES + grade); composed into every LIT pass so the world grades as one picture.
//! - `shadow_common.wgsl` — the group-2 sun-shadow/SSAO/cloud-shade bindings and lookups; only
//!   for pipelines whose layout carries that bind group (scene, vehicle, terrain).
//! - `noise_common.wgsl` — the world-anchored detail noise (hash, lattice, octave frames) the
//!   ground passes share, so terrain and the statics standing on it carry ONE grain.
//!
//! The composed sources are validated by the wgsl_layout tests, including a dedup lock that each
//! final shader contains exactly one copy of the shared declarations.

pub(crate) const CAMERA_COMMON_WGSL: &str = include_str!("shaders/camera_common.wgsl");
pub(crate) const LIGHTING_COMMON_WGSL: &str = include_str!("shaders/lighting_common.wgsl");
pub(crate) const SHADOW_COMMON_WGSL: &str = include_str!("shaders/shadow_common.wgsl");
pub(crate) const NOISE_COMMON_WGSL: &str = include_str!("shaders/noise_common.wgsl");

pub(crate) fn compose_shader(parts: &[&str]) -> String {
    parts.join("\n")
}
