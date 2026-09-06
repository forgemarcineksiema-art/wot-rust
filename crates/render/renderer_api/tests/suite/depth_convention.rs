use renderer_api::{CameraProjectionPolicy, DepthRange};

#[test]
fn camera_projection_uses_webgpu_zero_to_one_depth_range() {
    let policy = CameraProjectionPolicy::webgpu_default();

    assert_eq!(policy.depth_range(), DepthRange::ZeroToOne);
    assert_eq!(policy.near_plane_m(), 0.5);
    // RENEGOTIATED (Immersja A3.2): 2000 m clipped the border apron's outer rim (~2500 m
    // from a far-border camera). Depth precision is dominated by the near plane, so the
    // raise costs nothing the eye can see; scene_build's apron-reach lock computes the
    // actual bound.
    assert_eq!(policy.far_plane_m(), 2_600.0);
}
