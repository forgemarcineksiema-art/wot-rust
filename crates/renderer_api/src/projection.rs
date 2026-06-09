#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthRange {
    ZeroToOne,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraProjectionPolicy {
    depth_range: DepthRange,
    near_plane_m: f32,
    far_plane_m: f32,
}

impl CameraProjectionPolicy {
    pub fn webgpu_default() -> Self {
        Self { depth_range: DepthRange::ZeroToOne, near_plane_m: 0.1, far_plane_m: 3000.0 }
    }

    pub fn depth_range(self) -> DepthRange {
        self.depth_range
    }

    pub fn near_plane_m(self) -> f32 {
        self.near_plane_m
    }

    pub fn far_plane_m(self) -> f32 {
        self.far_plane_m
    }
}
