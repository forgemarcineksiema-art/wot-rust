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
        // 0.5 m near keeps depth precision usable across a 2000 m far plane (precision is
        // dominated by the near value); 2000 m comfortably covers the 1000 m map diagonal.
        // The client render path and the offscreen examples read these instead of hardcoding.
        Self { depth_range: DepthRange::ZeroToOne, near_plane_m: 0.5, far_plane_m: 2000.0 }
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
