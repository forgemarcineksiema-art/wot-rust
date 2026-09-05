use naga::{AddressSpace, ResourceBinding};
use renderer_api::RenderError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgslUniformBinding {
    pub name: String,
    pub group: u32,
    pub binding: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgslValidationReport {
    pub label: String,
    pub entry_points: Vec<String>,
    pub uniform_bindings: Vec<WgslUniformBinding>,
}

impl WgslValidationReport {
    pub fn has_uniform_binding(&self, name: &str, group: u32, binding: u32) -> bool {
        self.uniform_bindings
            .iter()
            .any(|item| item.name == name && item.group == group && item.binding == binding)
    }
}

pub fn validate_wgsl_shader(
    label: impl Into<String>,
    source: &str,
) -> Result<WgslValidationReport, RenderError> {
    let label = label.into();
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|error| RenderError::new(format!("{label} WGSL parse failed: {error}")))?;

    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|error| RenderError::new(format!("{label} WGSL validation failed: {error}")))?;

    Ok(WgslValidationReport {
        label,
        entry_points: module.entry_points.iter().map(|entry| entry.name.clone()).collect(),
        uniform_bindings: uniform_bindings(&module),
    })
}

fn uniform_bindings(module: &naga::Module) -> Vec<WgslUniformBinding> {
    module
        .global_variables
        .iter()
        .filter_map(|(_handle, variable)| {
            let ResourceBinding { group, binding } = variable.binding?;
            (variable.space == AddressSpace::Uniform).then(|| WgslUniformBinding {
                name: variable.name.clone().unwrap_or_default(),
                group,
                binding,
            })
        })
        .collect()
}

/// One resource binding read by one entry point (stage) of a shader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgslStageBindingUse {
    pub entry_point: String,
    pub stage: naga::ShaderStage,
    pub name: String,
    pub group: u32,
    pub binding: u32,
}

/// Every resource binding each entry point of `source` actually reads, from naga's own
/// per-entry-point global-use analysis. The renderer's bind-group layouts declare which STAGE
/// may see a binding; a shader that reads a binding from a stage the layout did not grant is
/// refused by wgpu at pipeline creation — through the uncaptured-error log, with no panic and
/// no frame. The layouts are pinned to this list by tests so that refusal cannot ship.
pub fn wgsl_stage_binding_uses(
    label: &str,
    source: &str,
) -> Result<Vec<WgslStageBindingUse>, RenderError> {
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|error| RenderError::new(format!("{label} WGSL parse failed: {error}")))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|error| RenderError::new(format!("{label} WGSL validation failed: {error}")))?;

    let mut uses = Vec::new();
    for (index, entry) in module.entry_points.iter().enumerate() {
        let entry_info = info.get_entry_point(index);
        for (handle, variable) in module.global_variables.iter() {
            let Some(ResourceBinding { group, binding }) = variable.binding else {
                continue;
            };
            if entry_info[handle].is_empty() {
                continue;
            }
            uses.push(WgslStageBindingUse {
                entry_point: entry.name.clone(),
                stage: entry.stage,
                name: variable.name.clone().unwrap_or_default(),
                group,
                binding,
            });
        }
    }
    Ok(uses)
}
