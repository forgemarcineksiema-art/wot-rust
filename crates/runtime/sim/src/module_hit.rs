use game_core::{ArmorZone, HitboxProfile, ModuleSlot, ShellType, TrackHit, TrackSide};
use glam::Vec3;

use crate::TankState;

/// Degrade the struck track band by `chunk` HP and report what happened (which side, and whether
/// this hit was the one that threw it) so the client can call it out. A direct track-zone hit
/// degrades that side; an HE burst degrades BOTH bands (the blast takes the running gear across
/// the hull); a suspension-module penetration degrades the SIDE it entered — the torsion bars
/// and final drives are independent left/right assemblies, and a shell in the left bank must
/// not cripple the right (W0.7 honesty fix; it used to degrade both). `None` means the shell
/// touched no track.
pub(crate) fn apply_track_damage_for_hit(
    target: &mut TankState,
    module: Option<ModuleSlot>,
    zone: ArmorZone,
    shell_type: ShellType,
    penetrated: bool,
    chunk: u8,
    hit_local_x: f32,
) -> Option<TrackHit> {
    match zone {
        ArmorZone::LeftTrack => Some(degrade_side(target, TrackSide::Left, chunk)),
        ArmorZone::RightTrack => Some(degrade_side(target, TrackSide::Right, chunk)),
        // A skirt hangs OUTSIDE the belt, so a shell that met the skirt crossed the running
        // gear behind it — and an HE burst on the sheet goes off a hand's width from the same
        // belt. Either way it is THIS flank's band, never the far one.
        ArmorZone::Skirt => {
            let side = if hit_local_x < 0.0 { TrackSide::Left } else { TrackSide::Right };
            Some(degrade_side(target, side, chunk))
        }
        _ if shell_type == ShellType::HighExplosive && !penetrated => {
            Some(degrade_both(target, chunk, hit_local_x))
        }
        _ if module == Some(ModuleSlot::Suspension) => {
            let side = if hit_local_x < 0.0 { TrackSide::Left } else { TrackSide::Right };
            Some(degrade_side(target, side, chunk))
        }
        _ => None,
    }
}

/// The band a burst BESIDE a hull throws: the side the blast came from, chipped by `chunk`.
///
/// Splash damages what lives OUTSIDE the plate and nothing behind it. A burst that did not
/// penetrate has no business reaching an engine or a rack, but the running gear is bare metal in
/// the open, and a shell going off next to it is the classic way a track comes off.
pub(crate) fn splash_track_damage(
    target: &mut TankState,
    burst_local_x: f32,
    chunk: u8,
) -> TrackHit {
    let side = if burst_local_x < 0.0 { TrackSide::Left } else { TrackSide::Right };
    degrade_side(target, side, chunk)
}

fn degrade_side(target: &mut TankState, side: TrackSide, chunk: u8) -> TrackHit {
    let was_broken = target.tracks.is_broken(side);
    target.tracks.damage(side, chunk);
    TrackHit { side, broke: !was_broken && target.tracks.is_broken(side) }
}

/// An HE burst against the hull takes the running gear across it — but not EVENLY: the band the
/// blast went off beside eats the full `chunk`, and the far band, screened by the whole hull,
/// takes half (frequency-relief pass: the old symmetric bite double-threw tracks, and a burst
/// cannot hit what the hull is in the way of as hard as what it is next to). Reports the side
/// that newly broke (else the near side) as the callout's subject.
fn degrade_both(target: &mut TankState, chunk: u8, hit_local_x: f32) -> TrackHit {
    let (near, far) = if hit_local_x < 0.0 {
        (TrackSide::Left, TrackSide::Right)
    } else {
        (TrackSide::Right, TrackSide::Left)
    };
    let near_hit = degrade_side(target, near, chunk);
    let far_hit = degrade_side(target, far, chunk / 2);
    if far_hit.broke && !near_hit.broke { far_hit } else { near_hit }
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
    module_zone_exposes(candidate, zone, local_hit, hitbox).then_some(candidate.slot)
}

/// A module the struck zone can expose, and how much of that zone actually exposes it.
///
/// `exposure` is NOT a probability, and calling it one (it was `hit_chance`) invited exactly the
/// wrong mental model. There is no roll: [`module_zone_fraction`] is a smooth function of WHERE
/// on the hull the shell landed, thresholded against this number, so the outcome is a fixed map
/// of bands over the plate — deterministic, replay-stable, and the same on every machine, which
/// is the whole point of a game with no ±25% dice. What the number controls is how much of the
/// zone's area falls inside those bands, not how often a die comes up.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ModuleHitCandidate {
    slot: ModuleSlot,
    exposure: f32,
}

fn module_volume_at_hit(
    zone: ArmorZone,
    local_hit: Vec3,
    hitbox: HitboxProfile,
) -> Option<ModuleHitCandidate> {
    let half_length = hitbox.half_length_m.max(0.01);
    let z = local_hit.z / half_length;

    let (slot, exposure) = match zone {
        ArmorZone::LeftTrack | ArmorZone::RightTrack => (ModuleSlot::Suspension, 1.0),
        ArmorZone::Mantlet => (ModuleSlot::Gun, 1.0),
        ArmorZone::TurretFront | ArmorZone::Roof => (ModuleSlot::Turret, 0.80),
        // A penetration through the commander's drum is inside the turret's crown works —
        // traverse gear, vision train: the turret module, and a high chance of meeting it.
        ArmorZone::Cupola => (ModuleSlot::Turret, 0.90),
        // The hull deck is over the engine bay and the fighting compartment, not over the
        // turret race: a deck penetration meets the engine, as a plunging hit should.
        ArmorZone::HullDeck => (ModuleSlot::Engine, 0.85),
        ArmorZone::TurretSide | ArmorZone::TurretRear => (ModuleSlot::AmmoRack, 0.85),
        ArmorZone::HullRear => (ModuleSlot::Engine, 0.90),
        // A skirt is sheet metal over air, but the SIDE PLATE is behind that air: a shell the
        // armour model resolved as penetrating the skirt stack is inside the hull, so it meets
        // exactly what a bare-flank penetration meets. Returning `None` here made the only two
        // skirted vehicles in the fleet (Centurion, Tiger II) immune to engine, ammo-rack and
        // suspension damage from the whole height of their flank.
        ArmorZone::HullSide | ArmorZone::Skirt if z < -0.35 => (ModuleSlot::Engine, 0.90),
        ArmorZone::HullSide | ArmorZone::Skirt if local_hit.y < -hitbox.half_height_m * 0.20 => {
            (ModuleSlot::Suspension, 1.0)
        }
        // A port penetration is inside the driver's station / bow compartment — same interior
        // band a glacis penetration meets.
        ArmorZone::UpperGlacis | ArmorZone::LowerPlate | ArmorZone::GlacisPort => {
            (ModuleSlot::Gun, 0.80)
        }
        ArmorZone::HullSide | ArmorZone::Skirt => (ModuleSlot::Suspension, 0.65),
    };
    Some(ModuleHitCandidate { slot, exposure })
}

fn module_zone_exposes(
    candidate: ModuleHitCandidate,
    zone: ArmorZone,
    local_hit: Vec3,
    hitbox: HitboxProfile,
) -> bool {
    if candidate.exposure >= 1.0 {
        return true;
    }
    module_zone_fraction(zone, local_hit, hitbox) <= candidate.exposure
}

fn module_zone_fraction(zone: ArmorZone, local_hit: Vec3, hitbox: HitboxProfile) -> f32 {
    let x = local_hit.x.abs() / hitbox.half_width_m.max(0.01);
    let y = local_hit.y.abs() / hitbox.half_height_m.max(0.01);
    let z = local_hit.z.abs() / hitbox.half_length_m.max(0.01);
    ((x * 0.53) + (y * 0.19) + (z * 0.31) + zone_band_offset(zone)).fract()
}

fn zone_band_offset(zone: ArmorZone) -> f32 {
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
        ArmorZone::HullDeck => 0.57,
        ArmorZone::LeftTrack => 0.59,
        ArmorZone::RightTrack => 0.61,
        ArmorZone::Skirt => 0.67,
        ArmorZone::Cupola => 0.71,
        ArmorZone::GlacisPort => 0.73,
    }
}

#[cfg(test)]
mod tests {
    use game_core::HitboxProfile;

    use super::*;

    /// A skirt hangs OUTSIDE the running gear, so a shell resolved on it crossed the belt and
    /// the side plate to get where it got: it reaches the same modules a bare-flank penetration
    /// reaches, and it degrades THIS flank's band — never the far one. (It used to reach
    /// neither, which made the Centurion and Tiger II — the only skirted vehicles — immune to
    /// engine, ammo-rack and suspension damage across the whole height of their flank.)
    #[test]
    fn a_skirt_hit_reaches_the_flank_behind_it_and_only_that_flanks_band() {
        let hitbox = HitboxProfile::new(1.75, 1.19, 3.20, 1.14, 0.66);
        // Low on the flank: the running gear, exactly as a bare hull-side hit there resolves.
        assert_eq!(
            module_volume_at_hit(ArmorZone::Skirt, Vec3::new(1.7, -0.6, 0.5), hitbox)
                .map(|c| c.slot),
            module_volume_at_hit(ArmorZone::HullSide, Vec3::new(1.7, -0.6, 0.5), hitbox)
                .map(|c| c.slot),
            "a skirt penetration meets what a bare-flank penetration meets"
        );
        // Well aft: the engine bay, again matching the bare flank.
        assert_eq!(
            module_volume_at_hit(ArmorZone::Skirt, Vec3::new(1.7, 0.0, -2.0), hitbox)
                .map(|c| c.slot),
            Some(ModuleSlot::Engine),
        );

        let mut target = crate::tank_factory::fresh_tank(
            game_core::TankId(1),
            game_core::TeamId(1),
            game_core::VehicleKind::T54_1951.spec(),
            Vec3::ZERO,
            0.0,
        );
        let hit = apply_track_damage_for_hit(
            &mut target,
            None,
            ArmorZone::Skirt,
            ShellType::ArmorPiercing,
            true,
            40,
            0.6,
        )
        .expect("a skirt hit crosses the belt behind it");
        assert_eq!(hit.side, TrackSide::Right, "the struck flank, from the hull-local x");
        assert!(target.tracks.hp(TrackSide::Right) < game_core::TRACK_HP_MAX);
        assert_eq!(
            target.tracks.hp(TrackSide::Left),
            game_core::TRACK_HP_MAX,
            "the far band must not share this flank's wound"
        );
    }

    /// W0.7: the suspension is an independent left/right assembly. A penetration into the
    /// suspension module on one flank degrades THAT side's band only — it used to cripple both.
    /// An HE burst still takes the running gear across the hull.
    #[test]
    fn a_suspension_penetration_degrades_only_the_struck_side() {
        let mut target = crate::tank_factory::fresh_tank(
            game_core::TankId(2),
            game_core::TeamId(1),
            game_core::VehicleKind::T54_1951.spec(),
            Vec3::ZERO,
            0.0,
        );
        let hit = apply_track_damage_for_hit(
            &mut target,
            Some(ModuleSlot::Suspension),
            ArmorZone::HullSide,
            ShellType::ArmorPiercing,
            true,
            40,
            -0.9,
        )
        .expect("a suspension penetration reports the band");
        assert_eq!(hit.side, TrackSide::Left, "the LEFT flank was struck");
        assert!(target.tracks.hp(TrackSide::Left) < game_core::TRACK_HP_MAX);
        assert_eq!(
            target.tracks.hp(TrackSide::Right),
            game_core::TRACK_HP_MAX,
            "the right band must not share the left flank's wound"
        );
    }

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
