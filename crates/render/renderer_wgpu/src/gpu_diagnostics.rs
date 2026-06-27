#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WgpuLabelPolicy;

impl WgpuLabelPolicy {
    pub fn required_startup_labels() -> &'static [&'static str] {
        &[
            "terrain_depth_prepass_pipeline",
            "tank_pbr_pipeline",
            "shell_tracer_vertex_buffer",
            "shadow_map_2048",
        ]
    }

    pub fn is_valid_label(label: &str) -> bool {
        !label.is_empty()
            && label.chars().all(|character| {
                character.is_ascii_lowercase() || character == '_' || character.is_ascii_digit()
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuErrorPolicy {
    uses_error_scopes: bool,
    installs_uncaptured_error_handler: bool,
    uncaptured_errors_are_fatal: bool,
}

impl Default for GpuErrorPolicy {
    fn default() -> Self {
        Self {
            uses_error_scopes: true,
            installs_uncaptured_error_handler: true,
            uncaptured_errors_are_fatal: true,
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
