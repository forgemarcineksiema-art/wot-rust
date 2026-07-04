use game_core::{ArmorFacing, TankId, TankSpec, TeamId, resolve_penetration_at_distance_on_zone};
use glam::{Mat4, Vec3, Vec4};
use net::TankSnapshot;
use terrain::{HeightMap, StaticCoverObject};

const RETICLE_MATCH_TOLERANCE_M: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReticleStatus {
    Clear,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReticleFeedback {
    pub status: ReticleStatus,
    pub aim_world_point: Vec3,
    pub gun_world_point: Vec3,
    pub actual_impact_world_point: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PenetrationHint {
    pub penetrates: bool,
    pub shell_pen_mm: f32,
    pub armor_mm: f32,
    pub facing: ArmorFacing,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ReticleFeedbackQuery<'a> {
    pub heightmap: &'a HeightMap,
    pub cover: &'a [StaticCoverObject],
    pub tanks: &'a [TankSnapshot],
    /// Spec of the firing (local) vehicle. Penetration hints read the shell from here so the HUD
    /// shows the *player's* gun, not the gun of whatever tank happens to be under the reticle.
    pub player_spec: &'a TankSpec,
    pub owner: TankId,
    pub owner_team: TeamId,
    pub muzzle: Vec3,
    pub aim: Vec3,
    /// World-space unit direction of the barrel (hull pose already applied).
    pub gun_direction: Vec3,
    pub muzzle_velocity_mps: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReticleReport {
    pub feedback: ReticleFeedback,
    pub penetration: Option<PenetrationHint>,
}

/// One authoritative-shaped ballistic trace serves both the reticle status and the penetration
/// hint — running the identical trace twice per frame doubled the most expensive HUD work.
pub(crate) fn reticle_report(query: ReticleFeedbackQuery<'_>) -> ReticleReport {
    let outcome =
        crate::hud::reticle_sweep::reticle_trace(crate::hud::reticle_sweep::ReticleTraceQuery {
            heightmap: query.heightmap,
            cover: query.cover,
            tanks: query.tanks,
            owner: query.owner,
            owner_team: query.owner_team,
            muzzle: query.muzzle,
            gun_direction: query.gun_direction,
            muzzle_velocity_mps: query.muzzle_velocity_mps,
        });
    ReticleReport {
        feedback: feedback_from_outcome(&query, &outcome),
        penetration: penetration_from_outcome(&query, &outcome),
    }
}

#[cfg(test)]
pub(crate) fn reticle_feedback(query: ReticleFeedbackQuery<'_>) -> ReticleFeedback {
    reticle_report(query).feedback
}

#[cfg(test)]
pub(crate) fn penetration_hint(query: ReticleFeedbackQuery<'_>) -> Option<PenetrationHint> {
    reticle_report(query).penetration
}

fn feedback_from_outcome(
    query: &ReticleFeedbackQuery<'_>,
    outcome: &sim::TraceOutcome,
) -> ReticleFeedback {
    let target_pitch =
        crate::aim::gun_pitch_to_hit(query.muzzle, query.aim, query.muzzle_velocity_mps);
    let actual_impact_world_point = outcome.impact_point();
    // The arc is the simulation's gun arc, not a client-side copy that could drift from it.
    let in_arc = (sim::MIN_GUN_PITCH_RAD..=sim::MAX_GUN_PITCH_RAD).contains(&target_pitch);
    let status = if in_arc
        && terrain_line_clear(query.heightmap, query.muzzle, query.aim)
        && actual_impact_world_point.distance(query.aim) <= RETICLE_MATCH_TOLERANCE_M
    {
        ReticleStatus::Clear
    } else {
        ReticleStatus::Blocked
    };
    let distance = query.aim.distance(query.muzzle).max(1.0);
    let gun_world_point = query.muzzle + query.gun_direction.normalize_or_zero() * distance;

    ReticleFeedback {
        status,
        aim_world_point: query.aim,
        gun_world_point,
        actual_impact_world_point,
    }
}

fn penetration_from_outcome(
    query: &ReticleFeedbackQuery<'_>,
    outcome: &sim::TraceOutcome,
) -> Option<PenetrationHint> {
    // Only a tank impact has armor to penetrate; terrain/cover/open-sky shots have no hint.
    let sim::TraceOutcome::Tank { id, facing, zone, impact_angle_degrees, distance_m, .. } =
        *outcome
    else {
        return None;
    };
    // The player fires their own shell at the *target's* armor — read the shell from the player's
    // spec, not from the tank we are pointing at.
    let target_hull = query.tanks.iter().find(|tank| tank.tank_id == id)?.vehicle.spec().hull;
    let result = resolve_penetration_at_distance_on_zone(
        &query.player_spec.gun.shell,
        &target_hull,
        zone,
        impact_angle_degrees,
        distance_m,
    );
    Some(PenetrationHint {
        penetrates: result.penetrated,
        shell_pen_mm: result.effective_armor_mm + result.remaining_penetration_mm,
        armor_mm: result.effective_armor_mm,
        facing,
    })
}

pub(crate) fn world_to_clip_xy(point: Vec3, view_projection: [[f32; 4]; 4]) -> Option<[f32; 2]> {
    let clip =
        Mat4::from_cols_array_2d(&view_projection) * Vec4::new(point.x, point.y, point.z, 1.0);
    if clip.w <= f32::EPSILON {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    (ndc.x.abs() <= 1.2 && ndc.y.abs() <= 1.2).then_some([ndc.x, ndc.y])
}

fn terrain_line_clear(heightmap: &HeightMap, from: Vec3, to: Vec3) -> bool {
    let segment = to - from;
    let steps = (segment.length() / 1.0).ceil().max(1.0) as u32;
    for step in 1..steps {
        let point = from + segment * (step as f32 / steps as f32);
        if heightmap.sample_height(point.x, point.z).is_some_and(|ground| ground > point.y + 0.15) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;
