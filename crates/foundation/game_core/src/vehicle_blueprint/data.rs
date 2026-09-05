//! Per-vehicle blueprint data. The SOURCE is the RON file per vehicle
//! (`crates/foundation/game_core/blueprints/<slug>.blueprint.ron`, loaded by `source.rs`);
//! migrated vehicles return `Some`, the rest return `None` and keep the legacy hand-authored
//! hitbox/mounts/armour + recipe until they are migrated.
//!
//! Until 2026-09-05 the old Rust constructors survived here as `#[cfg(test)]` golden fixtures
//! for a transitional lock (`parsed_ron_equals_rust_fixture`). They are gone: every data change
//! paid for a second edit in them, and the bake-hash goldens (`vehicle_recipes/goldens/
//! bake_hashes.txt`) were always the outer judge of a RON edit. A number lives once, in RON.

use super::{VehicleBlueprint, source};
use crate::VehicleKind;

pub(super) fn blueprint(kind: VehicleKind) -> Option<VehicleBlueprint> {
    source::load_blueprint(kind)
}
