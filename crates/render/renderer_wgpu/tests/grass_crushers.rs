//! Executable contract for the meadow's crushers (Jedna Trawa P9): the renderer takes the
//! tanks it is ALREADY handed in the vehicle frame and hands the grass shader the nearest of
//! them. Nothing new crosses the client API, so the failure this guards against is the
//! renderer quietly reading the wrong thing — or nothing.
//!
//! Runs on the headless adapter; skips if none.

use renderer_api::{MaterialHandle, MeshHandle, RenderFrame, RenderObject};
use renderer_wgpu::{GpuContext, SceneRenderer};

fn tank_object(id: u64, x: f32, z: f32) -> RenderObject {
    let mut transform = [[0.0f32; 4]; 4];
    for (i, row) in transform.iter_mut().enumerate().take(3) {
        row[i] = 1.0;
    }
    transform[3] = [x, 0.0, z, 1.0];
    RenderObject {
        tank_id: Some(game_core::TankId(id)),
        mesh: MeshHandle(1),
        material: MaterialHandle(0),
        transform,
        tint: [1.0; 3],
    }
}

/// A tank is many objects (hull, turret, tracks) but ONE crusher, the nearest tanks win the
/// slots, and a frame with no tanks leaves every slot disabled — which is the bit-exact
/// no-op every grass-free scene relies on.
#[test]
fn the_nearest_tanks_take_the_crusher_slots() {
    let Ok(ctx) = GpuContext::headless() else {
        eprintln!("no headless adapter — skipped");
        return;
    };
    let Ok(mut renderer) = SceneRenderer::for_offscreen(&ctx, &[], &[]) else {
        eprintln!("no scene renderer — skipped");
        return;
    };
    let eye = [0.0, 3.0, 0.0];

    // No vehicle frame at all: every slot disabled.
    assert!(
        renderer.grass_crusher_slots_for_test(eye).iter().all(|slot| slot[3] == 0.0),
        "a scene with no tanks presses no grass"
    );

    // One tank far away, one near, each submitting several parts.
    let frame = RenderFrame {
        objects: vec![
            tank_object(7, 40.0, 0.0),
            tank_object(7, 40.4, 0.2),
            tank_object(3, 5.0, 0.0),
            tank_object(3, 5.3, 0.1),
        ],
        ..RenderFrame::default()
    };
    renderer.set_vehicle_render_frame(&ctx, &frame);
    let slots = renderer.grass_crusher_slots_for_test(eye);

    let live: Vec<_> = slots.iter().filter(|slot| slot[3] > 0.0).collect();
    assert_eq!(live.len(), 2, "two tanks, two crushers — parts of one tank do not each get one");
    assert!(
        (live[0][0] - 5.0).abs() < 1.0e-3,
        "the nearest tank takes the first slot, got x={}",
        live[0][0]
    );
    assert!((live[1][0] - 40.0).abs() < 1.0e-3, "the far tank follows");
    for slot in live {
        assert_eq!(
            slot[3],
            renderer_api::GRASS_CRUSH_RADIUS_M,
            "a live slot carries the hull's crush radius"
        );
    }

    // The crushers are per-FRAME state, not accumulated history: an empty vehicle frame
    // (every tank dead or out of view) must release the meadow.
    renderer.set_vehicle_render_frame(&ctx, &RenderFrame::default());
    assert!(
        renderer.grass_crusher_slots_for_test(eye).iter().all(|slot| slot[3] == 0.0),
        "an empty vehicle frame releases the grass instead of pinning it flat forever"
    );
}
