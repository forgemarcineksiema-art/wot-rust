use game_core::{ArmorZone, HitboxProfile, ModuleSlot, ShellType, TrackSide};
use glam::Vec3;

use crate::TankState;

pub(crate) fn apply_track_damage_for_hit(
    target: &mut TankState,
    module: Option<ModuleSlot>,
    zone: ArmorZone,
    shell_type: ShellType,
    penetrated: bool,
) {
    match zone {
        ArmorZone::LeftTrack => target.tracks.damage(TrackSide::Left),
        ArmorZone::RightTrack => target.tracks.damage(TrackSide::Right),
        _ if shell_type == ShellType::HighExplosive && !penetrated => target.tracks.damage_both(),
        _ if module == Some(ModuleSlot::Suspension) => target.tracks.damage_both(),
        _ => {}
    }
}

pub(crate) fn impacted_module(
    shell_type: ShellType,
    penetrated: bool,
    zone: ArmorZone,
    local_hit: Vec3,
    hitbox: HitboxProfile,
) -> Option<ModuleSlot> {
    if shell_type == ShellType::HighExplosive && !penetrated {
        return Some(ModuleSlot::Suspension);
    }
    let candidate = penetrated.then(|| module_volume_at_hit(zone, local_hit, hitbox)).flatten()?;
    module_hit_roll_allows(candidate, zone, local_hit, hitbox).then_some(candidate.slot)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ModuleHitCandidate {
    slot: ModuleSlot,
    hit_chance: f32,
}

fn module_volume_at_hit(
    zone: ArmorZone,
    local_hit: Vec3,
    hitbox: HitboxProfile,
) -> Option<ModuleHitCandidate> {
    let half_length = hitbox.half_length_m.max(0.01);
    let z = local_hit.z / half_length;

    let (slot, hit_chance) = match zone {
        ArmorZone::LeftTrack | ArmorZone::RightTrack => (ModuleSlot::Suspension, 1.0),
        ArmorZone::Mantlet => (ModuleSlot::Gun, 1.0),
        ArmorZone::TurretFront | ArmorZone::Roof => (ModuleSlot::Turret, 0.80),
        ArmorZone::TurretSide | ArmorZone::TurretRear => (ModuleSlot::AmmoRack, 0.85),
        ArmorZone::HullRear => (ModuleSlot::Engine, 0.90),
        ArmorZone::HullSide if z < -0.35 => (ModuleSlot::Engine, 0.90),
        ArmorZone::HullSide if local_hit.y < -hitbox.half_height_m * 0.20 => {
            (ModuleSlot::Suspension, 1.0)
        }
        ArmorZone::UpperGlacis | ArmorZone::LowerPlate => (ModuleSlot::Gun, 0.80),
        ArmorZone::HullSide => (ModuleSlot::Suspension, 0.65),
    };
    Some(ModuleHitCandidate { slot, hit_chance })
}

fn module_hit_roll_allows(
    candidate: ModuleHitCandidate,
    zone: ArmorZone,
    local_hit: Vec3,
    hitbox: HitboxProfile,
) -> bool {
    if candidate.hit_chance >= 1.0 {
        return true;
    }
    module_hit_roll(zone, local_hit, hitbox) <= candidate.hit_chance
}

fn module_hit_roll(zone: ArmorZone, local_hit: Vec3, hitbox: HitboxProfile) -> f32 {
    let x = local_hit.x.abs() / hitbox.half_width_m.max(0.01);
    let y = local_hit.y.abs() / hitbox.half_height_m.max(0.01);
    let z = local_hit.z.abs() / hitbox.half_length_m.max(0.01);
    ((x * 0.53) + (y * 0.19) + (z * 0.31) + zone_roll_bias(zone)).fract()
}

fn zone_roll_bias(zone: ArmorZone) -> f32 {
    match zone {
        ArmorZone::UpperGlacis => 0.11,
        ArmorZone::LowerPlate => 0.17,
        ArmorZone::HullSide => 0.23,
        ArmorZone::HullRear => 0.29,
        ArmorZone::TurretFront => 0.37,
        ArmorZone::Mantlet => 0.41,
        ArmorZone::TurretSide => 0.43,
        ArmorZone::TurretRear => 0.47,
        ArmorZone::Roof => 0.53,
        ArmorZone::LeftTrack => 0.59,
        ArmorZone::RightTrack => 0.61,
    }
}

#[cfg(test)]
mod tests {
    use game_core::HitboxProfile;

    use super::*;

    #[test]
    fn generic_side_penetration_can_deterministically_miss_modules() {
        let hitbox = HitboxProfile::new(1.75, 1.19, 3.20, 1.14, 0.66);
        let local_hit = Vec3::new(1.75, 0.05, 0.25);

        let first =
            impacted_module(ShellType::ArmorPiercing, true, ArmorZone::HullSide, local_hit, hitbox);
        let second =
            impacted_module(ShellType::ArmorPiercing, true, ArmorZone::HullSide, local_hit, hitbox);

        assert_eq!(first, None);
        assert_eq!(second, first, "module hit chance must be replay-stable");
    }

    #[test]
    fn exposed_track_zone_still_damages_suspension_without_a_roll() {
        let hitbox = HitboxProfile::new(1.75, 1.19, 3.20, 1.14, 0.66);

        assert_eq!(
            impacted_module(
                ShellType::ArmorPiercing,
                true,
                ArmorZone::LeftTrack,
                Vec3::new(-1.75, -0.7, 0.0),
                hitbox,
            ),
            Some(ModuleSlot::Suspension)
        );
    }
}
