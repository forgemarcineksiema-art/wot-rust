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
        // 0.5 m near keeps depth precision usable (precision is dominated by the near
        // value, so the far raise below moves it negligibly). 2600 m (Immersja A3.2):
        // the border apron continues the ground 1500 m past the red line, and from a
        // camera at the far border of a 1000 m map that outer rim sits ~2500 m away —
        // the old 2000 m plane CLIPPED the world's own horizon. The lock in scene_build
        // computes this bound from the apron's actual reach instead of trusting prose.
        Self { depth_range: DepthRange::ZeroToOne, near_plane_m: 0.5, far_plane_m: 2_600.0 }
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
