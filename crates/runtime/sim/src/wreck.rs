//! What still happens to a hull after it stops being a tank.
//!
//! A wreck does not drive, steer or slide — but it is a physical object standing on the same
//! ground every live hull stands on, and the ground moves. The drive step is skipped for dead
//! hulls (see `state`), which used to mean their VERTICAL was skipped with it, so two things
//! never resolved:
//!
//! * a hull killed in mid-flight (shot off a crest, off the railway embankment) hung at the
//!   altitude it died at, for the rest of the battle;
//! * a wreck standing where a later high-explosive round dug a crater (protocol v31 folds the
//!   ledger into the heightmap the next tick) floated over the hole it should have slumped into.
//!
//! Both are the kind of thing the honesty doctrine exists to forbid, and wrecks are not
//! cosmetic: they block shells and hulls as `StaticCoverKind::Wreck`.
//!
//! Only the vertical is resolved, and it runs for every dead hull whether or not anything sent
//! it a command this tick — the same rule drowning follows.

use glam::Vec3;
use physics::{TankKinematicState, is_grounded, resolve_vertical, support_height};
use terrain::HeightMap;

use crate::tank_state::TankState;

/// Let every wreck fall to, or settle onto, the ground under it. A no-op on terrain-free modes
/// and for any wreck already resting on its support — which is the overwhelming common case
/// (a hull dies on the ground it was already grounded to), so replays stay bit-identical.
pub(crate) fn settle_wrecks(tanks: &mut [TankState], heightmap: Option<&HeightMap>, dt: f32) {
    let Some(heightmap) = heightmap else {
        return;
    };
    for tank in tanks.iter_mut() {
        if tank.hit_points > 0 {
            continue;
        }
        // The same ride height a living hull reads: the running-gear support envelope, with the
        // centre probe as the fallback, so a wreck rests exactly where the tank rested.
        let footprint = tank.spec.contact_footprint();
        let ground = support_height(heightmap, tank.position, tank.yaw_rad, &footprint)
            .or_else(|| heightmap.sample_height(tank.position.x, tank.position.z));
        let Some(ground) = ground else {
            continue;
        };
        let was_grounded = is_grounded(tank.position.y, ground);
        let mut falling = TankKinematicState {
            position: tank.position,
            // A wreck carries no horizontal momentum — only whatever fall it was already in.
            velocity: Vec3::new(0.0, tank.velocity_mps.y, 0.0),
            yaw_rad: tank.yaw_rad,
            ..Default::default()
        };
        // `moved_xz_m` is zero: a wreck covers no ground, so the follow limit is the standing
        // one and a hull hanging over a hole drops instead of being carried across it.
        resolve_vertical(&mut falling, ground, was_grounded, 0.0, dt);
        tank.position.y = falling.position.y;
        tank.velocity_mps = falling.velocity;
    }
}
