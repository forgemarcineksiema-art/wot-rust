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

pub fn basic_tank_shader_source() -> &'static str {
    include_str!("shaders/basic_tank.wgsl")
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
