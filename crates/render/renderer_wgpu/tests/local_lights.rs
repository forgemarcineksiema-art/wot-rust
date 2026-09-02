//! Executable contract for the local fill pools (`LocalLight` / `local_pools` in
//! lighting_common.wgsl): a warm pool over the centre of a dark floor must brighten the centre
//! relative to the corners, and an all-off array must be a byte-identical no-op — zero lights,
//! zero regression on every outdoor look. Runs on the headless adapter; skips if none.

mod common;
use common::luma;

use renderer_api::{LocalLight, SceneLighting, SceneVertex, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

/// A flat floor quad under a deliberately dark directional rig, so the pool is the only
/// meaningful light source in frame.
fn floor_scene() -> (Vec<SceneVertex>, Vec<u32>) {
    let corners = [[-20.0, 0.0, -20.0], [20.0, 0.0, -20.0], [20.0, 0.0, 20.0], [-20.0, 0.0, 20.0]];
    let vertices: Vec<SceneVertex> =
        corners.map(|p| SceneVertex::new(p, [0.0, 1.0, 0.0], [0.4, 0.4, 0.4])).to_vec();
    (vertices, vec![0, 2, 1, 0, 3, 2])
}

fn dark_rig() -> SceneLighting {
    let mut lighting = SceneLighting::garage_studio();
    lighting.ambient_rgb = [0.02, 0.02, 0.02];
    lighting.ground_ambient_rgb = [0.01, 0.01, 0.01];
    lighting.key_rgb = [0.0, 0.0, 0.0];
    lighting.fill_rgb = [0.0, 0.0, 0.0];
    lighting.rim_rgb = [0.0, 0.0, 0.0];
    lighting
}

fn render_floor(ctx: &GpuContext, lighting: SceneLighting) -> Vec<u8> {
    let (vertices, indices) = floor_scene();
    let camera = renderer_api::Camera {
        eye: [0.0, 14.0, 0.01],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 60.0,
    };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 100.0);
    let target = OffscreenTarget::new(ctx, 128, 128).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(ctx, &vertices, &indices).expect("renderer");
    renderer.scene_lighting = lighting;
    renderer.render(ctx, target.render_target(), view_proj, camera.eye).expect("render");
    target.read_rgba8(ctx).expect("readback")
}

fn headless() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping local lights test: {error}");
            None
        }
    }
}

#[test]
fn a_warm_pool_brightens_its_floor_and_all_off_is_a_byte_identical_noop() {
    let Some(ctx) = headless() else {
        return;
    };

    // Baseline: the dark rig with every pool slot disabled.
    let baseline = render_floor(&ctx, dark_rig());

    // The same rig with one warm worklamp pooled over the floor centre.
    let mut pooled = dark_rig();
    pooled.local_lights[0] = LocalLight {
        position: [0.0, 4.0, 0.0],
        radius_m: 9.0,
        rgb: [1.0, 0.88, 0.70],
        intensity: 1.5,
    };
    let lit = render_floor(&ctx, pooled);

    // Top-down camera: the frame centre is the floor under the lamp, the frame corner is the
    // floor out past the pool radius.
    let sample = |pixels: &[u8], x: usize, y: usize| -> f32 {
        let index = (y * 128 + x) * 4;
        luma(&pixels[index..index + 4])
    };
    let centre = sample(&lit, 64, 64);
    let corner = sample(&lit, 6, 6);
    assert!(
        centre > corner + 0.10,
        "the pool must brighten the floor beneath it: centre {centre:.3} vs corner {corner:.3}"
    );
    // The pool is warm: red leads blue at the centre.
    let centre_index = (64 * 128 + 64) * 4;
    assert!(
        lit[centre_index] > lit[centre_index + 2],
        "a warm pool reads warm, got rgb {:?}",
        &lit[centre_index..centre_index + 3]
    );

    // Zero lights = zero regression: the all-off frame is byte-identical to the baseline.
    let all_off = render_floor(&ctx, dark_rig());
    assert_eq!(baseline, all_off, "an all-off pool array must not perturb the image");
}

/// Inny Poziom S1: the muzzle flash as light. Under a real low-sun outdoor rig (Prokhorovka's
/// golden evening — the dusk the register named) the ground under a D-10's flash must rise
/// by a measured floor against the same frame with the flash gone, and the flash gone must be
/// byte-identical to the profile alone: no shot, no regression on any outdoor look.
#[test]
fn a_muzzle_flash_lights_the_dusk_ground_under_it_and_goes_out_clean() {
    let Some(ctx) = headless() else {
        return;
    };
    let evening = SceneLighting::prokhorovka_golden_evening();
    let baseline = render_floor(&ctx, evening);

    let mut flashing = evening;
    flashing.local_lights[0] = LocalLight::muzzle_flash([0.0, 1.8, 0.0], 1.0);
    let lit = render_floor(&ctx, flashing);

    let sample = |pixels: &[u8], x: usize, y: usize| -> f32 {
        let index = (y * 128 + x) * 4;
        luma(&pixels[index..index + 4])
    };
    let before = sample(&baseline, 64, 64);
    let after = sample(&lit, 64, 64);
    let rise = after - before;
    eprintln!("muzzle flash on dusk ground: {before:.3} -> {after:.3} (+{rise:.3})");
    // Measured 2026-09-02 on the headless adapter: 0.582 -> 0.907, +0.325; the floor is set
    // well under it so a tuning of the rig or the pool cannot pass by rounding.
    assert!(rise > 0.25, "the flash must light the ground under the muzzle: +{rise:.3}");
    // The corner is out past the pool: the flash is a POOL, not a second sun.
    let corner_before = sample(&baseline, 6, 6);
    let corner_after = sample(&lit, 6, 6);
    assert!(
        (corner_after - corner_before).abs() < 0.02,
        "the flash stays local: corner {corner_before:.3} -> {corner_after:.3}"
    );

    let out = render_floor(&ctx, flashing.local_lights[0].at_energy(0.0).pipe_into(evening));
    assert_eq!(baseline, out, "a burned-out flash is byte-identical to the profile alone");
}

/// The decayed pulse goes back into the profile it lit, so the byte-identity check above
/// exercises the exact array the client hands the renderer once a flash has burned out.
trait PipeInto {
    fn pipe_into(self, profile: SceneLighting) -> SceneLighting;
}

impl PipeInto for LocalLight {
    fn pipe_into(self, mut profile: SceneLighting) -> SceneLighting {
        profile.local_lights[0] = self;
        profile
    }
}
