//! The floor under the fleet's central promise: every vehicle a player can take into battle
//! resolves shells against ITS OWN convex armour volumes.
//!
//! History this file exists to not repeat. `vehicle_armor_volumes` ended in a wildcard
//! (`_ => return None`), and `sim::shell_trace::tank` treats `None` as "fall back to box bands".
//! So a vehicle missing from that table shipped with the game's stated identity — armour resolved
//! against real 3D plates — quietly switched off: no panic, no log, nothing red.
//!
//! The defence that should have caught it did not, because the fleet loops that walk
//! `VehicleKind::ALL` write `let Some(volumes) = … else { continue }` and so step over exactly the
//! case that is broken. A loop that skips missing data proves only that present data is fine.
//!
//! This asserts PRESENCE. It is the shape `spaced_armor.rs` already uses (count what you checked,
//! then assert a floor) applied to the one table where absence is silent.

use game_core::{ArmorZone, VehicleKind, vehicle_armor_volumes};

#[test]
fn every_playable_vehicle_owns_its_armour_volumes() {
    let missing: Vec<_> = VehicleKind::PLAYABLE
        .into_iter()
        .filter(|kind| vehicle_armor_volumes(*kind).is_none())
        .collect();

    assert!(
        missing.is_empty(),
        "these playable vehicles resolve shells against BOX BANDS, not their own armour: \
         {missing:?} — add them to `vehicle_armor_volumes` or they ship dishonest"
    );
    assert!(
        VehicleKind::PLAYABLE.len() >= 8,
        "the floor means nothing if the roster is empty: {} playable",
        VehicleKind::PLAYABLE.len()
    );
}

/// Owning volumes is not enough — they have to describe a whole tank. A hull with no flank, or a
/// turret with no face, would pass a presence check and still resolve nothing where it matters.
#[test]
fn those_volumes_carry_a_plane_for_every_zone_a_shell_arrives_at() {
    for kind in VehicleKind::PLAYABLE {
        let volumes =
            vehicle_armor_volumes(kind).unwrap_or_else(|| panic!("{kind:?} has no armour volumes"));
        let zones: Vec<ArmorZone> = volumes
            .hull
            .iter()
            .chain(std::iter::once(&volumes.turret))
            .flat_map(|volume| volume.planes.iter().map(|plane| plane.zone))
            .collect();
        for wanted in [
            ArmorZone::UpperGlacis,
            ArmorZone::LowerPlate,
            ArmorZone::HullSide,
            ArmorZone::HullRear,
            ArmorZone::TurretFront,
            ArmorZone::TurretSide,
        ] {
            assert!(
                zones.contains(&wanted),
                "{kind:?} has no plane tagged {wanted:?} — a shell arriving there finds no steel"
            );
        }
    }
}
