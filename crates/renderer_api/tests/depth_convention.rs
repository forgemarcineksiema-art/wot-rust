use renderer_api::{CameraProjectionPolicy, DepthRange};

#[test]
fn camera_projection_uses_webgpu_zero_to_one_depth_range() {
    let policy = CameraProjectionPolicy::webgpu_default();

    assert_eq!(policy.depth_range(), DepthRange::ZeroToOne);
    assert_eq!(policy.near_plane_m(), 0.1);
    assert_eq!(policy.far_plane_m(), 3000.0);
}
