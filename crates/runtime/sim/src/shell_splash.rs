//! The high-explosive burst: blast damage thrown past the impact point at everything inside the
//! shell's explosive radius, attenuated by distance and soaked by the victim's thinnest external
//! plate — the blast finds the roof and the engine deck, not the glacis, so heavies shrug off
//! what mediums feel.

use game_core::math::world_to_tank_local;
use game_core::{
    ArmorFacing, ArmorProfile, ArmorZone, DamageCause, DamageEvent, ShellType, TankId,
};
use glam::Vec3;
use terrain::HeightMap;

use crate::{ShellState, TankState};

const SPLASH_DAMAGE_FACTOR: f32 = 0.5;
/// Terrain march step for the blast line-of-sight. Finer than the shell sweep (1 m): an HE
/// radius is only a few metres, so hull points sit close to the burst and a coarse step would
/// skip the ridge between them.
const SPLASH_LOS_STEP_M: f32 = 0.4;
/// Both LOS endpoints ride this high, so a surface burst is not blocked by its own ground
/// texel while a real crest still shields what hides behind it.
const SPLASH_LOS_LIFT_M: f32 = 0.35;
const SPLASH_ARMOR_ABSORPTION: f32 = 1.3;

/// A high-explosive burst throws damage past its impact point: every vehicle inside the
/// explosive radius takes attenuated blast damage. The directly-struck tank already took the
/// surface-burst damage from the armor test and is skipped; allies are protected exactly like
/// direct fire, but the owner's own HE can absolutely hurt the owner.
pub(crate) fn burst_he_splash(
    shell: &ShellState,
    burst_point: Vec3,
    tanks: &mut [TankState],
    damage_events: &mut Vec<DamageEvent>,
    direct_target: Option<TankId>,
    heightmap: Option<&HeightMap>,
) {
    let radius = shell.shell.explosive_radius_m;
    if shell.shell.shell_type != ShellType::HighExplosive || radius <= 0.0 {
        return;
    }
    let owner_team = tanks.iter().find(|tank| tank.id == shell.owner).map(|tank| tank.team);
    for tank in tanks.iter_mut() {
        if tank.hit_points == 0 || Some(tank.id) == direct_target {
            continue;
        }
        if Some(tank.team) == owner_team && tank.id != shell.owner {
            continue;
        }
        let (distance, hull_point, burst_local) = hull_contact(burst_point, tank);
        let falloff = 1.0 - distance / radius;
        if falloff <= 0.0 {
            continue;
        }
        // The honest-tank rule holds for blast: terrain between the burst and the hull kills
        // the splash - a ridge that stops the shell stops its pressure wave too.
        if !splash_line_clear(heightmap, burst_point, hull_point) {
            continue;
        }
        let soaked = facing_plate_mm(&tank.spec.hull, burst_local) * SPLASH_ARMOR_ABSORPTION;
        let damage =
            (shell.shell.damage_hp as f32 * SPLASH_DAMAGE_FACTOR * falloff - soaked).round();
        if damage < 1.0 {
            continue;
        }
        let damage = damage as u32;
        tank.hit_points = tank.hit_points.saturating_sub(damage);
        damage_events.push(DamageEvent {
            source: shell.owner,
            target: tank.id,
            hit_position: burst_point,
            damage_hp: damage,
            penetrated: false,
            cause: DamageCause::Splash,
            shell_type: shell.shell.shell_type,
            ..Default::default()
        });
    }
}

/// Distance from the burst point to the tank hull surface (its hitbox slab, not its centre),
/// the world-space point ON the hull nearest the burst (the blast LOS target), and the burst
/// position in the hull frame (which plate faces the blast).
fn hull_contact(point: Vec3, tank: &TankState) -> (f32, Vec3, Vec3) {
    let hitbox = tank.spec.hitbox;
    let local = world_to_tank_local(point, tank.position, hitbox.center_y_m, tank.hull_pose());
    let half = Vec3::new(hitbox.half_width_m, hitbox.half_height_m, hitbox.half_length_m);
    let clamped = local.clamp(-half, half);
    let hull_world =
        tank.hull_pose().basis() * clamped + tank.position + Vec3::new(0.0, hitbox.center_y_m, 0.0);
    (local.distance(clamped), hull_world, local)
}

/// Whether the blast reaches from the burst to the hull point over the terrain: a stepped
/// march with both endpoints lifted a little, so a surface burst is not blocked by its own
/// ground texel while a real crest still shields what hides behind it.
fn splash_line_clear(heightmap: Option<&HeightMap>, burst: Vec3, hull_point: Vec3) -> bool {
    let Some(heightmap) = heightmap else {
        return true;
    };
    let from = burst + Vec3::Y * SPLASH_LOS_LIFT_M;
    let to = hull_point + Vec3::Y * SPLASH_LOS_LIFT_M;
    let segment = to - from;
    let length = segment.length();
    // A contact burst (essentially on the hull) is never self-occluded; the lift keeps both
    // endpoints above their own ground texel so only a real crest between them can block.
    if length <= 0.15 {
        return true;
    }
    let steps = (length / SPLASH_LOS_STEP_M).ceil().max(2.0) as u32;
    for step in 1..steps {
        let point = from + segment * (step as f32 / steps as f32);
        if heightmap.sample_height(point.x, point.z).is_some_and(|ground| ground > point.y) {
            return false;
        }
    }
    true
}

/// The plate the blast actually strikes, picked by the burst direction in the hull frame: a
/// detonation over the deck soaks by the roof, against the bow by the glacis, beside the hull
/// by the side. The old rule took the thinnest external plate regardless of direction, so a
/// frontal burst was soaked by the roof it never touched.
fn facing_plate_mm(armor: &ArmorProfile, burst_local: Vec3) -> f32 {
    let direction = burst_local.normalize_or_zero();
    if direction.y > direction.x.abs().max(direction.z.abs()) {
        return armor.plate(ArmorZone::Roof).nominal_thickness_mm;
    }
    if direction.x.abs() >= direction.z.abs() {
        armor.nominal_thickness_mm(ArmorFacing::HullSide)
    } else if direction.z >= 0.0 {
        armor.nominal_thickness_mm(ArmorFacing::HullFront)
    } else {
        armor.nominal_thickness_mm(ArmorFacing::HullRear)
    }
}

#[cfg(test)]
mod tests {
    use game_core::{ShellType, TankId, TankSpec, TeamId};
    use glam::Vec3;
    use terrain::HeightMap;

    use super::*;
    use crate::shell_trace::SHELL_MAX_AGE_SECONDS;
    use crate::tank_factory::fresh_tank;

    fn he_shell(owner: TankId, at: Vec3) -> ShellState {
        let spec = TankSpec::t54_1951();
        let shell = spec
            .gun
            .ammo_options()
            .iter()
            .copied()
            .find(|s| s.shell_type == ShellType::HighExplosive)
            .expect("t54 carries HE");
        ShellState {
            id: game_core::ShellId::default(),
            owner,
            position: at,
            velocity_mps: Vec3::new(0.0, -50.0, 0.0),
            shell,
            age_seconds: 0.0,
            traveled_m: 0.0,
            max_age_seconds: SHELL_MAX_AGE_SECONDS,
            ricocheted_once: false,
            last_penetrated_target: None,
        }
    }

    fn splash_damage_at(burst: Vec3, tank_pos: Vec3, heightmap: Option<&HeightMap>) -> u32 {
        let shooter =
            fresh_tank(TankId(1), TeamId(1), TankSpec::t54_1951(), Vec3::new(500.0, 0.0, 0.0), 0.0);
        let victim = fresh_tank(TankId(2), TeamId(2), TankSpec::t54_1951(), tank_pos, 0.0);
        let hp_before = victim.hit_points;
        let mut tanks = vec![shooter, victim];
        let shell = he_shell(TankId(1), burst);
        let mut events = Vec::new();
        burst_he_splash(&shell, burst, &mut tanks, &mut events, None, heightmap);
        hp_before - tanks[1].hit_points
    }

    /// The honest-tank rule holds for blast: a wall of ground between the burst and the hull
    /// kills the splash entirely, while the same burst in the open still wounds. The 1.5 m HE
    /// radius keeps the geometry tight, so the test runs on a fine 0.25 m grid.
    #[test]
    fn a_ridge_between_burst_and_hull_blocks_the_splash() {
        // A 6 m-tall, one-cell (0.25 m) ground wall across z = 7.25..7.5 m.
        let mut samples = vec![0.0f32; 64 * 64];
        for x in 0..64 {
            samples[29 * 64 + x] = 6.0;
        }
        let ridged = HeightMap::new(64, 64, 0.25, samples).expect("ridged map");
        let open = HeightMap::flat(64, 64, 0.25, 0.0).expect("flat map");

        // Victim faces +Z; its rear plate sits at z = 11.0 - half_length. The burst lands
        // 0.9 m behind that plate, with the ground wall in between on the ridged map.
        let burst = Vec3::new(8.0, 0.2, 6.9);
        let victim = Vec3::new(8.0, 0.0, 11.0);
        let wounded_open = splash_damage_at(burst, victim, Some(&open));
        let wounded_ridged = splash_damage_at(burst, victim, Some(&ridged));
        assert!(wounded_open > 0, "the open-field control burst must wound: {wounded_open}");
        assert_eq!(wounded_ridged, 0, "the ground wall must kill the blast entirely");
    }

    /// The soak is directional: a burst over the deck is resisted by the thin roof, one against
    /// the bow by the thick glacis - so the deck burst hurts MORE at the same range.
    #[test]
    fn the_blast_soaks_by_the_plate_that_faces_it() {
        let tank_pos = Vec3::new(50.0, 0.0, 50.0);
        let spec = TankSpec::t54_1951();
        let above = Vec3::new(50.0, spec.hitbox.center_y_m + spec.hitbox.half_height_m + 1.0, 50.0);
        let front = Vec3::new(50.0, spec.hitbox.center_y_m, 50.0 + spec.hitbox.half_length_m + 1.0);
        let deck_wound = splash_damage_at(above, tank_pos, None);
        let bow_wound = splash_damage_at(front, tank_pos, None);
        assert!(deck_wound > 0, "a deck burst at 1 m must wound: {deck_wound}");
        assert!(
            deck_wound > bow_wound,
            "the thin roof must soak less than the glacis: deck {deck_wound} vs bow {bow_wound}"
        );
    }
}
