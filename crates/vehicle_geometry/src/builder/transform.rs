use crate::{Axis, GeometryMesh};

use super::MeshBuilder;

impl MeshBuilder {
    pub fn append(mut self, mesh: &GeometryMesh) -> Self {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(mesh.vertices());
        self.indices.extend(mesh.indices().iter().map(|&i| base + i));
        self
    }

    pub fn mirror(mut self, axis: Axis) -> Self {
        let vert_count = self.vertices.len() as u32;
        let originals_v: Vec<_> = self.vertices.drain(..).collect();
        let originals_i: Vec<_> = self.indices.drain(..).collect();

        self.vertices = originals_v.clone();
        for vertex in &originals_v {
            let mut v = *vertex;
            match axis {
                Axis::X => {
                    v.position.x = -v.position.x;
                    v.normal.x = -v.normal.x;
                }
                Axis::Y => {
                    v.position.y = -v.position.y;
                    v.normal.y = -v.normal.y;
                }
                Axis::Z => {
                    v.position.z = -v.position.z;
                    v.normal.z = -v.normal.z;
                }
            }
            self.vertices.push(v);
        }
        self.indices = originals_i.clone();
        for tri in originals_i.chunks_exact(3) {
            self.indices.push(vert_count + tri[0]);
            self.indices.push(vert_count + tri[2]);
            self.indices.push(vert_count + tri[1]);
        }
        self
    }

    pub fn array_along(mut self, axis: Axis, step: f32, count: usize) -> Self {
        if count < 2 {
            return self;
        }
        let existing_v = self.vertices.clone();
        let existing_i = self.indices.clone();

        for i in 1..count {
            let offset = step * i as f32;
            let base = self.vertices.len() as u32;
            for vertex in &existing_v {
                let mut v = *vertex;
                match axis {
                    Axis::X => v.position.x += offset,
                    Axis::Y => v.position.y += offset,
                    Axis::Z => v.position.z += offset,
                }
                self.vertices.push(v);
            }
            for &idx in &existing_i {
                self.indices.push(base + idx);
            }
        }
        self
    }
}
