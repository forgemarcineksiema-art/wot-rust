//! The world-handedness lock — the assertion class the 2026-08-12 mirror audit found missing
//! everywhere. Every layer can be SELF-consistently inverted (the blueprints, the Studio tiles,
//! the side asserts all agreed with each other while every asymmetric fitting rendered on the
//! wrong side, and the tile chirality test passed because its camera names and its basis were
//! inverted together). What no test held was the chain from an authored blueprint side through
//! the REAL pose math to a viewer's screen. This holds it.
//!
//! Convention, once, in words: the world is right-handed with +Y up; a vehicle's local +Z is
//! its bow; therefore local **+X is the vehicle's PORT (left) side** (`right = forward x up`
//! points to -X). A T-54's commander cupola is on the port side; head-on — facing each other —
//! the tank's port is on the viewer's RIGHT, exactly as the reference sheet's front view draws
//! the cupola.

use game_core::math::hull_basis;
use game_core::{VehicleBlueprint, VehicleKind};
use glam::Vec3;

/// Screen-right for a viewer with view direction `view` and +Y up — the standard right-handed
/// look-at frame (`cross(view, up)`), the same construction the client camera and the Forge
/// tile rasterizer use.
fn screen_right(view: Vec3) -> Vec3 {
    view.cross(Vec3::Y).normalize()
}

#[test]
fn the_t54_cupola_is_authored_on_the_port_side() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    assert!(
        bp.turret.cupola_x > 0.0,
        "the commander's cupola sits on the PORT side, which is +X: got {}",
        bp.turret.cupola_x
    );
    let fittings = bp.complete_visual().expect("visual").fittings;
    assert!(
        fittings.driver_hatch_center.x > 0.0,
        "driver (port): {}",
        fittings.driver_hatch_center.x
    );
    assert!(fittings.headlight_center.x > 0.0, "headlight (port): {}", fittings.headlight_center.x);
    let detail = bp.complete_visual().expect("visual").detail;
    assert!(detail.dshk_mount_center.x < 0.0, "DShK (starboard): {}", detail.dshk_mount_center.x);
    assert!(detail.exhaust_center.x > 0.0, "exhaust (port): {}", detail.exhaust_center.x);
}

#[test]
fn a_port_fitting_lands_on_the_viewers_right_in_a_head_on_view() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let cupola_local = Vec3::new(bp.turret.cupola_x, bp.turret.roof_y, bp.turret.cupola_z);

    // Sweep the REAL hull orientation math — the one basis sim and render both build through —
    // and for each yaw put the viewer dead ahead of the bow, looking back at the tank.
    for yaw in [0.0_f32, 0.7, std::f32::consts::FRAC_PI_2, 2.4, -1.1] {
        let basis = hull_basis(yaw, 0.0, 0.0);
        let world_bow = basis * Vec3::Z;
        let world_cupola = basis * cupola_local;
        let view = -world_bow; // the viewer stands ahead of the bow, looking at the tank
        let x_on_screen = world_cupola.dot(screen_right(view));
        assert!(
            x_on_screen > 0.0,
            "head-on at yaw {yaw}: the port cupola belongs on the viewer's RIGHT \
             (screen x {x_on_screen:.3}) — a negative value here is the global mirror"
        );
    }
}
