use std::collections::BTreeMap;

use crate::{MaterialHandle, MeshHandle, SceneVertex};

#[derive(Debug, Clone, PartialEq)]
pub struct MeshAsset {
    vertices: Vec<SceneVertex>,
    indices: Vec<u32>,
}

impl MeshAsset {
    pub fn new(vertices: Vec<SceneVertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    pub fn vertices(&self) -> &[SceneVertex] {
        &self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn index_count(&self) -> usize {
        self.indices.len()
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MeshRegistry {
    labels: BTreeMap<String, MeshHandle>,
    meshes: Vec<(String, MeshAsset)>,
}

impl MeshRegistry {
    pub fn register(&mut self, label: impl Into<String>, mesh: MeshAsset) -> MeshHandle {
        let label = label.into();
        if let Some(handle) = self.labels.get(&label) {
            return *handle;
        }
        let handle = MeshHandle(self.meshes.len() as u32);
        self.labels.insert(label.clone(), handle);
        self.meshes.push((label, mesh));
        handle
    }

    pub fn mesh(&self, handle: MeshHandle) -> Option<&MeshAsset> {
        self.meshes.get(handle.0 as usize).map(|(_, mesh)| mesh)
    }

    pub fn mesh_label(&self, handle: MeshHandle) -> Option<&str> {
        self.meshes.get(handle.0 as usize).map(|(label, _)| label.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialDescriptor {
    pub label: String,
    pub base_color: [f32; 3],
}

impl MaterialDescriptor {
    pub fn new(label: impl Into<String>, base_color: [f32; 3]) -> Self {
        Self { label: label.into(), base_color }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderMaterialRegistry {
    materials: Vec<MaterialDescriptor>,
}

impl RenderMaterialRegistry {
    pub fn register(&mut self, material: MaterialDescriptor) -> MaterialHandle {
        if let Some(index) = self.materials.iter().position(|existing| existing == &material) {
            return MaterialHandle(index as u32);
        }
        let handle = MaterialHandle(self.materials.len() as u32);
        self.materials.push(material);
        handle
    }

    pub fn material(&self, handle: MaterialHandle) -> Option<&MaterialDescriptor> {
        self.materials.get(handle.0 as usize)
    }
}
