use glam::{Mat4, Vec4};
use renderer_api::{Camera, view_projection_matrix};

#[test]
fn depth_maps_near_to_zero_and_far_to_one() {
    // Camera at the origin looking down -Z, WebGPU [0,1] depth convention.
    let camera =
        Camera { eye: [0.0, 0.0, 0.0], target: [0.0, 0.0, -1.0], vertical_fov_degrees: 60.0 };
    let matrix = Mat4::from_cols_array_2d(&view_projection_matrix(&camera, 1.6, 0.1, 100.0));

    let near = matrix * Vec4::new(0.0, 0.0, -0.1, 1.0);
    let far = matrix * Vec4::new(0.0, 0.0, -100.0, 1.0);

    assert!((near.z / near.w).abs() < 1e-3, "near depth should be ~0, got {}", near.z / near.w);
    assert!((far.z / far.w - 1.0).abs() < 1e-3, "far depth should be ~1, got {}", far.z / far.w);
}

#[test]
fn forward_point_lands_in_front_of_the_camera() {
    let camera =
        Camera { eye: [0.0, 0.0, 0.0], target: [0.0, 0.0, -1.0], vertical_fov_degrees: 60.0 };
    let matrix = Mat4::from_cols_array_2d(&view_projection_matrix(&camera, 1.0, 0.1, 100.0));

    // A point straight ahead must project to the center of the screen (x≈0, y≈0).
    let clip = matrix * Vec4::new(0.0, 0.0, -10.0, 1.0);
    assert!(clip.w > 0.0, "point ahead should have positive w");
    assert!((clip.x / clip.w).abs() < 1e-4);
    assert!((clip.y / clip.w).abs() < 1e-4);
}
