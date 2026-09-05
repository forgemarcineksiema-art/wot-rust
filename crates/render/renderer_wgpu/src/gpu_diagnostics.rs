/// The GPU labels a capture of this renderer must show. These are the labels the code GIVES its
/// resources — `gpu_diagnostics.rs` (tests) reads the crate's source and fails if one of them is
/// not there. The list used to name four labels no resource had ever carried
/// (`tank_pbr_pipeline`, `shadow_map_2048`, ...), and its test checked the list against itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WgpuLabelPolicy;

impl WgpuLabelPolicy {
    pub fn required_startup_labels() -> &'static [&'static str] {
        &[
            "scene_pipeline",
            "vehicle_pipeline",
            "terrain_pipeline",
            "shadow_pipeline_scene",
            "sun_shadow_map",
            "scene_camera",
            "hdr_resolve",
            "scene_depth",
            "scene_fx_v",
        ]
    }

    pub fn is_valid_label(label: &str) -> bool {
        !label.is_empty()
            && label.chars().all(|character| {
                character.is_ascii_lowercase() || character == '_' || character.is_ascii_digit()
            })
    }
}

/// The GPU-error stance this renderer takes, DESCRIBING what `GpuContext` actually installs at
/// device creation — not an aspiration. The device-lost callback and the uncaptured-error handler
/// are wired there (`gpu_context::new_with_options`); they LOG rather than abort, because a shipped
/// game must not crash a player on a transient driver quirk. No `push_error_scope` is used anywhere
/// in this crate, and the field says so. Keep the fields here in step with that wiring — this
/// struct is only useful while it stays true (`gpu_diagnostics.rs` tests hold it to the source).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuErrorPolicy {
    uses_error_scopes: bool,
    installs_uncaptured_error_handler: bool,
    uncaptured_errors_are_fatal: bool,
}

impl Default for GpuErrorPolicy {
    fn default() -> Self {
        Self {
            uses_error_scopes: false,
            // Installed for real in `gpu_context` (was a claim with no handler behind it).
            installs_uncaptured_error_handler: true,
            // The handler logs; it does NOT abort. Crashing a player on an escaped GPU warning is
            // worse than the warning.
            uncaptured_errors_are_fatal: false,
        }
    }
}

impl GpuErrorPolicy {
    pub fn uses_error_scopes(self) -> bool {
        self.uses_error_scopes
    }

    pub fn installs_uncaptured_error_handler(self) -> bool {
        self.installs_uncaptured_error_handler
    }

    pub fn uncaptured_errors_are_fatal(self) -> bool {
        self.uncaptured_errors_are_fatal
    }
}
